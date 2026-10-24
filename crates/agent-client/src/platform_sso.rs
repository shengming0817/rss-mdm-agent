//! Immediate mTLS registration control, independent of task polling and Desktop sessions.
use crate::{store::encode, Client, Clock, Error, IdentityProvider};
use reqwest::Method;
use rss_mdm_agent_wire::platform_sso as w;
use uuid::Uuid;
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Register system-owned keys; never sends an employee bearer or arbitrary endpoint.
    pub async fn psso_bootstrap(
        &self,
        input: &w::Bootstrap,
    ) -> Result<w::BootstrapResponse, Error> {
        self.self_service_json(
            None,
            Method::POST,
            "platform-sso/bootstrap",
            Some(encode(input)?),
        )
        .await
    }
    /// Complete exactly one device-authenticated challenge. None is a definite rejection;
    /// transport, malformed success and server failures remain unknown outcomes.
    pub async fn psso_register(
        &self,
        input: &w::Registration,
    ) -> Result<Option<w::RegistrationReceipt>, Error> {
        let request = (|| -> Result<_, Error> {
            self.active()?;
            self.now()?;
            let body = encode(input)?;
            if body.len() > 131072 {
                return Err(Error::Capacity);
            }
            Ok(self
                .http
                .post(self.url("platform-sso/register")?)
                .timeout(
                    self.store
                        .cfg
                        .limits
                        .request_timeout
                        .min(std::time::Duration::from_secs(2)),
                )
                .header("content-type", "application/json")
                .body(body))
        })();
        // Every preparation failure precedes transport; it cannot have committed a source.
        let Ok(request) = request else {
            return Ok(None);
        };
        let (status, bytes) = self
            .agent_json_transport(request, self.store.cfg.limits.response_bytes)
            .await?;
        // These endpoint responses precede transaction commit; 408/5xx and lost replies do not.
        if matches!(status.as_u16(), 400 | 401 | 403 | 404 | 409 | 422) {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(Error::Unavailable);
        }
        crate::store::decode(&bytes).map(Some)
    }
    /// Recover a lost registration result using current public source/key coordinates.
    pub async fn psso_status(&self, binding: Option<Uuid>) -> Result<w::Status, Error> {
        let path = binding.map_or_else(
            || "platform-sso/status".into(),
            |id| format!("platform-sso/status?bindingId={id}"),
        );
        self.self_service_json(None, Method::GET, &path, None).await
    }
    /// Revoke one exact source; local system cleanup remains a separate fact.
    pub async fn psso_revoke(
        &self,
        source: Uuid,
        source_version: i64,
    ) -> Result<w::RevocationReceipt, Error> {
        if source.is_nil() || source_version < 1 {
            return Err(Error::Protocol);
        }
        self.self_service_json(
            None,
            Method::POST,
            &format!("platform-sso/sources/{source}/revoke"),
            Some(encode(&w::Revocation { source_version })?),
        )
        .await
    }
}
