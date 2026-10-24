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
    /// Complete exactly one device-authenticated challenge.
    pub async fn psso_register(
        &self,
        input: &w::Registration,
    ) -> Result<w::RegistrationReceipt, Error> {
        self.self_service_json(
            None,
            Method::POST,
            "platform-sso/register",
            Some(encode(input)?),
        )
        .await
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
