//! A device key never leaves the platform store. A pending operation is published before
//! creating any key, requesting authorization, contacting the CA, or switching credentials.
use super::*;
use crate::http as h;
use rss_mdm_agent_wire::DeviceId;

#[derive(Serialize, Deserialize)]
pub(super) struct Pending {
    pub operation: Uuid,
    issue: Uuid,
    activate: Uuid,
    expected: Option<DeviceId>,
    administrator: bool,
    csr: Option<String>,
    pub grant: Option<wire::Grant>,
    renewal_dispatched: bool,
    issue_started: bool,
    candidate: Option<enroll::Candidate>,
    validated_validity: Option<(i64, i64)>,
    pub unknown: bool,
    pub issuance_id: Option<Uuid>,
}
impl Pending {
    fn key(&self) -> String {
        format!("da-key-{}", self.operation)
    }
    fn secret(&self) -> String {
        format!("da-secret-{}", self.operation)
    }
}
#[derive(Serialize, Deserialize)]
pub(super) struct Active {
    pub receipt: wire::Registration,
    pub chain: Vec<String>,
    pub key: String,
    pub validity: (i64, i64),
}

impl<S: Credentials, C: Clock> Client<S, C> {
    pub(super) fn active(&self) -> Result<Option<Active>, Error> {
        read(&self.journal.db, "active")
    }
    pub(super) fn pending(&self) -> Result<Option<Pending>, Error> {
        Ok(read(&self.journal.db, "enrollment")?.flatten())
    }
    /// An unsent grant can expire locally. Once dispatched, only the issuer can settle it.
    pub(super) fn live_pending(&self) -> Result<Option<Pending>, Error> {
        let Some(p) = self.pending()? else {
            return Ok(None);
        };
        if !p.issue_started {
            let now = self.now()?;
            if p.grant
                .as_ref()
                .is_some_and(|grant| grant.expires_at <= now)
            {
                self.end_grant(&p, enroll::ErrorCode::GrantExpired)?;
                return Ok(None);
            }
            // The key has not gone to the CA without a returned grant. Once the original
            // credential expires, a fresh administrator operation can recover the same device.
            if p.renewal_dispatched
                && p.grant.is_none()
                && self
                    .active()?
                    .is_some_and(|active| active.validity.1 <= now)
            {
                self.end_grant(&p, enroll::ErrorCode::CertificateExpired)?;
                return Ok(None);
            }
        }
        Ok(Some(p))
    }
    fn save(&self, p: &Pending) -> Result<(), Error> {
        write(&self.journal.db, "enrollment", &Some(p))
    }
    fn begin(&self, operation: Uuid, expected: Option<DeviceId>) -> Result<Pending, Error> {
        if operation.is_nil() {
            return Err(Error::Configuration);
        }
        if let Some(p) = self.live_pending()? {
            if p.operation != operation || p.expected != expected {
                return Err(Error::Conflict);
            }
            return Ok(p);
        }
        if read::<serde_json::Value>(&self.journal.db, &format!("registration/{operation}"))?
            .is_some()
        {
            return Err(Error::Conflict);
        }
        let p = Pending {
            operation,
            issue: Uuid::new_v4(),
            activate: Uuid::new_v4(),
            expected,
            administrator: false,
            csr: None,
            grant: None,
            renewal_dispatched: false,
            issue_started: false,
            candidate: None,
            validated_validity: None,
            unknown: false,
            issuance_id: None,
        };
        let tx = self.journal.db.unchecked_transaction()?;
        write(&tx, "enrollment", &Some(&p))?;
        tx.execute("DELETE FROM state WHERE name='retired-registration'", [])?;
        tx.commit()?;
        Ok(p)
    }
    pub(super) fn release_settled(&self) -> Result<(), Error> {
        let jobs: Vec<(String, bool)> = read(&self.journal.db, "release")?.unwrap_or_default();
        for (reference, key) in &jobs {
            self.secrets.release(reference, *key)?;
        }
        self.journal
            .db
            .execute("DELETE FROM state WHERE name='release'", [])?;
        Ok(())
    }
    fn end_grant(&self, p: &Pending, reason: enroll::ErrorCode) -> Result<(), Error> {
        let tx = self.journal.db.unchecked_transaction()?;
        write(
            &tx,
            &format!("registration/{}", p.operation),
            &(reason, &p.grant),
        )?;
        let mut release: Vec<(String, bool)> = read(&tx, "release")?.unwrap_or_default();
        release.extend([(p.key(), true), (p.secret(), false)]);
        write(&tx, "release", &release)?;
        write(&tx, "enrollment", &None::<Pending>)?;
        tx.commit()?;
        Ok(())
    }
    fn request(&self, p: &mut Pending) -> Result<wire::GrantRequest, Error> {
        let secret = self.secrets.create_secret(&p.secret())?;
        if p.csr.is_none() {
            self.secrets.create_key(&p.key())?;
            p.csr = Some(crate::identity::csr_with_subject(
                &self.secrets,
                &p.key(),
                "CN=rss-da-device"
                    .parse()
                    .map_err(|_| Error::Configuration)?,
            )?);
            self.save(p)?;
        }
        Ok(wire::GrantRequest {
            operation_id: p.operation,
            secret,
            csr: p.csr.clone().ok_or(Error::Storage)?,
            expires_in_seconds: 3600,
        })
    }
    /// Hand the CSR-bound request to an administrator through protected local storage.
    pub fn prepare<T>(
        &self,
        operation: Uuid,
        export: impl FnOnce(wire::GrantRequest) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let expected = self.active()?.map(|active| active.receipt.device);
        let mut p = self.begin(operation, expected)?;
        if p.grant.is_some() || p.renewal_dispatched {
            return Err(Error::DaGrantPending);
        }
        let exported = export(self.request(&mut p)?)?;
        p.administrator = true;
        self.save(&p)?;
        Ok(exported)
    }
    /// Attach the exact administrator grant to its locally prepared CSR.
    pub fn import(&self, grant: wire::Grant) -> Result<(), Error> {
        let Some(mut p) = self.pending()? else {
            let completed: Option<Vec<u8>> =
                read(&self.journal.db, &format!("grant/{}", grant.operation_id))?;
            return if completed == Some(crate::store::encode(&grant)?) {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        };
        if grant.operation_id != p.operation
            || grant.grant_id.is_nil()
            || p.csr.is_none()
            || p.expected.is_some_and(|device| device != grant.device)
        {
            return Err(Error::Identity);
        }
        if let Some(old) = &p.grant {
            if crate::store::encode(old)? != crate::store::encode(&grant)? {
                return Err(Error::Conflict);
            }
        }
        if !p.issue_started && grant.expires_at <= self.now()? {
            p.grant = Some(grant);
            self.end_grant(&p, enroll::ErrorCode::GrantExpired)?;
            return Err(Error::Expired);
        }
        p.grant = Some(grant);
        self.save(&p)
    }
    /// Reserve or resume a replacement credential for the active registration.
    pub fn start_renewal(&self) -> Result<Uuid, Error> {
        let active = self.active()?.ok_or(Error::Identity)?;
        let operation = self
            .live_pending()?
            .map_or_else(Uuid::new_v4, |p| p.operation);
        self.begin(operation, Some(active.receipt.device))?;
        Ok(operation)
    }
    pub(super) async fn enroll(&self, trust: &VerifiedTrust) -> Result<(), Error> {
        let Some(mut p) = self.pending()? else {
            return Ok(());
        };
        if p.grant.is_none() {
            if p.administrator {
                return Ok(());
            }
            let Some(expected) = p.expected else {
                return Ok(());
            };
            let active = self.active()?.ok_or(Error::Identity)?;
            let (_, _, device) = crate::tls::device_validity(
                &active.chain,
                &active.key,
                &self.secrets,
                self.now()?,
                trust,
            )?;
            if expected != device {
                return Err(Error::Identity);
            }
            let http = self.http(Some((&active.key, &active.chain)))?;
            let request = self.request(&mut p)?;
            if !p.renewal_dispatched {
                p.renewal_dispatched = true;
                self.save(&p)?;
            }
            self.import(self.call::<h::DaRenew>(&http, None, Some(&request)).await?)?;
            p = self.pending()?.ok_or(Error::Storage)?;
        }
        let grant = p.grant.as_ref().ok_or(Error::Conflict)?;
        let http = self.http(None)?;
        // Progress is keyed by the grant operation, including after a lost activation reply.
        let progress = self
            .call::<h::DaProgress>(
                &http,
                None,
                Some(&enroll::StatusRequest {
                    wire_version: 1,
                    operation_id: p.operation,
                    grant_id: grant.grant_id,
                    secret: self.secrets.resolve(&p.secret())?,
                }),
            )
            .await?;
        if crate::store::encode(&progress.grant)? != crate::store::encode(grant)? {
            return Err(Error::Identity);
        }
        if p.issuance_id.is_some() && p.issuance_id != progress.issuance_id {
            return Err(Error::Identity);
        }
        if progress.issuance_unknown {
            p.issuance_id = progress.issuance_id;
            p.unknown = true;
            self.save(&p)?;
            return Err(Error::IssuanceUnknown);
        }
        if p.candidate.is_none() {
            let request = wire::CertificateRequest {
                operation_id: p.issue,
                grant_id: grant.grant_id,
                secret: self.secrets.resolve(&p.secret())?,
                csr: p.csr.clone().ok_or(Error::Storage)?,
            };
            if !p.issue_started {
                p.issue_started = true;
                self.save(&p)?;
            }
            let candidate = match self
                .exchange::<h::DaCertificate>(&http, None, Some(&request))
                .await?
            {
                Ok(candidate) => candidate,
                Err(fault) => match fault.code {
                    enroll::ErrorCode::IssuanceUnknown => {
                        p.issuance_id = fault.issuance_id;
                        p.unknown = true;
                        self.save(&p)?;
                        return Err(Error::IssuanceUnknown);
                    }
                    enroll::ErrorCode::GrantExpired | enroll::ErrorCode::GrantRevoked
                        if progress.registration.is_none() =>
                    {
                        self.end_grant(&p, fault.code)?;
                        return Err(crate::identity::enrollment_error(fault.code));
                    }
                    code => return Err(crate::identity::enrollment_error(code)),
                },
            };
            if candidate.operation_id != p.issue
                || candidate.issuance_id.is_nil()
                || progress
                    .issuance_id
                    .is_some_and(|id| id != candidate.issuance_id)
            {
                return Err(Error::Identity);
            }
            p.issuance_id = Some(candidate.issuance_id);
            p.candidate = Some(candidate);
            p.unknown = false;
            self.save(&p)?;
        }
        let grant = p.grant.clone().ok_or(Error::Storage)?;
        let candidate = p.candidate.clone().ok_or(Error::Storage)?;
        let key = p.key();
        let device = grant.device;
        if p.expected.is_some_and(|expected| expected != device)
            || progress
                .issuance_id
                .is_some_and(|id| id != candidate.issuance_id)
        {
            return Err(Error::Identity);
        }
        let receipt = match progress.registration {
            Some(receipt) => receipt,
            None => {
                if candidate.activate_before <= self.now()? {
                    self.end_grant(&p, enroll::ErrorCode::GrantExpired)?;
                    return Err(Error::Expired);
                }
                let (before, after, certified) = match crate::tls::device_validity(
                    &candidate.certificate_chain,
                    &key,
                    &self.secrets,
                    self.now()?,
                    trust,
                ) {
                    Err(Error::CertificateExpired) => {
                        self.end_grant(&p, enroll::ErrorCode::GrantExpired)?;
                        return Err(Error::Expired);
                    }
                    value => value?,
                };
                if certified != device {
                    return Err(Error::Identity);
                }
                p.validated_validity = Some((before, after));
                self.save(&p)?;
                let peer = self.http(Some((&key, &candidate.certificate_chain)))?;
                match self
                    .exchange::<h::DaActivate>(
                        &peer,
                        None,
                        Some(&enroll::ActivationRequest {
                            wire_version: 1,
                            operation_id: p.activate,
                            issuance_id: candidate.issuance_id,
                        }),
                    )
                    .await?
                {
                    Ok(receipt) => receipt,
                    Err(fault) => {
                        if matches!(
                            fault.code,
                            enroll::ErrorCode::GrantExpired | enroll::ErrorCode::GrantRevoked
                        ) {
                            self.end_grant(&p, fault.code)?;
                        }
                        return Err(crate::identity::enrollment_error(fault.code));
                    }
                }
            }
        };
        if receipt.service_id != self.backend.instance
            || receipt.tenant != self.backend.tenant
            || receipt.device != device
            || receipt.operation_id != p.activate
            || receipt.issuance_id != candidate.issuance_id
            || receipt.credential.is_nil()
        {
            return Err(Error::Identity);
        }
        let mut release: Vec<(String, bool)> =
            read(&self.journal.db, "release")?.unwrap_or_default();
        if let Some(previous) = self.active()? {
            if previous.key == key {
                return Err(Error::Identity);
            }
            release.push((previous.key, true));
        }
        release.push((p.secret(), false));
        let tx = self.journal.db.unchecked_transaction()?;
        write(&tx, &format!("registration/{}", p.operation), &receipt)?;
        write(
            &tx,
            "active",
            &Active {
                receipt,
                key,
                chain: candidate.certificate_chain.clone(),
                validity: p.validated_validity.ok_or(Error::Identity)?,
            },
        )?;
        write(
            &tx,
            &format!("grant/{}", grant.operation_id),
            &crate::store::encode(&grant)?,
        )?;
        write(&tx, "release", &release)?;
        write(&tx, "enrollment", &None::<Pending>)?;
        tx.commit()?;
        Ok(())
    }
}

/// Reopen the platform references required to resume this exact registration. Expired
/// certificates remain inspectable during maintenance; this check never authorizes TLS.
pub fn validate_keys(
    root: &Path,
    backend: &Backend,
    installation: Uuid,
    secrets: &impl IdentityProvider,
) -> Result<(), Error> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use der::{Decode, Encode};
    let Some(db) = storage::inspect(root, backend, installation)? else {
        return Ok(());
    };
    let check_chain = |chain: &[String], key: &str| {
        let certified =
            rustls::sign::CertifiedKey::new(crate::tls::certificates(chain)?, secrets.key(key)?);
        certified.keys_match().map_err(|_| Error::Identity)
    };
    if let Some(active) = read::<Active>(&db, "active")? {
        if active.receipt.service_id != backend.instance || active.receipt.tenant != backend.tenant
        {
            return Err(Error::Identity);
        }
        check_chain(&active.chain, &active.key)?;
    }
    if let Some(p) = read::<Option<Pending>>(&db, "enrollment")?.flatten() {
        if let Some(csr) = &p.csr {
            let request = x509_cert::request::CertReq::from_der(
                &STANDARD.decode(csr).map_err(|_| Error::Storage)?,
            )
            .map_err(|_| Error::Storage)?;
            let key = secrets.key(&p.key())?;
            if key.public_key().ok_or(Error::Identity)?.as_ref()
                != request
                    .info
                    .public_key
                    .to_der()
                    .map_err(|_| Error::Storage)?
            {
                return Err(Error::Identity);
            }
            secrets.resolve(&p.secret())?;
        }
        if let Some(candidate) = &p.candidate {
            check_chain(&candidate.certificate_chain, &p.key())?;
        }
    }
    Ok(())
}
