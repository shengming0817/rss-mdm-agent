//! Independent Device Access device protocol on the administrator-pinned DA listener.
use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Public HTTPS bootstrap authority supplied through the protected installation profile.
pub struct Backend {
    /// Explicit HTTPS employee portal for an administrator-selected installation profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<Browser>,

    /// Expected DA service installation.
    pub instance: Uuid,
    /// Tenant selected by the administrator.
    pub tenant: Uuid,
    /// Device listener HTTPS origin.
    pub origin: String,
    /// Public root that authenticates the device listener.
    pub ca_pem: String,
}
/// Browser issuer is independent of device mTLS.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Browser {
    /// Trusted HTTPS origin.
    pub origin: String,
    /// Optional organization CA for the browser listener.
    pub ca_pem: Option<String>,
}
impl Backend {
    /// Validate public coordinates and the pinned authority.
    pub fn validate(&self) -> Result<(), Error> {
        let url = url::Url::parse(&self.origin).map_err(|_| Error::Configuration)?;
        if self.instance.is_nil()
            || self.tenant.is_nil()
            || url.scheme() != "https"
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(Error::Configuration);
        }
        if let Some(browser) = &self.browser {
            let u = url::Url::parse(&browser.origin).map_err(|_| Error::Configuration)?;
            if u.scheme() != "https"
                || u.host().is_none()
                || !u.username().is_empty()
                || u.password().is_some()
                || u.path() != "/"
                || u.query().is_some()
                || u.fragment().is_some()
            {
                return Err(Error::Configuration);
            }
            if let Some(pem) = &browser.ca_pem {
                crate::validate_public_ca(pem.as_bytes())?;
            }
        }
        crate::validate_public_ca(self.ca_pem.as_bytes())
    }
}

mod configuration;
mod operations;
mod platform_sso;
mod registration;
pub use operations::{verify_employee, AccountCompletion, Operations, RecoveryUseDisposition};
pub use registration::validate_keys;
mod storage;
use crate::{
    http::{Call, Endpoint},
    trust::VerifiedTrust,
    Clock, IdentityProvider,
};
pub use configuration::{mac_assigned, ManagedSnapshot, ManagedSource, PolicyConsumer, Selection};
use rss_mdm_agent_wire::{da as wire, enroll, pki};
use std::{path::Path, sync::Arc, time::Duration};
pub use storage::{has_live_credentials, retired_receipt};
use storage::{read, write};

