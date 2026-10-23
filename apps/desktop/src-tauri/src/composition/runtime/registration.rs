//! Native authorization hands off to normal account activation only after device activation.
use super::*;
use crate::self_service::{error, Result};
use execution_ipc::host::{
    CredentialStatus, RegistrationContext, RegistrationPhase, RegistrationStatus, Reply, Request,
    ServiceClient,
};

async fn exchange(request: Request) -> Result<Reply> {
    tokio::task::spawn_blocking(move || {
        ServiceClient::installed()
            .and_then(|client| client.request(request))
            .map_err(|_| error("service_unavailable", "本机注册服务不可用"))
    })
    .await
    .map_err(|_| error("service_unavailable", "本机注册服务不可用"))?
}
async fn context() -> Result<RegistrationContext> {
    match exchange(Request::RegistrationContext {}).await? {
        Reply::RegistrationContext { value } => Ok(value),
        Reply::Rejected => Err(error("registration_denied", "当前桌面没有本机注册权限")),
        _ => Err(error("service_unavailable", "本机注册状态不可用")),
    }
}
impl DesktopRuntime {
    pub async fn register_device(
        &self,
        credentials: Option<super::super::account::EnterpriseCredentials>,
    ) -> Result<RegistrationStatus> {
        let _switch = self.switching.lock().await;
        let initial = context().await?;
        let mut account = self.identity.session()?;
        if !account
            .as_ref()
            .is_some_and(|a| a.matches_registration(&initial.origin, &initial.tenant_id))
        {
            account = None;
        }
        // An explicit login form submission can replace an expired session; a passive retry cannot.
        if account.is_none() || credentials.is_some() {
            let credentials = credentials.ok_or_else(|| {
                error(
                    "registration_login_required",
                    "请登录安装包所属组织以注册此设备",
                )
            })?;
            credentials.validate()?;
            let organization = self
                .organizations
                .lock()
                .map_err(|_| unavailable())?
                .get(&credentials.organization_id)?;
            if super::super::account::registration_origin(&organization)? != initial.origin
                || organization.tenant_id != initial.tenant_id
            {
                return Err(error(
                    "registration_organization_mismatch",
                    "所选组织与本机安装包不一致",
                ));
            }
            let login = credentials.login;
            let password = credentials.password;
            self.retire_locked().await?;
            let session =
                super::super::account::EnterpriseSession::login(organization, &login, password)
                    .await?;
            let next = session.context()?;
            self.identity.set_session(Some(session.clone()))?;
            // Publishing a user view is separate from disclosing credentials to execution IPC.
            let published =
                self.identity
                    .pending
                    .lock()
                    .map_err(|_| unavailable())
                    .map(|mut pending| {
                        *pending = Some(next);
                    });
            if let Err(e) = published {
                self.identity.set_session(None)?;
                let _ = session.logout().await;
                return Err(e);
            }
            account = Some(session);
        }
        let mut account = account.ok_or_else(unavailable)?;
        account.verify().await?;
        self.identity.set_session(Some(account))?;
        let generation = self
            .identity
            .pending
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
            .map(|u| u.generation.clone())
            .or_else(|| self.identity.snapshot().ok().map(|u| u.generation))
            .ok_or_else(unavailable)?;
        let intent = self.registration_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        drop(_switch);
        let mut prepared = initial.status.registered;
        let mut authorized = None;
        loop {
            self.registration_current(intent, &generation)?;
            if !prepared {
                match exchange(Request::RequestRegistration {}).await {
                    Ok(Reply::RegistrationStatus { .. }) => prepared = true,
                    Ok(Reply::Rejected) => {
                        return Err(error("registration_denied", "本机注册请求被拒绝"))
                    }
                    _ => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                }
            }
            let current = match context().await {
                Ok(current) => current,
                Err(e) if e.code == "service_unavailable" => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
                Err(e) => return Err(e),
            };
            if current.origin != initial.origin || current.tenant_id != initial.tenant_id {
                return Err(error(
                    "registration_organization_mismatch",
                    "安装组织发生变化，已停止授权",
                ));
            }
            if self
                .registration_handoff(&current, intent, &generation)
                .await?
            {
                return Ok(current.status);
            }
            if current.status.phase == RegistrationPhase::AwaitingAuthorization {
                let _switch = self.switching.lock().await;
                self.registration_current(intent, &generation)?;
                let id = current
                    .status
                    .grant_id
                    .as_deref()
                    .and_then(|v| uuid::Uuid::parse_str(v).ok())
                    .ok_or_else(|| error("registration_contract", "本机注册请求无效"))?;
                if current
                    .status
                    .public_key_fingerprint
                    .as_ref()
                    .is_none_or(|v| v.len() != 64 || !v.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    return Err(error("registration_contract", "本机公钥摘要不可用"));
                }
                let mut account = self.identity.session()?.ok_or_else(unavailable)?;
                match if authorized == Some(id) {
                    Ok(())
                } else {
                    account.authorize_registration(id).await
                } {
                    Ok(()) => {
                        self.identity.set_session(Some(account))?;
                        authorized = Some(id);
                    }
                    Err(e)
                        if super::super::account::failure_snapshot(&e).is_some_and(|f| {
                            matches!(
                                f.reason,
                                ai_session_contract::AccountFailureKind::Unavailable
                                    | ai_session_contract::AccountFailureKind::RateLimited
                            )
                        }) => {}
                    Err(e) => return Err(e),
                }
            } else if matches!(
                current.status.phase,
                RegistrationPhase::Rejected | RegistrationPhase::Paused
            ) || (current.status.registered
                && current.status.credential != CredentialStatus::Active)
            {
                return Ok(current.status);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    fn registration_current(
        &self,
        intent: u64,
        generation: &ai_session_contract::Id,
    ) -> Result<ai_session_contract::UserContext> {
        if self.closed() || self.registration_epoch.load(Ordering::Acquire) != intent {
            return Err(error(
                "registration_cancelled",
                "已停止注册等待；已提交的原请求仍保留",
            ));
        }
        if let Some(pending) = self
            .identity
            .pending
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
        {
            if &pending.generation == generation {
                return Ok(pending.clone());
            }
        }
        self.current(generation.as_str())
    }
    async fn registration_handoff(
        &self,
        current: &RegistrationContext,
        intent: u64,
        generation: &ai_session_contract::Id,
    ) -> Result<bool> {
        if !current.status.registered || current.status.credential != CredentialStatus::Active {
            return Ok(false);
        }
        // ref: tokio sync/mutex.rs@tokio-1.53.1 — serialize identity I/O across await.
        let _switch = self.switching.lock().await;
        let next = self.registration_current(intent, generation)?;
        let account = self.identity.session()?.ok_or_else(unavailable)?;
        if !account.matches_registration(&current.origin, &current.tenant_id) {
            return Err(error(
                "registration_organization_mismatch",
                "安装组织发生变化，已停止授权",
            ));
        }
        if next.execution_binding.is_some() {
            return Ok(true);
        }
        // A lost bind ACK may have installed a native context. Retire it before retrying.
        if self.execution.revoke_context().await.is_err() {
            return Ok(false);
        }
        // Activation publishes only after current-user settlement. Any ambiguous result
        // retains this same pending generation for the shared account recovery entry.
        Ok(self
            .activate_context(next)
            .await?
            .execution_binding
            .is_some())
    }
    pub async fn cancel_registration_wait(&self) {
        let _switch = self.switching.lock().await;
        self.registration_epoch.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::account::{
        http_tests::{access, identity, server},
        EnterpriseSession,
    };
    struct NoKey;
    impl KeyBackend for NoKey {
        fn read(
            &self,
        ) -> std::result::Result<Option<Vec<u8>>, crate::composition::credentials::KeyUnavailable>
        {
            Ok(None)
        }
        fn create(
            &self,
            _: &[u8],
        ) -> std::result::Result<(), crate::composition::credentials::KeyUnavailable> {
            Ok(())
        }
    }
    #[tokio::test]
    async fn activated_registration_retries_lost_binding_ack_then_queries_directory() {
        let root =
            std::env::temp_dir().join(format!("rss-registration-handoff-{}", uuid::Uuid::new_v4()));
        let mut runtime = DesktopRuntime::start_with_key_backend(
            &root,
            &root.join("missing"),
            ai_session_contract::HostStatusSource::DevelopmentOverride,
            NoKey,
        )
        .await
        .unwrap();
        let (organization, server) = server(vec![
            (200, identity()),
            (200, identity()),
            (200, access()),
            (200, serde_json::json!({})),
        ])
        .await;
        let mut current = RegistrationContext {
            origin: organization.origin.clone(),
            tenant_id: organization.tenant_id.clone(),
            status: RegistrationStatus {
                task_trust: None,
                pki_trust: None,
                failure: None,
                phase: RegistrationPhase::Authorized,
                grant_id: Some(uuid::Uuid::new_v4().to_string()),
                expires_at: None,
                public_key_fingerprint: Some("a".repeat(64)),
                registered: false,
                credential: CredentialStatus::Absent,
            },
        };
        let account = EnterpriseSession::login_with_client(
            reqwest::Client::new(),
            organization,
            "Alice",
            zeroize::Zeroizing::new("password".into()),
        )
        .await
        .unwrap();
        let next = account.context().unwrap();
        let generation = next.generation.clone();
        runtime.identity.set_session(Some(account)).unwrap();
        *runtime.identity.pending.lock().unwrap() = Some(next);
        let attempts = Arc::new(AtomicU64::new(0));
        let native_bound = Arc::new(AtomicBool::new(false));
        let calls = attempts.clone();
        let bound = native_bound.clone();
        runtime.execution = runtime
            .execution
            .clone()
            .with_context_transport(move |request| match request {
                Request::RetireEnterpriseContext { .. } => Ok(Reply::CurrentUserSync {
                    value: Default::default(),
                }),
                Request::RevokeContext {} => {
                    bound.store(false, Ordering::Release);
                    Ok(Reply::ContextRevoked)
                }
                Request::BindContext { value, expected } => {
                    assert!(expected.is_none());
                    assert!(matches!(
                        value.access,
                        execution_ipc::host::NativeAccess::User { .. }
                    ));
                    assert!(
                        !bound.swap(true, Ordering::AcqRel),
                        "uncertain prior binding must be retired"
                    );
                    if calls.fetch_add(1, Ordering::AcqRel) == 0 {
                        return Ok(Reply::CurrentUserSync {
                            value: execution_contract::CurrentUserSync {
                                phase: execution_contract::CurrentUserSyncPhase::BindingUnknown,
                                current: None,
                            },
                        });
                    }
                    Ok(Reply::ContextBound {
                        value: execution_contract::NativeContextRef {
                            id: execution_contract::Id::new("registered-user").unwrap(),
                            generation: value.generation,
                        },
                    })
                }
                Request::PolicyDirectory { context, .. } => {
                    assert!(bound.load(Ordering::Acquire));
                    assert_eq!(context.id.as_str(), "registered-user");
                    Ok(Reply::PolicyDirectory {
                        value: rss_mdm_agent_wire::runtime::SelfServiceDirectoryResponse {
                            items: vec![],
                            next_cursor: None,
                            as_of: 42,
                        },
                    })
                }
                _ => panic!("handoff must not issue another machine registration"),
            });
        let intent = runtime.registration_epoch.load(Ordering::Acquire);
        assert!(!runtime
            .registration_handoff(&current, intent, &generation)
            .await
            .unwrap());
        assert_eq!(attempts.load(Ordering::Acquire), 0);
        assert!(runtime
            .policy_execution_for(generation.as_str())
            .await
            .is_err());
        current.status.phase = RegistrationPhase::Active;
        current.status.registered = true;
        // A retained identity with no active credential still cannot disclose the cookie.
        assert!(!runtime
            .registration_handoff(&current, intent, &generation)
            .await
            .unwrap());
        assert_eq!(attempts.load(Ordering::Acquire), 0);
        current.status.credential = CredentialStatus::Active;
        assert_eq!(
            runtime
                .registration_handoff(&current, intent, &generation)
                .await
                .unwrap_err()
                .code,
            "binding_unknown"
        );
        assert!(runtime.identity.snapshot().is_err());
        assert_eq!(
            runtime.account_status().unwrap().binding,
            ai_session_contract::AccountBindingState::BindingUnknown
        );
        assert!(native_bound.load(Ordering::Acquire));
        assert!(runtime
            .policy_execution_for(generation.as_str())
            .await
            .is_err());
        assert!(runtime
            .registration_handoff(&current, intent, &generation)
            .await
            .unwrap());
        let directory = runtime
            .policy_execution_for(generation.as_str())
            .await
            .unwrap()
            .policy_directory(crate::self_service::PolicyDirectoryQuery { after: None })
            .await
            .unwrap();
        assert_eq!(directory.as_of, 42);
        assert_eq!(runtime.identity.snapshot().unwrap().generation, generation);
        assert!(runtime
            .registration_handoff(&current, intent, &generation)
            .await
            .unwrap());
        assert_eq!(attempts.load(Ordering::Acquire), 2);
        runtime.cancel_registration_wait().await;
        assert_eq!(
            runtime
                .registration_handoff(&current, intent, &generation)
                .await
                .unwrap_err()
                .code,
            "registration_cancelled"
        );
        let intent = runtime.registration_epoch.load(Ordering::Acquire);
        let stale = uuid::Uuid::new_v4().to_string().try_into().unwrap();
        assert!(runtime
            .registration_handoff(&current, intent, &stale)
            .await
            .is_err());
        current.origin.push_str("/other");
        assert_eq!(
            runtime
                .registration_handoff(&current, intent, &generation)
                .await
                .unwrap_err()
                .code,
            "registration_organization_mismatch"
        );
        assert_eq!(attempts.load(Ordering::Acquire), 2);
        runtime.shutdown().await;
        server.await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
