//! Native-only product credentials. HTTP contracts consumed from Identity v2 authentication and rss-mdm business authorization v1.
//! HTTP source: rss-identity 88a33594a3e83d38a89c520294671cfa2d0ec8fa (handlers/dto),
//! rss-mdm 533b4c3df7d7e8ef3cd599becdaeec4db41d854f (authorization/http).
//! ref: reqwest src/async_impl/{client,response}.rs@v0.13.5.
use crate::self_service::{error, Result};
pub use ai_session_contract::AccountOrganization as Organization;
use ai_session_contract::{
    AccountFailure, AccountFailureKind as Reason, AccountFailureStage as Stage, Counter,
};
use reqwest::{header, Client, Method};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

fn failure(stage: Stage, reason: Reason) -> crate::self_service::ServiceError {
    macro_rules! code {
        ($stage:literal) => {
            match reason {
                Reason::Configuration => concat!("account/", $stage, "/configuration"),
                Reason::Denied => concat!("account/", $stage, "/denied"),
                Reason::Unavailable => concat!("account/", $stage, "/unavailable"),
                Reason::RateLimited => concat!("account/", $stage, "/rate_limited"),
                Reason::Contract => concat!("account/", $stage, "/contract"),
            }
        };
    }
    let code = match stage {
        Stage::Configuration => code!("configuration"),
        Stage::Login => code!("login"),
        Stage::Session => code!("session"),
        Stage::Authorization => code!("authorization"),
        Stage::Logout => code!("logout"),
    };
    error(code, "账户操作未完成")
}
fn unavailable() -> crate::self_service::ServiceError {
    failure(Stage::Configuration, Reason::Unavailable)
}
fn invalid() -> crate::self_service::ServiceError {
    failure(Stage::Configuration, Reason::Configuration)
}
fn status_error(stage: Stage, status: u16) -> crate::self_service::ServiceError {
    failure(
        stage,
        match status {
            401 | 403 => Reason::Denied,
            429 => Reason::RateLimited,
            408 | 500..=599 => Reason::Unavailable,
            _ => Reason::Contract,
        },
    )
}
pub fn failure_snapshot(error: &crate::self_service::ServiceError) -> Option<AccountFailure> {
    let (stage, reason) = if let Some(code) = error.code.strip_prefix("account/") {
        let (stage, reason) = code.split_once('/')?;
        (stage.parse().ok()?, reason.parse().ok()?)
    } else {
        match error.code {
            "logout_unconfirmed" => (Stage::Logout, Reason::Unavailable),
            "users_unavailable" => (Stage::Configuration, Reason::Unavailable),
            "ai_unavailable" => (Stage::Session, Reason::Unavailable),
            _ => return None,
        }
    };
    Some(AccountFailure {
        stage,
        reason,
        observed_at_ms: Counter(super::execution::now().unwrap_or(0) as i64),
    })
}
fn configured_organization(
    config: crate::organization_config::OrganizationConfiguration,
) -> std::result::Result<Organization, crate::organization_config::ConfigurationError> {
    let config = config.normalize()?;
    Ok(Organization {
        id: format!(
            "{:x}",
            Sha256::digest(format!("{}\n{}", config.origin, config.tenant))
        ),
        origin: config.origin,
        tenant_id: config.tenant,
        label: config.label,
    })
}
fn normalize_organization(value: &mut Organization) -> Result<()> {
    *value = configured_organization(crate::organization_config::OrganizationConfiguration {
        origin: value.origin.clone(),
        tenant: value.tenant_id.clone(),
        label: value.label.clone(),
        ca_pem: None,
        agent_origin: None,
    })
    .map_err(|_| invalid())?;
    Ok(())
}

pub fn bundled_organization(
) -> std::result::Result<Option<Organization>, crate::organization_config::ConfigurationError> {
    let value: Option<crate::organization_config::OrganizationConfiguration> =
        serde_json::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/default_organization.json"
        )))
        .map_err(|_| {
            crate::organization_config::ConfigurationError("the compiled backend connection")
        })?;
    value.map(configured_organization).transpose()
}

