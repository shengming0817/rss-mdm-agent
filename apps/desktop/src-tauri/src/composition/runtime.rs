//! Stable desktop owners and one replaceable AI Host incarnation.
use super::{
    control::Control,
    credentials::{KeyBackend, MasterKey},
    execution::ExecutionHandle,
    host::{self, Fault, Phase, Process},
};
use crate::self_service as ui;
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
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
fn unavailable() -> ui::ServiceError {
    ui::error("ai_unavailable", "AI 服务不可用；已登记任务仍可查询")
}
struct Connection {
    reader: Mutex<mpsc::Receiver<Value>>,
    stop: CancellationToken,
    control: Arc<Control>,
}
enum Host {
    Closed,
    Stopped,
    Starting,
    Running(Arc<Process>),
    Failed(Fault),
}
struct HostState {
    generation: u64,
    owner: Host,
}
pub struct DesktopRuntime {
    pub execution: ExecutionHandle,
    pub users: Arc<std::sync::Mutex<super::users::Users>>,
    switching: Mutex<()>,
    host: std::sync::Mutex<HostState>,
    root: PathBuf,
    artifact: PathBuf,
    source: ai_session_contract::HostStatusSource,
    master: Arc<MasterKey>,
    recent: std::sync::Mutex<Vec<(u64, Fault, ai_session_contract::HostDiagnostic)>>,
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
fn configuration(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let path = root.join("host.json");
    let value = json!({"version":1,"databasePath":root.join("ai.sqlite"),"nativeDirectory":root.join("native"),"workingDirectory":root.join("workspace")});
    let temporary = root.join(format!("host-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(&value)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, &path)?;
    Ok(path)
}
impl DesktopRuntime {
    pub async fn start(
        root: &Path,
        artifact: &Path,
        source: ai_session_contract::HostStatusSource,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_with_key_backend(root, artifact, source, super::credentials::Keychain).await
    }
    pub async fn start_with_key_backend(
        root: &Path,
        artifact: &Path,
        source: ai_session_contract::HostStatusSource,
        backend: impl KeyBackend + 'static,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        private_directory(root)?;
        private_directory(&root.join("workspace"))?;
        let users = Arc::new(std::sync::Mutex::new(
            super::users::Users::open(root).map_err(|_| "user registry unavailable")?,
        ));
        let execution = ExecutionHandle::start(&root.join("execution.sqlite"))?
            .with_trusted_users(users.clone());
        let runtime = Self {
            execution,
            users,
            switching: Mutex::new(()),
            host: std::sync::Mutex::new(HostState {
                generation: 0,
                owner: Host::Stopped,
            }),
            root: root.into(),
            artifact: artifact.into(),
            source,
            master: Arc::new(MasterKey::new(backend)),
            recent: std::sync::Mutex::new(Vec::new()),
            connections: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
        };
        runtime.restart(0).await;
        Ok(runtime)
    }
    fn epoch(&self) -> u64 {
        self.host.lock().unwrap().generation
    }
    fn process(&self) -> Option<Arc<Process>> {
        match &self.host.lock().unwrap().owner {
            Host::Running(process) => Some(process.clone()),
            _ => None,
        }
    }
    pub fn status(&self) -> ai_session_contract::HostStatus {
        let (generation, phase) = {
            let state = self.host.lock().unwrap();
            let phase = match &state.owner {
                Host::Closed | Host::Stopped => Phase::Stopped,
                Host::Starting => Phase::Starting,
                Host::Failed(fault) => Phase::Failed(*fault),
                Host::Running(process) => process.phase(),
            };
            (state.generation, phase)
        };
        let (phase, fault) = match phase {
            Phase::Starting => ("starting", None),
            Phase::Ready => ("ready", None),
            Phase::Stopping => ("stopping", None),
            Phase::Stopped => ("stopped", None),
            Phase::Failed(fault) => ("failed", Some(fault)),
        };
        let mut recent = self.recent.lock().unwrap();
        if let Some(fault) = fault {
            if recent
                .last()
                .is_none_or(|(g, f, _)| *g != generation || *f != fault)
            {
                recent.push((
                    generation,
                    fault,
                    fault.diagnostic(&self.source.to_string()),
                ));
                if recent.len() > 64 {
                    recent.remove(0);
                }
            }
        }
        let mut value = json!({"schemaVersion":5,"kind":"hostStatus","generation":generation,"phase":phase,"source":self.source,"version":env!("CARGO_PKG_VERSION"),"recent":recent.iter().map(|(_,_,d)|d).collect::<Vec<_>>()});
        if fault.is_some() {
            value["diagnostic"] = serde_json::to_value(&recent.last().unwrap().2).unwrap();
        }
        serde_json::from_value(value).expect("schema-owned status")
    }
    /// expected is the UI's observed incarnation. Concurrent clicks cannot start another owner.
    pub async fn restart(&self, expected: u64) -> ai_session_contract::HostStatus {
        let _switch = self.switching.lock().await;
        if matches!(self.host.lock().unwrap().owner, Host::Closed) || expected != self.epoch() {
            return self.status();
        }
        self.detach_views().await;
        if let Some(process) = self.process() {
            let reaped = process.close().await;
            self.status(); // Record cleanup evidence before replacing the incarnation.
            if !reaped {
                return self.status();
            }
        }
        {
            let mut state = self.host.lock().unwrap();
            state.generation += 1;
            state.owner = Host::Starting;
        }
        let next = match configuration(&self.root).map_err(|_| Fault::Configuration) {
            Ok(configuration) => match host::launch(
                &self.artifact,
                &configuration,
                &self.execution,
                self.master.clone(),
            )
            .await
            {
                Ok(process) => Host::Running(process),
                Err(fault) => Host::Failed(fault),
            },
            Err(_) => Host::Failed(Fault::Configuration),
        };
        self.host.lock().unwrap().owner = next;
        self.status()
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
        self.detach_views().await;
        if let Some(previous) = previous {
            if let Some(process) = self.process() {
                if process.ready() {
                    process
                        .control
                        .suspend(&previous, Duration::from_secs(15))
                        .await?;
                } else {
                    let reaped = process.close().await;
                    self.status();
                    if !reaped {
                        return Err(unavailable());
                    }
                }
            }
        }
        self.users.lock().map_err(|_| unavailable())?.commit(page)
    }
    pub async fn connect(&self, generation: &str) -> ui::Result<String> {
        let _switch = self.switching.lock().await;
        let context = self
            .users
            .lock()
            .map_err(|_| unavailable())?
            .require(generation)?;
        let process = self
            .process()
            .filter(|p| p.ready())
            .ok_or_else(unavailable)?;
        let mut connections = self.connections.lock().await;
        if connections.len() >= 4 {
            return Err(unavailable());
        }
        let id = format!("view-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let reader = process.control.view(id.clone())?;
        if let Err(error) = process
            .control
            .attach(&id, &context, Duration::from_secs(15))
            .await
        {
            process.control.detach(&id);
            return Err(error);
        }
        connections.insert(
            id.clone(),
            Arc::new(Connection {
                reader: Mutex::new(reader),
                stop: process.stop.child_token(),
                control: process.control.clone(),
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
            message = reader.recv() => message.map(Some).ok_or_else(unavailable),
        }
    }
    pub async fn send(&self, id: &str, message: Value) -> ui::Result<()> {
        if serde_json::to_vec(&message)
            .map_err(|_| unavailable())?
            .len()
            > 262144
        {
            return Err(unavailable());
        }
        let connection = self.connection(id).await?;
        if connection.stop.is_cancelled() {
            return Err(unavailable());
        }
        connection.control.send(id, message).await
    }
    pub async fn save_connection(
        &self,
        generation: &str,
        connection: ai_session_contract::Connection,
        expected: Option<u64>,
        secret: Option<String>,
    ) -> ui::Result<Value> {
        self.current(generation)?;
        let epoch = self.epoch();
        let process = self
            .process()
            .filter(|p| p.ready())
            .ok_or_else(unavailable)?;
        let result = process
            .control
            .save_connection(
                generation,
                connection,
                expected,
                secret,
                Duration::from_secs(100),
            )
            .await?;
        self.current(generation)?;
        if epoch != self.epoch() {
            return Err(unavailable());
        }
        Ok(result)
    }
    pub async fn disconnect(&self, id: &str) {
        let connection = self.connections.lock().await.remove(id);
        if let Some(connection) = connection {
            connection.stop.cancel();
            connection.control.detach(id);
            let _ = connection
                .control
                .detach_remote(id, Duration::from_secs(2))
                .await;
        }
    }
    pub async fn detach_views(&self) {
        let connections = std::mem::take(&mut *self.connections.lock().await);
        for (id, connection) in connections {
            connection.stop.cancel();
            connection.control.detach(&id);
            let _ = connection
                .control
                .detach_remote(&id, Duration::from_secs(2))
                .await;
        }
    }
    pub async fn shutdown(&self) {
        let _switch = self.switching.lock().await;
        self.detach_views().await;
        if let Some(process) = self.process() {
            process.close().await;
        }
        self.host.lock().unwrap().owner = Host::Closed;
        self.execution.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoKey;
    impl KeyBackend for NoKey {
        fn read(&self) -> Result<Option<Vec<u8>>, super::super::credentials::KeyUnavailable> {
            Ok(None)
        }
        fn create(&self, _: &[u8]) -> Result<(), super::super::credentials::KeyUnavailable> {
            Ok(())
        }
    }
    #[tokio::test]
    async fn missing_runtime_preserves_users_and_execution_and_coalesces_restart() {
        let root = std::env::temp_dir().join(format!("rss-host-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let runtime = DesktopRuntime::start_with_key_backend(
            &root,
            &root.join("missing"),
            ai_session_contract::HostStatusSource::DevelopmentOverride,
            NoKey,
        )
        .await
        .unwrap();
        let status = serde_json::to_value(runtime.status()).unwrap();
        assert_eq!(status["phase"], "failed");
        assert_eq!(status["diagnostic"]["code"], "runtime_missing");
        assert!(!status.to_string().contains(root.to_str().unwrap()));
        let user = runtime.select_user("Alice").await.unwrap();
        assert!(runtime.execution_for(user.generation.as_str()).is_ok());
        let generation = status["generation"].as_u64().unwrap();
        let (first, second) =
            tokio::join!(runtime.restart(generation), runtime.restart(generation));
        assert_eq!(first.generation.0, second.generation.0);
        assert_eq!(runtime.epoch(), generation + 1);
        assert!(runtime.connect(user.generation.as_str()).await.is_err());
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    fn fixture(root: &Path, mode: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let artifact = root.join("artifact");
        private_directory(&artifact.join("bin")).unwrap();
        std::fs::write(artifact.join("manifest.json"),r#"{"status":"passed","desktopProtocol":1,"contractVersion":5,"verification":{"platform":"darwin","arch":"arm64"},"runtimeTreeSha256":"0000000000000000000000000000000000000000000000000000000000000000"}"#).unwrap();
        for name in [
            "bin/node",
            "package.json",
            "pnpm-lock.yaml",
            "node_modules/@rss-mdm-agent/ai-host-app/dist/cli.js",
            "node_modules/@rss-mdm-agent/ai-host/dist/index.js",
            "node_modules/@rss-mdm-agent/ai-contract/dist/index.js",
            "node_modules/@rss-mdm-agent/ai-store-sqlite/dist/index.js",
            "node_modules/@rss-mdm-agent/ai-access/dist/index.js",
        ] {
            let path = artifact.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"fixture").unwrap();
            if name == "bin/node" {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        let script = format!(
            r#"#!/usr/bin/python3
import socket,json,os,sys,time,signal
if {mode:?} == 'ignore_term': signal.signal(signal.SIGTERM, signal.SIG_IGN)
if {mode:?} == 'exit': sys.exit(7)
stream=socket.socket(fileno=3).makefile('rwb',buffering=0)
for line in stream:
    frame=json.loads(line)
    if frame['kind'] != 'nativeCall': continue
    if frame['method']=='health' and {mode:?} == 'delayed': time.sleep(0.4)
    if frame['method']=='health' and {mode:?} == 'no_health': time.sleep(60)
    value={{'schemaVersion':5,'kind':'hostHealth','ready':True,'protocol':1}} if frame['method']=='health' else True
    stream.write((json.dumps({{'schemaVersion':5,'kind':'nativeReply','id':frame['id'],'ok':True,'value':value}})+'\n').encode())
"#
        );
        let executable = artifact.join("bin/rss-ai-host");
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        artifact
    }
    #[tokio::test]
    async fn ready_requires_health_and_restart_reaps_the_previous_incarnation() {
        let root = std::env::temp_dir().join(format!("rss-health-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = fixture(&root, "ready");
        let runtime = DesktopRuntime::start_with_key_backend(
            &root,
            &artifact,
            ai_session_contract::HostStatusSource::DevelopmentOverride,
            NoKey,
        )
        .await
        .unwrap();
        assert_eq!(
            runtime.status().phase,
            ai_session_contract::HostStatusPhase::Ready
        );
        let context = runtime.select_user("Alice").await.unwrap();
        let connection = runtime.connect(context.generation.as_str()).await.unwrap();
        let old = runtime.process().unwrap();
        let generation = runtime.epoch();
        let (a, b) = tokio::join!(runtime.restart(generation), runtime.restart(generation));
        assert_eq!(a.generation.0, b.generation.0);
        assert_eq!(a.phase, ai_session_contract::HostStatusPhase::Ready);
        assert!(old.stop.is_cancelled());
        assert!(runtime.receive(&connection).await.is_err());
        assert!(runtime.connections.lock().await.is_empty());
        assert!(runtime.execution_for(context.generation.as_str()).is_ok());
        runtime.shutdown().await;
        assert_eq!(
            runtime.restart(a.generation.0 as u64).await.phase,
            ai_session_contract::HostStatusPhase::Stopped
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn early_exit_and_invalid_package_have_distinct_redacted_diagnostics() {
        let root = std::env::temp_dir().join(format!("rss-health-fail-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = fixture(&root, "exit");
        let runtime = DesktopRuntime::start_with_key_backend(
            &root,
            &artifact,
            ai_session_contract::HostStatusSource::DevelopmentOverride,
            NoKey,
        )
        .await
        .unwrap();
        if let Some(process) = runtime.process() {
            for _ in 0..100 {
                if matches!(process.phase(), Phase::Failed(Fault::Exited)) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        assert_eq!(
            serde_json::to_value(runtime.status()).unwrap()["diagnostic"]["code"],
            "host_exited"
        );
        std::fs::write(artifact.join("manifest.json"), r#"{"desktopProtocol":0}"#).unwrap();
        let generation = runtime.epoch();
        let failed = runtime.restart(generation).await;
        assert_eq!(
            serde_json::to_value(failed).unwrap()["diagnostic"]["code"],
            "unsupported_version"
        );
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn starting_snapshot_is_coherent_and_no_channel_opens_before_health() {
        let root = std::env::temp_dir().join(format!("rss-starting-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = root.join("artifact");
        let runtime = Arc::new(
            DesktopRuntime::start_with_key_backend(
                &root,
                &artifact,
                ai_session_contract::HostStatusSource::DevelopmentOverride,
                NoKey,
            )
            .await
            .unwrap(),
        );
        let user = runtime.select_user("Alice").await.unwrap();
        let epoch = runtime.epoch();
        fixture(&root, "delayed");
        let r = runtime.clone();
        let restart = tokio::spawn(async move { r.restart(epoch).await });
        for _ in 0..100 {
            if runtime.status().phase == ai_session_contract::HostStatusPhase::Starting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let status = runtime.status();
        assert_eq!(status.generation.0 as u64, epoch + 1);
        assert_eq!(status.phase, ai_session_contract::HostStatusPhase::Starting);
        assert!(tokio::time::timeout(
            Duration::from_millis(40),
            runtime.connect(user.generation.as_str())
        )
        .await
        .is_err());
        assert_eq!(
            restart.await.unwrap().phase,
            ai_session_contract::HostStatusPhase::Ready
        );
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn a_lost_control_pipe_fences_user_switch_until_forced_exit_and_keeps_cleanup_evidence() {
        let root = std::env::temp_dir().join(format!("rss-forced-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = fixture(&root, "ignore_term");
        let runtime = Arc::new(
            DesktopRuntime::start_with_key_backend(
                &root,
                &artifact,
                ai_session_contract::HostStatusSource::DevelopmentOverride,
                NoKey,
            )
            .await
            .unwrap(),
        );
        let alice = runtime.select_user("Alice").await.unwrap();
        runtime.process().unwrap().control.close();
        let r = runtime.clone();
        let switching = tokio::spawn(async move { r.select_user("Bob").await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!switching.is_finished());
        assert_eq!(
            runtime
                .users
                .lock()
                .unwrap()
                .current()
                .unwrap()
                .generation
                .as_str(),
            alice.generation.as_str()
        );
        let bob = tokio::time::timeout(Duration::from_secs(12), switching)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_ne!(bob.generation.as_str(), alice.generation.as_str());
        assert_eq!(
            serde_json::to_value(runtime.status()).unwrap()["diagnostic"]["code"],
            "cleanup_incomplete"
        );
        fixture(&root, "ready");
        let status = runtime.restart(runtime.epoch()).await;
        assert_eq!(status.phase, ai_session_contract::HostStatusPhase::Ready);
        assert!(status
            .recent
            .iter()
            .any(|d| d.code.to_string() == "cleanup_incomplete"));
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn absent_health_is_bounded_and_never_ready() {
        let root = std::env::temp_dir().join(format!("rss-timeout-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = fixture(&root, "no_health");
        let runtime = tokio::time::timeout(
            Duration::from_secs(20),
            DesktopRuntime::start_with_key_backend(
                &root,
                &artifact,
                ai_session_contract::HostStatusSource::DevelopmentOverride,
                NoKey,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            serde_json::to_value(runtime.status()).unwrap()["diagnostic"]["code"],
            "readiness_timeout"
        );
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn preflight_rejects_missing_node_cli_and_dependencies_without_launching() {
        let root = std::env::temp_dir().join(format!("rss-preflight-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        for file in [
            "bin/node",
            "node_modules/@rss-mdm-agent/ai-host-app/dist/cli.js",
            "node_modules/@rss-mdm-agent/ai-store-sqlite/dist/index.js",
        ] {
            let artifact = fixture(&root, "ready");
            std::fs::remove_file(artifact.join(file)).unwrap();
            let runtime = DesktopRuntime::start_with_key_backend(
                &root,
                &artifact,
                ai_session_contract::HostStatusSource::DevelopmentOverride,
                NoKey,
            )
            .await
            .unwrap();
            assert_eq!(
                serde_json::to_value(runtime.status()).unwrap()["diagnostic"]["code"],
                "runtime_invalid"
            );
            assert!(runtime.process().is_none());
            runtime.shutdown().await;
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
