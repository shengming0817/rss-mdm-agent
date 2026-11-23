//! Independent Device Access device protocol on the administrator-pinned DA listener.
use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Public HTTPS bootstrap authority supplied through the protected installation profile.
pub struct Backend {
    /// Expected DA service installation.
    pub instance: Uuid,
    /// Tenant selected by the administrator.
    pub tenant: Uuid,
    /// Device listener HTTPS origin.
    pub origin: String,
    /// Public root that authenticates the device listener.
    pub ca_pem: String,
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
        crate::validate_public_ca(self.ca_pem.as_bytes())
    }
}

mod configuration;
mod registration;
mod storage;
use crate::{
    http::{Call, Endpoint},
    trust::VerifiedTrust,
    Clock, IdentityProvider,
};
pub use configuration::{ManagedSnapshot, ManagedSource, PolicyConsumer};
use rss_mdm_agent_wire::{da as wire, enroll, pki};
use std::{path::Path, sync::Arc, time::Duration};
use storage::{read, write};

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
    /// Original grant operation awaiting completion.
    pub pending_operation: Option<Uuid>,
    /// A dispatched issue needs reconciliation at the CA.
    pub issuance_unknown: bool,
    /// Last allocated periodic report sequence.
    pub status_sequence: i64,
    /// Last allocated application-report sequence.
    pub application_sequence: i64,
    /// Backend artifact associated with confirmed effective settings.
    pub effective_digest: Option<String>,
    /// Most recent closed local or protocol failure.
    pub diagnostic: Option<Error>,
}

/// Inspect retained identity and report watermarks without creating a key or acquiring the writer.
pub fn inspect(
    root: &Path,
    backend: &Backend,
    installation: Uuid,
) -> Result<Option<Status>, Error> {
    let Some(c) = storage::inspect(root, backend, installation)? else {
        return Ok(None);
    };
    let active = read::<registration::Active>(&c, "active")?;
    let pending = read::<Option<registration::Pending>>(&c, "enrollment")?.flatten();
    Ok(Some(Status {
        registration: active.map(|a| a.receipt),
        pending_operation: pending.as_ref().map(|p| p.operation),
        issuance_unknown: pending.is_some_and(|p| p.unknown),
        status_sequence: read(&c, "status-sequence")?.unwrap_or(0),
        application_sequence: read(&c, "application-sequence")?.unwrap_or(0),
        effective_digest: read(&c, "effective-digest")?.flatten(),
        diagnostic: read(&c, "last-error")?.flatten(),
    }))
}

impl<S: IdentityProvider, C: Clock> Client<S, C> {
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
        let response = Call::<E>::new(http, &self.origin, &[], query, input)?
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
        let status = response.status();
        let bytes = crate::client::bounded(response, 1024 * 1024).await?;
        if !status.is_success() {
            let body: enroll::ErrorBody = crate::store::decode(&bytes)?;
            return Err(crate::identity::enrollment_error(body.code));
        }
        if E::SPEC.media.is_none() {
            if !bytes.is_empty() {
                return Err(Error::Protocol);
            }
            return crate::store::decode(b"null");
        }
        crate::store::decode(&bytes)
    }
    async fn pki(&self) -> Result<VerifiedTrust, Error> {
        let trust: pki::Trust = self
            .call::<crate::http::DaPki>(
                &self.http(None)?,
                Some(&format!("tenantId={}", self.backend.tenant)),
                None,
            )
            .await?;
        if url::Url::parse(&trust.origin)
            .map_err(|_| Error::Untrusted)?
            .origin()
            != self.origin.origin()
        {
            return Err(Error::Untrusted);
        }
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
        write(
            &self.journal.db,
            "last-error",
            &result.as_ref().err().copied(),
        )?;
        result
    }
    async fn maintain(&self, consumer: &mut impl PolicyConsumer) -> Result<(), Error> {
        let snapshot = consumer.managed_snapshot();
        if self.active()?.is_none() && self.pending()?.is_none_or(|p| p.grant.is_none()) {
            return Ok(());
        }
        let pki = self.pki().await?;
        self.enroll(&pki).await?;
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
        if after - self.now()? < (active.lifetime / 3).clamp(60, 86400) && self.pending()?.is_none()
        {
            self.start_renewal()?;
        }
        self.trust_and_reports(&http, &active.receipt, &pki.document, consumer, snapshot)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
