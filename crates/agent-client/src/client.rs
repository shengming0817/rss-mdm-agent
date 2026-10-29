use crate::http::{self as h, Call, Endpoint};
use crate::protocol::*;
use crate::{
    store::{decode, encode, Store},
    wire, *,
};
use reqwest::{Response, StatusCode};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;
/// A verified offer; public callers cannot deserialize or invent this wrapper.
#[derive(Debug)]
pub struct Offer {
    pub(crate) signed: wire::SignedTask,
}
impl Offer {
    /// Complete verified wire payload, preserving software steps and detection.
    pub fn payload(&self) -> &wire::TaskPayload {
        &self.signed.payload
    }
    /// Absolute original Run deadline, independent of the short offer permit.
    pub fn preparation_deadline(&self) -> Option<i64> {
        task_deadline(self.payload())
    }
    /// Exact remote task.
    pub fn task_id(&self) -> Uuid {
        self.payload().task_id()
    }
    /// Exact remote attempt.
    pub fn attempt_id(&self) -> Uuid {
        self.payload().attempt_id()
    }
}
/// A verified Start token bounded by the original Run deadline.
#[derive(Clone)]
pub struct Start {
    pub(crate) signed: wire::SignedTask,
}
impl Start {
    /// Exact verified signed input.
    pub fn payload(&self) -> &wire::TaskPayload {
        &self.signed.payload
    }
    /// Compare immutable signed facts, excluding the protocol permit and its expiry.
    pub fn matches_payload(&self, payload: &wire::TaskPayload) -> bool {
        same_input(payload, self.payload())
    }
}
/// One bounded claim and independent cancellation page.
pub struct Claim {
    /// At most one accepted offer.
    pub offer: Option<Offer>,
    /// Exact remote attempts that must be reconciled/stopped by the host.
    pub cancellations: Vec<wire::TaskCancellation>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimState {
    request: wire::TaskClaimRequest,
    recover_until: i64,
}
/// Fully recovered protocol wrappers; the offer is historical and never grants launch.
pub struct ResumedStart {
    /// Original immutable offer, used for trusted host adaptation.
    pub offer: Offer,
    /// Current verified replay of the original Start permit.
    pub start: Start,
    /// Locked, reverified complete cached materials; recovery never downloads new bytes.
    pub materials: Materials,
}
/// Explicitly driven V1 client; SQLite transactions never cross HTTP awaits.
/// A private-root lease permits one driving owner; bounded HTTP tasks never access its state.
pub struct Client<S, C> {
    pub(crate) store: Store,
    pub(crate) http: reqwest::Client,
    pub(crate) bootstrap: reqwest::Client,
    pub(crate) native_http: reqwest::Client,
    pub(crate) runtime_gate: crate::RuntimeGate,
    pub(crate) task_trust_started: bool,
    pub(crate) pki_observed: bool,
    pub(crate) secrets: S,
    pub(crate) clock: C,
    profiles: Vec<wire::ExecutorProfile>,
    environment: rss_mdm_agent_wire::CollectionEnvironment,
    pub(crate) delivery_owner: Uuid,
    pub(crate) transfers: crate::Transfers,
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Set current configured executors for future polls; a pending retry retains its exact input.
    pub fn set_profiles(&mut self, profiles: Vec<wire::ExecutorProfile>) -> Result<(), Error> {
        wire::TaskClaimRequest::new(Uuid::new_v4(), profiles.clone(), self.execution_context()?)?;
        self.profiles = profiles;
        Ok(())
    }
    /// Observe current collection prerequisites; pending claims retain their original evidence.
    pub fn set_collection_environment(
        &mut self,
        environment: rss_mdm_agent_wire::CollectionEnvironment,
    ) -> Result<(), Error> {
        environment.validate()?;
        self.environment = environment;
        Ok(())
    }
    /// Access the original content consumer owner while a network future is in flight.
    pub fn transfers(&self) -> crate::Transfers {
        self.transfers.clone()
    }
    /// Current persisted context, independent of frozen pending operations.
    pub fn execution_context(&self) -> Result<wire::SoftwareExecutionContext, Error> {
        self.store.get("execution_context")?.ok_or(Error::Storage)
    }
    /// Publish a new observed context generation; exact pending HTTP requests remain immutable.
    pub fn set_execution_context(
        &mut self,
        mut context: wire::SoftwareExecutionContext,
    ) -> Result<(), Error> {
        context.validate_for(self.store.cfg.platform)?;
        let old = self.execution_context()?;
        context.revision = old.revision;
        if context != old {
            context.revision = old.revision.checked_add(1).ok_or(Error::Capacity)?;
        }
        context.validate()?;
        self.store.put("execution_context", &context)
    }
    /// Independently supplied deployment configuration, excluding credentials.
    pub fn configuration(&self) -> &Config {
        &self.store.cfg
    }
    /// Protect known device/enrollment credentials in task results without discarding ordinary output.
    pub fn output_policy(&self) -> Result<crate::CredentialRedactor, Error> {
        let state = self.identity_state()?;
        let mut secrets = Vec::new();
        if let Some(reference) = state.active.as_ref().map(|v| &v.secret_reference) {
            secrets.push(self.secrets.resolve(reference)?);
        }
        if let Some(pending) = &state.pending {
            secrets.push(self.secrets.resolve(&pending.secret_reference)?);
            if let Some(configuration) = &pending.configuration {
                secrets.push(self.secrets.resolve(&configuration.secret_reference)?);
            }
        }
        Ok(crate::CredentialRedactor {
            device: secrets,
            software: self
                .store
                .cfg
                .execution_context
                .source_credentials
                .iter()
                .map(|c| self.secrets.software_read_token(c))
                .collect::<Result<_, _>>()?,
            employee: Vec::new(),
        })
    }
    /// Bounded retained transport tasks; reading this list does not grant another Start.
    pub fn pending_tasks(&self, limit: usize) -> Result<Vec<Uuid>, Error> {
        if limit == 0 || limit > self.store.cfg.limits.pending_tasks {
            return Err(Error::Capacity);
        }
        let mut query = self
            .store
            .conn
            .prepare("SELECT id FROM tasks ORDER BY rowid LIMIT ?1")?;
        let ids = query
            .query_map([limit + 1], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if ids.len() > limit {
            return Err(Error::Capacity);
        }
        ids.into_iter()
            .map(|s| Uuid::parse_str(&s).map_err(|_| Error::Storage))
            .collect()
    }
    /// Existing journal association, used for recovery and reporting rather than redispatch.
    pub fn bound_request(
        &self,
        task: Uuid,
    ) -> Result<Option<execution_contract::RequestId>, Error> {
        self.store
            .get::<crate::delivery::Binding>(&format!("binding/{task}"))
            .map(|v| v.map(|b| b.request))
    }
    /// Create or recover exactly one fixed endpoint/tenant communication namespace.
    pub fn open(
        root: &Path,
        config: Config,
        mode: OpenMode,
        secrets: S,
        clock: C,
    ) -> Result<Self, Error> {
        config.validate()?;
        let store = Store::open(root, config, mode)?;
        let bootstrap = crate::tls::client(&store.cfg, None, &secrets)?;
        // Software sources have server trust, never the device signer or its TLS sessions.
        let native_http = crate::tls::client(&store.cfg, None, &secrets)?;
        let http = bootstrap.clone();
        let transfers = crate::Transfers::open(
            &store.root,
            store.cfg.limits.pending_tasks.saturating_mul(64).min(4096),
            store._lock.clone(),
        )?;
        let mut client = Self {
            transfers,
            delivery_owner: Uuid::new_v4(),
            store,
            http,
            bootstrap,
            native_http,
            runtime_gate: crate::RuntimeGate::default(),
            task_trust_started: false,
            pki_observed: false,
            secrets,
            clock,
            profiles: Vec::new(),
            environment: Default::default(),
        };
        client.initialize_connections()?;
        client.prepare_pending_enrollment()?;
        // Opening retained state must not depend on an expired or unavailable predecessor signer.
        if client.active().is_ok() {
            client.restore_runtime_transport()?;
        }
        client.publish_runtime_gate();
        Ok(client)
    }
    pub(crate) fn now(&self) -> Result<i64, Error> {
        self.store.time(self.clock.now()?)
    }
    pub(crate) fn active(&self) -> Result<(), Error> {
        let identity = self.identity_state()?;
        identity.check_runtime_failure()?;
        let now = self.now()?;
        identity.active_identity(now)?;
        let trust = self.verified_trust()?;
        identity.validate_runtime(&self.store.cfg, &self.secrets, now, &trust)?;
        Ok(())
    }

    pub(crate) fn call<E: Endpoint>(
        &self,
        params: &[&str],
        query: Option<&str>,
        input: Option<&E::Input>,
    ) -> Result<Call<E>, Error> {
        Call::new(&self.http, &self.connection_origin()?, params, query, input)
    }
    pub(crate) async fn checked_response(&self, response: Response) -> Result<Response, Error> {
        if response.status().is_success() {
            return Ok(response);
        }
        let max = self.store.cfg.limits.response_bytes;
        let bytes = bounded(response, max).await.map_err(|error| {
            if error == Error::Network {
                Error::Unavailable
            } else {
                error
            }
        })?;
        self.runtime_error(&bytes)?;
        Err(Error::Protocol)
    }
    pub(crate) fn runtime_error(&self, bytes: &[u8]) -> Result<(), Error> {
        let body: wire::ErrorBody = decode(bytes)?;
        use rss_mdm_agent_wire::enroll::ErrorCode as E;
        use wire::ErrorCode as R;
        let error = match body.code {
            R::CredentialRevoked => {
                self.identity_failure(E::CredentialRevoked)?;
                Error::CredentialRevoked
            }
            R::CertificateExpired => {
                self.identity_failure(E::CertificateExpired)?;
                Error::CertificateExpired
            }
            R::CredentialRetired => {
                self.identity_failure(E::CredentialRetired)?;
                Error::CredentialRetired
            }
            R::InvalidIdentity => Error::Identity,
            R::PermissionDenied => Error::Denied,
            R::OperationConflict => Error::Conflict,
            R::OperationUnknown | R::ServiceUnavailable => Error::Unavailable,
            _ => Error::Protocol,
        };
        Err(error)
    }
    pub(crate) async fn runtime_json<E: Endpoint>(
        &self,
        params: &[&str],
        input: Option<&E::Input>,
    ) -> Result<E::Output, Error> {
        self.runtime_reply(self.call::<E>(params, None, input)?)
            .await
    }
    pub(crate) async fn runtime_frozen_json<E: Endpoint>(
        &self,
        params: &[&str],
        bytes: &[u8],
    ) -> Result<E::Output, Error> {
        self.runtime_reply(Call::<E>::frozen(
            &self.http,
            &self.connection_origin()?,
            params,
            bytes,
        )?)
        .await
    }
    async fn runtime_reply<E: Endpoint>(&self, call: Call<E>) -> Result<E::Output, Error> {
        self.active()?;
        self.now()?;
        let request = call.timeout(self.store.cfg.limits.request_timeout);
        let (status, bytes) = self
            .agent_json_transport(request, self.store.cfg.limits.response_bytes)
            .await?;
        if !status.is_success() {
            self.runtime_error(&bytes)?;
        }
        self.active()?;
        decode(&bytes)
    }
    /// The sole durable runtime binding, derived from the activation receipt.
    pub fn registration(&self) -> Result<wire::BindingProfile, Error> {
        self.store.registration()
    }
    /// Allocate report identity and sequence in the same transaction as its immutable body.
    pub fn queue_report(
        &mut self,
        dataset: &str,
        body: wire::ReportBody,
        observed_at: i64,
    ) -> Result<Uuid, Error> {
        self.active()?;
        let definition = self
            .store
            .registration()?
            .collections
            .into_iter()
            .find(|d| d.dataset() == dataset)
            .ok_or(Error::Protocol)?;
        self.now()?;
        let tx = self.store.conn.transaction()?;
        let count: u64 = tx.query_row("SELECT count(*) FROM reports", [], |r| r.get(0))?;
        if count >= self.store.cfg.limits.pending_reports as u64 {
            return Err(Error::Capacity);
        }
        let sequence: u64 = tx.query_row("SELECT sequence FROM metadata", [], |r| r.get(0))?;
        let id = Uuid::new_v4();
        let request = wire::ReportRequest::new(id, sequence, observed_at, definition, body)?;
        let bytes = request.canonical()?;
        tx.execute(
            "INSERT INTO reports VALUES(?1,?2)",
            params![id.to_string(), bytes],
        )?;
        let next = sequence
            .checked_add(1)
            .and_then(|v| i64::try_from(v).ok())
            .ok_or(Error::Capacity)?;
        tx.execute("UPDATE metadata SET sequence=?1", [next])?;
        tx.commit()?;
        Ok(id)
    }
    /// One bounded at-least-once batch; only matching durable ACKs remove reports.
    pub async fn flush_reports(&mut self, limit: usize) -> Result<usize, Error> {
        if limit == 0 || limit > self.store.cfg.limits.pending_reports {
            return Err(Error::Configuration);
        }
        let reports = {
            let mut stmt=self.store.conn.prepare("SELECT id,CASE WHEN typeof(body)='blob' AND length(body)<=16384 THEN body END FROM reports ORDER BY rowid LIMIT ?1")?;
            let rows = stmt
                .query_map([limit as i64], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let mut sent = 0;
        for (id, body) in reports {
            let ack: wire::ReportAck = self.runtime_frozen_json::<h::Report>(&[], &body).await?;
            if ack.report_id.to_string() != id {
                return Err(Error::Protocol);
            }
            self.store
                .conn
                .execute("DELETE FROM reports WHERE id=?1", [id])?;
            sent += 1;
        }
        Ok(sent)
    }
    /// Persistent intake/projection status is separate from delivery acknowledgement.
    pub async fn report_status(&self, id: Uuid) -> Result<wire::ReportStatus, Error> {
        let result: wire::ReportStatus = self
            .runtime_json::<h::ReportStatus>(&[&id.to_string()], None)
            .await?;
        if result.ack.report_id != id {
            return Err(Error::Protocol);
        }
        Ok(result)
    }
    pub(crate) fn verify(
        &self,
        signed: &wire::SignedTask,
        permit: wire::TaskPermit,
    ) -> Result<(), Error> {
        self.active()?;
        let now = self.now()?;
        let p = &signed.payload;
        let received = permit == wire::TaskPermit::Offer
            && self.store.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM requests WHERE key=?1 AND accepted=1)",
                [format!("received/{}/{}", p.task_id(), p.attempt_id())],
                |r| r.get::<_, bool>(0),
            )?;
        if (if received {
            task_deadline(p)
        } else {
            Some(p.expires_at())
        })
        .is_some_and(|until| now >= until)
        {
            return Err(Error::Expired);
        }
        let registration = self.store.registration()?;
        let key = self.task_verification_key(&signed.key_id)?;
        crate::protocol::verify_task_binding(
            signed,
            &TaskVerification {
                key_id: &signed.key_id,
                public_key: &key,
                tenant_id: self.store.cfg.tenant,
                device_id: &registration.device_id,
                platform: self.store.cfg.platform,
                architecture: self.store.cfg.architecture,
                registration_id: registration.registration_id,
                generation: registration.generation,
                task_id: p.task_id(),
                attempt_id: p.attempt_id(),
                permit,
                now,
            },
        )
        .map_err(|_| Error::Untrusted)?;
        if let wire::TaskPayload::Collection(v) = p {
            if !v
                .execution
                .matches_context(&v.plan, &self.execution_context()?)
            {
                return Err(Error::Untrusted);
            }
            if !v.plan.supported_by(&self.environment.native_engines) {
                return Err(Error::Unsupported);
            }
        }
        Ok(())
    }
    /// Receive one offer and cancellation page. Transport failures retain the exact claim;
    /// a verified response durably completes it independently of execution settlement.
    pub async fn claim(&mut self) -> Result<Claim, Error> {
        self.task_trust_ready()?;
        if self.retiring()? {
            return Err(Error::CredentialRetired);
        }
        let now = self.now()?;
        let mut state = self.store.get::<ClaimState>("claim")?;
        if state.as_ref().is_some_and(|v| now >= v.recover_until) {
            self.store
                .conn
                .execute("DELETE FROM state WHERE key='claim'", [])?;
            state = None;
        }
        let state = match state {
            Some(state) => state,
            None => {
                let grace = i64::try_from(self.store.cfg.limits.request_timeout.as_secs())
                    .map_err(|_| Error::Configuration)?
                    .checked_add(60)
                    .ok_or(Error::Clock)?;
                let state = ClaimState {
                    request: wire::TaskClaimRequest::new(
                        Uuid::new_v4(),
                        self.profiles.clone(),
                        self.execution_context()?,
                    )?
                    .with_environment(self.environment.clone())?,
                    recover_until: now.checked_add(grace).ok_or(Error::Clock)?,
                };
                self.store.put("claim", &state)?;
                state
            }
        };
        let body = encode(&state.request)?;
        let result: wire::TaskClaimResponse =
            self.runtime_frozen_json::<h::Claim>(&[], &body).await?;
        let cancellations = result.cancellations().to_owned();
        let offer = if let Some(signed) = result.into_task() {
            self.verify(&signed, wire::TaskPermit::Offer)?;
            let count: u64 = self
                .store
                .conn
                .query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))?;
            let task = signed.payload.task_id().to_string();
            let old:Option<Vec<u8>>=self.store.conn.query_row("SELECT CASE WHEN typeof(body)='blob' AND length(body)<=16777216 THEN body END FROM tasks WHERE id=?1",[&task],|r|r.get(0)).optional()?;
            if old.is_none() && count >= self.store.cfg.limits.pending_tasks as u64 {
                // A new offer must not hide cancellation of existing work at capacity.
                // Keep the exact claim for replay after the host settles those tasks;
                // neither acknowledge nor persist the unadmitted offer.
                return Ok(Claim {
                    offer: None,
                    cancellations,
                });
            }
            let old = old.as_deref().map(decode::<wire::SignedTask>).transpose()?;
            let retirement = old
                .as_ref()
                .filter(|old| old.payload.attempt_id() != signed.payload.attempt_id())
                .map(|old| {
                    self.transfers
                        .retire_attempt(old.payload.task_id(), old.payload.attempt_id())
                })
                .transpose()?;
            let tx = self.store.conn.unchecked_transaction()?;
            if let Some(old) = old {
                if old.payload.attempt_id() != signed.payload.attempt_id() {
                    // A new signed Offer retires only expired Received requests. Any Start,
                    // source event or other settlement request may still need exact replay.
                    let unsettled: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM requests WHERE task=?1 AND (key NOT LIKE 'received/%' OR source IS NOT NULL))",
                        [&task], |r| r.get(0),
                    )?;
                    if unsettled || now < old.payload.expires_at() {
                        return Err(Error::Conflict);
                    }
                    tx.execute("DELETE FROM requests WHERE task=?1 AND key LIKE 'received/%' AND source IS NULL", [&task])?;
                    tx.execute("DELETE FROM cache_refs WHERE task=?1", [&task])?;
                    retirement.as_ref().ok_or(Error::Storage)?.persist(&tx)?;
                } else if old != signed {
                    return Err(Error::Conflict);
                }
            }
            tx.execute(
                "INSERT INTO tasks VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET body=excluded.body",
                params![task, encode(&signed)?],
            )?;
            tx.execute("DELETE FROM state WHERE key='claim'", [])?;
            tx.commit()?;
            if let Some(retirement) = retirement {
                retirement.committed();
            }
            Some(Offer { signed })
        } else {
            self.store
                .conn
                .execute("DELETE FROM state WHERE key='claim'", [])?;
            None
        };
        Ok(Claim {
            offer,
            cancellations,
        })
    }
    pub(crate) fn stored_offer(&self, task: Uuid) -> Result<Offer, Error> {
        let body:Vec<u8>=self.store.conn.query_row("SELECT CASE WHEN typeof(body)='blob' AND length(body)<=16777216 THEN body END FROM tasks WHERE id=?1",[task.to_string()],|r|r.get(0))?;
        let signed: wire::SignedTask = decode(&body)?;
        if signed.payload.task_id() != task {
            return Err(Error::Storage);
        }
        Ok(Offer { signed })
    }
    /// Recover a current offer; expiry is never ignored for a new Start or download.
    pub fn offer(&self, task: Uuid) -> Result<Offer, Error> {
        let offer = self.stored_offer(task)?;
        self.verify(&offer.signed, wire::TaskPermit::Offer)?;
        Ok(offer)
    }
    /// Recover a previously frozen Start after a real process restart. Missing materials
    /// fail closed; only the original operation is replayed and Start expiry is rechecked.
    pub async fn recover_start(&mut self, task: Uuid) -> Result<ResumedStart, Error> {
        self.active()?;
        self.now()?;
        let offer = self.stored_offer(task)?;
        let key = format!("start/{}/{}", task, offer.attempt_id());
        let pending: bool = self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM requests WHERE key=?1)",
            [key],
            |r| r.get(0),
        )?;
        if !pending {
            return Err(Error::Conflict);
        }
        let materials = self.recover_materials(&offer)?;
        let start = self.start_inner(&offer, &materials).await?;
        Ok(ResumedStart {
            offer,
            start,
            materials,
        })
    }
    pub(crate) fn event_request(
        &self,
        key: &str,
        task: Uuid,
        attempt: Uuid,
        event: wire::TaskEvent,
        source: Option<&str>,
    ) -> Result<wire::TaskEventRequest, Error> {
        let existing:Option<Vec<u8>>=self.store.conn.query_row("SELECT CASE WHEN typeof(body)='blob' AND length(body)<=1114112 THEN body END FROM requests WHERE key=?1",[key],|r|r.get(0)).optional()?;
        if let Some(bytes) = existing {
            let req: wire::TaskEventRequest = decode(&bytes)?;
            if req.attempt_id() != attempt || req.event() != &event {
                return Err(Error::Conflict);
            }
            return Ok(req);
        }
        let count: u64 = self
            .store
            .conn
            .query_row("SELECT count(*) FROM requests", [], |r| r.get(0))?;
        if count >= self.store.cfg.limits.pending_tasks as u64 * 70 {
            return Err(Error::Capacity);
        }
        let request =
            wire::TaskEventRequest::new(Uuid::new_v4(), attempt, event, self.execution_context()?)?;
        self.store.conn.execute(
            "INSERT INTO requests(key,task,source,body) VALUES(?1,?2,?3,?4)",
            params![key, task.to_string(), source, encode(&request)?],
        )?;
        Ok(request)
    }
    pub(crate) fn delivery_request(
        &self,
        key: &str,
        task: Uuid,
        attempt: Uuid,
        event: wire::TaskEvent,
        source: &str,
    ) -> Result<wire::TaskEventRequest, Error> {
        let event = match event {
            wire::TaskEvent::CollectionResult(result) => {
                use sha2::{Digest, Sha256};
                let bytes = serde_json::to_vec(&result).map_err(|_| Error::Protocol)?;
                let offer = self.stored_offer(task)?;
                let wire::TaskPayload::Collection(spec) = offer.payload() else {
                    return Err(Error::Protocol);
                };
                result.validate_for(&spec.plan, self.now()?)?;
                if bytes.len() > wire::OUTPUT_CHUNK_BYTES && !spec.plan.requires_inline() {
                    let manifest = wire::OutputManifest::new(
                        bytes.len().try_into().map_err(|_| Error::Capacity)?,
                        Sha256::digest(&bytes).into(),
                    )?;
                    for (index, bytes) in bytes.chunks(wire::OUTPUT_CHUNK_BYTES).enumerate() {
                        let chunk = wire::OutputChunk::new(manifest.clone(), index as u16, bytes)?;
                        let key = format!("chunk/{task}/{attempt}/{:02}", chunk.index());
                        self.event_request(
                            &key,
                            task,
                            attempt,
                            wire::TaskEvent::OutputChunk(chunk),
                            None,
                        )?;
                    }
                    wire::TaskEvent::ChunkedCollectionResult { manifest }
                } else {
                    wire::TaskEvent::CollectionResult(result)
                }
            }
            wire::TaskEvent::Result(result)
                if serde_json::to_vec(result.output())
                    .map_err(|_| Error::Protocol)?
                    .len()
                    > wire::OUTPUT_CHUNK_BYTES =>
            {
                let (reference, chunks) = wire::ChunkedTaskResult::split(result)?;
                for chunk in chunks {
                    let key = format!("chunk/{task}/{attempt}/{:02}", chunk.index());
                    self.event_request(
                        &key,
                        task,
                        attempt,
                        wire::TaskEvent::OutputChunk(chunk),
                        None,
                    )?;
                }
                wire::TaskEvent::ChunkedResult(reference)
            }
            event => event,
        };
        self.event_request(key, task, attempt, event, Some(source))
    }
    pub(crate) async fn send_output_chunks(
        &self,
        task: Uuid,
        request: &wire::TaskEventRequest,
    ) -> Result<(), Error> {
        let manifest = match request.event() {
            wire::TaskEvent::ChunkedResult(reference) => reference.manifest(),
            wire::TaskEvent::ChunkedCollectionResult { manifest } => manifest,
            _ => return Ok(()),
        };
        let prefix = format!("chunk/{task}/{}/", request.attempt_id());
        let rows = {
            let mut query=self.store.conn.prepare("SELECT key,body,accepted FROM requests WHERE task=?1 AND substr(key,1,length(?2))=?2 ORDER BY key LIMIT 65")?;
            let rows = query
                .query_map(params![task.to_string(), prefix], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, bool>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        if rows.len() != manifest.count() {
            return Err(Error::Storage);
        }
        for (index, (key, body, accepted)) in rows.into_iter().enumerate() {
            let chunk_request: wire::TaskEventRequest = decode(&body)?;
            let wire::TaskEvent::OutputChunk(chunk) = chunk_request.event() else {
                return Err(Error::Storage);
            };
            if usize::from(chunk.index()) != index
                || chunk.manifest() != manifest
                || chunk_request.attempt_id() != request.attempt_id()
            {
                return Err(Error::Storage);
            }
            if !accepted {
                let ack = self.send_event(task, &chunk_request).await?;
                if ack.permit().is_some() {
                    return Err(Error::Protocol);
                }
                self.store
                    .conn
                    .execute("UPDATE requests SET accepted=1 WHERE key=?1", [key])?;
            }
        }
        Ok(())
    }
    pub(crate) async fn send_event(
        &self,
        task: Uuid,
        request: &wire::TaskEventRequest,
    ) -> Result<wire::TaskEventAck, Error> {
        self.runtime_json::<h::Event>(&[&task.to_string()], Some(request))
            .await
    }
    /// Acknowledgement of an authenticated offer never grants execution.
    pub async fn received(&mut self, offer: &Offer) -> Result<(), Error> {
        let key = format!("received/{}/{}", offer.task_id(), offer.attempt_id());
        let accepted: Option<bool> = self
            .store
            .conn
            .query_row("SELECT accepted FROM requests WHERE key=?1", [&key], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(accepted) = accepted {
            self.active()?;
            if offer
                .preparation_deadline()
                .is_some_and(|until| self.now().is_ok_and(|now| now >= until))
                || self.stored_offer(offer.task_id())?.signed != offer.signed
            {
                return Err(Error::Expired);
            }
            if accepted {
                return Ok(());
            }
        } else {
            self.verify(&offer.signed, wire::TaskPermit::Offer)?;
        }
        let request = self.event_request(
            &key,
            offer.task_id(),
            offer.attempt_id(),
            wire::TaskEvent::Received,
            None,
        )?;
        let ack = self.send_event(offer.task_id(), &request).await?;
        if ack.permit().is_some() {
            return Err(Error::Protocol);
        }
        if ack.cancel_requested() {
            return Err(Error::Denied);
        }
        self.store
            .conn
            .execute("UPDATE requests SET accepted=1 WHERE key=?1", [key])?;
        Ok(())
    }
    /// Request Start only after all materials are verified. User-initiated software requires
    /// an explicit trusted-host call to start_user_initiated, never an automatic boolean.
    pub async fn request_start(
        &mut self,
        offer: &Offer,
        materials: &Materials,
    ) -> Result<Start, Error> {
        if matches!(offer.payload(),wire::TaskPayload::Software(v) if matches!(v.authorization, wire::TaskAuthorization::SelfService { .. }))
            || matches!(offer.payload(),wire::TaskPayload::Script(v) if matches!(v.authorization, wire::TaskAuthorization::SelfService { .. }))
            || matches!(offer.payload(),wire::TaskPayload::Enrollment(v) if matches!(v.authorization, wire::TaskAuthorization::SelfService { .. }))
        {
            return Err(Error::Denied);
        }
        self.start_inner(offer, materials).await
    }
    /// Trusted local interaction endpoint. Host must authenticate and bind the user action.
    pub async fn start_user_initiated(
        &mut self,
        offer: &Offer,
        materials: &Materials,
    ) -> Result<Start, Error> {
        self.start_inner(offer, materials).await
    }
    async fn start_inner(&mut self, offer: &Offer, materials: &Materials) -> Result<Start, Error> {
        self.active()?;
        self.now()?;
        let start_key = format!("start/{}/{}", offer.task_id(), offer.attempt_id());
        let replay: bool = self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM requests WHERE key=?1)",
            [&start_key],
            |r| r.get(0),
        )?;
        if replay {
            let stored:Vec<u8>=self.store.conn.query_row("SELECT CASE WHEN typeof(body)='blob' AND length(body)<=16777216 THEN body END FROM tasks WHERE id=?1",[offer.task_id().to_string()],|r|r.get(0))?;
            if decode::<wire::SignedTask>(&stored)? != offer.signed {
                return Err(Error::Untrusted);
            }
        } else {
            self.verify(&offer.signed, wire::TaskPermit::Offer)?;
        }
        materials.validate_offer(offer)?;
        let received = format!("received/{}/{}", offer.task_id(), offer.attempt_id());
        let ok: bool = self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM requests WHERE key=?1 AND accepted=1)",
            [received],
            |r| r.get(0),
        )?;
        if !ok {
            return Err(Error::Conflict);
        }
        let key = format!("start/{}/{}", offer.task_id(), offer.attempt_id());
        let request = self.event_request(
            &key,
            offer.task_id(),
            offer.attempt_id(),
            wire::TaskEvent::Start,
            None,
        )?;
        let ack = self.send_event(offer.task_id(), &request).await?;
        if ack.cancel_requested() {
            return Err(Error::Denied);
        }
        let signed = ack.into_permit().ok_or(Error::Protocol)?;
        self.verify(&signed, wire::TaskPermit::Start)?;
        if !same_input(offer.payload(), &signed.payload) {
            return Err(Error::Untrusted);
        }
        self.store
            .conn
            .execute("UPDATE requests SET accepted=1 WHERE key=?1", [key])?;
        Ok(Start { signed })
    }
    /// Revalidate the permit immediately before the bridge submits to local admission.
    pub fn validate_start(&self, start: &Start) -> Result<(), Error> {
        self.verify(&start.signed, wire::TaskPermit::Start)?;
        let key = format!(
            "start/{}/{}",
            start.payload().task_id(),
            start.payload().attempt_id()
        );
        let accepted: bool = self.store.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM requests WHERE key=?1 AND accepted=1)",
            [key],
            |r| r.get(0),
        )?;
        if !accepted {
            return Err(Error::Conflict);
        }
        let offer = self.stored_offer(start.payload().task_id())?;
        if !accepted || !same_input(offer.payload(), start.payload()) {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    /// Release completed transport/cache associations only after the host has settled delivery.
    pub(crate) fn release(&self, task: Uuid) -> Result<(), Error> {
        let tx = self.store.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM requests WHERE task=?1", [task.to_string()])?;
        tx.execute("DELETE FROM cache_refs WHERE task=?1", [task.to_string()])?;
        tx.execute("DELETE FROM tasks WHERE id=?1", [task.to_string()])?;
        tx.execute(
            "DELETE FROM state WHERE key IN(?1,?2,?3)",
            rusqlite::params![
                format!("binding/{task}"),
                format!("projection/{task}"),
                format!("access-observation-last/{task}")
            ],
        )?;
        tx.commit()?;
        self.transfers.release(task)?;
        Ok(())
    }
}
pub(crate) fn same_input(a: &wire::TaskPayload, b: &wire::TaskPayload) -> bool {
    match (a, b) {
        (wire::TaskPayload::DeviceAccess(a), wire::TaskPayload::DeviceAccess(b)) => {
            let mut b = b.clone();
            b.permit = a.permit;
            b.expires_at = a.expires_at;
            a == &b
        }
        (wire::TaskPayload::Unmanage(a), wire::TaskPayload::Unmanage(b)) => {
            let mut b = b.clone();
            b.permit = a.permit;
            b.expires_at = a.expires_at;
            a == &b
        }
        (wire::TaskPayload::Credential(a), wire::TaskPayload::Credential(b)) => {
            let mut b = b.clone();
            b.permit = a.permit;
            b.expires_at = a.expires_at;
            &b == a
        }
        (wire::TaskPayload::Enrollment(a), wire::TaskPayload::Enrollment(b)) => {
            let mut b = b.clone();
            b.permit = a.permit;
            b.expires_at = a.expires_at;
            &b == a
        }
        (wire::TaskPayload::Collection(a), wire::TaskPayload::Collection(b)) => {
            let mut b = b.clone();
            b.permit = a.permit;
            b.expires_at = a.expires_at;
            &b == a
        }
        (wire::TaskPayload::Script(a), wire::TaskPayload::Script(b)) => {
            let mut b = b.clone();
            b.permit = a.permit;
            b.expires_at = a.expires_at;
            &b == a
        }
        (wire::TaskPayload::Software(a), wire::TaskPayload::Software(b)) => {
            let mut b = b.clone();
            b.permit = a.permit;
            b.expires_at = a.expires_at;
            &b == a
        }
        _ => false,
    }
}

