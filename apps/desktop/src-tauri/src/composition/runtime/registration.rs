//! Explicit device authorization reuses the native session, without binding execution credentials.
use super::*;
use crate::self_service::{error, Result};
use execution_ipc::host::{
    RegistrationContext, RegistrationPhase, RegistrationStatus, Reply, Request, ServiceClient,
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
    pub async fn register_device<R: tauri::Runtime>(
        &self,
        app: tauri::AppHandle<R>,
        organization_id: Option<String>,
        login: Option<String>,
    ) -> Result<RegistrationStatus> {
        let _switch = self.switching.lock().await;
        let initial = context().await?;
        if initial.status.registered {
            return Ok(initial.status);
        }
        let mut account = self.identity.session()?;
        if !account
            .as_ref()
            .is_some_and(|a| a.matches_registration(&initial.origin, &initial.tenant_id))
        {
            account = None;
        }
        // An explicit login form submission can replace an expired session; a passive retry cannot.
        if account.is_none() || organization_id.is_some() {
            let id = organization_id.ok_or_else(|| {
                error(
                    "registration_login_required",
                    "请登录安装包所属组织以注册此设备",
                )
            })?;
            let organization = self
                .organizations
                .lock()
                .map_err(|_| unavailable())?
                .get(&id)?;
            if organization.origin != initial.origin || organization.tenant_id != initial.tenant_id
            {
                return Err(error(
                    "registration_organization_mismatch",
                    "所选组织与本机安装包不一致",
                ));
            }
            let login = login
                .filter(|v| {
                    !v.trim().is_empty() && v.len() <= 256 && !v.chars().any(char::is_control)
                })
                .ok_or_else(|| error("input", "请输入企业账号"))?;
            let target = format!(
                "注册此设备\n服务：{}\n租户：{}\n账号：{}",
                organization.origin, organization.tenant_id, login
            );
            let password =
                super::super::credentials::enter_enterprise_password(app, target).await?;
            self.retire_locked().await?;
            let session =
                super::super::account::EnterpriseSession::login(organization, &login, password)
                    .await?;
            let next = session.context()?;
            self.identity.set_session(Some(session.clone()))?;
            // Publishing a user view is separate from disclosing credentials to execution IPC.
            let published = self.users.lock().map_err(|_| unavailable())?.activate(next);
            if let Err(e) = published {
                self.identity.set_session(None)?;
                let _ = session.logout().await;
                return Err(e);
            }
            account = Some(session);
        }
        let mut account = account.ok_or_else(unavailable)?;
        account.verify().await?;
        let intent = self.registration_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        drop(_switch);
        let mut prepared = false;
        loop {
            if self.closed() || self.registration_epoch.load(Ordering::Acquire) != intent {
                return Err(error(
                    "registration_cancelled",
                    "已停止注册等待；已提交的原请求仍保留",
                ));
            }
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
            if current.status.phase == RegistrationPhase::AwaitingAuthorization {
                let _switch = self.switching.lock().await;
                if self.registration_epoch.load(Ordering::Acquire) != intent {
                    continue;
                }
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
                match account.authorize_registration(id).await {
                    Ok(()) => {
                        self.identity.set_session(Some(account))?;
                        return Ok(context().await?.status);
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
            } else if !matches!(
                current.status.phase,
                RegistrationPhase::Unregistered | RegistrationPhase::Preparing
            ) {
                return Ok(current.status);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    pub async fn cancel_registration_wait(&self) {
        let _switch = self.switching.lock().await;
        self.registration_epoch.fetch_add(1, Ordering::AcqRel);
    }
}
