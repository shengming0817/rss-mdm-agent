//! Native registration uses the DA device listener. Only employee discovery carries a user session.
use super::Operations;
use crate::{
    http::{self as h, Call},
    Error,
};
use rss_mdm_agent_wire::{platform_sso as w, runtime};
use std::time::Duration;
use uuid::Uuid;

impl Operations<'_> {
    /// Discover existing bindings for the authenticated employee on this DA device.
    pub async fn mac_bindings(&self, proof: &str) -> Result<runtime::AccessDirectory, Error> {
        if proof.len() != 64
            || !proof
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(Error::Identity);
        }
        let response =
            Call::<h::DaAccessDirectory>::new(&self.http, self.origin, &[], Some("limit=1"), None)?
                .header("x-rss-user-session", proof)
                .timeout(Duration::from_secs(2))
                .send()
                .await?;
        Self::decode(response).await
    }

    /// Begin registration for the current system callback.
    pub async fn psso_bootstrap(
        &self,
        input: &w::Bootstrap,
    ) -> Result<w::BootstrapResponse, Error> {
        self.send::<h::DaPssoBootstrap>(&[], Some(input)).await
    }
    /// A lost response leaves the original source pending until status reconciliation.
    pub async fn psso_register(
        &self,
        input: &w::Registration,
    ) -> Result<Option<w::RegistrationReceipt>, Error> {
        let response =
            Call::<h::DaPssoRegister>::new(&self.http, self.origin, &[], None, Some(input))?
                .body_limit(131072)?
                .timeout(Duration::from_secs(2))
                .send()
                .await?;
        if matches!(
            response.status().as_u16(),
            400 | 401 | 403 | 404 | 409 | 422
        ) {
            return Ok(None);
        }
        Self::decode(response).await.map(Some)
    }
    /// Read the selected binding's original source.
    pub async fn psso_status(&self, binding: Option<Uuid>) -> Result<w::Status, Error> {
        let query = binding.map(|id| format!("bindingId={id}"));
        let response =
            Call::<h::DaPssoStatus>::new(&self.http, self.origin, &[], query.as_deref(), None)?
                .timeout(Duration::from_secs(2))
                .send()
                .await?;
        Self::decode(response).await
    }
    /// Retire exactly the observed source version.
    pub async fn psso_revoke(
        &self,
        source: Uuid,
        version: i64,
    ) -> Result<w::RevocationReceipt, Error> {
        if source.is_nil() || version < 1 {
            return Err(Error::Protocol);
        }
        self.send::<h::DaPssoRevoke>(
            &[&source.to_string()],
            Some(&w::Revocation {
                source_version: version,
            }),
        )
        .await
    }
}