fn uuid(value: &str) -> Result<String> {
    let id = Uuid::parse_str(value).map_err(|_| invalid())?;
    if id.is_nil() {
        return Err(invalid());
    }
    Ok(id.to_string())
}
/// One current login request; partial legacy credentials are rejected.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnterpriseCredentials {
    pub organization_id: String,
    pub login: String,
    pub password: zeroize::Zeroizing<String>,
}
impl EnterpriseCredentials {
    pub fn validate(&self) -> Result<()> {
        if self.login.trim().is_empty()
            || self.login.len() > 256
            || self.login.chars().any(char::is_control)
            || self.password.trim().is_empty()
            || self.password.len() > 16384
            || self.password.chars().any(char::is_control)
        {
            return Err(error("input", "请输入有效的企业账号与密码"));
        }
        Ok(())
    }
}
pub struct Organizations {
    path: PathBuf,
    values: Vec<Organization>,
    default: Option<Organization>,
}
impl Organizations {
    pub fn open(root: &Path, mut default: Option<Organization>) -> Result<Self> {
        let path = root.join("organizations.json");
        let mut values: Vec<Organization> = if path.exists() {
            serde_json::from_slice(
                &platform_private_storage::read(&path, 65536).map_err(|_| unavailable())?,
            )
            .map_err(|_| unavailable())?
        } else {
            vec![]
        };
        for v in &mut values {
            normalize_organization(v)?;
        }
        if let Some(value) = &mut default {
            normalize_organization(value)?;
        }
        Ok(Self {
            path,
            values,
            default,
        })
    }
    pub fn list(&self) -> Vec<Organization> {
        self.default
            .iter()
            .cloned()
            .chain(
                self.values
                    .iter()
                    .filter(|value| {
                        self.default
                            .as_ref()
                            .is_none_or(|default| value.id != default.id)
                    })
                    .cloned(),
            )
            .collect()
    }
    pub fn selected(&self) -> Option<String> {
        self.default.as_ref().map(|value| value.id.clone())
    }
    pub fn get(&self, id: &str) -> Result<Organization> {
        self.list()
            .iter()
            .find(|v| v.id == id)
            .cloned()
            .ok_or_else(invalid)
    }
    pub fn save(&mut self, mut value: Organization) -> Result<Organization> {
        normalize_organization(&mut value)?;
        if let Some(default) = &self.default {
            if value.id == default.id {
                if value.label == default.label
                    && value.origin == default.origin
                    && value.tenant_id == default.tenant_id
                {
                    return Ok(default.clone());
                }
                return Err(error(
                    "default_organization",
                    "默认组织名称来自内置配置；请选择其它连接",
                ));
            }
        }
        let saved = value.clone();
        let mut next = self.values.clone();
        if let Some(old) = next.iter_mut().find(|v| v.id == value.id) {
            *old = value;
        } else {
            next.push(value);
        }
        let bytes = serde_json::to_vec(&next).map_err(|_| unavailable())?;
        if bytes.len() > 65536 {
            return Err(invalid());
        }
        let temporary = self.path.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = platform_private_storage::create_new(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            platform_private_storage::replace(&temporary, &self.path)?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temporary);
            return Err(unavailable());
        }
        self.values = next;
        Ok(saved)
    }
}
// Never Serialize or Debug. Only the employee secret may cross authenticated native IPC; cookie/CSRF stay here.
#[derive(Clone)]
pub struct EnterpriseSession {
    client: Client,
    organization: Organization,
    registration_origin: String,
    cookie: String,
    csrf: String,
    principal: String,
    instance: String,
    session: String,
    expires: u64,
}
async fn body(mut response: reqwest::Response, stage: Stage) -> Result<Value> {
    if !response.status().is_success() {
        return Err(status_error(stage, response.status().as_u16()));
    }
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| failure(stage, Reason::Unavailable))?
    {
        if data.len() + chunk.len() > 262144 {
            return Err(failure(stage, Reason::Contract));
        }
        data.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&data).map_err(|_| failure(stage, Reason::Contract))
}
fn configured_ca(
    config: Option<crate::organization_config::OrganizationConfiguration>,
    organization: &Organization,
) -> Result<Option<Vec<u8>>> {
    let Some(config) = config else {
        return Ok(None);
    };
    let config = config.normalize().map_err(|_| invalid())?;
    if config.origin != organization.origin || config.tenant != organization.tenant_id {
        return Ok(None);
    }
    Ok(config.ca_pem.map(String::into_bytes))
}
fn organization_ca(organization: &Organization) -> Result<Option<Vec<u8>>> {
    let config = serde_json::from_str(include_str!(concat!(
        env!("OUT_DIR"),
        "/default_organization.json"
    )))
    .map_err(|_| invalid())?;
    configured_ca(config, organization)
}
fn configured_registration_origin(
    config: Option<crate::organization_config::OrganizationConfiguration>,
    organization: &Organization,
) -> Result<String> {
    if let Some(config) = config {
        let config = config.normalize().map_err(|_| invalid())?;
        if config.origin == organization.origin && config.tenant == organization.tenant_id {
            return Ok(config.agent_origin.unwrap_or(config.origin));
        }
    }
    Ok(organization.origin.clone())
}
pub(super) fn registration_origin(organization: &Organization) -> Result<String> {
    let config = serde_json::from_str(include_str!(concat!(
        env!("OUT_DIR"),
        "/default_organization.json"
    )))
    .map_err(|_| invalid())?;
    configured_registration_origin(config, organization)
}
// ref: reqwest v0.13.5 src/async_impl/client.rs (per-client root certificates).
fn enterprise_client(ca: Option<Vec<u8>>) -> Result<Client> {
    let mut builder = Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(4));
    if let Some(pem) = ca {
        builder = builder
            .add_root_certificate(reqwest::Certificate::from_pem(&pem).map_err(|_| invalid())?);
    }
    builder
        .build()
        .map_err(|_| failure(Stage::Login, Reason::Unavailable))
}