// reqwest preserves the underlying rustls error; TLS failures must never authorize failover.
// ref: hyper-util src/client/legacy/connect/dns.rs (GaiResolver returns io::Error);
// tokio-rustls src/common/mod.rs (TLS protocol errors remain in the error source chain).
// io::Error::source can skip a wrapped custom error; inspect get_ref as well.
// A bounded walk fails closed if a future transport adds an unexpectedly deep wrapper.
fn contains_tls_error(error: &(dyn std::error::Error + 'static), depth: u8) -> bool {
    if depth == 0 || error.downcast_ref::<rustls::Error>().is_some() {
        return true;
    }
    if let Some(io) = error.downcast_ref::<std::io::Error>() {
        if io
            .get_ref()
            .is_some_and(|inner| contains_tls_error(inner, depth - 1))
        {
            return true;
        }
    }
    error
        .source()
        .is_some_and(|source| contains_tls_error(source, depth - 1))
}
pub(crate) fn transport_error(error: reqwest::Error) -> Error {
    if contains_tls_error(&error, 16) {
        return Error::Unavailable;
    }
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&error);
    let mut network = error.is_timeout() || error.is_dns();
    while let Some(current) = source {
        if current.downcast_ref::<rustls::Error>().is_some() {
            return Error::Unavailable;
        }
        if let Some(http) = current.downcast_ref::<hyper::Error>() {
            network |= http.is_incomplete_message();
        }
        if let Some(io) = current.downcast_ref::<std::io::Error>() {
            use std::io::ErrorKind as K;
            network |= matches!(
                io.kind(),
                K::UnexpectedEof
                    | K::ConnectionRefused
                    | K::ConnectionReset
                    | K::ConnectionAborted
                    | K::NotConnected
                    | K::TimedOut
                    | K::NetworkUnreachable
                    | K::HostUnreachable
            ) || (error.is_connect() && io.kind() == K::Other);
        }
        source = current.source();
    }
    if network {
        Error::Network
    } else {
        Error::Unavailable
    }
}

