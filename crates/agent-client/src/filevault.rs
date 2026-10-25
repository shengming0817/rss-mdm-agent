//! Device-authenticated CMS custody, independent of ordinary tasks and employee sessions.
//! ref: RustCrypto cms 0.2.3 enveloped_data.rs; Apple DeviceManagement/FDERecoveryKeyEscrow.
use crate::{
    store::{encode, hash},
    Client, Clock, Error, IdentityProvider,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use der::Decode;
use execution_contract::FilevaultState as Status;
use reqwest::Method;
use rss_mdm_agent_wire::filevault as w;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Closed native reader result; neither paths nor material bytes enter diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// No system escrow material has been produced.
    Missing,
    /// Metadata, link, size or concurrent-change checks failed.
    Unsafe,
    /// The fixed file cannot currently be read.
    Unavailable,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    input: w::Upload,
    attempted: bool,
    rejected: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    registration: Uuid,
    generation: u64,
    pending: Option<Pending>,
    receipt: Option<w::Receipt>,
    next_check: i64,
    next_retry: i64,
    status: Status,
}
impl State {
    fn save(&self, store: &crate::store::Store) -> Result<(), Error> {
        let body = encode(self)?;
        // Upload carriers include base64 overhead, independently of
        // the smaller HTTP response budget. SQLite's existing page ceiling still applies.
        if body.len() > w::REQUEST_BYTES + 8192 {
            return Err(Error::Capacity);
        }
        store.conn.execute(
            "INSERT INTO state VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",
            rusqlite::params![KEY, body],
        )?;
        Ok(())
    }
    fn new(registration: Uuid, generation: u64) -> Self {
        Self {
            registration,
            generation,
            pending: None,
            receipt: None,
            next_check: 0,
            next_retry: 0,
            status: Status::NotConfigured,
        }
    }
}
const KEY: &str = "filevault-custody";
fn config_valid(c: &w::Configuration) -> Result<(), Error> {
    if c.revision < 0 || c.material_revision < 0 || c.custody_epoch < 0 {
        return Err(Error::Protocol);
    }
    if !c.enabled {
        return Ok(());
    }
    if c.revision == 0 || c.custody_id.is_none_or(|v| v.is_nil()) || c.custody_epoch == 0 {
        return Err(Error::Protocol);
    }
    let id = c.recipient_id.as_deref().ok_or(Error::Protocol)?;
    let cert = c.recipient_certificate.as_deref().ok_or(Error::Protocol)?;
    if id.len() != 64 || cert.len() > 32768 {
        return Err(Error::Protocol);
    }
    let der = STANDARD.decode(cert).map_err(|_| Error::Protocol)?;
    if hash(&der) != id {
        return Err(Error::Untrusted);
    }
    x509_cert::Certificate::from_der(&der).map_err(|_| Error::Protocol)?;
    Ok(())
}
fn recipient_matches(bytes: &[u8], c: &w::Configuration) -> Result<bool, Error> {
    use cms::{
        content_info::ContentInfo,
        enveloped_data::{EnvelopedData, RecipientIdentifier, RecipientInfo},
    };
    if bytes.is_empty() || bytes.len() > w::CMS_BYTES {
        return Err(Error::Capacity);
    }
    let info = ContentInfo::from_der(bytes).map_err(|_| Error::Protocol)?;
    if info.content_type != der::asn1::ObjectIdentifier::new_unwrap("1.2.840.113549.1.7.3") {
        return Err(Error::Protocol);
    }
    let envelope: EnvelopedData = info.content.decode_as().map_err(|_| Error::Protocol)?;
    if envelope
        .encrypted_content
        .encrypted_content
        .as_ref()
        .is_none_or(|v| v.as_bytes().is_empty())
    {
        return Err(Error::Protocol);
    }
    let certificate = STANDARD
        .decode(c.recipient_certificate.as_deref().ok_or(Error::Protocol)?)
        .map_err(|_| Error::Protocol)?;
    let cert = x509_cert::Certificate::from_der(&certificate).map_err(|_| Error::Protocol)?;
    Ok(envelope
        .recip_infos
        .0
        .iter()
        .any(|recipient| match recipient {
            RecipientInfo::Ktri(key) => match &key.rid {
                RecipientIdentifier::IssuerAndSerialNumber(id) => {
                    id.issuer == cert.tbs_certificate.issuer
                        && id.serial_number == cert.tbs_certificate.serial_number
                }
                RecipientIdentifier::SubjectKeyIdentifier(id) => cert
                    .tbs_certificate
                    .extensions
                    .as_ref()
                    .is_some_and(|extensions| {
                        extensions.iter().any(|e| {
                            e.extn_id == der::asn1::ObjectIdentifier::new_unwrap("2.5.29.14")
                                && der::asn1::OctetString::from_der(e.extn_value.as_bytes())
                                    .is_ok_and(|v| v.as_bytes() == id.0.as_bytes())
                        })
                    }),
            },
            _ => false,
        }))
}
fn same_material(receipt: &w::Receipt, c: &w::Configuration, digest: &str) -> bool {
    receipt.custody_id == c.custody_id.unwrap_or_default()
        && receipt.custody_epoch == c.custody_epoch
        && receipt.configuration_revision == c.revision
        && Some(receipt.recipient_id.as_str()) == c.recipient_id.as_deref()
        && receipt.cms_digest == digest
        && receipt.escrowed
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Nonsecret durable diagnostic only; it never grants login or proves recovery validity.
    pub fn filevault_status(&self) -> Result<Status, Error> {
        if self.retiring()? {
            return Ok(Status::Retired);
        }
        let Some(state) = self.store.get::<State>(KEY)? else {
            return Ok(Status::NotConfigured);
        };
        let current = match self.registration() {
            Ok(value) => value,
            Err(Error::Identity) => return Ok(Status::NotConfigured),
            Err(error) => return Err(error),
        };
        Ok(
            if (state.registration, state.generation)
                == (current.registration_id, current.generation)
            {
                state.status
            } else {
                Status::NotConfigured
            },
        )
    }
    /// Start a bounded collection check immediately after host startup/configuration replacement.
    /// A frozen pending operation and its transport payload remain unchanged.
    pub fn refresh_filevault(&mut self) -> Result<(), Error> {
        if let Some(mut state) = self.store.get::<State>(KEY)? {
            state.next_check = 0;
            state.save(&self.store)?;
        }
        Ok(())
    }
    /// One bounded independent drive. The host supplies only its fixed-file reader; tests supply
    /// explicit non-device fixtures. An unknown operation is recovered before reading new bytes.
    pub async fn drive_filevault(
        &mut self,
        read: impl FnOnce() -> Result<Vec<u8>, ReadError>,
    ) -> Result<Status, Error> {
        if self.retiring()? {
            return Ok(Status::Retired);
        }
        let current = self.registration()?;
        let mut state = self
            .store
            .get::<State>(KEY)?
            .unwrap_or_else(|| State::new(current.registration_id, current.generation));
        if (state.registration, state.generation) != (current.registration_id, current.generation) {
            // A confirmed registration replacement revokes the old uploader. Discard its local
            // carrier, never transplant its operation/configuration into the new authenticated source.
            state = State::new(current.registration_id, current.generation);
            state.save(&self.store)?;
        }
        let now = self.now()?;
        if state.pending.as_ref().is_some_and(|p| !p.rejected) {
            if now >= state.next_retry {
                self.submit_filevault(&mut state, now).await?;
            }
            return Ok(state.status);
        }
        if now < state.next_check {
            return Ok(state.status);
        }
        state.next_check = now.saturating_add(60);
        state.save(&self.store)?;
        let config: w::Configuration = match self
            .self_service_json(None, Method::GET, "filevault/configuration", None)
            .await
        {
            Ok(v) => v,
            Err(error) => {
                state.status = Status::Unavailable;
                state.next_check = now.saturating_add(30);
                state.save(&self.store)?;
                return Err(error);
            }
        };
        if let Err(error) = config_valid(&config) {
            state.status = Status::Unavailable;
            state.save(&self.store)?;
            return Err(error);
        }
        if !config.enabled {
            state.pending = None;
            state.status = Status::NotConfigured;
            state.save(&self.store)?;
            return Ok(state.status);
        }
        let bytes = match read() {
            Ok(v) => v,
            Err(error) => {
                state.status = match error {
                    ReadError::Missing => Status::MissingMaterial,
                    ReadError::Unsafe => Status::UnsafeMaterial,
                    ReadError::Unavailable => Status::Unavailable,
                };
                state.save(&self.store)?;
                return Ok(state.status);
            }
        };
        match recipient_matches(&bytes, &config) {
            Ok(true) => (),
            Ok(false) => {
                state.status = Status::RecipientMismatch;
                state.save(&self.store)?;
                return Ok(state.status);
            }
            Err(_) => {
                state.status = Status::UnsafeMaterial;
                state.save(&self.store)?;
                return Ok(state.status);
            }
        }
        let digest = hash(&bytes);
        if state
            .receipt
            .as_ref()
            .is_some_and(|r| same_material(r, &config, &digest))
        {
            state.status = Status::EscrowedUnverified;
            state.save(&self.store)?;
            return Ok(state.status);
        }
        if state.pending.as_ref().is_some_and(|p| {
            p.rejected
                && p.input.cms_digest == digest
                && p.input.custody_id == config.custody_id.unwrap_or_default()
                && p.input.custody_epoch == config.custody_epoch
                && p.input.configuration_revision == config.revision
                && Some(p.input.recipient_id.as_str()) == config.recipient_id.as_deref()
                && p.input.expected_revision == config.material_revision
        }) {
            state.status = Status::Rejected;
            state.save(&self.store)?;
            return Ok(state.status);
        }
        let input = w::Upload {
            operation_id: Uuid::new_v4(),
            custody_id: config.custody_id.ok_or(Error::Protocol)?,
            custody_epoch: config.custody_epoch,
            configuration_revision: config.revision,
            expected_revision: config.material_revision,
            recipient_id: config.recipient_id.ok_or(Error::Protocol)?,
            cms_digest: digest,
            cms: STANDARD.encode(bytes),
        };
        input.decode()?;
        state.pending = Some(Pending {
            input,
            attempted: false,
            rejected: false,
        });
        state.status = Status::Pending;
        state.save(&self.store)?; // FULL-synchronous SQLite publication precedes transport.
        self.submit_filevault(&mut state, now).await?;
        Ok(state.status)
    }
    async fn upload_filevault(&self, input: &w::Upload) -> Result<Option<w::Receipt>, Error> {
        self.active()?;
        self.now()?;
        input.decode()?;
        let body = encode(input)?;
        if body.len() > w::REQUEST_BYTES {
            return Err(Error::Capacity);
        }
        let request = self
            .http
            .post(self.url("filevault/escrows")?)
            .timeout(
                self.store
                    .cfg
                    .limits
                    .request_timeout
                    .min(std::time::Duration::from_secs(2)),
            )
            .header("content-type", "application/json")
            .body(body);
        let (status, bytes) = self
            .agent_json_transport(request, self.store.cfg.limits.response_bytes)
            .await?;
        if status.is_success() {
            return crate::store::decode(&bytes).map(Some);
        }
        let failure: crate::wire::ErrorBody = crate::store::decode(&bytes)?;
        if status.is_client_error()
            && matches!(
                failure.code,
                crate::wire::ErrorCode::MalformedRequest
                    | crate::wire::ErrorCode::PermissionDenied
                    | crate::wire::ErrorCode::OperationConflict
            )
        {
            return Ok(None);
        }
        self.runtime_error(&bytes)?;
        Err(Error::Unavailable)
    }
    async fn submit_filevault(&mut self, state: &mut State, now: i64) -> Result<(), Error> {
        let pending = state.pending.as_mut().ok_or(Error::Protocol)?;
        let attempted = pending.attempted;
        let input = pending.input.clone();
        pending.attempted = true;
        state.next_retry = now.saturating_add(30);
        state.status = Status::Pending;
        state.save(&self.store)?;
        let result = if attempted {
            match self
                .self_service_json::<Option<w::Receipt>>(
                    None,
                    Method::GET,
                    &format!("filevault/operations/{}", input.operation_id),
                    None,
                )
                .await
            {
                Ok(Some(receipt)) => Ok(Some(receipt)),
                Ok(None) => self.upload_filevault(&input).await,
                // A failed status read says nothing about the original upload outcome.
                Err(error) => Err(error),
            }
        } else {
            self.upload_filevault(&input).await
        };
        match result {
            Ok(Some(receipt)) if receipt.matches(&input) => {
                state.pending = None;
                state.receipt = Some(receipt);
                state.status = Status::EscrowedUnverified;
                state.next_check = now.saturating_add(60);
                state.save(&self.store)?;
            }
            Ok(Some(_)) => return Err(Error::Untrusted), // Unknown outcome: retain the exact frozen input.
            Ok(None) => {
                state.pending.as_mut().ok_or(Error::Protocol)?.rejected = true;
                state.status = Status::Rejected;
                state.next_check = now.saturating_add(60);
                state.save(&self.store)?;
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }
}