impl EnterpriseSession {
    pub(super) fn employee_proof(&self) -> Result<zeroize::Zeroizing<String>> {
        let proof = self
            .cookie
            .strip_prefix("__Host-identity-session=")
            .ok_or_else(invalid)?;
        if proof.len() != 64 || !proof.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        Ok(zeroize::Zeroizing::new(proof.to_owned()))
    }
    pub(super) fn native_session(
        &self,
        generation: &str,
    ) -> Result<execution_ipc::host::NativeContextSession> {
        let secret = self
            .cookie
            .strip_prefix("__Host-identity-session=")
            .filter(|v| {
                v.len() == 64
                    && v.bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            })
            .ok_or_else(invalid)?;
        Ok(execution_ipc::host::NativeContextSession {
            access: execution_ipc::host::NativeAccess::User {
                credential: zeroize::Zeroizing::new(secret.to_owned()),
                credential_origin: self.registration_origin.clone(),
                user: rss_mdm_agent_wire::runtime::SelfServiceUser {
                    tenant_id: Uuid::parse_str(&self.organization.tenant_id)
                        .map_err(|_| invalid())?,
                    instance_id: Uuid::parse_str(&self.instance).map_err(|_| invalid())?,
                    principal_id: Uuid::parse_str(&self.principal).map_err(|_| invalid())?,
                },
            },
            generation: execution_contract::Id::new(generation).map_err(|_| invalid())?,
            expires_at_unix_ms: self.expires,
        })
    }

