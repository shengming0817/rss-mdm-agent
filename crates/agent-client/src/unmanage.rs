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
    accepted_at: i64,
    drained: bool,
    outcome: Option<wire::UnmanageOutcome>,
}
/// An accepted original Start, never a caller-created uninstall capability.
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
    /// A prior terminal local observation; it never authorizes running cleanup again.
    pub fn outcome(&self) -> Option<wire::UnmanageOutcome> {
        self.0.outcome
    }
}
/// An in-memory transport for the original retirement, retained before deleting keys.
/// It cannot authorize work or create a replacement identity.
pub struct UnmanageTransport {
    http: reqwest::Client,
    url: url::Url,
    max: usize,
    timeout: std::time::Duration,
    task: Uuid,
    attempt: Uuid,
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
            accepted_at: self.now()?,
            drained: false,
            outcome: None,
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
        if permit.0.outcome.is_some() {
            return Err(Error::Conflict);
        }
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
            url: self.url(&format!("tasks/{}/events", permit.task().task_id))?,
            max: self.store.cfg.limits.response_bytes,
            timeout: self.store.cfg.limits.request_timeout,
            task: permit.task().task_id,
            attempt: permit.task().attempt_id,
        })
    }
    /// Freeze and send the installation owner's observed result through the existing requests table.
    /// No active-key check is repeated after destruction; only this pre-bound transport is usable.
    pub async fn finish_unmanage(
        &mut self,
        permit: &UnmanagePermit,
        transport: UnmanageTransport,
        outcome: wire::UnmanageOutcome,
    ) -> Result<(), Error> {
        self.current_unmanage(permit)?;
        if !permit.drained()
            || transport.task != permit.task().task_id
            || transport.attempt != permit.task().attempt_id
            || permit.outcome().is_some_and(|old| old != outcome)
        {
            return Err(Error::Conflict);
        }
        let mut record = permit.0.clone();
        record.outcome = Some(outcome);
        let key = format!("unmanage-result/{}", transport.task);
        let request = self.event_request(
            &key,
            transport.task,
            transport.attempt,
            wire::TaskEvent::UnmanageResult { outcome },
            None,
        )?;
        self.store.put("unmanage", &record)?;
        let (status, body) = crate::client::json_transport(
            transport
                .http
                .post(transport.url)
                .timeout(transport.timeout)
                .header("content-type", "application/json")
                .body(encode(&request)?),
            transport.max,
        )
        .await?;
        if !status.is_success() {
            return Err(Error::Unavailable);
        }
        let ack: wire::TaskEventAck = decode(&body)?;
        if ack.permit().is_some() {
            return Err(Error::Protocol);
        }
        self.store
            .conn
            .execute("UPDATE requests SET accepted=1 WHERE key=?1", [key])?;
        Ok(())
    }
    /// Enumerate only credentials referenced by this installation's current enrollment state.
    /// The protected credential owner also retains its own complete creation catalog.
    pub fn unmanage_credentials(
        &self,
        permit: &UnmanagePermit,
    ) -> Result<Vec<(String, bool)>, Error> {
        self.current_unmanage(permit)?;
        if !permit.drained() {
            return Err(Error::Conflict);
        }
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
    let key = cfg.keys.get(&record.start.key_id).ok_or(Error::Untrusted)?;
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
