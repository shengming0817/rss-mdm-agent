//! Desktop-owned Node child and bounded ACP connections. MCP uses the child's pipes.
// ref: Tauri crates/tauri/src/app.rs@tauri-v2.11.2
use super::execution::ExecutionHandle;
use crate::self_service::{self as ui, fixtures};
use execution_mcp::{ExecutionMcp, McpLimits};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    net::{
        unix::{OwnedReadHalf, OwnedWriteHalf},
        UnixStream,
    },
    process::{Child, Command},
    sync::Mutex,
};
use tokio_util::{
    codec::{FramedRead, FramedWrite, LinesCodec},
    sync::CancellationToken,
};

fn unavailable() -> ui::ServiceError {
    ui::error("ai_unavailable", "AI 服务不可用；已登记任务仍可查询")
}
struct Connection {
    reader: Mutex<FramedRead<OwnedReadHalf, LinesCodec>>,
    writer: Mutex<FramedWrite<OwnedWriteHalf, LinesCodec>>,
    stop: CancellationToken,
}
pub struct DesktopRuntime {
    pub execution: ExecutionHandle,
    socket: PathBuf,
    child: Mutex<Option<Child>>,
    mcp_stop: CancellationToken,
    connections: Mutex<BTreeMap<String, Arc<Connection>>>,
    next: AtomicU64,
}
fn private_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if !path.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("private directory required".into());
    }
    Ok(())
}
fn configuration(
    root: &Path,
) -> Result<(PathBuf, PathBuf, super::origin::AiBinding), Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let path = root.join("client.json");
    if !path.exists() {
        let home = std::env::var_os("HOME").ok_or("user home unavailable")?;
        let user = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".codex"));
        let value = json!({"databasePath":root.join("ai.sqlite"),"socketPath":root.join("ai.sock"),"nativeDirectory":root.join("native"),"workingDirectory":root.join("workspace"),
            "caller":{"tenantId":"s1-test","principalId":"fixture-actor","authorityId":"desktop-fixture"},
            "session":{"provider":"codex","accountRef":"s1-user-codex","config":{"id":"s1-local","revision":"r1"},"profile":"controlled_tools"},
            "connection":{"source":"existing_user_config","directory":user}});
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?
            .write_all(&serde_json::to_vec_pretty(&value)?)?;
    }
    // The Node configuration owner performs exact validation; Rust reads only the endpoint.
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let before = std::fs::symlink_metadata(&path)?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.permissions().mode() & 0o077 != 0
    {
        return Err("private configuration file required".into());
    }
    let file = std::fs::File::open(&path)?;
    let actual = file.metadata()?;
    if before.ino() != actual.ino() || before.dev() != actual.dev() {
        return Err("configuration identity changed".into());
    }
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err("configuration size".into());
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    let socket = PathBuf::from(value["socketPath"].as_str().ok_or("socket configuration")?);
    if socket != root.join("ai.sock") {
        return Err("desktop socket must remain in its private directory".into());
    }
    Ok((
        path,
        socket,
        super::origin::AiBinding::from_configuration(&value)?,
    ))
}
impl DesktopRuntime {
    pub async fn start(root: &Path, artifact: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        private_directory(root)?;
        private_directory(&root.join("workspace"))?;
        let (configuration, socket, ai_binding) = configuration(root)?;
        let execution = ExecutionHandle::start(&root.join("execution.sqlite"), ai_binding)?;
        let mcp_stop = CancellationToken::new();
        let mut child = None;
        // Missing AI credentials/artifact never substitute fixture conversations or erase tasks.
        if let Ok(mut process) = Command::new(artifact.join("bin/rss-ai-host"))
            .arg(configuration)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            let reader = process.stdout.take().ok_or("MCP stdout unavailable")?;
            let writer = process.stdin.take().ok_or("MCP stdin unavailable")?;
            let mut diagnostics = process.stderr.take().ok_or("diagnostic pipe unavailable")?;
            tokio::spawn(async move {
                let mut bytes = [0; 4096];
                while matches!(diagnostics.read(&mut bytes).await, Ok(n) if n > 0) {}
            });
            let limits = McpLimits {
                frame_bytes: 262144,
                response_bytes: 262144,
                json_depth: 64,
                json_nodes: 16384,
                in_flight: 16,
                session_frames: 100000,
                request_timeout: Duration::from_secs(10),
                io_timeout: Duration::from_secs(3600),
                catalog: fixtures::CATALOG_LIMITS,
                parameters: fixtures::PARAMETERS,
            };
            let server = ExecutionMcp::new(Arc::new(execution.clone()), limits)?;
            let stop = mcp_stop.clone();
            tokio::spawn(async move {
                let _ = server.serve(reader, writer, stop).await;
            });
            child = Some(process);
        }
        Ok(Self {
            execution,
            socket,
            child: Mutex::new(child),
            mcp_stop,
            connections: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
        })
    }
    pub async fn connect(&self) -> ui::Result<String> {
        let mut connections = self.connections.lock().await;
        if connections.len() >= 4 || self.mcp_stop.is_cancelled() {
            return Err(unavailable());
        }
        let stream = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                {
                    let mut child = self.child.lock().await;
                    if child
                        .as_mut()
                        .ok_or_else(unavailable)?
                        .try_wait()
                        .map_err(|_| unavailable())?
                        .is_some()
                    {
                        return Err(unavailable());
                    }
                }
                if let Ok(stream) = UnixStream::connect(&self.socket).await {
                    return Ok(stream);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .map_err(|_| unavailable())??;
        let (reader, writer) = stream.into_split();
        let id = format!("view-{}", self.next.fetch_add(1, Ordering::Relaxed));
        connections.insert(
            id.clone(),
            Arc::new(Connection {
                reader: Mutex::new(FramedRead::new(
                    reader,
                    LinesCodec::new_with_max_length(262144),
                )),
                writer: Mutex::new(FramedWrite::new(
                    writer,
                    LinesCodec::new_with_max_length(262144),
                )),
                stop: self.mcp_stop.child_token(),
            }),
        );
        Ok(id)
    }
    async fn connection(&self, id: &str) -> ui::Result<Arc<Connection>> {
        self.connections
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(unavailable)
    }
    pub async fn receive(&self, id: &str) -> ui::Result<Option<Value>> {
        let c = self.connection(id).await?;
        let mut reader = c.reader.try_lock().map_err(|_| unavailable())?;
        tokio::select! {
            _ = c.stop.cancelled() => Err(unavailable()),
            _ = tokio::time::sleep(Duration::from_secs(20)) => Ok(None),
            message = reader.next() => match message {
                Some(Ok(line)) => serde_json::from_str(&line).map(Some).map_err(|_| unavailable()),
                _ => { c.stop.cancel(); Err(unavailable()) }
            }
        }
    }
    pub async fn send(&self, id: &str, message: Value) -> ui::Result<()> {
        let bytes = serde_json::to_string(&message).map_err(|_| unavailable())?;
        if bytes.len() > 262144 {
            return Err(unavailable());
        }
        let c = self.connection(id).await?;
        let mut writer = c.writer.try_lock().map_err(|_| unavailable())?;
        tokio::select! {
            _ = c.stop.cancelled() => Err(unavailable()),
            result = tokio::time::timeout(Duration::from_secs(5), writer.send(bytes)) => match result {
                Ok(Ok(())) => Ok(()), _ => { c.stop.cancel(); Err(unavailable()) }
            }
        }
    }
    pub async fn disconnect(&self, id: &str) {
        if let Some(c) = self.connections.lock().await.remove(id) {
            c.stop.cancel();
        }
    }
    pub async fn detach_views(&self) {
        for (_, c) in std::mem::take(&mut *self.connections.lock().await) {
            c.stop.cancel();
        }
    }
    pub async fn shutdown(&self) {
        self.detach_views().await;
        if let Some(mut child) = self.child.lock().await.take() {
            if let Some(pid) = child.id() {
                // Bounded graceful Host shutdown; this is process lifecycle, never business cancel.
                let _ = Command::new("/bin/kill")
                    .args(["-TERM", &pid.to_string()])
                    .status()
                    .await;
            }
            if tokio::time::timeout(Duration::from_secs(8), child.wait())
                .await
                .is_err()
            {
                let _ = child.kill().await;
            }
        }
        self.mcp_stop.cancel();
        self.execution.close().await;
    }
}