    pub(super) fn matches_registration(&self, origin: &str, tenant: &str) -> bool {
        self.registration_origin == origin && self.organization.tenant_id == tenant
    }
    /// Authorize only a machine request obtained through authenticated local IPC.
    pub(super) async fn authorize_registration(&mut self, id: uuid::Uuid) -> Result<()> {
        self.verify().await?;
        if id.is_nil() {
            return Err(invalid());
        }
        let response = self
            .client
            .post(format!(
                "{}/api/v1/agent-requests/{id}/authorize",
                self.organization.origin
            ))
            .header(header::COOKIE, &self.cookie)
            .header(header::ORIGIN, &self.organization.origin)
            .header("x-identity-request", "1")
            .header("x-csrf-token", &self.csrf)
            // Stable per-request authorization intent; retries never invent another operation.
            .json(&json!({"operationId": id}))
            .send()
            .await
            .map_err(|_| failure(Stage::Authorization, Reason::Unavailable))?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(if status == 401 {
                error(
                    "registration_authentication_required",
                    "企业会话已失效，请重新登录",
                )
            } else if status == 403 {
                error("registration_forbidden", "当前账号没有注册资格或额度")
            } else {
                status_error(Stage::Authorization, status)
            });
        }
        let grant: rss_mdm_agent_wire::enroll::GrantReceipt =
            serde_json::from_value(body(response, Stage::Authorization).await?)
                .map_err(|_| failure(Stage::Authorization, Reason::Contract))?;
        if grant.grant_id != id {
            return Err(failure(Stage::Authorization, Reason::Contract));
        }
        Ok(())
    }

    pub async fn login(
        organization: Organization,
        login: &str,
        password: zeroize::Zeroizing<String>,
    ) -> Result<Self> {
        if login.is_empty() || login.len() > 256 || password.is_empty() || password.len() > 16384 {
            return Err(failure(Stage::Login, Reason::Denied));
        }
        let client = enterprise_client(organization_ca(&organization)?)?;
        Self::login_with_client(client, organization, login, password).await
    }
    pub(super) async fn login_with_client(
        client: Client,
        organization: Organization,
        login: &str,
        password: zeroize::Zeroizing<String>,
    ) -> Result<Self> {
        let registration_origin = registration_origin(&organization)?;
        let response = client
            .post(format!(
                "{}/api/v1/identity/tenants/{}/login",
                organization.origin, organization.tenant_id
            ))
            .header(header::ORIGIN, &organization.origin)
            .header("x-identity-request", "1")
            .json(&json!({"login":login,"password":password}))
            .send()
            .await
            .map_err(|_| failure(Stage::Login, Reason::Unavailable))?;
        let cookies: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|h| h.to_str().ok())
            .filter(|s| s.starts_with("__Host-identity-session="))
            .collect();
        let cookie = if cookies.len() == 1 {
            cookies[0].split(';').next().unwrap().to_owned()
        } else {
            String::new()
        };
        let value = body(response, Stage::Login).await?;
        if cookie.is_empty() || cookie.len() > 4096 {
            return Err(failure(Stage::Login, Reason::Contract));
        }
        let mut session = Self {
            client,
            organization,
            registration_origin,
            cookie,
            csrf: String::new(),
            principal: String::new(),
            instance: String::new(),
            session: String::new(),
            expires: 0,
        };
        session.accept_identity(&value, false)?;
        if let Err(error) = session.verify().await {
            let _ = session.logout().await;
            return Err(error);
        }
        Ok(session)
    }
    fn accept_identity(&mut self, value: &Value, existing: bool) -> Result<()> {
        let stage = if existing {
            Stage::Session
        } else {
            Stage::Login
        };
        let contract = || failure(stage, Reason::Contract);
        let principal = uuid(
            value["identity"]["principalId"]
                .as_str()
                .ok_or_else(contract)?,
        )
        .map_err(|_| contract())?;
        let session =
            uuid(value["session"]["id"].as_str().ok_or_else(contract)?).map_err(|_| contract())?;
        if existing && (self.principal != principal || self.session != session) {
            return Err(failure(stage, Reason::Denied));
        }
        let idle = value["session"]["idleExpiresAt"]
            .as_u64()
            .ok_or_else(contract)?;
        let absolute = value["session"]["absoluteExpiresAt"]
            .as_u64()
            .ok_or_else(contract)?;
        let expires = idle
            .min(absolute)
            .checked_mul(1000)
            .filter(|n| *n <= 9007199254740991)
            .ok_or_else(contract)?;
        if expires <= super::execution::now().map_err(|_| failure(stage, Reason::Unavailable))? {
            return Err(failure(stage, Reason::Denied));
        }
        let csrf = value["csrfToken"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 4096)
            .ok_or_else(contract)?;
        self.principal = principal;
        self.session = session;
        self.expires = expires;
        self.csrf = csrf.into();
        Ok(())
    }
    async fn request(&self, stage: Stage, method: Method, path: &str) -> Result<reqwest::Response> {
        self.client
            .request(method, format!("{}{path}", self.organization.origin))
            .header(header::COOKIE, &self.cookie)
            .header(header::ORIGIN, &self.organization.origin)
            .header("x-identity-request", "1")
            .header("x-csrf-token", &self.csrf)
            .send()
            .await
            .map_err(|_| failure(stage, Reason::Unavailable))
    }
    pub async fn verify(&mut self) -> Result<()> {
        let value = body(
            self.request(
                Stage::Session,
                Method::GET,
                &format!(
                    "/api/v1/identity/tenants/{}/session",
                    self.organization.tenant_id
                ),
            )
            .await?,
            Stage::Session,
        )
        .await?;
        self.accept_identity(&value, true)?;
        let access = body(
            self.request(Stage::Authorization, Method::GET, "/api/v1/authorization")
                .await?,
            Stage::Authorization,
        )
        .await?;
        let instance = uuid(
            access["instanceId"]
                .as_str()
                .ok_or_else(|| failure(Stage::Authorization, Reason::Contract))?,
        )
        .map_err(|_| failure(Stage::Authorization, Reason::Contract))?;
        if access["tenantId"].as_str() != Some(&self.organization.tenant_id)
            || access["principalId"].as_str() != Some(&self.principal)
            || (!self.instance.is_empty() && self.instance != instance)
        {
            return Err(failure(Stage::Authorization, Reason::Denied));
        }
        let _grants = access["grants"]
            .as_array()
            .ok_or_else(|| failure(Stage::Authorization, Reason::Contract))?;
        self.instance = instance;
        Ok(())
    }
    pub async fn logout(&self) -> Result<()> {
        let response = self
            .request(
                Stage::Logout,
                Method::POST,
                &format!(
                    "/api/v1/identity/tenants/{}/session/logout",
                    self.organization.tenant_id
                ),
            )
            .await?;
        if response.status().is_success() || response.status().as_u16() == 401 {
            Ok(())
        } else {
            Err(status_error(Stage::Logout, response.status().as_u16()))
        }
    }
    pub fn context(&self) -> Result<ai_session_contract::UserContext> {
        // UUID claims from different HTTPS services are not the same authority.
        // Bind the configured TLS origin as well as the server's instance ID.
        let authority = format!(
            "mdm-{:x}",
            Sha256::digest(format!("{}\n{}", self.organization.origin, self.instance))
        );
        // Local actor keys cannot collide across issuers or tenants or claim an old random test actor.
        let actor = format!(
            "enterprise-{:x}",
            Sha256::digest(format!(
                "{}\n{}\n{}",
                authority, self.organization.tenant_id, self.principal
            ))
        );
        serde_json::from_value(json!({"schemaVersion":1,"kind":"userContext","generation":Uuid::new_v4().to_string(),
            "user":{"schemaVersion":1,"kind":"testUser","userId":actor,"displayName":self.organization.label,"nameKey":"enterprise"},
            "identity":{"mode":"user_device","authorityId":authority,"tenantId":self.organization.tenant_id,"principalId":self.principal,"organizationId":self.organization.id,"expiresAtMs":self.expires}})).map_err(|_| failure(Stage::Session, Reason::Contract))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_ca_is_bound_to_origin_and_tenant() {
        let mut org = organization();
        normalize_organization(&mut org).unwrap();
        let config = crate::organization_config::OrganizationConfiguration {
            origin: org.origin.clone(),
            tenant: org.tenant_id.clone(),
            label: org.label.clone(),
            ca_pem: Some("public-ca".into()),
            agent_origin: Some("https://agent.fixture.test:9445".into()),
        };
        assert_eq!(
            configured_ca(Some(config.clone()), &org).unwrap(),
            Some(b"public-ca".to_vec())
        );
        assert_eq!(
            configured_registration_origin(Some(config.clone()), &org).unwrap(),
            "https://agent.fixture.test:9445"
        );
        let mut other = org.clone();
        other.origin = "https://other.fixture.test".into();
        assert!(configured_ca(Some(config.clone()), &other)
            .unwrap()
            .is_none());
        assert_eq!(
            configured_registration_origin(Some(config.clone()), &other).unwrap(),
            other.origin
        );
        other = org.clone();
        other.tenant_id = Uuid::new_v4().to_string();
        assert_eq!(
            configured_registration_origin(Some(config.clone()), &other).unwrap(),
            other.origin
        );
        let mut same_origin = config.clone();
        same_origin.agent_origin = None;
        assert_eq!(
            configured_registration_origin(Some(same_origin), &org).unwrap(),
            org.origin
        );
        assert!(configured_ca(Some(config), &other).unwrap().is_none());
        assert!(configured_ca(None, &org).unwrap().is_none());
        assert!(enterprise_client(Some(b"not a certificate".to_vec())).is_err());
    }

    #[tokio::test]
    async fn enterprise_ca_trusts_only_verified_tls_without_system_installation() {
        use std::process::{Command, Stdio};
        struct Server(std::process::Child, PathBuf);
        impl Drop for Server {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
                let _ = std::fs::remove_dir_all(&self.1);
            }
        }
        let root = std::env::temp_dir().join(format!("rss-local-ca-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let cert = root.join("cert.pem");
        let key = root.join("key.pem");
        assert!(Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
                "-addext",
                "subjectAltName=DNS:localhost",
                "-addext",
                "basicConstraints=critical,CA:TRUE",
                "-keyout"
            ])
            .arg(&key)
            .arg("-out")
            .arg(&cert)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        let server_cert = root.join("server.pem");
        let server_key = root.join("server.key");
        let csr = root.join("server.csr");
        let extensions = root.join("server.ext");
        std::fs::write(&extensions, "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n").unwrap();
        assert!(Command::new("openssl")
            .args([
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-subj",
                "/CN=localhost",
                "-keyout"
            ])
            .arg(&server_key)
            .arg("-out")
            .arg(&csr)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        assert!(Command::new("openssl")
            .args(["x509", "-req", "-days", "1", "-in"])
            .arg(&csr)
            .arg("-CA")
            .arg(&cert)
            .arg("-CAkey")
            .arg(&key)
            .arg("-CAcreateserial")
            .arg("-extfile")
            .arg(&extensions)
            .arg("-out")
            .arg(&server_cert)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let process = Command::new("openssl")
            .args(["s_server", "-quiet", "-www", "-accept"])
            .arg(format!("127.0.0.1:{port}"))
            .arg("-cert")
            .arg(&server_cert)
            .arg("-key")
            .arg(&server_key)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let _server = Server(process, root);
        let address = format!("127.0.0.1:{port}");
        let mut ready = false;
        for _ in 0..100 {
            if std::net::TcpStream::connect(&address).is_ok() {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(ready);
        let url = format!("https://localhost:{port}/");
        assert!(enterprise_client(None)
            .unwrap()
            .get(&url)
            .send()
            .await
            .is_err());
        let client = enterprise_client(Some(std::fs::read(cert).unwrap())).unwrap();
        assert_eq!(client.get(url).send().await.unwrap().status(), 200);
        assert!(client
            .get(format!("https://127.0.0.1:{port}/"))
            .send()
            .await
            .is_err());
    }

    #[test]
    fn default_is_not_persisted_and_all_read_paths_use_the_same_effective_list() {
        let root =
            std::env::temp_dir().join(format!("rss-default-organization-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let mut default = organization();
        normalize_organization(&mut default).unwrap();
        let mut store = Organizations::open(&root, Some(default.clone())).unwrap();
        assert_eq!(store.selected(), Some(default.id.clone()));
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.get(&default.id).unwrap().origin, default.origin);
        store.save(default.clone()).unwrap();
        assert!(!root.join("organizations.json").exists());
        let mut renamed = default.clone();
        renamed.label = "Renamed".into();
        assert_eq!(
            store.save(renamed).err().unwrap().code,
            "default_organization"
        );
        assert!(!root.join("organizations.json").exists());
        let mut custom = organization();
        custom.origin = "https://custom.fixture.test".into();
        let custom = store.save(custom).unwrap();
        let bytes = std::fs::read(root.join("organizations.json")).unwrap();
        assert_eq!(
            serde_json::from_slice::<Vec<Organization>>(&bytes)
                .unwrap()
                .len(),
            1
        );
        let mut collision = default.clone();
        collision.label = "Stored name".into();
        std::fs::write(
            root.join("organizations.json"),
            serde_json::to_vec(&vec![collision, custom.clone()]).unwrap(),
        )
        .unwrap();
        let before = std::fs::read(root.join("organizations.json")).unwrap();
        let store = Organizations::open(&root, Some(default.clone())).unwrap();
        assert_eq!(store.list().len(), 2);
        assert_eq!(store.get(&default.id).unwrap().label, default.label);
        assert_eq!(
            std::fs::read(root.join("organizations.json")).unwrap(),
            before
        );
        // A previous compiled default never becomes a user-owned record.
        std::fs::write(root.join("organizations.json"), bytes).unwrap();
        let mut next = default.clone();
        next.origin = "https://next.fixture.test".into();
        let store = Organizations::open(&root, Some(next)).unwrap();
        assert!(store.get(&default.id).is_err());
        assert!(store.get(&custom.id).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn organization_count_does_not_limit_reopening_or_changing_defaults() {
        let root = std::env::temp_dir().join(format!("rss-default-capacity-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let mut store = Organizations::open(&root, None).unwrap();
        for i in 0..40 {
            let mut value = organization();
            value.origin = format!("https://org-{i}.fixture.test");
            store.save(value).unwrap();
        }
        assert_eq!(store.list().len(), 40);
        let mut store = Organizations::open(&root, Some(organization())).unwrap();
        assert_eq!(store.list().len(), 41);
        let mut extra = organization();
        extra.origin = "https://extra.fixture.test".into();
        let extra = store.save(extra).unwrap();
        let mut default = organization();
        default.origin = "https://next.fixture.test".into();
        let mut store = Organizations::open(&root, Some(default)).unwrap();
        assert_eq!(store.list().len(), 42);
        assert_eq!(store.get(&extra.id).unwrap().origin, extra.origin);
        let before = std::fs::read(root.join("organizations.json")).unwrap();
        let mut value = organization();
        value.origin = "https://MDM.EXAMPLE./".into();
        assert!(store.save(value).is_err());
        assert_eq!(
            std::fs::read(root.join("organizations.json")).unwrap(),
            before
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn status_classification_keeps_stage_and_distinguishes_retry_from_denial() {
        for (status, reason) in [
            (401, Reason::Denied),
            (403, Reason::Denied),
            (408, Reason::Unavailable),
            (429, Reason::RateLimited),
            (503, Reason::Unavailable),
            (302, Reason::Contract),
        ] {
            let error = status_error(Stage::Authorization, status);
            let snapshot = failure_snapshot(&error).unwrap();
            assert_eq!(snapshot.stage, Stage::Authorization);
            assert_eq!(snapshot.reason, reason);
        }
    }
    fn organization() -> Organization {
        Organization {
            id: String::new(),
            label: " Example ".into(),
            origin: "https://EXAMPLE.com/".into(),
            tenant_id: "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into(),
        }
    }
    #[test]
    fn organizations_reopen_normalized_records_and_refuse_corrupt_or_oversized_storage() {
        let root = std::env::temp_dir().join(format!("rss-organizations-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let mut store = Organizations::open(&root, None).unwrap();
        let saved = store.save(organization()).unwrap();
        let reopened = Organizations::open(&root, None)
            .unwrap()
            .get(&saved.id)
            .unwrap();
        assert_eq!(reopened.label, "Example");
        assert_eq!(reopened.origin, "https://example.com");
        assert_eq!(reopened.tenant_id, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
        for bytes in [b"{not-json".to_vec(), vec![b' '; 65537]] {
            std::fs::write(root.join("organizations.json"), bytes).unwrap();
            assert!(Organizations::open(&root, None).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_destination_replacement_preserves_live_and_previous_persisted_configuration() {
        let root =
            std::env::temp_dir().join(format!("rss-organization-replace-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let mut store = Organizations::open(&root, None).unwrap();
        let saved = store.save(organization()).unwrap();
        let original = std::fs::read(root.join("organizations.json")).unwrap();
        let blocked = root.join("blocked.json");
        std::fs::create_dir(&blocked).unwrap();
        std::fs::write(blocked.join("marker"), "keep").unwrap();
        store.path = blocked;
        let mut changed = organization();
        changed.label = "Changed".into();
        assert!(store.save(changed).is_err());
        assert_eq!(store.get(&saved.id).unwrap().label, "Example");
        assert_eq!(
            std::fs::read(root.join("organizations.json")).unwrap(),
            original
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn organization_requires_an_exact_https_origin_and_non_nil_tenant() {
        for origin in [
            "http://example.com",
            "https://user:password@example.com",
            "https://example.com/path",
            "https://example.com/?q=token",
            "https://example.com/#token",
        ] {
            let mut organization = Organization {
                id: String::new(),
                label: "Org".into(),
                origin: origin.into(),
                tenant_id: Uuid::new_v4().to_string(),
            };
            assert!(
                normalize_organization(&mut organization).is_err(),
                "{origin}"
            );
        }
    }
    #[test]
    fn enterprise_actor_is_not_a_display_name_and_cannot_claim_test_data() {
        let mut s = EnterpriseSession {
            client: Client::new(),
            organization: Organization {
                id: "org".into(),
                label: "Same name".into(),
                origin: "https://example.com".into(),
                tenant_id: Uuid::new_v4().to_string(),
            },
            registration_origin: "https://example.com".into(),
            cookie: "secret".into(),
            csrf: "secret".into(),
            principal: Uuid::new_v4().to_string(),
            instance: Uuid::new_v4().to_string(),
            session: Uuid::new_v4().to_string(),
            expires: 9999999999999,
        };
        let a = s.context().unwrap();
        s.organization.origin = "https://different-service.example.com".into();
        let other_service = s.context().unwrap();
        assert_ne!(
            serde_json::to_value(&a.identity).unwrap()["authorityId"],
            serde_json::to_value(&other_service.identity).unwrap()["authorityId"]
        );
        assert_ne!(a.user.user_id, other_service.user.user_id);
        s.organization.origin = "https://example.com".into();
        s.organization.tenant_id = Uuid::new_v4().to_string();
        let b = s.context().unwrap();
        assert_ne!(a.user.user_id, b.user.user_id);
        assert_ne!(a.generation, b.generation);
        assert!(!serde_json::to_string(&a).unwrap().contains("secret"));
        s.principal = Uuid::new_v4().to_string();
        assert_ne!(b.user.user_id, s.context().unwrap().user.user_id);
    }
}

#[cfg(test)]
pub(super) mod http_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    const TENANT: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const PRINCIPAL: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    pub(crate) fn identity() -> Value {
        json!({"identity":{"principalId":PRINCIPAL,"hasLocalPassword":true},"session":{"id":"cccccccc-cccc-4ccc-8ccc-cccccccccccc","idleExpiresAt":9999999999u64,"absoluteExpiresAt":9999999999u64},"csrfToken":"csrf-canary"})
    }
    pub(crate) fn access() -> Value {
        json!({"instanceId":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","tenantId":TENANT,"principalId":PRINCIPAL,"grants":[{"grant":{"operation":"inventory_read","scope":{"kind":"all_devices"}}}]})
    }
    pub(crate) async fn server(
        responses: Vec<(u16, Value)>,
    ) -> (Organization, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = vec![];
            for (index, (status, body)) in responses.into_iter().enumerate() {
                let (mut stream, _) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0; 4096];
                    let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer))
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    let text = String::from_utf8_lossy(&bytes);
                    if let Some(end) = text.find("\r\n\r\n") {
                        let length = text[..end]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|n| n.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                    assert!(bytes.len() < 32768);
                }
                requests.push(String::from_utf8(bytes).unwrap());
                let data = serde_json::to_string(&body).unwrap();
                let cookie = if index == 0 {
                    "Set-Cookie: __Host-identity-session=cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc; Secure; HttpOnly; Path=/\r\n"
                } else {
                    ""
                };
                let response = format!("HTTP/1.1 {status} Result\r\n{cookie}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{data}", data.len());
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        (
            Organization {
                id: "org".into(),
                label: "Org".into(),
                origin,
                tenant_id: TENANT.into(),
            },
            task,
        )
    }
    #[tokio::test]
    async fn native_http_session_checks_authn_and_authz_then_revokes_without_exporting_secrets() {
        let (organization, task) = server(vec![
            (200, identity()),
            (200, identity()),
            (200, access()),
            (200, json!({})),
        ])
        .await;
        let mut session = EnterpriseSession::login_with_client(
            Client::new(),
            organization,
            "Alice",
            zeroize::Zeroizing::new("password-canary".into()),
        )
        .await
        .unwrap();
        // User HTTP remains on the business endpoint; authenticated IPC binds to Agent transport.
        session.registration_origin = "https://agent.fixture.test:9445".into();
        assert!(session.matches_registration("https://agent.fixture.test:9445", TENANT));
        assert!(!session.matches_registration(&session.organization.origin, TENANT));
        assert!(!session.matches_registration(
            "https://agent.fixture.test:9445",
            &Uuid::new_v4().to_string()
        ));
        let native = session.native_session(&Uuid::new_v4().to_string()).unwrap();
        match native.access {
            execution_ipc::host::NativeAccess::User {
                credential_origin, ..
            } => {
                assert_eq!(credential_origin, "https://agent.fixture.test:9445");
            }
            _ => panic!("expected user context"),
        }
        let context = serde_json::to_string(&session.context().unwrap()).unwrap();
        for secret in [
            "password-canary",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "csrf-canary",
        ] {
            assert!(!context.contains(secret));
        }
        session.logout().await.unwrap();
        let requests = task.await.unwrap();
        assert!(requests[0].starts_with(&format!("POST /api/v1/identity/tenants/{TENANT}/login ")));
        assert!(requests[0].contains("password-canary"));
        assert!(!requests[0]
            .contains("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"));
        assert!(requests[1].starts_with(&format!("GET /api/v1/identity/tenants/{TENANT}/session ")));
        assert!(requests[1]
            .contains("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"));
        assert!(requests[2].starts_with("GET /api/v1/authorization "));
        assert!(requests[3].starts_with(&format!(
            "POST /api/v1/identity/tenants/{TENANT}/session/logout "
        )));
        assert!(requests[3].contains("csrf-canary"));
    }
    #[tokio::test]
    async fn native_registration_authorization_sends_stable_intent_only_to_the_user_api() {
        let id = Uuid::new_v4();
        let grant = json!({"wireVersion":1,"grantId":id,"state":"available","platform":"macos","expiresAt":9999999999i64});
        let (organization, task) = server(vec![
            (200, identity()),
            (200, identity()),
            (200, access()),
            (200, identity()),
            (200, access()),
            (200, grant),
        ])
        .await;
        let mut session = EnterpriseSession::login_with_client(
            Client::new(),
            organization.clone(),
            "Alice",
            zeroize::Zeroizing::new("password-canary".into()),
        )
        .await
        .unwrap();
        assert!(session.matches_registration(&organization.origin, TENANT));
        assert!(!session.matches_registration("https://foreign.example", TENANT));
        session.authorize_registration(id).await.unwrap();
        let requests = task.await.unwrap();
        let request = &requests[5];
        assert!(request.starts_with(&format!("POST /api/v1/agent-requests/{id}/authorize ")));
        assert!(request.contains(&format!("\"operationId\":\"{id}\"")));
        assert!(request.contains("csrf-canary"));
        for forbidden in [
            "password-canary",
            "lifetimeSeconds",
            "quantity",
            "unlimited",
        ] {
            assert!(!request.contains(forbidden));
        }
    }
    #[tokio::test]
    async fn registration_denial_is_not_reported_as_successful_device_activation() {
        for (status, code) in [
            (401, "registration_authentication_required"),
            (403, "registration_forbidden"),
        ] {
            let (organization, task) = server(vec![
                (200, identity()),
                (200, identity()),
                (200, access()),
                (200, identity()),
                (200, access()),
                (status, json!({})),
            ])
            .await;
            let mut session = EnterpriseSession::login_with_client(
                Client::new(),
                organization,
                "Alice",
                zeroize::Zeroizing::new("password-canary".into()),
            )
            .await
            .unwrap();
            assert_eq!(
                session
                    .authorize_registration(Uuid::new_v4())
                    .await
                    .unwrap_err()
                    .code,
                code
            );
            assert_eq!(task.await.unwrap().len(), 6);
        }
    }
    #[tokio::test]
    async fn authentication_rejects_wrong_tenant_or_principal() {
        for field in ["tenantId", "principalId"] {
            let mut invalid = access();
            invalid[field] = if field == "grants" {
                json!([])
            } else {
                json!(Uuid::new_v4().to_string())
            };
            let (organization, task) = server(vec![
                (200, identity()),
                (200, identity()),
                (200, invalid),
                (200, json!({})),
            ])
            .await;
            assert!(EnterpriseSession::login_with_client(
                Client::new(),
                organization,
                "Alice",
                zeroize::Zeroizing::new("password-canary".into())
            )
            .await
            .is_err());
            assert!(task.await.unwrap()[3].contains("/session/logout"));
        }
    }
    #[tokio::test]
    async fn authenticated_identity_with_empty_product_grants_can_converse() {
        let mut authenticated = access();
        authenticated["grants"] = json!([]);
        let (organization, task) = server(vec![
            (200, identity()),
            (200, identity()),
            (200, authenticated),
        ])
        .await;
        let session = EnterpriseSession::login_with_client(
            Client::new(),
            organization,
            "Alice",
            zeroize::Zeroizing::new("password-canary".into()),
        )
        .await
        .unwrap();
        assert!(matches!(
            session.context().unwrap().identity,
            ai_session_contract::AccountIdentity::UserDevice { .. }
        ));
        assert_eq!(task.await.unwrap().len(), 3);
    }
    #[tokio::test]
    async fn revocation_is_observed_by_the_next_passive_check() {
        let (organization, task) = server(vec![
            (200, identity()),
            (200, identity()),
            (200, access()),
            (401, json!({"code":"invalid_credential"})),
        ])
        .await;
        let mut session = EnterpriseSession::login_with_client(
            Client::new(),
            organization,
            "Alice",
            zeroize::Zeroizing::new("password-canary".into()),
        )
        .await
        .unwrap();
        assert!(session.verify().await.is_err());
        assert_eq!(task.await.unwrap().len(), 4);
    }
}