// Pure bounded transport; the caller owns all identity, decoding and storage decisions.
pub(crate) async fn json_transport(
    request: reqwest::RequestBuilder,
    max: usize,
) -> Result<(StatusCode, Vec<u8>), Error> {
    let task = tokio::spawn(async move {
        let mut response = request.send().await.map_err(transport_error)?;
        let status = response.status();
        if status != StatusCode::NO_CONTENT
            && !response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| {
                    v.split(';')
                        .next()
                        .is_some_and(|v| v.trim() == "application/json")
                })
        {
            return Err(Error::Protocol);
        }
        if response.content_length().is_some_and(|n| n > max as u64) {
            return Err(Error::Capacity);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            if status.is_success() {
                transport_error(error)
            } else {
                Error::Unavailable
            }
        })? {
            if bytes.len().checked_add(chunk.len()).is_none_or(|n| n > max) {
                return Err(Error::Capacity);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((status, bytes))
    });
    // Dropping an owner operation cancels its transport instead of detaching it.
    let _cancel = CancelTransport(task.abort_handle());
    task.await.map_err(|_| Error::Unavailable)?
}
pub(crate) async fn bounded(mut response: Response, max: usize) -> Result<Vec<u8>, Error> {
    if response.content_length().is_some_and(|n| n > max as u64) {
        return Err(Error::Capacity);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
        if bytes.len().checked_add(chunk.len()).is_none_or(|n| n > max) {
            return Err(Error::Capacity);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
struct CancelTransport(tokio::task::AbortHandle);
impl Drop for CancelTransport {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn task_deadline(payload: &wire::TaskPayload) -> Option<i64> {
    match payload {
        wire::TaskPayload::Script(p) => Some(p.deadline),
        wire::TaskPayload::Software(_) | wire::TaskPayload::Enrollment(_) => None,
        wire::TaskPayload::Collection(p) => Some(p.deadline),
        wire::TaskPayload::Credential(p) => Some(p.deadline),
        wire::TaskPayload::DeviceAccess(p) => Some(p.deadline),
        wire::TaskPayload::Unmanage(p) => Some(p.deadline),
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[derive(Debug)]
    struct FailingDns {
        tls: bool,
    }

    impl reqwest::dns::Resolve for FailingDns {
        fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
            let tls = self.tls;
            Box::pin(async move {
                // InvalidData is deliberately outside the network IO allowlist.
                // The resolver stage, rather than the OS error kind/text, is authoritative.
                let error: Box<dyn std::error::Error + Send + Sync> = if tls {
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        rustls::Error::General("test TLS failure".into()),
                    ))
                } else {
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "test DNS failure",
                    ))
                };
                Err(error)
            })
        }
    }

    #[tokio::test]
    async fn dns_stage_is_network_independent_of_os_error_kind_but_tls_stays_closed() {
        for tls in [false, true] {
            let error = reqwest::Client::builder()
                .no_proxy()
                .dns_resolver(std::sync::Arc::new(FailingDns { tls }))
                .build()
                .unwrap()
                .get("https://rss-agent-unreachable.invalid")
                .send()
                .await
                .unwrap_err();
            assert!(error.is_connect());
            assert!(error.is_dns());
            assert_eq!(
                transport_error(error),
                if tls {
                    Error::Unavailable
                } else {
                    Error::Network
                }
            );
        }
    }

    #[test]
    fn bounded_http_body_finishes_while_owner_checks_os_facts() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (received, ready) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            received.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
        });
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let request = reqwest::Client::new()
                .get(format!("http://{address}"))
                .timeout(std::time::Duration::from_millis(150));
            let future = json_transport(request, 2);
            tokio::pin!(future);
            tokio::select! {
                result = &mut future => panic!("unexpected early response: {result:?}"),
                _ = async {
                    while ready.try_recv().is_err() {
                        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    }
                } => ()
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
            assert_eq!(future.await, Ok((StatusCode::OK, b"{}".to_vec())));
        });
        server.join().unwrap();
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dropping_owner_operation_closes_pending_http() {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (received, ready) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            received.send(()).unwrap();
            assert_eq!(stream.read(&mut request).unwrap(), 0);
        });
        {
            let future = json_transport(reqwest::Client::new().get(format!("http://{address}")), 2);
            tokio::pin!(future);
            tokio::select! {
                result = &mut future => panic!("unexpected response: {result:?}"),
                _ = async {
                    while ready.try_recv().is_err() {
                        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    }
                } => ()
            }
        }
        tokio::task::spawn_blocking(move || server.join().unwrap())
            .await
            .unwrap();
    }
}
