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
    pub users: Arc<std::sync::Mutex<super::users::Users>>,
    pub vault: Arc<std::sync::Mutex<super::credentials::Vault>>,
    switching: Mutex<()>,
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
fn configuration(root: &Path) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let path = root.join("host.json");
    let socket = root.join("ai.sock");
    let value = json!({"version":1,"databasePath":root.join("ai.sqlite"),"socketPath":socket,"credentialSocket":root.join("credentials.sock"),"usersPath":root.join("users.json"),"nativeDirectory":root.join("native"),"workingDirectory":root.join("workspace")});
    let temporary = root.join(format!("host-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(&value)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, &path)?;
    Ok((path, socket))
}
impl DesktopRuntime {
    pub async fn start(root: &Path, artifact: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        private_directory(root)?;
        private_directory(&root.join("workspace"))?;
        let users = Arc::new(std::sync::Mutex::new(
            super::users::Users::open(root).map_err(|_| "user registry unavailable")?,
        ));
        let vault = Arc::new(std::sync::Mutex::new(
            super::credentials::Vault::open(root).map_err(|_| "credential registry unavailable")?,
        ));
        let execution = ExecutionHandle::start(&root.join("execution.sqlite"))?;
        let (configuration, socket) = configuration(root)?;
        let mcp_stop = CancellationToken::new();
        super::credentials::serve(
            root.join("credentials.sock"),
            users.clone(),
            vault.clone(),
            mcp_stop.child_token(),
        )
        .await?;
        let mut child = None;
        // Missing AI credentials/artifact never substitute fixture conversations or erase tasks.
        let launched = Command::new(artifact.join("bin/rss-ai-host"))
            .arg(configuration)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn();
        if let Err(error) = &launched {
            eprintln!("AI Host spawn failed: {:?}", error.kind());
        }
        if let Ok(mut process) = launched {
            let reader = process.stdout.take().ok_or("MCP stdout unavailable")?;
            let writer = process.stdin.take().ok_or("MCP stdin unavailable")?;
            let diagnostics = process.stderr.take().ok_or("diagnostic pipe unavailable")?;
            tokio::spawn(async move {
                let mut lines = FramedRead::new(diagnostics, LinesCodec::new_with_max_length(256));
                while let Some(line) = lines.next().await {
                    if let Ok(line) = line {
                        if let Some(value) = diagnostic(&line) {
                            eprintln!("{value}");
                        }
                    }
                }
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
            users,
            vault,
            switching: Mutex::new(()),
            socket,
            child: Mutex::new(child),
            mcp_stop,
            connections: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
        })
    }
    pub fn current(&self, generation: &str) -> ui::Result<ai_session_contract::UserContext> {
        let _switch = self.switching.try_lock().map_err(|_| unavailable())?;
        self.users
            .lock()
            .map_err(|_| unavailable())?
            .require(generation)
    }
    pub fn execution_for(&self, generation: &str) -> ui::Result<ExecutionHandle> {
        let context = self.current(generation)?;
        self.execution
            .for_caller(context.user.user_id.as_str())
            .map_err(|_| unavailable())
    }
    pub async fn select_user(&self, name: &str) -> ui::Result<ai_session_contract::UserContext> {
        let _switch = self.switching.lock().await;
        let (page, previous) = {
            let users = self.users.lock().map_err(|_| unavailable())?;
            (users.prepare(name)?, users.page().current)
        };
        page.current.as_ref().ok_or_else(unavailable)?;
        self.detach_views().await;
        // Fence the old persistent caller before committing the new native generation.
        // Without a deployed Host there can be no model work; self-service remains available.
        if let Some(previous) = &previous {
            if self.child.lock().await.is_some() {
                tokio::time::timeout(Duration::from_secs(15), async {
                    let stream = loop {
                        if let Some(status) = self
                            .child
                            .lock()
                            .await
                            .as_mut()
                            .ok_or_else(unavailable)?
                            .try_wait()
                            .map_err(|_| unavailable())?
                        {
                            let _ = status;
                            return Err(unavailable());
                        }
                        if let Ok(stream) = UnixStream::connect(&self.socket).await {
                            break stream;
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    };
                    let (reader, writer) = stream.into_split();
                    let mut writer =
                        FramedWrite::new(writer, LinesCodec::new_with_max_length(262144));
                    let mut reader =
                        FramedRead::new(reader, LinesCodec::new_with_max_length(262144));
                    writer
                        .send(
                            json!({"type":"suspend_user","generation":previous.generation})
                                .to_string(),
                        )
                        .await
                        .map_err(|_| unavailable())?;
                    let line = reader
                        .next()
                        .await
                        .ok_or_else(unavailable)?
                        .map_err(|_| unavailable())?;
                    let value: serde_json::Value =
                        serde_json::from_str(&line).map_err(|_| unavailable())?;
                    if value != json!({"ok":true}) {
                        return Err(unavailable());
                    }
                    Ok(())
                })
                .await
                .map_err(|_| unavailable())??;
            }
        }
        // Match credential entry's users -> vault lock order. No old-generation
        // editor can stage a new reference between cleanup and the user commit.
        let mut users = self.users.lock().map_err(|_| unavailable())?;
        if let Some(previous) = previous {
            self.vault
                .lock()
                .map_err(|_| unavailable())?
                .discard_generation(previous.user.user_id.as_str(), previous.generation.as_str())
                .map_err(|_| unavailable())?;
        }
        users.commit(page)
    }
    pub async fn connect(&self, generation: &str) -> ui::Result<String> {
        self.current(generation)?;
        let mut connections = self.connections.lock().await;
        if connections.len() >= 4 || self.mcp_stop.is_cancelled() {
            return Err(unavailable());
        }
        let stream = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                {
                    let mut child = self.child.lock().await;
                    if let Some(status) = child
                        .as_mut()
                        .ok_or_else(unavailable)?
                        .try_wait()
                        .map_err(|_| unavailable())?
                    {
                        eprintln!("AI Host exited: {status}");
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
        self.current(generation)?;
        let (reader, writer) = stream.into_split();
        let mut writer = FramedWrite::new(writer, LinesCodec::new_with_max_length(262144));
        writer
            .send(json!({"type":"attach","generation":generation}).to_string())
            .await
            .map_err(|_| unavailable())?;
        let id = format!("view-{}", self.next.fetch_add(1, Ordering::Relaxed));
        connections.insert(
            id.clone(),
            Arc::new(Connection {
                reader: Mutex::new(FramedRead::new(
                    reader,
                    LinesCodec::new_with_max_length(262144),
                )),
                writer: Mutex::new(writer),
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

// Only product-owned closed diagnostics cross the native stderr boundary.
fn diagnostic(line: &str) -> Option<String> {
    if line == "AI Host cleanup incomplete" {
        return Some(line.into());
    }
    let (stage, code) = line.strip_prefix("AI Host ")?.split_once(": ")?;
    if ![
        "could not start",
        "admission",
        "dispatch",
        "observe",
        "recovery",
        "credential",
        "close",
    ]
    .contains(&stage)
    {
        return None;
    }
    if ![
        "configuration_file",
        "configuration_invalid",
        "authentication_required",
        "startup_failed",
        "unavailable",
        "unsupported_version",
        "unsupported_capability",
        "permission_denied",
        "invalid_input",
        "stale_binding",
        "content_conflict",
        "reconciliation_required",
        "limit_exceeded",
        "storage_corrupt",
        "expired",
        "cancelled",
        "timeout",
    ]
    .contains(&code)
    {
        return None;
    }
    Some(format!("AI Host {stage}: {code}"))
}
#[cfg(test)]
mod tests {
    #[test]
    fn diagnostics_accept_only_closed_product_codes() {
        assert_eq!(
            super::diagnostic("AI Host could not start: authentication_required").as_deref(),
            Some("AI Host could not start: authentication_required")
        );
        assert!(super::diagnostic("AI Host recovery: unavailable").is_some());
        assert!(super::diagnostic("AI Host credential: unavailable").is_some());
        assert!(super::diagnostic("AI Host credential: secret-token").is_none());
        assert!(super::diagnostic("AI Host cleanup incomplete").is_some());
        for line in [
            "native secret-token",
            "AI Host recovery: secret-token",
            "AI Host raw: unavailable",
        ] {
            assert!(super::diagnostic(line).is_none());
        }
    }
}
