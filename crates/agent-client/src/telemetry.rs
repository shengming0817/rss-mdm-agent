//! Authenticated periodic collection and an independent bounded spool.
//! ref: reqwest src/async_impl/client.rs; rusqlite 0.32.1 src/transaction.rs.
use crate::{client::bounded, store::decode, wire as w, Client, Clock, Error, IdentityProvider};
use base64::Engine;
use reqwest::Method;
use uuid::Uuid;
mod spool;
pub use spool::{GapReason, Limits, Pending, PendingGap, Spool, Summary};

/// A configuration authenticated against this device's acknowledged signing authority.
#[derive(Clone)]
pub struct Configuration(w::SignedTelemetryConfiguration);
impl Configuration {
    /// Exact server-owned instructions; no caller can construct an authenticated wrapper.
    pub fn payload(&self) -> &w::TelemetryConfiguration {
        &self.0.configuration
    }
    pub(crate) fn signed(&self) -> &w::SignedTelemetryConfiguration {
        &self.0
    }
}
/// Closed transport failure, retaining the remote retry delay independently of the error.
#[derive(Debug)]
pub struct Failure {
    /// Sanitized failure category.
    pub error: Error,
    /// Relative delay in seconds, including a bounded HTTP-date Retry-After.
    pub retry_after: Option<u64>,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self {
            error,
            retry_after: None,
        }
    }
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Open telemetry storage under the same private namespace and single communication lease.
    pub fn telemetry_spool(&self, limits: Limits) -> Result<Spool, Error> {
        Spool::open(&self.store.root, &self.store.cfg, limits)
    }
    /// Authenticate retained or freshly fetched instructions without granting a new lease.
    pub fn verify_telemetry(
        &self,
        signed: w::SignedTelemetryConfiguration,
    ) -> Result<Configuration, Error> {
        self.active()?;
        if !self.task_trust_acknowledged()? {
            return Err(Error::Untrusted);
        }
        let p = &signed.configuration;
        let now = self.now()?;
        let r = self.registration()?;
        if p.tenant_id != self.store.cfg.tenant
            || p.device_id != r.device_id
            || p.registration_id != r.registration_id
            || p.generation != r.generation
            || p.issued_at > now
            || p.expires_at <= now
            || signed.signature.len() != 86
        {
            return Err(Error::Untrusted);
        }
        if matches!(&p.collector, w::TelemetryCollector::Native { mappings, .. }
            if mappings.values().any(|m| m.counter_normalization.is_some()))
            && !r
                .capabilities
                .contains(&w::Capability::TelemetryCounterLocalV1)
        {
            return Err(Error::Unsupported);
        }
        let bytes = p.signing_bytes(&signed.key_id)?;
        let key = self.task_verification_key(&signed.key_id)?;
        let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&signed.signature)
            .map_err(|_| Error::Untrusted)?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key)
            .verify(&bytes, &signature)
            .map_err(|_| Error::Untrusted)?;
        Ok(Configuration(signed))
    }
    /// Fetch a complete directory. Errors must never be interpreted as an empty assignment set.
    pub async fn telemetry_directory(&self) -> Result<w::TelemetryDirectory, Failure> {
        let bytes = self
            .telemetry_request(Method::GET, "telemetry/configurations", None)
            .await?;
        let value: w::TelemetryDirectory = decode(&bytes)?;
        let mut keys = std::collections::BTreeSet::new();
        if value.assignments.len() > 256
            || value
                .assignments
                .iter()
                .any(|a| a.policy.is_nil() || a.version.is_nil() || !keys.insert(a.policy))
        {
            return Err(Error::Protocol.into());
        }
        Ok(value)
    }
    /// Fetch and verify one exact immutable assignment.
    pub async fn telemetry_configuration(
        &self,
        policy: Uuid,
        version: Uuid,
    ) -> Result<Configuration, Failure> {
        let bytes = self
            .telemetry_request(
                Method::GET,
                &format!("telemetry/configurations/{policy}/{version}"),
                None,
            )
            .await?;
        let signed: w::SignedTelemetryConfiguration = decode(&bytes)?;
        if signed.configuration.policy != policy || signed.configuration.policy_version != version {
            return Err(Error::Protocol.into());
        }
        self.verify_telemetry(signed).map_err(Into::into)
    }
    /// Download the signed script material and verify its exact length and digest.
    pub async fn telemetry_content(
        &self,
        configuration: &Configuration,
    ) -> Result<Vec<u8>, Failure> {
        self.verify_telemetry(configuration.0.clone())?;
        let p = configuration.payload();
        let w::TelemetryCollector::Script { content, .. } = &p.collector else {
            return Err(Error::Unsupported.into());
        };
        if content.length > 4 * 1024 * 1024 {
            return Err(Error::Capacity.into());
        }
        let bytes = self
            .telemetry_request(
                Method::GET,
                &format!(
                    "telemetry/configurations/{}/{}/content",
                    p.policy, p.policy_version
                ),
                None,
            )
            .await?;
        use sha2::{Digest, Sha256};
        if bytes.len() as u64 != content.length
            || Sha256::digest(&bytes).as_slice() != content.sha256
        {
            return Err(Error::Untrusted.into());
        }
        Ok(bytes)
    }
    /// Resolve a previous ambiguous submission using its original registration and identity.
    pub async fn telemetry_status(
        &self,
        pending: &Pending,
    ) -> Result<Option<w::TelemetryReceipt>, Failure> {
        self.check_telemetry_owner(pending)?;
        match self
            .telemetry_request(
                Method::GET,
                &format!("telemetry/batches/{}", pending.id()),
                None,
            )
            .await
        {
            Ok(bytes) => Ok(Some(receipt(&bytes, pending.id())?)),
            Err(Failure {
                error: Error::NotFound,
                ..
            }) => Ok(None),
            Err(e) => Err(e),
        }
    }
    /// Send the immutable persisted body, never rebuilt from current configuration or samples.
    pub async fn telemetry_submit(
        &self,
        pending: &Pending,
    ) -> Result<w::TelemetryReceipt, Failure> {
        self.check_telemetry_owner(pending)?;
        let bytes = self
            .telemetry_request(Method::POST, "telemetry/batches", Some(pending.body()))
            .await?;
        receipt(&bytes, pending.id()).map_err(Into::into)
    }
    /// Resolve an uncertain report using its original registration and fixed report identity.
    pub async fn telemetry_gap_status(
        &self,
        pending: &PendingGap,
    ) -> Result<Option<w::TelemetryGapReceipt>, Failure> {
        self.check_gap_owner(pending)?;
        match self
            .telemetry_request(
                Method::GET,
                &format!("telemetry/gaps/{}", pending.id()),
                None,
            )
            .await
        {
            Ok(bytes) => Ok(Some(gap_receipt(&bytes, pending.id())?)),
            Err(Failure {
                error: Error::NotFound,
                ..
            }) => Ok(None),
            Err(e) => Err(e),
        }
    }
    /// Submit the exact frozen report; neither identity nor creation time is regenerated.
    pub async fn telemetry_gap_submit(
        &self,
        pending: &PendingGap,
    ) -> Result<w::TelemetryGapReceipt, Failure> {
        self.check_gap_owner(pending)?;
        let bytes = self
            .telemetry_request(Method::POST, "telemetry/gaps", Some(pending.body()))
            .await?;
        gap_receipt(&bytes, pending.id()).map_err(Into::into)
    }
    fn check_gap_owner(&self, pending: &PendingGap) -> Result<(), Error> {
        self.active()?;
        if !pending.belongs_to(&self.registration()?) {
            return Err(Error::Untrusted);
        }
        if self.now()?.checked_mul(1_000_000).ok_or(Error::Clock)? >= pending.expires() {
            return Err(Error::Expired);
        }
        Ok(())
    }
    fn check_telemetry_owner(&self, pending: &Pending) -> Result<(), Error> {
        self.active()?;
        let r = self.registration()?;
        if !pending.belongs_to(&r) {
            return Err(Error::Untrusted);
        }
        Ok(())
    }
    async fn telemetry_request(
        &self,
        method: Method,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<Vec<u8>, Failure> {
        self.active()?;
        let now = self.now()?;
        let mut request = self
            .http
            .request(method, self.url(path)?)
            .timeout(self.store.cfg.limits.request_timeout);
        if let Some(body) = body {
            request = request
                .header("content-type", "application/json")
                .body(body.to_vec());
        }
        let response = request
            .send()
            .await
            .map_err(|_| Failure::from(Error::Unavailable))?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                v.parse::<u64>().ok().or_else(|| {
                    httpdate::parse_http_date(v)
                        .ok()?
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()?
                        .as_secs()
                        .checked_sub(now as u64)
                })
            })
            .map(|v| v.min(86400));
        let bytes = bounded(
            response,
            self.store.cfg.limits.response_bytes.min(4 * 1024 * 1024),
        )
        .await?;
        self.active()?;
        if status.is_success() {
            return Ok(bytes);
        }
        // Keep the existing identity-failure transition, including certificate revocation.
        let identity = self.runtime_error(&bytes).err();
        if let Some(
            e @ (Error::CredentialRevoked
            | Error::CertificateExpired
            | Error::CredentialRetired
            | Error::Identity),
        ) = identity
        {
            return Err(e.into());
        }
        let error = match status.as_u16() {
            404 => Error::NotFound,
            401 | 403 => Error::Denied,
            409 => Error::Conflict,
            408 | 429 | 503 => Error::Unavailable,
            500..=599 => Error::Unavailable,
            _ => Error::Protocol,
        };
        Err(Failure { error, retry_after })
    }
}
fn receipt(bytes: &[u8], id: Uuid) -> Result<w::TelemetryReceipt, Error> {
    let value: w::TelemetryReceipt = decode(bytes)?;
    if value.batch_id != id || value.received_at < 0 || value.received_at == i64::MAX {
        return Err(Error::Protocol);
    }
    Ok(value)
}

fn gap_receipt(bytes: &[u8], id: Uuid) -> Result<w::TelemetryGapReceipt, Error> {
    let value: w::TelemetryGapReceipt = decode(bytes)?;
    if value.report_id != id || value.received_at < 0 || value.received_at == i64::MAX {
        return Err(Error::Protocol);
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
