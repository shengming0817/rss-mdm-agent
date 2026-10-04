//! Device-authenticated employee HTTP; no caller can supply a different endpoint or credential origin.
use crate::{
    client::json_transport,
    store::{decode, encode},
    wire, Client, Clock, Error, IdentityProvider,
};
use reqwest::Method;
use serde::de::DeserializeOwned;
use uuid::Uuid;
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    async fn employee_json<T: DeserializeOwned>(
        &self,
        secret: &str,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<T, Error> {
        self.active()?;
        self.now()?;
        if secret.len() != 64
            || !secret
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Identity);
        }
        let mut request = self
            .http
            .request(method, self.url(path)?)
            .header("x-rss-user-session", secret)
            .timeout(
                self.store
                    .cfg
                    .limits
                    .request_timeout
                    .min(std::time::Duration::from_secs(2)),
            );
        if let Some(body) = body {
            if body.len() > 131072 {
                return Err(Error::Capacity);
            }
            request = request
                .header("content-type", "application/json")
                .body(body);
        }
        let (status, bytes) = json_transport(request, self.store.cfg.limits.response_bytes).await?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::NotFound);
        }
        if !status.is_success() {
            self.runtime_error(&bytes)?;
        }
        decode(&bytes)
    }
    /// Current authorized directory for an origin attested by the system service.
    pub async fn self_service_directory(
        &self,
        secret: &str,
        query: &wire::SelfServiceDirectory,
    ) -> Result<wire::SelfServiceDirectoryResponse, Error> {
        let source = match query.source {
            wire::ScriptCaller::Human => "human",
            wire::ScriptCaller::Ai => "ai",
        };
        let mut path = format!("self-service/scripts?source={source}");
        if let Some(after) = query.after {
            path.push_str(&format!("&after={after}"));
        }
        let response: wire::SelfServiceDirectoryResponse =
            self.employee_json(secret, Method::GET, &path, None).await?;
        if response.items.len() > 64 || response.as_of < 0 {
            return Err(Error::Protocol);
        }
        Ok(response)
    }
    /// Send the original journal-frozen invocation. A lost response is recovered by request ID.
    pub async fn invoke_self_service(
        &self,
        secret: &str,
        request: &wire::SelfServiceInvoke,
    ) -> Result<wire::SelfServiceAccepted, Error> {
        let response: wire::SelfServiceAccepted = self
            .employee_json(
                secret,
                Method::POST,
                "self-service/scripts/invocations",
                Some(encode(request)?),
            )
            .await?;
        if response.request_id != request.operation_id
            || response.policy_id != request.policy_id
            || response.policy_revision != request.policy_revision
            || response.policy_version != request.policy_version
            || response.run_id.is_nil()
        {
            return Err(Error::Untrusted);
        }
        Ok(response)
    }
    /// Employee-authorized status and structured result of the same original invocation.
    pub async fn self_service_status(
        &self,
        secret: &str,
        request: Uuid,
    ) -> Result<wire::SelfServiceStatus, Error> {
        let response: wire::SelfServiceStatus = self
            .employee_json(
                secret,
                Method::GET,
                &format!("self-service/scripts/invocations/{request}"),
                None,
            )
            .await?;
        if response.request_id != request || response.run_id.is_nil() {
            return Err(Error::Untrusted);
        }
        Ok(response)
    }
}
