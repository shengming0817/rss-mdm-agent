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
        atomic::{AtomicBool, AtomicU64, Ordering},
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
    generation: String,
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
    pub organizations: std::sync::Mutex<super::account::Organizations>,
    account: Mutex<Option<super::account::Session>>,
    cleanup_pending: AtomicBool,
    host: std::sync::Mutex<HostState>,
    root: PathBuf,
    artifact: PathBuf,
    source: ai_session_contract::HostStatusSource,
    version: ai_session_contract::HostStatusVersion,
    master: Arc<MasterKey>,
    recent: std::sync::Mutex<Vec<(u64, Fault, ai_session_contract::HostDiagnostic)>>,
    connections: Mutex<BTreeMap<String, Arc<Connection>>>,
    next: AtomicU64,
}
fn private_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    native_process::private_storage::directory(path)?;
    Ok(())
}
fn configuration(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::io::Write;
    let path = root.join("host.json");
    let value = json!({"version":1,"databasePath":root.join("ai.sqlite"),"nativeDirectory":root.join("native"),"workingDirectory":root.join("workspace")});
    let temporary = root.join(format!("host-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = native_process::private_storage::create_new(&temporary)?;
    file.write_all(&serde_json::to_vec(&value)?)?;
    file.sync_all()?;
    drop(file);
    native_process::private_storage::replace(&temporary, &path)?;
    Ok(path)
}
impl DesktopRuntime {
    pub async fn start(
        root: &Path,
        artifact: &Path,
        source: ai_session_contract::HostStatusSource,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_with_key_backend(
            root,
            artifact,
            source,
            super::credentials::platform_backend(),
        )
        .await
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
            organizations: std::sync::Mutex::new(
                super::account::Organizations::open(root)
                    .map_err(|_| "organization settings unavailable")?,
            ),
            account: Mutex::new(None),
            cleanup_pending: AtomicBool::new(false),
            host: std::sync::Mutex::new(HostState {
                generation: 0,
                owner: Host::Stopped,
            }),
            root: root.into(),
            artifact: artifact.into(),
            source,
            version: env!("CARGO_PKG_VERSION")
                .try_into()
                .map_err(|_| "invalid application version")?,
            master: Arc::new(MasterKey::new(backend)),
            recent: std::sync::Mutex::new(Vec::new()),
            connections: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
        };
        runtime.restart(0).await;
        Ok(runtime)
    }
    pub fn closed(&self) -> bool {
        matches!(self.host.lock().unwrap().owner, Host::Closed)
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
            Phase::Starting => (ai_session_contract::HostStatusPhase::Starting, None),
            Phase::Ready => (ai_session_contract::HostStatusPhase::Ready, None),
            Phase::Stopping => (ai_session_contract::HostStatusPhase::Stopping, None),
            Phase::Stopped => (ai_session_contract::HostStatusPhase::Stopped, None),
            Phase::Failed(fault) => (ai_session_contract::HostStatusPhase::Failed, Some(fault)),
        };
        let mut recent = self.recent.lock().unwrap();
        if let Some(fault) = fault {
            if recent
                .last()
                .is_none_or(|(g, f, _)| *g != generation || *f != fault)
            {
                recent.push((generation, fault, fault.diagnostic(self.source)));
                if recent.len() > 64 {
                    recent.remove(0);
                }
            }
        }
        ai_session_contract::HostStatus {
            schema_version: ai_session_contract::HostStatusSchemaVersion::VALUE,
            kind: ai_session_contract::HostStatusKind::HostStatus,
            generation: ai_session_contract::Counter(generation as i64),
            phase,
            source: self.source,
            version: self.version.clone(),
            recent: recent.iter().map(|(_, _, d)| d.clone()).collect(),
            diagnostic: fault.and_then(|_| recent.last().map(|(_, _, d)| d.clone())),
        }
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
        self.cleanup_pending.store(false, Ordering::Release);
        {
            let mut state = self.host.lock().unwrap();
            state.generation += 1;
            state.owner = Host::Starting;
        }
        let trusted_digest = match self.source {
            ai_session_contract::HostStatusSource::DevelopmentOverride => None,
            ai_session_contract::HostStatusSource::BundledResource => {
                match option_env!("RSS_BUNDLED_RUNTIME_SHA256") {
                    Some(digest) => Some(digest),
                    None => {
                        self.host.lock().unwrap().owner = Host::Failed(Fault::Invalid);
                        return self.status();
                    }
                }
            }
        };
        let next = match configuration(&self.root).map_err(|_| Fault::Configuration) {
            Ok(configuration) => match host::launch(
                &self.artifact,
                &configuration,
                &self.execution,
                self.master.clone(),
                trusted_digest,
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
        if self.cleanup_pending.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        self.users
            .lock()
            .map_err(|_| unavailable())?
            .require(generation)
    }
    pub fn execution_for(&self, generation: &str) -> ui::Result<ExecutionHandle> {
        let context = self.current(generation)?;
        if context
            .identity
            .as_ref()
            .is_some_and(|i| matches!(i, ai_session_contract::AccountIdentity::Enterprise { .. }))
        {
            return Err(ui::error(
                "enterprise_execution_unavailable",
                "企业执行尚未接线，请使用测试用户入口体验 S1",
            ));
        }
        self.execution
            .for_caller(context.user.user_id.as_str())
            .map_err(|_| unavailable())
    }
    pub async fn select_user(&self, name: &str) -> ui::Result<ai_session_contract::UserContext> {
        let _switch = self.switching.lock().await;
        self.retry_cleanup().await?;
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
        if self.account.lock().await.is_some() {
            self.users.lock().map_err(|_| unavailable())?.clear()?;
            self.logout_remote().await?;
        }
        self.users.lock().map_err(|_| unavailable())?.commit(page)
    }
    async fn retry_cleanup(&self) -> ui::Result<()> {
        if self.cleanup_pending.load(Ordering::Acquire) {
            if let Some(process) = self.process() {
                if !process.close().await {
                    return Err(unavailable());
                }
            }
            self.cleanup_pending.store(false, Ordering::Release);
        }
        Ok(())
    }
    async fn revoke_locked(&self) -> ui::Result<()> {
        self.retry_cleanup().await?;
        let previous = self.users.lock().map_err(|_| unavailable())?.page().current;
        let cleared = self.users.lock().map_err(|_| unavailable())?.clear();
        self.detach_views().await;
        if let (Some(previous), Some(process)) = (previous, self.process()) {
            let suspend_failed = !process.ready()
                || process
                    .control
                    .suspend(&previous, Duration::from_secs(15))
                    .await
                    .is_err();
            if suspend_failed && !process.close().await {
                self.cleanup_pending.store(true, Ordering::Release);
                return Err(unavailable());
            }
        }
        cleared
    }
    pub async fn logout(&self) -> ui::Result<()> {
        let _switch = self.switching.lock().await;
        let revoke = self.revoke_locked().await;
        let remote = self.logout_remote().await;
        revoke?;
        remote
    }
    async fn logout_remote(&self) -> ui::Result<()> {
        match self.account.lock().await.take() {
            Some(account) => account.logout().await.map_err(|_| {
                ui::error(
                    "logout_unconfirmed",
                    "本机会话已退出，服务端注销未确认；请在企业账户中撤销会话，然后重试切换",
                )
            }),
            None => Ok(()),
        }
    }
    pub async fn select_guest(&self) -> ui::Result<ai_session_contract::UserContext> {
        let _switch = self.switching.lock().await;
        let next = self.users.lock().map_err(|_| unavailable())?.guest()?;
        self.revoke_locked().await?;
        self.logout_remote().await?;
        self.users.lock().map_err(|_| unavailable())?.activate(next)
    }
    pub async fn login(
        &self,
        organization: super::account::Organization,
        login: &str,
        password: String,
    ) -> ui::Result<ai_session_contract::UserContext> {
        self.login_with(|| super::account::Session::login(organization, login, password))
            .await
    }
    async fn login_with<F, Fut>(
        &self,
        authenticate: F,
    ) -> ui::Result<ai_session_contract::UserContext>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ui::Result<super::account::Session>>,
    {
        let _switch = self.switching.lock().await;
        // Revoke before authenticating another organization: a failed login cannot retain the old grant.
        self.revoke_locked().await?;
        self.logout_remote().await?;
        let account = authenticate().await?;
        let next = account.context()?;
        let result = self.users.lock().map_err(|_| unavailable())?.activate(next);
        if result.is_err() {
            let _ = account.logout().await;
        } else {
            *self.account.lock().await = Some(account);
        }
        result
    }
    /// Passive checks never refresh idle or persist an enterprise bearer.
    pub async fn verify_account(&self) -> ui::Result<()> {
        let _switch = self.switching.lock().await;
        self.retry_cleanup().await?;
        let verified = match self.account.lock().await.as_mut() {
            Some(account) => account.verify().await,
            None => Ok(()),
        };
        if let Err(error) = verified {
            let isolation = self.revoke_locked().await;
            if let Some(account) = self.account.lock().await.take() {
                let _ = account.logout().await;
            }
            isolation?;
            return Err(error);
        }
        Ok(())
    }
    pub async fn connect(&self, generation: &str) -> ui::Result<String> {
        self.verify_account().await?;
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
                generation: generation.into(),
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
        self.verify_account().await?;
        self.current(&c.generation)?;
        let mut reader = c.reader.try_lock().map_err(|_| unavailable())?;
        let value = tokio::select! {
            _ = c.stop.cancelled() => Err(unavailable()),
            _ = tokio::time::sleep(Duration::from_secs(20)) => Ok(None),
            message = reader.recv() => message.map(Some).ok_or_else(unavailable),
        };
        self.current(&c.generation)?;
        value
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
        self.verify_account().await?;
        self.current(&connection.generation)?;
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
        self.verify_account().await?;
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
        let _ = self.users.lock().map(|mut users| {
            if users
                .page()
                .current
                .as_ref()
                .is_some_and(|c| c.identity.is_some())
            {
                let _ = users.clear();
            }
        });
        if let Some(account) = self.account.lock().await.take() {
            let _ = account.logout().await;
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
    #[tokio::test]
    async fn guest_and_logout_revoke_the_old_ipc_generation_without_claiming_test_history() {
        let root = std::env::temp_dir().join(format!("rss-account-{}", uuid::Uuid::new_v4()));
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
        let test = runtime.select_user("访客").await.unwrap();
        let guest = runtime.select_guest().await.unwrap();
        assert_ne!(test.user.user_id, guest.user.user_id);
        assert!(runtime.execution_for(test.generation.as_str()).is_err());
        assert!(runtime.execution_for(guest.generation.as_str()).is_ok());
        runtime.logout().await.unwrap();
        assert!(runtime.execution_for(guest.generation.as_str()).is_err());
        let again = runtime.select_user("访客").await.unwrap();
        assert_eq!(test.user.user_id, again.user.user_id);
        runtime.shutdown().await;
        assert_eq!(
            super::super::users::Users::open(&root)
                .unwrap()
                .current()
                .unwrap()
                .user
                .user_id,
            test.user.user_id
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn organization_switch_revokes_a_before_authenticating_b_even_when_b_fails() {
        use super::super::account::{
            http_tests::{access, identity, server},
            Session,
        };
        for fail_b in [false, true] {
            let root =
                std::env::temp_dir().join(format!("rss-organizations-{}", uuid::Uuid::new_v4()));
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
            let (a_org, a_server) = server(vec![
                (200, identity()),
                (200, identity()),
                (200, access()),
                (200, json!({})),
            ])
            .await;
            let a = runtime
                .login_with(|| {
                    Session::login_with_client(
                        reqwest::Client::new(),
                        a_org,
                        "Alice",
                        "password".into(),
                    )
                })
                .await
                .unwrap();
            assert_eq!(
                runtime
                    .execution_for(a.generation.as_str())
                    .err()
                    .unwrap()
                    .code,
                "enterprise_execution_unavailable"
            );
            let responses = if fail_b {
                vec![(401, json!({"code":"invalid_credential"}))]
            } else {
                vec![
                    (200, identity()),
                    (200, identity()),
                    (200, access()),
                    (200, json!({})),
                ]
            };
            let (b_org, b_server) = server(responses).await;
            let b = runtime
                .login_with(|| async {
                    assert!(runtime.users.lock().unwrap().current().is_err());
                    let requests = a_server.await.unwrap();
                    assert!(requests.last().unwrap().contains("/session/logout"));
                    Session::login_with_client(
                        reqwest::Client::new(),
                        b_org,
                        "Alice",
                        "password".into(),
                    )
                    .await
                })
                .await;
            assert!(runtime.current(a.generation.as_str()).is_err());
            if fail_b {
                assert!(b.is_err());
                assert!(runtime.users.lock().unwrap().current().is_err());
            } else {
                let b = b.unwrap();
                assert_ne!(a.generation, b.generation);
                assert_ne!(
                    serde_json::to_value(&a.identity).unwrap()["authorityId"],
                    serde_json::to_value(&b.identity).unwrap()["authorityId"]
                );
                runtime.logout().await.unwrap();
                assert!(runtime.current(b.generation.as_str()).is_err());
            }
            b_server.await.unwrap();
            runtime.shutdown().await;
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[tokio::test]
    async fn every_switch_reports_failed_remote_logout_after_revoking_local_access() {
        use super::super::account::{
            http_tests::{access, identity, server},
            Session,
        };
        for mode in ["test", "guest", "enterprise"] {
            let root = std::env::temp_dir().join(format!("rss-logout-{}", uuid::Uuid::new_v4()));
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
            let (org, server) = server(vec![
                (200, identity()),
                (200, identity()),
                (200, access()),
                (503, json!({})),
            ])
            .await;
            let a = runtime
                .login_with(|| {
                    Session::login_with_client(
                        reqwest::Client::new(),
                        org,
                        "Alice",
                        "password".into(),
                    )
                })
                .await
                .unwrap();
            let result = match mode {
                "test" => runtime.select_user("Alice").await,
                "guest" => runtime.select_guest().await,
                _ => runtime.login_with(|| async { panic!("must not authenticate a new organization before reporting unconfirmed logout") }).await,
            };
            assert_eq!(result.err().unwrap().code, "logout_unconfirmed");
            assert!(runtime.current(a.generation.as_str()).is_err());
            assert!(runtime.users.lock().unwrap().current().is_err());
            server.await.unwrap();
            runtime.shutdown().await;
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    fn fixture_binary() -> &'static Path {
        static BINARY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        BINARY.get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(3)
                .unwrap();
            let output = std::process::Command::new("cargo")
                .args([
                    "build",
                    "--locked",
                    "--message-format=json",
                    "-p",
                    "native-process",
                    "--example",
                    "host-fixture",
                ])
                .current_dir(root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .find_map(|value| {
                    (value["target"]["name"] == "host-fixture")
                        .then(|| value["executable"].as_str().map(PathBuf::from))
                        .flatten()
                })
                .unwrap()
        })
    }
    fn fixture_alive(pid: i32) -> bool {
        #[cfg(unix)]
        {
            unsafe { libc::kill(pid, 0) == 0 }
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::{Foundation::*, System::Threading::*};
            unsafe {
                let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid as u32);
                if handle.is_null() {
                    return GetLastError() == ERROR_ACCESS_DENIED;
                }
                let alive = WaitForSingleObject(handle, 0) == WAIT_TIMEOUT;
                CloseHandle(handle);
                alive
            }
        }
    }
    fn fixture(root: &Path, mode: &str) -> PathBuf {
        let artifact = root.join("artifact");
        private_directory(&artifact.join("bin")).unwrap();
        let suffix = if cfg!(windows) { ".exe" } else { "" };
        for name in [
            "package.json",
            "pnpm-lock.yaml",
            "NODE-LICENSE",
            "worker-manifest.json",
            "node_modules/@rss-mdm-agent/ai-host/dist/index.js",
            "node_modules/@rss-mdm-agent/ai-contract/dist/index.js",
            "node_modules/@rss-mdm-agent/ai-store-sqlite/dist/index.js",
            "node_modules/@rss-mdm-agent/ai-access/dist/index.js",
        ] {
            let path = artifact.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"fixture").unwrap();
        }
        for binary in ["node", "rss-ai-worker-launcher", "rss-private-storage"] {
            std::fs::copy(
                fixture_binary(),
                artifact.join(format!("bin/{binary}{suffix}")),
            )
            .unwrap();
        }
        let script = artifact.join("node_modules/@rss-mdm-agent/ai-host-app/dist/cli.js");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(script, mode).unwrap();
        let manifest = json!({"status":"passed","desktopProtocol":4,"contractVersion":5,
            "verification":{"platform":if cfg!(windows){"win32"}else{"darwin"},"arch":if cfg!(windows){"x64"}else{"arm64"}},
            "runtimeTreeSha256":super::super::runtime_package::digest(&artifact).unwrap()});
        std::fs::write(
            artifact.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
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
    async fn ready_host_nonzero_shutdown_remains_cleanup_incomplete() {
        let root = std::env::temp_dir().join(format!("rss-nonzero-stop-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = fixture(&root, "nonzero_term");
        let runtime = DesktopRuntime::start_with_key_backend(
            &root,
            &artifact,
            ai_session_contract::HostStatusSource::DevelopmentOverride,
            NoKey,
        )
        .await
        .unwrap();
        let process = runtime.process().unwrap();
        assert_eq!(process.phase(), Phase::Ready);
        assert!(process.close().await);
        assert_eq!(process.phase(), Phase::Failed(Fault::Cleanup));
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn surviving_scope_blocks_restart_until_the_same_owner_confirms_empty() {
        let root = std::env::temp_dir().join(format!("rss-scope-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = fixture(&root, "leak");
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
        let generation = runtime.epoch();
        let blocked = runtime.restart(generation).await;
        assert_eq!(blocked.generation.0 as u64, generation);
        assert_eq!(blocked.phase, ai_session_contract::HostStatusPhase::Failed);
        let pid: i32 = std::fs::read_to_string(root.join("descendant.pid"))
            .unwrap()
            .parse()
            .unwrap();
        for _ in 0..500 {
            if !fixture_alive(pid) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!fixture_alive(pid));
        fixture(&root, "ready");
        let ready = runtime.restart(generation).await;
        assert_eq!(ready.generation.0 as u64, generation + 1);
        assert_eq!(ready.phase, ai_session_contract::HostStatusPhase::Ready);
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn revoked_account_retries_the_same_incomplete_host_cleanup() {
        let root =
            std::env::temp_dir().join(format!("rss-revoke-cleanup-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = fixture(&root, "leak");
        let runtime = DesktopRuntime::start_with_key_backend(
            &root,
            &artifact,
            ai_session_contract::HostStatusSource::DevelopmentOverride,
            NoKey,
        )
        .await
        .unwrap();
        let context = runtime.select_user("Alice").await.unwrap();
        let process = runtime.process().unwrap();
        process.control.close();
        assert!(runtime.logout().await.is_err());
        assert!(runtime.cleanup_pending.load(Ordering::Acquire));
        assert!(runtime.current(context.generation.as_str()).is_err());
        // No account remains. The background verifier must still retry the same Host scope.
        assert!(runtime.account.lock().await.is_none());
        for _ in 0..100 {
            if runtime.verify_account().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(!runtime.cleanup_pending.load(Ordering::Acquire));
        assert!(Arc::ptr_eq(&process, &runtime.process().unwrap()));
        assert!(runtime.users.lock().unwrap().current().is_err());
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
        let capabilities_retained = runtime.process().is_some_and(|p| !p.control.closed());
        let pid: i32 = std::fs::read_to_string(root.join("fixture.pid"))
            .unwrap()
            .parse()
            .unwrap();
        // SAFETY: signal zero only tests process existence; it sends no signal.
        let alive = fixture_alive(pid);
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            !capabilities_retained,
            "failed health must revoke the private control owner before returning"
        );
        assert!(
            !alive,
            "failed health must reap the spawned process before returning"
        );
    }
    #[tokio::test]
    async fn changed_runtime_bytes_are_rejected_before_process_creation() {
        let root =
            std::env::temp_dir().join(format!("rss-runtime-integrity-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let artifact = fixture(&root, "ready");
        let executable = if cfg!(windows) {
            "bin/node.exe"
        } else {
            "bin/node"
        };
        std::fs::write(artifact.join(executable), b"modified executable bytes").unwrap();
        let runtime = DesktopRuntime::start_with_key_backend(
            &root,
            &artifact,
            ai_session_contract::HostStatusSource::DevelopmentOverride,
            NoKey,
        )
        .await
        .unwrap();
        let status = serde_json::to_value(runtime.status()).unwrap();
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
        assert_eq!(status["diagnostic"]["code"], "runtime_invalid");
    }
    #[tokio::test]
    async fn bad_health_and_closed_bootstrap_frames_reap_the_process_and_reach_status_history() {
        for (mode, expected) in [
            ("bad_health", "unsupported_version"),
            ("diagnostic_configuration_invalid", "configuration_invalid"),
            (
                "diagnostic_authentication_required",
                "authentication_required",
            ),
            ("diagnostic_storage_corrupt", "storage_corrupt"),
            ("diagnostic_unsupported_version", "unsupported_version"),
            ("diagnostic_host_start_failed", "host_start_failed"),
            ("diagnostic_cleanup_incomplete", "cleanup_incomplete"),
        ] {
            let root = std::env::temp_dir().join(format!("rss-bootstrap-{}", uuid::Uuid::new_v4()));
            private_directory(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let artifact = fixture(&root, mode);
            let runtime = DesktopRuntime::start_with_key_backend(
                &root,
                &artifact,
                ai_session_contract::HostStatusSource::DevelopmentOverride,
                NoKey,
            )
            .await
            .unwrap();
            let status = serde_json::to_value(runtime.status()).unwrap();
            let pid: i32 = std::fs::read_to_string(root.join("fixture.pid"))
                .unwrap()
                .parse()
                .unwrap();
            // SAFETY: signal zero only checks this fixture's process existence.
            let alive = fixture_alive(pid);
            runtime.shutdown().await;
            std::fs::remove_dir_all(root).unwrap();
            assert!(!alive, "{mode} must be reaped");
            assert_eq!(status["diagnostic"]["code"], expected, "{mode}");
            if mode == "diagnostic_unsupported_version" {
                assert_eq!(status["diagnostic"]["stage"], "storage");
                assert_eq!(status["diagnostic"]["action"], "check_storage");
            }
            assert_eq!(
                status["recent"].as_array().unwrap().last().unwrap()["code"],
                expected
            );
        }
    }
    #[tokio::test]
    async fn preflight_rejects_missing_node_cli_and_dependencies_without_launching() {
        let root = std::env::temp_dir().join(format!("rss-preflight-{}", uuid::Uuid::new_v4()));
        private_directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        for file in [
            if cfg!(windows) {
                "bin/node.exe"
            } else {
                "bin/node"
            },
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
