//! Native-only product credentials. HTTP contracts consumed from Identity v2 and MDM v1.
//! HTTP source: rss-identity 88a33594a3e83d38a89c520294671cfa2d0ec8fa (handlers/dto),
//! rss-mdm 533b4c3df7d7e8ef3cd599becdaeec4db41d854f (authorization/http).
//! ref: reqwest src/async_impl/{client,response}.rs@v0.13.5.
use crate::self_service::{error, Result};
use reqwest::{header, Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

fn unavailable() -> crate::self_service::ServiceError {
    error(
        "identity_unavailable",
        "无法核验企业会话，请检查组织连接后重新登录",
    )
}
fn invalid() -> crate::self_service::ServiceError {
    error(
        "invalid_organization",
        "请输入有效的 HTTPS 服务地址与租户 UUID",
    )
}
fn denied() -> crate::self_service::ServiceError {
    error(
        "identity_denied",
        "账号、组织或业务授权无效，请联系组织管理员",
    )
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Organization {
    pub id: String,
    pub label: String,
    pub origin: String,
    pub tenant_id: String,
}
impl Organization {
    pub fn validate(&mut self) -> Result<()> {
        let url = url::Url::parse(&self.origin).map_err(|_| invalid())?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(invalid());
        }
        self.origin = url.origin().ascii_serialization();
        self.tenant_id = uuid(&self.tenant_id)?;
        self.label = self.label.trim().to_owned();
        if self.label.is_empty()
            || self.label.chars().count() > 64
            || self.label.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        let digest = Sha256::digest(format!("{}\n{}", self.origin, self.tenant_id));
        self.id = format!("{digest:x}");
        Ok(())
    }
}
fn uuid(value: &str) -> Result<String> {
    let id = Uuid::parse_str(value).map_err(|_| invalid())?;
    if id.is_nil() {
        return Err(invalid());
    }
    Ok(id.to_string())
}
pub struct Organizations {
    path: PathBuf,
    values: Vec<Organization>,
}
impl Organizations {
    pub fn open(root: &Path) -> Result<Self> {
        let path = root.join("organizations.json");
        let mut values: Vec<Organization> = if path.exists() {
            serde_json::from_slice(
                &native_process::private_storage::read(&path, 65536).map_err(|_| unavailable())?,
            )
            .map_err(|_| unavailable())?
        } else {
            vec![]
        };
        if values.len() > 32 {
            return Err(invalid());
        }
        for v in &mut values {
            v.validate()?;
        }
        Ok(Self { path, values })
    }
    pub fn list(&self) -> Vec<Organization> {
        self.values.clone()
    }
    pub fn get(&self, id: &str) -> Result<Organization> {
        self.values
            .iter()
            .find(|v| v.id == id)
            .cloned()
            .ok_or_else(invalid)
    }
    pub fn save(&mut self, mut value: Organization) -> Result<Organization> {
        value.validate()?;
        let saved = value.clone();
        let mut next = self.values.clone();
        if let Some(old) = next.iter_mut().find(|v| v.id == value.id) {
            *old = value;
        } else {
            if next.len() >= 32 {
                return Err(invalid());
            }
            next.push(value);
        }
        let temporary = self.path.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = native_process::private_storage::create_new(&temporary)?;
            file.write_all(&serde_json::to_vec(&next)?)?;
            file.sync_all()?;
            drop(file);
            native_process::private_storage::replace(&temporary, &self.path)?;
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
// Never Serialize or Debug: cookie/CSRF cannot cross IPC or enter diagnostics.
pub struct Session {
    client: Client,
    organization: Organization,
    cookie: String,
    csrf: String,
    principal: String,
    instance: String,
    session: String,
    expires: u64,
}
async fn body(mut response: reqwest::Response) -> Result<Value> {
    if !response.status().is_success() {
        return Err(if response.status().is_client_error() {
            denied()
        } else {
            unavailable()
        });
    }
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if data.len() + chunk.len() > 262144 {
            return Err(unavailable());
        }
        data.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&data).map_err(|_| unavailable())
}
impl Session {
    pub async fn login(organization: Organization, login: &str, password: String) -> Result<Self> {
        if login.is_empty() || login.len() > 256 || password.is_empty() || password.len() > 16384 {
            return Err(denied());
        }
        let client = Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(4))
            .build()
            .map_err(|_| unavailable())?;
        Self::login_with_client(client, organization, login, password).await
    }
    pub(super) async fn login_with_client(
        client: Client,
        organization: Organization,
        login: &str,
        password: String,
    ) -> Result<Self> {
        let response = client
            .post(format!(
                "{}/api/v2/tenants/{}/login",
                organization.origin, organization.tenant_id
            ))
            .header(header::ORIGIN, &organization.origin)
            .header("x-identity-request", "1")
            .json(&json!({"login":login,"password":password}))
            .send()
            .await
            .map_err(|_| unavailable())?;
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
        let value = body(response).await?;
        if cookie.is_empty() || cookie.len() > 4096 {
            return Err(unavailable());
        }
        let mut session = Self {
            client,
            organization,
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
        let principal = uuid(
            value["identity"]["principalId"]
                .as_str()
                .ok_or_else(denied)?,
        )?;
        let session = uuid(value["session"]["id"].as_str().ok_or_else(denied)?)?;
        if existing && (self.principal != principal || self.session != session) {
            return Err(denied());
        }
        let idle = value["session"]["idleExpiresAt"]
            .as_u64()
            .ok_or_else(denied)?;
        let absolute = value["session"]["absoluteExpiresAt"]
            .as_u64()
            .ok_or_else(denied)?;
        let expires = idle.min(absolute).checked_mul(1000).ok_or_else(denied)?;
        if expires <= super::execution::now().map_err(|_| unavailable())? {
            return Err(denied());
        }
        let csrf = value["csrfToken"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 4096)
            .ok_or_else(denied)?;
        self.principal = principal;
        self.session = session;
        self.expires = expires;
        self.csrf = csrf.into();
        Ok(())
    }
    async fn request(&self, method: Method, path: &str) -> Result<reqwest::Response> {
        self.client
            .request(method, format!("{}{path}", self.organization.origin))
            .header(header::COOKIE, &self.cookie)
            .header(header::ORIGIN, &self.organization.origin)
            .header("x-identity-request", "1")
            .header("x-csrf-token", &self.csrf)
            .send()
            .await
            .map_err(|_| unavailable())
    }
    pub async fn verify(&mut self) -> Result<()> {
        let value = body(
            self.request(
                Method::GET,
                &format!("/api/v2/tenants/{}/session", self.organization.tenant_id),
            )
            .await?,
        )
        .await?;
        self.accept_identity(&value, true)?;
        let access = body(self.request(Method::GET, "/api/v1/authorization").await?).await?;
        let instance = uuid(access["instanceId"].as_str().ok_or_else(denied)?)?;
        if access["tenantId"].as_str() != Some(&self.organization.tenant_id)
            || access["principalId"].as_str() != Some(&self.principal)
            || (!self.instance.is_empty() && self.instance != instance)
        {
            return Err(denied());
        }
        // A successful AuthN response is insufficient: the product must return at least one effective grant.
        if access["grants"].as_array().is_none_or(|g| g.is_empty()) {
            return Err(denied());
        }
        self.instance = instance;
        Ok(())
    }
    pub async fn logout(&self) -> Result<()> {
        let response = self
            .request(
                Method::POST,
                &format!(
                    "/api/v2/tenants/{}/session/logout",
                    self.organization.tenant_id
                ),
            )
            .await?;
        if response.status().is_success() || response.status().as_u16() == 401 {
            Ok(())
        } else {
            Err(unavailable())
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
        serde_json::from_value(json!({"schemaVersion":5,"kind":"userContext","generation":Uuid::new_v4().to_string(),
            "user":{"schemaVersion":5,"kind":"testUser","userId":actor,"displayName":self.organization.label,"nameKey":"enterprise"},
            "identity":{"mode":"enterprise","authorityId":authority,"tenantId":self.organization.tenant_id,"principalId":self.principal,"organizationId":self.organization.id,"expiresAtMs":self.expires}})).map_err(|_| unavailable())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
            assert!(organization.validate().is_err(), "{origin}");
        }
    }
    #[test]
    fn enterprise_actor_is_not_a_display_name_and_cannot_claim_test_data() {
        let mut s = Session {
            client: Client::new(),
            organization: Organization {
                id: "org".into(),
                label: "Same name".into(),
                origin: "https://example.com".into(),
                tenant_id: Uuid::new_v4().to_string(),
            },
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
                    "Set-Cookie: __Host-identity-session=cookie-canary; Secure; HttpOnly; Path=/\r\n"
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
        let session = Session::login_with_client(
            Client::new(),
            organization,
            "Alice",
            "password-canary".into(),
        )
        .await
        .unwrap();
        let context = serde_json::to_string(&session.context().unwrap()).unwrap();
        for secret in ["password-canary", "cookie-canary", "csrf-canary"] {
            assert!(!context.contains(secret));
        }
        session.logout().await.unwrap();
        let requests = task.await.unwrap();
        assert!(requests[0].starts_with("POST /api/v2/tenants/"));
        assert!(requests[0].contains("password-canary"));
        assert!(!requests[0].contains("cookie-canary"));
        assert!(requests[1].contains("cookie-canary"));
        assert!(requests[2].starts_with("GET /api/v1/authorization "));
        assert!(requests[3].contains("/session/logout"));
        assert!(requests[3].contains("csrf-canary"));
    }
    #[tokio::test]
    async fn authentication_alone_cannot_activate_wrong_tenant_principal_or_empty_grants() {
        for field in ["tenantId", "principalId", "grants"] {
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
            assert!(Session::login_with_client(
                Client::new(),
                organization,
                "Alice",
                "password-canary".into()
            )
            .await
            .is_err());
            assert!(task.await.unwrap()[3].contains("/session/logout"));
        }
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
        let mut session = Session::login_with_client(
            Client::new(),
            organization,
            "Alice",
            "password-canary".into(),
        )
        .await
        .unwrap();
        assert!(session.verify().await.is_err());
        assert_eq!(task.await.unwrap().len(), 4);
    }
}
