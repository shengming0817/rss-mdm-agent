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
    csr: Option<String>,
    pub grant: Option<wire::Grant>,
    candidate: Option<enroll::Candidate>,
    pub unknown: bool,
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
    pub lifetime: i64,
}

impl<S: Credentials, C: Clock> Client<S, C> {
    pub(super) fn active(&self) -> Result<Option<Active>, Error> {
        read(&self.journal.db, "active")
    }
    pub(super) fn pending(&self) -> Result<Option<Pending>, Error> {
        Ok(read(&self.journal.db, "enrollment")?.flatten())
    }
    fn save(&self, p: &Pending) -> Result<(), Error> {
        write(&self.journal.db, "enrollment", &Some(p))
    }
    fn begin(&self, operation: Uuid, expected: Option<DeviceId>) -> Result<Pending, Error> {
        if operation.is_nil() {
            return Err(Error::Configuration);
        }
        if let Some(p) = self.pending()? {
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
            csr: None,
            grant: None,
            candidate: None,
            unknown: false,
        };
        self.save(&p)?;
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
        let error = crate::identity::enrollment_error(reason);
        let tx = self.journal.db.unchecked_transaction()?;
        write(&tx, &format!("registration/{}", p.operation), &(reason, p))?;
        let mut release: Vec<(String, bool)> = read(&tx, "release")?.unwrap_or_default();
        release.extend([(p.key(), true), (p.secret(), false)]);
        write(&tx, "release", &release)?;
        write(&tx, "enrollment", &None::<Pending>)?;
        tx.commit()?;
        Err(error)
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
    pub fn prepare(&self, operation: Uuid) -> Result<wire::GrantRequest, Error> {
        if self.active()?.is_some() {
            return Err(Error::Conflict);
        }
        let mut p = self.begin(operation, None)?;
        self.request(&mut p)
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
            return Ok(());
        }
        if grant.expires_at <= self.now()? {
            p.grant = Some(grant);
            return self.end_grant(&p, enroll::ErrorCode::GrantExpired);
        }
        p.grant = Some(grant);
        self.save(&p)
    }
    /// Reserve or resume a replacement credential for the active registration.
    pub fn start_renewal(&self) -> Result<Uuid, Error> {
        let active = self.active()?.ok_or(Error::Identity)?;
        let operation = self.pending()?.map_or_else(Uuid::new_v4, |p| p.operation);
        self.begin(operation, Some(active.receipt.device))?;
        Ok(operation)
    }
    pub(super) async fn enroll(&self, trust: &VerifiedTrust) -> Result<(), Error> {
        let Some(mut p) = self.pending()? else {
            return Ok(());
        };
        if p.grant.is_none() {
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
        if progress.issuance_unknown {
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
            let candidate = match self
                .exchange::<h::DaCertificate>(&http, None, Some(&request))
                .await?
            {
                Ok(candidate) => candidate,
                Err(fault) => match fault.code {
                    enroll::ErrorCode::IssuanceUnknown => {
                        p.unknown = true;
                        self.save(&p)?;
                        return Err(Error::IssuanceUnknown);
                    }
                    enroll::ErrorCode::GrantExpired | enroll::ErrorCode::GrantRevoked
                        if progress.issuance_id.is_none() =>
                    {
                        return self.end_grant(&p, fault.code);
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
            p.candidate = Some(candidate);
            p.unknown = false;
            self.save(&p)?;
        }
        let grant = p.grant.as_ref().ok_or(Error::Storage)?;
        let candidate = p.candidate.as_ref().ok_or(Error::Storage)?;
        let key = p.key();
        let (before, after, device) = crate::tls::device_validity(
            &candidate.certificate_chain,
            &key,
            &self.secrets,
            self.now()?,
            trust,
        )?;
        if device != grant.device
            || p.expected.is_some_and(|expected| expected != device)
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
                    return self.end_grant(&p, enroll::ErrorCode::GrantExpired);
                }
                let peer = self.http(Some((&key, &candidate.certificate_chain)))?;
                self.call::<h::DaActivate>(
                    &peer,
                    None,
                    Some(&enroll::ActivationRequest {
                        wire_version: 1,
                        operation_id: p.activate,
                        issuance_id: candidate.issuance_id,
                    }),
                )
                .await?
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
                lifetime: after - before,
            },
        )?;
        write(
            &tx,
            &format!("grant/{}", grant.operation_id),
            &crate::store::encode(grant)?,
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
