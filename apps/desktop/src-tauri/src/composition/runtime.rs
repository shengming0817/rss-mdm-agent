//! Stable desktop owners and one replaceable AI Host incarnation.
mod registration;
use super::{
    credentials::{KeyBackend, MasterKey},
    execution::ExecutionHandle,
};
use crate::self_service as ui;
use serde_json::Value;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Mutex;
fn unavailable() -> ui::ServiceError {
    ui::error("ai_unavailable", "AI 服务不可用；已登记任务仍可查询")
}
pub struct DesktopRuntime {
    pub execution: ExecutionHandle,
    pub users: Arc<std::sync::Mutex<super::users::Users>>,
    switching: Mutex<()>,
    registration_epoch: AtomicU64,
    pub organizations: std::sync::Mutex<super::account::Organizations>,
    identity: super::identity::IdentityOwner,
    cleanup_pending: AtomicBool,
    account_failure: std::sync::Mutex<Option<ai_session_contract::AccountFailure>>,
    host: super::supervisor::HostSupervisor,
    master: Arc<MasterKey>,
    views: super::views::ViewChannels,
}
fn private_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    platform_private_storage::directory(path)?;
    Ok(())
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
        let execution = ExecutionHandle::new().with_trusted_users(users.clone());
        // Unit lifecycle/ACL tests must not contact the developer's installed Agent.
        #[cfg(test)]
        let execution = execution.with_context_transport(|request| {
            use execution_ipc::host::{Reply, Request};
            match request {
                Request::BindContext { value, expected } => Ok(Reply::ContextBound {
                    value: execution_contract::NativeContextRef {
                        id: expected.unwrap_or_else(|| {
                            execution_contract::Id::new(uuid::Uuid::new_v4().to_string()).unwrap()
                        }),
                        generation: value.generation,
                    },
                }),
                Request::RevokeContext {} => Ok(Reply::ContextRevoked),
                _ => Err(execution_app::Error::Unavailable),
            }
        });
        let runtime = Self {
            execution,
            identity: super::identity::IdentityOwner::new(users.clone()),
            users,
            switching: Mutex::new(()),
            registration_epoch: AtomicU64::new(0),
            organizations: std::sync::Mutex::new(
                super::account::Organizations::open(root, super::account::bundled_organization()?)
                    .map_err(|_| "organization settings unavailable")?,
            ),
            cleanup_pending: AtomicBool::new(false),
            account_failure: std::sync::Mutex::new(None),
            host: super::supervisor::HostSupervisor::new(root, artifact, source)?,
            master: Arc::new(MasterKey::new(backend)),
            views: super::views::ViewChannels::new(),
        };
        runtime.restart(0).await;
        let _ = runtime.select_device().await;
        Ok(runtime)
    }
    pub fn closed(&self) -> bool {
        self.host.closed()
    }
    fn epoch(&self) -> u64 {
        self.host.epoch()
    }
    fn process(&self) -> Option<Arc<super::host::Process>> {
        self.host.process()
    }
    pub fn status(&self) -> ai_session_contract::HostStatus {
        self.host.status()
    }
    /// A replacement retires old views and native bindings before issuing a new generation.
    pub async fn restart(&self, expected: u64) -> ai_session_contract::HostStatus {
        let _switch = self.switching.lock().await;
        if self.closed() || expected != self.epoch() {
            return self.status();
        }
        let previous = self.identity.snapshot().ok();
        if self.revoke_locked().await.is_err() {
            return self.status();
        }
        if let Some(process) = self.process() {
            let reaped = process.close().await;
            self.status();
            if !reaped {
                self.cleanup_pending.store(true, Ordering::Release);
                return self.status();
            }
        }
        if let Some(mut context) = previous {
            context.generation = uuid::Uuid::new_v4().to_string().try_into().unwrap();
            context.execution_binding = None;
            if self.activate_context(context).await.is_err() {
                return self.status();
            }
        }
        self.host.launch(&self.execution, self.master.clone()).await
    }
    pub fn current(&self, generation: &str) -> ui::Result<ai_session_contract::UserContext> {
        self.current_locked(generation)
    }
    fn current_locked(&self, generation: &str) -> ui::Result<ai_session_contract::UserContext> {
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
        self.execution
            .for_caller(context.user.user_id.as_str())
            .map_err(|_| unavailable())
    }
    pub async fn policy_execution_for(&self, generation: &str) -> ui::Result<ExecutionHandle> {
        let _switch = self.switching.lock().await;
        let context = self.current_locked(generation)?;
        let handle = self
            .execution
            .for_caller(context.user.user_id.as_str())
            .map_err(|_| unavailable())?;
        if context.execution_binding.is_none() {
            return Err(unavailable());
        }
        Ok(handle)
    }
    pub fn account_status(&self) -> ui::Result<ai_session_contract::AccountStatus> {
        Ok(ai_session_contract::AccountStatus {
            kind: ai_session_contract::AccountStatusKind::AccountStatus,
            schema_version: ai_session_contract::AccountStatusSchemaVersion::VALUE,
            current: self.users.lock().map_err(|_| unavailable())?.page().current,
            failure: self
                .account_failure
                .lock()
                .map_err(|_| unavailable())?
                .clone(),
        })
    }
    fn account_result<T>(&self, result: ui::Result<T>) -> ui::Result<T> {
        if let Ok(mut last) = self.account_failure.lock() {
            *last = result
                .as_ref()
                .err()
                .and_then(super::account::failure_snapshot);
        }
        result
    }
    // ref: tokio sync/mutex.rs@tokio-1.53.1 — one serialized I/O owner.
    // Every identity transition attempts remote retirement even after local persistence fails.
    async fn retire_locked(&self) -> ui::Result<()> {
        self.registration_epoch.fetch_add(1, Ordering::AcqRel);
        let local = self.revoke_locked().await;
        let remote = self.logout_remote().await;
        self.account_result(local.and(remote))
    }
    pub async fn select_user(&self, name: &str) -> ui::Result<ai_session_contract::UserContext> {
        let _switch = self.switching.lock().await;
        let page = self
            .users
            .lock()
            .map_err(|_| unavailable())?
            .prepare(name)?;
        self.retire_locked().await?;
        let context = self.users.lock().map_err(|_| unavailable())?.commit(page)?;
        self.account_result(self.activate_context(context).await)
    }
    async fn retry_cleanup(&self) -> ui::Result<()> {
        if self.cleanup_pending.load(Ordering::Acquire) {
            self.execution.revoke_context().await?;
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
        let previous = self.identity.snapshot().ok();
        let cleared = self.users.lock().map_err(|_| unavailable())?.clear();
        self.detach_views().await;
        if let Err(error) = self.execution.revoke_context().await {
            self.cleanup_pending.store(true, Ordering::Release);
            self.detach_views().await;
            if let Some(process) = self.process() {
                process.close().await;
            }
            return Err(error);
        }
        self.retry_cleanup().await?;
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
        self.retire_locked().await?;
        let context = self.users.lock().map_err(|_| unavailable())?.device()?;
        self.activate_context(context).await?;
        Ok(())
    }
    async fn logout_remote(&self) -> ui::Result<()> {
        match self.identity.take_session()? {
            Some(account) => account.logout().await.map_err(|_| {
                ui::error(
                    "logout_unconfirmed",
                    "本机会话已退出，服务端注销未确认；请在企业账户中撤销会话，然后重试切换",
                )
            }),
            None => Ok(()),
        }
    }
    pub async fn select_device(&self) -> ui::Result<ai_session_contract::UserContext> {
        let _switch = self.switching.lock().await;
        let next = self.users.lock().map_err(|_| unavailable())?.device()?;
        self.retire_locked().await?;
        self.account_result(self.activate_context(next).await)
    }
    pub async fn login(
        &self,
        organization: super::account::Organization,
        login: &str,
        password: String,
    ) -> ui::Result<ai_session_contract::UserContext> {
        self.login_with(|| super::account::EnterpriseSession::login(organization, login, password))
            .await
    }
    async fn login_with<F, Fut>(
        &self,
        authenticate: F,
    ) -> ui::Result<ai_session_contract::UserContext>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ui::Result<super::account::EnterpriseSession>>,
    {
        let _switch = self.switching.lock().await;
        // Revoke before authenticating another organization: a failed login cannot retain the old grant.
        self.retire_locked().await?;
        let account = self.account_result(authenticate().await)?;
        let next = account.context()?;
        self.identity.set_session(Some(account))?;
        let result = self.activate_context(next).await;
        let result = match result {
            Ok(context) => Ok(context),
            Err(error) => match self.logout_remote().await {
                Ok(()) => Err(error),
                Err(logout) => Err(logout),
            },
        };
        self.account_result(result)
    }
    async fn activate_context(
        &self,
        mut next: ai_session_contract::UserContext,
    ) -> ui::Result<ai_session_contract::UserContext> {
        let session = match self.identity.session()? {
            Some(account) => account.native_session(next.generation.as_str())?,
            None => execution_ipc::host::NativeContextSession {
                access: execution_ipc::host::NativeAccess::Device {},
                generation: execution_contract::Id::new(next.generation.as_str())
                    .map_err(|_| unavailable())?,
                expires_at_unix_ms: u64::MAX,
            },
        };
        let published = async {
            match self.execution.bind_context(session, None).await {
                Ok(binding) => {
                    next.execution_binding =
                        Some(binding.id.as_str().parse().map_err(|_| unavailable())?)
                }
                Err(error) => {
                    // Independent conversation is available without an installed execution owner.
                    // Uncertain retirement remains a barrier to activating another identity.
                    if self.cleanup_pending.load(Ordering::Acquire) {
                        return Err(error);
                    }
                }
            }
            self.users.lock().map_err(|_| unavailable())?.activate(next)
        }
        .await;
        if published.is_err() {
            // Ownership transfers only after local publication. Retain uncertain
            // native retirement so the background owner can retry the same cleanup.
            self.cleanup_pending.store(true, Ordering::Release);
            self.detach_views().await;
            let _ = self.retry_cleanup().await;
        }
        published
    }
    /// Single-flight refresh performs HTTP on a cloned session, outside identity and transition locks.
    pub async fn verify_account(&self) -> ui::Result<()> {
        if self.cleanup_pending.load(Ordering::Acquire) {
            let _switch = self.switching.lock().await;
            self.retry_cleanup().await?;
        }
        let requested = std::time::Instant::now();
        let Ok(context) = self.identity.snapshot() else {
            return Ok(());
        };
        let mut flight = self.identity.refresh.lock().await;
        if let Some((generation, completed, result)) = &flight.last {
            if generation == context.generation.as_str() && *completed >= requested {
                return result.clone();
            }
        }
        let result = self.verify_snapshot(&context.generation).await;
        flight.last = Some((
            context.generation.to_string(),
            std::time::Instant::now(),
            result.clone(),
        ));
        result
    }
    async fn verify_snapshot(
        &self,
        requested_generation: &ai_session_contract::Id,
    ) -> ui::Result<()> {
        let Ok(before) = self.identity.snapshot() else {
            return Ok(());
        };
        if &before.generation != requested_generation {
            return Ok(());
        }
        let Some(mut account) = self.identity.session()? else {
            return Ok(());
        };
        let verified = account.verify().await;
        let _switch = self.switching.lock().await;
        if self
            .identity
            .snapshot()
            .map_or(true, |current| current.generation != before.generation)
        {
            return Ok(());
        }
        if let Err(error) = verified {
            let usable = self
                .users
                .lock()
                .map_err(|_| unavailable())?
                .require(before.generation.as_str())
                .is_ok();
            if usable
                && super::account::failure_snapshot(&error).is_some_and(|failure| {
                    matches!(
                        failure.reason,
                        ai_session_contract::AccountFailureKind::Unavailable
                            | ai_session_contract::AccountFailureKind::RateLimited
                    )
                })
            {
                return Ok(());
            }
            self.retire_locked().await?;
            return self.account_result(Err(error));
        }
        let mut refreshed = account.context()?;
        refreshed.generation = before.generation.clone();
        refreshed.execution_binding = before.execution_binding.clone();
        if let Some(expected) = &before.execution_binding {
            self.execution
                .bind_context(
                    account.native_session(before.generation.as_str())?,
                    Some(
                        execution_contract::Id::new(expected.as_str())
                            .map_err(|_| unavailable())?,
                    ),
                )
                .await?;
        }
        self.identity.set_session(Some(account))?;
        self.users
            .lock()
            .map_err(|_| unavailable())?
            .replace_current(refreshed.clone())?;
        if let Some(process) = self.process() {
            process
                .control
                .refresh(&refreshed, Duration::from_secs(2))
                .await
                .map_err(|_| unavailable())?;
        }
        Ok(())
    }
    pub async fn connect(&self, generation: &str) -> ui::Result<String> {
        let _switch = self.switching.lock().await;
        let context = self.current(generation)?;
        let process = self
            .process()
            .filter(|p| p.ready())
            .ok_or_else(unavailable)?;
        self.views.connect(process, &context).await
    }
    pub async fn receive(&self, id: &str) -> ui::Result<Option<Value>> {
        let c = self.views.connection(id).await?;
        self.current(&c.generation)?;
        let value = self.views.receive(&c).await;
        self.current(&c.generation)?;
        value
    }
    pub async fn send(&self, id: &str, message: Value) -> ui::Result<()> {
        let c = self.views.connection(id).await?;
        self.current(&c.generation)?;
        self.views.send(id, &c, message).await
    }
    pub async fn save_connection(
        &self,
        generation: &str,
        connection: ai_session_contract::ConnectionDraft,
        expected: Option<u64>,
        secret: Option<zeroize::Zeroizing<String>>,
    ) -> ui::Result<Value> {
        self.current(generation)?;
        let epoch = self.epoch();
        let process = self
            .process()
            .filter(|p| p.ready())
            .ok_or_else(unavailable)?;
        self.master
            .save_connection(&process, generation, connection, expected, secret, || {
                if epoch != self.epoch() {
                    return Err(unavailable());
                }
                self.current(generation)
            })
            .await
    }
    pub async fn disconnect(&self, id: &str) {
        self.views.disconnect(id).await;
    }
    pub async fn detach_views(&self) {
        self.views.detach_all().await;
    }
    pub async fn shutdown(&self) {
        let _switch = self.switching.lock().await;
        if self.execution.revoke_context().await.is_err() {
            // Preserve the unresolved retirement; a subsequent identity cannot be activated.
            self.cleanup_pending.store(true, Ordering::Release);
            eprintln!("RSS_CONTEXT_RETIREMENT_UNCONFIRMED");
            self.detach_views().await;
            if let Some(process) = self.process() {
                process.close().await;
            }
            return;
        }
        self.detach_views().await;
        if let Some(process) = self.process() {
            process.close().await;
        }
        let _ = self.users.lock().map(|mut users| {
            if users.page().current.as_ref().is_some() {
                let _ = users.clear();
            }
        });
        if let Ok(Some(account)) = self.identity.take_session() {
            let _ = account.logout().await;
        }
        self.host.close();
        self.execution.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::super::host::{Fault, Phase};
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    struct NoKey;
    impl KeyBackend for NoKey {
        fn read(&self) -> Result<Option<Vec<u8>>, super::super::credentials::KeyUnavailable> {
            Ok(None)
        }
        fn create(&self, _: &[u8]) -> Result<(), super::super::credentials::KeyUnavailable> {
            Ok(())
        }
    }
    fn fail_publication_after_binding(
        runtime: &mut DesktopRuntime,
        root: &Path,
        lost_ack: Arc<AtomicBool>,
    ) -> Arc<AtomicBool> {
        use execution_ipc::host::{Reply, Request};
        let bound = Arc::new(AtomicBool::new(false));
        let native_bound = bound.clone();
        let users_path = root.join("users.json");
        runtime.execution = runtime
            .execution
            .clone()
            .with_context_transport(move |request| match request {
                Request::BindContext { value, .. } => {
                    native_bound.store(true, Ordering::Release);
                    std::fs::remove_file(&users_path).unwrap();
                    std::fs::create_dir(&users_path).unwrap();
                    Ok(Reply::ContextBound {
                        value: execution_contract::NativeContextRef {
                            id: execution_contract::Id::new("test-native-binding").unwrap(),
                            generation: value.generation,
                        },
                    })
                }
                Request::RevokeContext {} => {
                    if native_bound.load(Ordering::Acquire) && lost_ack.load(Ordering::Acquire) {
                        return Err(execution_app::Error::OutcomeUnknown);
                    }
                    native_bound.store(false, Ordering::Release);
                    Ok(Reply::ContextRevoked)
                }
                _ => panic!("unexpected context transport request"),
            });
        bound
    }
    #[tokio::test]
    async fn failed_publication_retires_binding_or_retains_the_same_cleanup_barrier() {
        for lost in [false, true] {
            let root = std::env::temp_dir().join(format!("rss-publish-{}", uuid::Uuid::new_v4()));
            let mut runtime = DesktopRuntime::start_with_key_backend(
                &root,
                &root.join("missing"),
                ai_session_contract::HostStatusSource::DevelopmentOverride,
                NoKey,
            )
            .await
            .unwrap();
            let lost_ack = Arc::new(AtomicBool::new(lost));
            let bound = fail_publication_after_binding(&mut runtime, &root, lost_ack.clone());
            let next = runtime.users.lock().unwrap().device().unwrap();
            assert_eq!(
                runtime.activate_context(next).await.unwrap_err().code,
                "users_unavailable"
            );
            assert!(runtime.identity.snapshot().is_err());
            assert_eq!(bound.load(Ordering::Acquire), lost);
            assert_eq!(runtime.cleanup_pending.load(Ordering::Acquire), lost);
            if lost {
                assert!(runtime.verify_account().await.is_err());
                assert!(runtime.cleanup_pending.load(Ordering::Acquire));
                lost_ack.store(false, Ordering::Release);
                runtime.verify_account().await.unwrap();
                assert!(!bound.load(Ordering::Acquire));
                assert!(!runtime.cleanup_pending.load(Ordering::Acquire));
            }
            runtime.shutdown().await;
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[tokio::test]
    async fn failed_login_publication_clears_account_owner_and_reports_remote_logout_failure() {
        use super::super::account::{
            http_tests::{access, identity, server},
            EnterpriseSession,
        };
        for logout_status in [200, 503] {
            let root =
                std::env::temp_dir().join(format!("rss-login-publish-{}", uuid::Uuid::new_v4()));
            let mut runtime = DesktopRuntime::start_with_key_backend(
                &root,
                &root.join("missing"),
                ai_session_contract::HostStatusSource::DevelopmentOverride,
                NoKey,
            )
            .await
            .unwrap();
            let bound = fail_publication_after_binding(
                &mut runtime,
                &root,
                Arc::new(AtomicBool::new(false)),
            );
            let (org, server) = server(vec![
                (200, identity()),
                (200, identity()),
                (200, access()),
                (logout_status, json!({})),
            ])
            .await;
            let error = runtime
                .login_with(|| {
                    EnterpriseSession::login_with_client(
                        reqwest::Client::new(),
                        org,
                        "Alice",
                        "password".into(),
                    )
                })
                .await
                .unwrap_err();
            assert_eq!(
                error.code,
                if logout_status == 200 {
                    "users_unavailable"
                } else {
                    "logout_unconfirmed"
                }
            );
            assert!(runtime.identity.session().unwrap().is_none());
            assert!(runtime.identity.snapshot().is_err());
            assert!(!bound.load(Ordering::Acquire));
            assert!(runtime.account_status().unwrap().failure.is_some());
            assert!(server
                .await
                .unwrap()
                .last()
                .unwrap()
                .contains("/session/logout"));
            runtime.shutdown().await;
            std::fs::remove_dir_all(root).unwrap();
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
        {
            let _switch = runtime.switching.lock().await;
            assert!(runtime.current(user.generation.as_str()).is_ok());
            assert!(runtime.current_locked(user.generation.as_str()).is_ok());
            assert!(runtime.current_locked("retired-generation").is_err());
        }
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
        let guest = runtime.select_device().await.unwrap();
        assert_ne!(test.user.user_id, guest.user.user_id);
        assert!(runtime.execution_for(test.generation.as_str()).is_err());
        assert!(runtime.execution_for(guest.generation.as_str()).is_ok());
        runtime.logout().await.unwrap();
        assert!(runtime.execution_for(guest.generation.as_str()).is_err());
        let again = runtime.select_user("访客").await.unwrap();
        assert_eq!(test.user.user_id, again.user.user_id);
        runtime.shutdown().await;
        assert!(super::super::users::Users::open(&root)
            .unwrap()
            .page()
            .users
            .iter()
            .any(|user| user.user_id == test.user.user_id));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn organization_switch_revokes_a_before_authenticating_b_even_when_b_fails() {
        use super::super::account::{
            http_tests::{access, identity, server},
            EnterpriseSession,
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
                    EnterpriseSession::login_with_client(
                        reqwest::Client::new(),
                        a_org,
                        "Alice",
                        "password".into(),
                    )
                })
                .await
                .unwrap();
            assert!(runtime.execution_for(a.generation.as_str()).is_ok());
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
                    EnterpriseSession::login_with_client(
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
            EnterpriseSession,
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
                    EnterpriseSession::login_with_client(
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
                "guest" => runtime.select_device().await,
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
    #[tokio::test]
    async fn failed_user_persistence_still_attempts_remote_logout_before_returning() {
        use super::super::account::{
            http_tests::{access, identity, server},
            EnterpriseSession,
        };
        let root = std::env::temp_dir().join(format!("rss-retire-{}", uuid::Uuid::new_v4()));
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
            (200, json!({})),
        ])
        .await;
        runtime
            .login_with(|| {
                EnterpriseSession::login_with_client(
                    reqwest::Client::new(),
                    org,
                    "Alice",
                    "password".into(),
                )
            })
            .await
            .unwrap();
        std::fs::remove_file(root.join("users.json")).unwrap();
        std::fs::create_dir(root.join("users.json")).unwrap();
        assert!(runtime.select_user("Bob").await.is_err());
        assert!(runtime.users.lock().unwrap().current().is_err());
        assert!(runtime.identity.session().unwrap().is_none());
        assert!(server
            .await
            .unwrap()
            .last()
            .unwrap()
            .contains("/session/logout"));
        runtime.shutdown().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn temporary_identity_failure_preserves_snapshot_and_coalesces_refresh() {
        use super::super::account::{
            http_tests::{access, identity, server},
            EnterpriseSession,
        };
        for status in [429, 503] {
            let root = std::env::temp_dir().join(format!("rss-refresh-{}", uuid::Uuid::new_v4()));
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
                (status, json!({})),
                (200, json!({})),
            ])
            .await;
            let context = runtime
                .login_with(|| {
                    EnterpriseSession::login_with_client(
                        reqwest::Client::new(),
                        org,
                        "Alice",
                        "password".into(),
                    )
                })
                .await
                .unwrap();
            let before = serde_json::to_value(&context).unwrap();
            // Ordinary identity reads are local, including while an unrelated transition lock is held.
            {
                let _switch = runtime.switching.lock().await;
                assert!(runtime.current(context.generation.as_str()).is_ok());
            }
            let (first, second) = tokio::join!(runtime.verify_account(), runtime.verify_account());
            first.unwrap();
            second.unwrap();
            assert_eq!(
                serde_json::to_value(runtime.current(context.generation.as_str()).unwrap())
                    .unwrap(),
                before
            );
            runtime.select_device().await.unwrap();
            assert_eq!(server.await.unwrap().len(), 5);
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
        let manifest = json!({"status":"passed","desktopProtocol":4,"contractVersion":8,
            "verification":{"platform":if cfg!(windows){"win32"}else{"darwin"},"arch":installation_security::Target::current().unwrap().architecture()},
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
        assert!(runtime.views.is_empty().await);
        assert!(runtime.execution_for(context.generation.as_str()).is_err());
        let renewed = runtime.identity.snapshot().unwrap();
        assert_ne!(renewed.generation, context.generation);
        assert_eq!(renewed.user.user_id, context.user.user_id);
        assert!(runtime.execution_for(renewed.generation.as_str()).is_ok());
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
        let path = artifact.join("manifest.json");
        let mut previous: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        previous["contractVersion"] = Value::from(6);
        std::fs::write(path, serde_json::to_vec(&previous).unwrap()).unwrap();
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
        assert!(runtime.users.lock().unwrap().current().is_err());
        assert!(runtime.current(alice.generation.as_str()).is_err());
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
        assert!(runtime.identity.session().unwrap().is_none());
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
