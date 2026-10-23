//! Exact accepted retirement authority, persisted in the existing communication store.
use crate::{
    store::{decode, encode},
    wire, Client, Clock, Error, IdentityProvider, Start,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    installation: Uuid,
    start: wire::SignedTask,
    verification_key: Vec<u8>,
    accepted_at: i64,
    drained: bool,
    binding: wire::BindingProfile,
    context: wire::SoftwareExecutionContext,
    result_operation: Uuid,
    references: Vec<(String, bool)>,
}
/// An accepted original Start, never a caller-created uninstall capability.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnmanagePermit(Record);
impl UnmanagePermit {
    /// Exact installation selected by the protected product owner.
    pub fn installation(&self) -> Uuid {
        self.0.installation
    }
    /// Frozen original task coordinates.
    pub fn task(&self) -> &wire::UnmanageTaskSpec {
        let wire::TaskPayload::Unmanage(task) = &self.0.start.payload else {
            unreachable!()
        };
        task
    }
    /// Whether the execution owner durably confirmed all attempts quiescent.
    pub fn drained(&self) -> bool {
        self.0.drained
    }
    /// Exact current enrollment references captured before the installation handoff.
    pub fn credentials(&self) -> &[(String, bool)] {
        &self.0.references
    }
    /// Validate a protected installation-owner handoff after the communication store was removed.
    pub fn verify_handoff(&self, cfg: &crate::Config, installation: Uuid) -> Result<(), Error> {
        if !self.drained() {
            return Err(Error::Denied);
        }
        validate_record(&self.0, cfg, installation, &self.0.binding)
    }
}
/// An in-memory transport for the original retirement, retained before deleting keys.
/// It cannot authorize work or create a replacement identity.
pub struct UnmanageTransport {
    http: reqwest::Client,
    urls: Vec<url::Url>,
    max: usize,
    timeout: std::time::Duration,
    attempt: Uuid,
    operation: Uuid,
    context: wire::SoftwareExecutionContext,
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Use the ordinary Received/Start exchange, including its immutable restart replay.
    pub async fn authorize_unmanage(
        &mut self,
        offer: &crate::Offer,
        installation: Uuid,
    ) -> Result<UnmanagePermit, Error> {
        if !matches!(offer.payload(), wire::TaskPayload::Unmanage(_)) {
            return Err(Error::Unsupported);
        }
        let key = format!("start/{}/{}", offer.task_id(), offer.attempt_id());
        let pending: bool = self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM requests WHERE key=?1)",
            [key],
            |r| r.get(0),
        )?;
        let start = if pending {
            self.recover_start(offer.task_id()).await?.start
        } else {
            self.received(offer).await?;
            let materials = self.prepare(offer).await?;
            self.request_start(offer, &materials).await?
        };
        self.accept_unmanage(&start, installation)
    }
    /// Accept a freshly verified signed Start once for this protected installation.
    pub fn accept_unmanage(
        &mut self,
        start: &Start,
        installation: Uuid,
    ) -> Result<UnmanagePermit, Error> {
        if installation.is_nil() || !matches!(start.payload(), wire::TaskPayload::Unmanage(_)) {
            return Err(Error::Untrusted);
        }
        self.validate_start(start)?;
        if let Some(old) = self.accepted_unmanage(installation)? {
            if !crate::client::same_input(&old.0.start.payload, start.payload()) {
                return Err(Error::Conflict);
            }
            return Ok(old);
        }
        let record = Record {
            installation,
            start: start.signed.clone(),
            verification_key: self.task_verification_key(&start.signed.key_id)?,
            accepted_at: self.now()?,
            drained: false,
            binding: self.store.registration()?,
            context: self.execution_context()?,
            result_operation: Uuid::new_v4(),
            references: self.current_credential_references()?,
        };
        self.store.put("unmanage", &record)?;
        Ok(UnmanagePermit(record))
    }
    /// Resume only already accepted authority. Its expiry never grants a new execution.
    pub fn accepted_unmanage(&self, installation: Uuid) -> Result<Option<UnmanagePermit>, Error> {
        let Some(record) = self.store.get::<Record>("unmanage")? else {
            return Ok(None);
        };
        validate_record(
            &record,
            &self.store.cfg,
            installation,
            &self.store.registration()?,
        )?;
        Ok(Some(UnmanagePermit(record)))
    }
    fn current_unmanage(&self, permit: &UnmanagePermit) -> Result<(), Error> {
        if self
            .accepted_unmanage(permit.installation())?
            .is_none_or(|p| p.0 != permit.0)
        {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    /// Record the installation owner's quiescence handoff without changing task identity.
    pub fn unmanage_drained(&mut self, permit: UnmanagePermit) -> Result<UnmanagePermit, Error> {
        self.current_unmanage(&permit)?;
        let mut record = permit.0;
        record.drained = true;
        self.store.put("unmanage", &record)?;
        Ok(UnmanagePermit(record))
    }
    /// Retain only the current authenticated in-memory transport before credential destruction.
    pub fn unmanage_transport(&self, permit: &UnmanagePermit) -> Result<UnmanageTransport, Error> {
        self.current_unmanage(permit)?;
        self.active()?;
        if !permit.drained() {
            return Err(Error::Conflict);
        }
        Ok(UnmanageTransport {
            http: self.http.clone(),
            urls: self.connection_urls(&format!(
                "api/v1/agent/runtime/tasks/{}/events",
                permit.task().task_id
            ))?,
            max: self.store.cfg.limits.response_bytes,
            timeout: self.store.cfg.limits.request_timeout,
            attempt: permit.task().attempt_id,
            operation: permit.0.result_operation,
            context: permit.0.context.clone(),
        })
    }
    fn current_credential_references(&self) -> Result<Vec<(String, bool)>, Error> {
        let state = self.identity_state()?;
        let mut refs = Vec::new();
        if let Some(v) = state.active {
            refs.extend([(v.key_reference, true), (v.secret_reference, false)]);
        }
        if let Some(v) = state.pending {
            refs.extend([(v.key_reference, true), (v.secret_reference, false)]);
        }
        refs.sort();
        refs.dedup();
        Ok(refs)
    }
    pub(crate) fn retiring(&self) -> Result<bool, Error> {
        Ok(self.store.get::<Record>("unmanage")?.is_some())
    }
}

fn validate_record(
    record: &Record,
    cfg: &crate::Config,
    installation: Uuid,
    binding: &wire::BindingProfile,
) -> Result<(), Error> {
    let wire::TaskPayload::Unmanage(task) = &record.start.payload else {
        return Err(Error::Untrusted);
    };
    if record.installation != installation || installation.is_nil() {
        return Err(Error::Identity);
    }
    let key = &record.verification_key;
    if key.len() != 32 {
        return Err(Error::Untrusted);
    }
    crate::protocol::verify_task(
        &record.start,
        &crate::protocol::TaskVerification {
            key_id: &record.start.key_id,
            public_key: key,
            tenant_id: cfg.tenant,
            device_id: &binding.device_id,
            platform: cfg.platform,
            architecture: cfg.architecture,
            registration_id: binding.registration_id,
            generation: binding.generation,
            task_id: task.task_id,
            attempt_id: task.attempt_id,
            permit: wire::TaskPermit::Start,
            now: record.accepted_at,
        },
    )?;
    Ok(())
}
/// Inspect the already accepted handoff before stopping a live service. This read-only
/// path grants nothing unless the original Start and the execution-owner drain are present.
pub fn inspect_unmanage(
    root: &std::path::Path,
    cfg: &crate::Config,
    installation: Uuid,
    task: Uuid,
) -> Result<UnmanagePermit, Error> {
    let conn = crate::store::read_existing(root, cfg)?;
    let tx = conn.unchecked_transaction()?;
    let raw: Vec<u8> = tx.query_row(
        "SELECT body FROM state WHERE key='unmanage' AND length(body)<=?1",
        [cfg.limits.response_bytes],
        |r| r.get(0),
    )?;
    let record: Record = decode(&raw)?;
    let raw: Vec<u8> = tx.query_row(
        "SELECT body FROM state WHERE key='identity' AND length(body)<=?1",
        [cfg.limits.response_bytes],
        |r| r.get(0),
    )?;
    let identity: crate::identity::IdentityState = decode(&raw)?;
    let binding = identity.active.ok_or(Error::Identity)?.receipt.runtime;
    validate_record(&record, cfg, installation, &binding)?;
    let permit = UnmanagePermit(record);
    if permit.task().task_id != task || !permit.drained() {
        return Err(Error::Denied);
    }
    let accepted: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM requests WHERE key=?1 AND accepted=1)",
        [format!("start/{task}/{}", permit.task().attempt_id)],
        |r| r.get(0),
    )?;
    if !accepted {
        return Err(Error::Denied);
    }
    Ok(permit)
}

impl UnmanageTransport {
    /// Send only a terminal observation already persisted by the protected installation owner.
    /// The exact operation, attempt and execution context were frozen before credential removal.
    /// Consuming the transport cannot recreate credentials or authorize additional work.
    pub async fn report(self, outcome: wire::UnmanageOutcome) -> Result<(), Error> {
        let request = wire::TaskEventRequest::new(
            self.operation,
            self.attempt,
            wire::TaskEvent::UnmanageResult { outcome },
            self.context,
        )?;
        let body = encode(&request)?;
        let mut accepted = None;
        for url in self.urls {
            match crate::client::json_transport(
                self.http
                    .post(url)
                    .timeout(self.timeout)
                    .header("content-type", "application/json")
                    .body(body.clone()),
                self.max,
            )
            .await
            {
                Err(Error::Network) => continue,
                Err(error) => return Err(error),
                Ok(response) => {
                    accepted = Some(response);
                    break;
                }
            }
        }
        let (status, body) = accepted.ok_or(Error::Unavailable)?;
        if !status.is_success() {
            return Err(Error::Unavailable);
        }
        let ack: wire::TaskEventAck = decode(&body)?;
        if ack.permit().is_some() {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}