/// Machine credential storage with idempotent, single-reference release after DA settlement.
pub trait Credentials: IdentityProvider {
    /// Delete the retired private key or bootstrap secret in this deployment namespace.
    fn release(&self, reference: &str, key: bool) -> Result<(), Error>;
}
/// Single durable DA identity and its pending protocol operations.
pub struct Client<S, C> {
    backend: Backend,
    origin: url::Url,
    journal: storage::Journal,
    secrets: S,
    clock: C,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// Secret-free readback of the current registration and diagnostics.
pub struct Status {
    /// Last committed activation receipt.
    pub registration: Option<wire::Registration>,
    /// Original administrative withdrawal, retained after the local key is released.
    pub revocation: Option<wire::RevocationReceipt>,
    /// Original grant operation awaiting completion.
    pub pending_operation: Option<Uuid>,
    /// A dispatched issue needs reconciliation at the CA.
    pub issuance_unknown: bool,
    /// Original issuer attempt to use for administrative recovery.
    pub issuance_id: Option<Uuid>,
    /// Last allocated periodic report sequence.
    pub status_sequence: i64,
    /// Last allocated application-report sequence.
    pub application_sequence: i64,
    /// Latest original server receipt for periodic health.
    pub status_receipt: Option<serde_json::Value>,
    /// Latest original server receipt for a configuration observation.
    pub application_receipt: Option<serde_json::Value>,
    /// Backend artifact associated with confirmed effective settings.
    pub effective_digest: Option<String>,
    /// Exclusive deadline of the last native observation.
    pub valid_until: Option<i64>,
    /// Failure of a replacement operation while the current credential remains usable.
    pub renewal_error: Option<Error>,
    /// Most recent closed local or protocol failure.
    pub diagnostic: Option<Error>,
    /// Last native delivery failure, independent of configuration maintenance.
    pub delivery_error: Option<Error>,
    /// Latest confirmed account and attempt with its original terminal settlement.
    pub account_settlement: Option<(Uuid, Uuid, wire::AccountSettlement)>,
    /// Latest confirmed recovery operation and attempt with its original settlement.
    pub credential_settlement: Option<(Uuid, Uuid, wire::CredentialSettlement)>,
    /// Last rejected account event and its exact attempt.
    pub rejected_account_event: Option<(Uuid, Uuid, Uuid, Error)>,
    /// Last definitively rejected offline use for this registration.
    pub rejected_recovery_use: Option<(Uuid, Error)>,
}

/// Inspect retained identity and report watermarks without creating a key or acquiring the writer.
pub fn inspect(
    root: &Path,
    backend: &Backend,
    installation: Uuid,
    now: i64,
) -> Result<Option<Status>, Error> {
    let Some(c) = storage::inspect(root, backend, installation)? else {
        return Ok(None);
    };
    let active = read::<registration::Active>(&c, "active")?;
    let pending = read::<Option<registration::Pending>>(&c, "enrollment")?.flatten();
    let receipt = |kind: &str| -> Result<Option<serde_json::Value>, Error> {
        use rusqlite::OptionalExtension;
        let bytes:Option<Vec<u8>>=c.query_row("SELECT receipt FROM reports WHERE kind=?1 AND receipt IS NOT NULL ORDER BY sequence DESC LIMIT 1",[kind],|r|r.get(0)).optional()?;
        bytes.as_deref().map(crate::store::decode).transpose()
    };
    let valid_until = read::<Option<configuration::AppliedProof>>(&c, "applied-proof")?
        .flatten()
        .map(|p| p.until);
    let valid = valid_until.is_some_and(|until| now < until)
        && read::<i64>(&c, "clock")?.is_some_and(|seen| now >= seen);
    let device = active.as_ref().map(|a| a.receipt.device);
    let delivery_key = |kind: &str| device.map(|d| operations::operation_key(d, kind));
    Ok(Some(Status {
        account_settlement: delivery_key("last-account-settlement")
            .map(|k| read(&c, &k))
            .transpose()?
            .flatten(),
        credential_settlement: delivery_key("last-credential-settlement")
            .map(|k| read(&c, &k))
            .transpose()?
            .flatten(),
        rejected_account_event: delivery_key("last-rejected-account")
            .map(|k| read(&c, &k))
            .transpose()?
            .flatten(),
        rejected_recovery_use: delivery_key("last-rejected-use")
            .map(|k| read(&c, &k))
            .transpose()?
            .flatten(),
        status_receipt: receipt("status")?,
        application_receipt: receipt("application")?,
        registration: active.map(|a| a.receipt),
        revocation: read(&c, "revocation")?,
        pending_operation: pending.as_ref().map(|p| p.operation),
        issuance_id: pending.as_ref().and_then(|p| p.issuance_id),
        issuance_unknown: pending.is_some_and(|p| p.unknown),
        status_sequence: read(&c, "status-sequence")?.unwrap_or(0),
        application_sequence: read(&c, "application-sequence")?.unwrap_or(0),
        valid_until,
        effective_digest: if valid {
            read(&c, "effective-digest")?.flatten()
        } else {
            None
        },
        diagnostic: read(&c, "last-error")?.flatten(),
        delivery_error: read(&c, "delivery-error")?.flatten(),
        renewal_error: read(&c, "renewal-error")?.flatten(),
    }))
}

impl<S: Credentials, C: Clock> Client<S, C> {
    /// Open the installation-bound private store and acquire its single writer lease.
    pub fn open(
        root: &Path,
        backend: Backend,
        installation: Uuid,
        secrets: S,
        clock: C,
    ) -> Result<Self, Error> {
        backend.validate()?;
        if installation.is_nil() {
            return Err(Error::Configuration);
        }
        let origin = url::Url::parse(&backend.origin).map_err(|_| Error::Configuration)?;
        let journal = storage::Journal::open(root, &backend, installation)?;
        Ok(Self {
            backend,
            origin,
            journal,
            secrets,
            clock,
        })
    }
    fn now(&self) -> Result<i64, Error> {
        let now = self.clock.now()?;
        if now <= 0 || read::<i64>(&self.journal.db, "clock")?.is_some_and(|last| now < last) {
            return Err(Error::Clock);
        }
        write(&self.journal.db, "clock", &now)?;
        Ok(now)
    }
    fn http(&self, key: Option<(&str, &[String])>) -> Result<reqwest::Client, Error> {
        let identity = key
            .map(|(reference, chain)| {
                let key = rustls::sign::CertifiedKey::new(
                    crate::tls::certificates(chain)?,
                    self.secrets.key(reference)?,
                );
                key.keys_match().map_err(|_| Error::Identity)?;
                Ok::<_, Error>(Arc::new(key))
            })
            .transpose()?;
        crate::tls::transport(
            Some(self.backend.ca_pem.as_bytes()),
            true,
            Duration::from_secs(3),
            Duration::from_secs(5),
            identity,
        )
    }
    async fn call<E: Endpoint>(
        &self,
        http: &reqwest::Client,
        query: Option<&str>,
        input: Option<&E::Input>,
    ) -> Result<E::Output, Error> {
        self.exchange::<E>(http, query, input)
            .await?
            .map_err(|e| crate::identity::enrollment_error(e.code))
    }
    async fn exchange<E: Endpoint>(
        &self,
        http: &reqwest::Client,
        query: Option<&str>,
        input: Option<&E::Input>,
    ) -> Result<Result<E::Output, enroll::ErrorBody>, Error> {
        let response = Call::<E>::new(http, &self.origin, &[], query, input)?
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
        let status = response.status();
        let bytes = crate::client::bounded(response, 1024 * 1024).await?;
        if !status.is_success() {
            let body: enroll::ErrorBody = crate::store::decode(&bytes)?;
            return Ok(Err(body));
        }
        if E::SPEC.media.is_none() {
            if !bytes.is_empty() {
                return Err(Error::Protocol);
            }
            return Ok(Ok(crate::store::decode(b"null")?));
        }
        Ok(Ok(crate::store::decode(&bytes)?))
    }
    async fn pki(&self) -> Result<VerifiedTrust, Error> {
        let trust: pki::Trust = self
            .call::<crate::http::DaPki>(
                &self.http(None)?,
                Some(&format!("tenantId={}", self.backend.tenant)),
                None,
            )
            .await?;
        let verified = VerifiedTrust::device(
            trust,
            self.backend.instance,
            self.backend.tenant,
            pki::Purpose::DaDevice,
            self.now()?,
        )?;
        let mut marks = read(&self.journal.db, "crl-watermarks")?.unwrap_or_default();
        if let Some(old) = read(&self.journal.db, "pki")? {
            verified.monotonic(&old, &marks)?;
        }
        marks.extend(verified.watermarks.clone());
        let tx = self.journal.db.unchecked_transaction()?;
        write(&tx, "pki", &verified.document)?;
        write(&tx, "crl-watermarks", &marks)?;
        tx.commit()?;
        Ok(verified)
    }
    /// Reconcile one device cycle using a fresh machine observation.
    pub async fn drive(&self, consumer: &mut impl PolicyConsumer) -> Result<(), Error> {
        let cleanup = self.release_settled();
        let result = self.maintain(consumer).await;
        if matches!(
            result,
            Err(Error::Untrusted
                | Error::CredentialRetired
                | Error::CredentialRevoked
                | Error::CertificateExpired
                | Error::Identity
                | Error::Clock
                | Error::Expired
                | Error::Protocol
                | Error::Denied)
        ) {
            self.withdraw(consumer)?;
        }
        let result = result.and(cleanup);
        write(
            &self.journal.db,
            "last-error",
            &result.as_ref().err().copied(),
        )?;
        result
    }
    async fn maintain(&self, consumer: &mut impl PolicyConsumer) -> Result<(), Error> {
        let snapshot = consumer.managed_snapshot();
        self.check_local(&snapshot, consumer)?;
        let pending = self.live_pending()?;
        if self.active()?.is_none() && pending.is_none_or(|p| p.grant.is_none()) {
            return Ok(());
        }
        let pki = self.pki().await?;
        let enrollment = self.enroll(&pki).await;
        if self.active()?.is_none() {
            enrollment?;
        }
        let active = self.active()?.ok_or(Error::Identity)?;
        let (_, after, device) = crate::tls::device_validity(
            &active.chain,
            &active.key,
            &self.secrets,
            self.now()?,
            &pki,
        )?;
        if device != active.receipt.device {
            return Err(Error::Identity);
        }
        let http = self.http(Some((&active.key, &active.chain)))?;
        if after - self.now()? < ((active.validity.1 - active.validity.0) / 3).clamp(60, 86400)
            && self.pending()?.is_none()
        {
            self.start_renewal()?;
        }
        let result = self
            .trust_and_reports(&http, &active.receipt, &pki.document, consumer, snapshot)
            .await;
        let delivery = self.flush(&http).await;
        // A failed replacement grant does not retire the credential that just authenticated.
        write(&self.journal.db, "renewal-error", &enrollment.err())?;
        result.and(delivery)
    }
}

#[cfg(test)]
mod tests;
