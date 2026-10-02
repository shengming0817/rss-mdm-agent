use crate::{
    store::{decode, encode, hash, Store},
    wire, *,
};
use reqwest::{Method, Response, StatusCode};
use rusqlite::{params, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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
    /// Stable local request identity across offers; remote attempts do not create new executions.
    pub fn request_id(&self) -> Result<execution_contract::RequestId, Error> {
        let bytes = match self.payload() {
            wire::TaskPayload::Script(v) => encode(&(
                v.tenant_id,
                &v.device_id,
                v.registration_id,
                v.generation,
                v.task_id,
            ))?,
            wire::TaskPayload::Software(v) => encode(&(
                v.tenant_id,
                &v.device_id,
                v.registration_id,
                v.generation,
                v.task_id,
            ))?,
            _ => return Err(Error::Unsupported),
        };
        execution_contract::RequestId::new(format!("agent-v5-{}", hash(&bytes)))
            .map_err(|_| Error::Protocol)
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
/// A verified short-lived Start token, not local execution authority.
#[derive(Clone)]
pub struct Start {
    pub(crate) signed: wire::SignedTask,
}
impl Start {
    /// Exact verified signed input.
    pub fn payload(&self) -> &wire::TaskPayload {
        &self.signed.payload
    }
}
/// One bounded claim and independent cancellation page.
pub struct Claim {
    /// At most one accepted offer.
    pub offer: Option<Offer>,
    /// Exact remote attempts that must be reconciled/stopped by the host.
    pub cancellations: Vec<wire::TaskCancellation>,
}
#[derive(Serialize, Deserialize, PartialEq)]
struct Enrollment {
    operation: Uuid,
    enrollment: Uuid,
    password: String,
    credential: String,
    capabilities: Vec<wire::Capability>,
    fingerprint: String,
    execution_context: wire::SoftwareExecutionContext,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimState {
    operation: Uuid,
    profiles: Vec<wire::ExecutorProfile>,
    recover_until: i64,
    execution_context: wire::SoftwareExecutionContext,
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
/// Explicitly driven V5 client; SQLite transactions never cross HTTP awaits.
/// A private-root lease permits one driving owner; bounded HTTP tasks never access its state.
pub struct Client<S, C> {
    pub(crate) store: Store,
    pub(crate) http: reqwest::Client,
    pub(crate) secrets: S,
    pub(crate) clock: C,
    profiles: Vec<wire::ExecutorProfile>,
}
impl<S: SecretProvider, C: Clock> Client<S, C> {
    /// Set current configured executors for future polls; a pending retry retains its exact input.
    pub fn set_profiles(&mut self, profiles: Vec<wire::ExecutorProfile>) -> Result<(), Error> {
        wire::TaskClaimRequest::new(Uuid::new_v4(), profiles.clone(), self.execution_context()?)?;
        self.profiles = profiles;
        Ok(())
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
        let enrollment = self
            .store
            .get::<Enrollment>("enrollment")?
            .ok_or(Error::Identity)?;
        Ok(crate::CredentialRedactor(vec![
            self.credential()?,
            self.secrets.resolve(&enrollment.password)?,
        ]))
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
            .query_map([limit], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
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
            .get::<crate::bridge::Binding>(&format!("binding/{task}"))
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
        // ref: reqwest 0.13.5 src/async_impl/client.rs (explicit redirect/TLS/timeouts).
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(config.limits.connect_timeout)
            .read_timeout(config.limits.read_timeout)
            .https_only(config.transport == Transport::Https);
        if let Some(pem) = &config.ca_pem {
            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(pem).map_err(|_| Error::Configuration)?,
            );
        }
        let http = builder.build().map_err(|_| Error::Configuration)?;
        Ok(Self {
            store: Store::open(root, config, mode)?,
            http,
            secrets,
            clock,
            profiles: Vec::new(),
        })
    }
    pub(crate) fn now(&self) -> Result<i64, Error> {
        self.store.time(self.clock.now()?)
    }
    pub(crate) fn active(&self) -> Result<(), Error> {
        if self.store.get::<bool>("blocked")?.unwrap_or(false) {
            return Err(Error::Identity);
        }
        Ok(())
    }
    pub(crate) fn credential(&self) -> Result<wire::Secret, Error> {
        self.active()?;
        self.secrets.resolve(&self.store.secret_reference()?)
    }
    pub(crate) fn url(&self, path: &str) -> Result<url::Url, Error> {
        self.store
            .cfg
            .origin
            .join(&format!("api/agent/v5/{path}"))
            .map_err(|_| Error::Configuration)
    }
    pub(crate) async fn checked_response(&self, response: Response) -> Result<Response, Error> {
        self.check_status(response.status())?;
        Ok(response)
    }
    fn check_status(&self, status: StatusCode) -> Result<(), Error> {
        match status {
            s if s.is_success() => Ok(()),
            StatusCode::UNAUTHORIZED => {
                self.store.put("blocked", &true)?;
                Err(Error::Identity)
            }
            StatusCode::FORBIDDEN => Err(Error::Denied),
            StatusCode::CONFLICT => Err(Error::Conflict),
            StatusCode::TOO_MANY_REQUESTS => Err(Error::Unavailable),
            s if s.is_server_error() => Err(Error::Unavailable),
            _ => Err(Error::Protocol),
        }
    }
    async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&[u8]>,
        authenticated: bool,
    ) -> Result<T, Error> {
        self.active()?;
        self.now()?;
        let mut request = self
            .http
            .request(method, self.url(path)?)
            .timeout(self.store.cfg.limits.request_timeout);
        if authenticated {
            request = request.bearer_auth(self.credential()?.expose());
        }
        if let Some(body) = body {
            request = request
                .header("content-type", "application/json")
                .body(body.to_owned());
        }
        let max = self.store.cfg.limits.response_bytes;
        let (status, bytes) = json_transport(request, max).await?;
        self.check_status(status)?;
        decode(&bytes)
    }
    /// Registration replay preserves the operation and protected secret references.
    /// Use a new private namespace for a new registration; old pending data is preserved.
    pub async fn register(
        &mut self,
        operation: Uuid,
        enrollment: Uuid,
        password_reference: &str,
        credential_reference: &str,
        capabilities: Vec<wire::Capability>,
    ) -> Result<wire::RegistrationReceipt, Error> {
        // Producer-owned MdmEnrollmentV5 opens the standard OS MDM enrollment UI;
        // it is distinct from this Agent registration and unsupported by this client.
        if password_reference.is_empty()
            || credential_reference.is_empty()
            || password_reference.len() > 256
            || credential_reference.len() > 256
            || capabilities.contains(&wire::Capability::MdmEnrollmentV5)
        {
            return Err(Error::Configuration);
        }
        let context = self
            .store
            .get::<Enrollment>("enrollment")?
            .map(|v| v.execution_context)
            .unwrap_or(self.execution_context()?);
        let request = wire::RegistrationRequest::new(
            operation,
            enrollment,
            self.secrets.resolve(password_reference)?,
            self.secrets.credential(credential_reference)?,
            capabilities.clone(),
            self.store.cfg.platform,
            self.store.cfg.architecture,
            context.clone(),
        )?;
        let body = encode(&request)?;
        let record = Enrollment {
            operation,
            enrollment,
            password: password_reference.into(),
            credential: credential_reference.into(),
            capabilities: capabilities.clone(),
            fingerprint: hash(&body),
            execution_context: context,
        };
        if let Some(old) = self.store.get::<Enrollment>("enrollment")? {
            if old != record {
                return Err(Error::Conflict);
            }
        } else {
            self.store.put("enrollment", &record)?;
        }
        let receipt: wire::RegistrationReceipt = self
            .json(Method::POST, "registrations", Some(&body), false)
            .await?;
        if receipt.operation_id != operation
            || receipt.source != wire::ReportSource::AgentBuiltin
            || receipt.capabilities != capabilities
        {
            return Err(Error::Protocol);
        }
        if self
            .store
            .get::<wire::RegistrationReceipt>("registration")?
            .is_some_and(|old| old != receipt)
        {
            self.store.put("blocked", &true)?;
            return Err(Error::Identity);
        }
        let tx = self.store.conn.unchecked_transaction()?;
        tx.execute("INSERT INTO state VALUES('registration',?1) ON CONFLICT(key) DO UPDATE SET body=excluded.body",[encode(&receipt)?])?;
        tx.execute("INSERT INTO state VALUES('credential',?1) ON CONFLICT(key) DO UPDATE SET body=excluded.body",[encode(&credential_reference)?])?;
        tx.commit()?;
        Ok(receipt)
    }
    /// Current durable registration; no credential is returned.
    pub fn registration(&self) -> Result<wire::RegistrationReceipt, Error> {
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
            let ack: wire::ReportAck = self
                .json(Method::POST, "reports", Some(&body), true)
                .await?;
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
            .json(Method::GET, &format!("reports/{id}"), None, true)
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
        if now >= p.expires_at() {
            return Err(Error::Expired);
        }
        let registration = self.store.registration()?;
        let key = self
            .store
            .cfg
            .keys
            .get(&signed.key_id)
            .ok_or(Error::Untrusted)?;
        signed
            .verify(&wire::TaskVerification {
                key_id: &signed.key_id,
                public_key: key,
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
            })
            .map_err(|_| Error::Untrusted)?;
        if matches!(p, wire::TaskPayload::Enrollment(_)) {
            return Err(Error::Unsupported);
        }
        Ok(())
    }
    /// Receive one offer and cancellation page. Transport failures retain the exact claim;
    /// a verified response durably completes it independently of execution settlement.
    pub async fn claim(&mut self) -> Result<Claim, Error> {
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
                    operation: Uuid::new_v4(),
                    profiles: self.profiles.clone(),
                    execution_context: self.execution_context()?,
                    recover_until: now.checked_add(grace).ok_or(Error::Clock)?,
                };
                self.store.put("claim", &state)?;
                state
            }
        };
        let operation = state.operation;
        let body = encode(&wire::TaskClaimRequest::new(
            operation,
            state.profiles.clone(),
            state.execution_context.clone(),
        )?)?;
        let result: wire::TaskClaimResponse = self
            .json(Method::POST, "tasks/claim", Some(&body), true)
            .await?;
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
                return Err(Error::Capacity);
            }
            let tx = self.store.conn.unchecked_transaction()?;
            if let Some(old) = old {
                let old: wire::SignedTask = decode(&old)?;
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
        let wire::TaskEvent::ChunkedResult(reference) = request.event() else {
            return Ok(());
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
        if rows.len() != reference.manifest().count() {
            return Err(Error::Storage);
        }
        for (index, (key, body, accepted)) in rows.into_iter().enumerate() {
            let chunk_request: wire::TaskEventRequest = decode(&body)?;
            let wire::TaskEvent::OutputChunk(chunk) = chunk_request.event() else {
                return Err(Error::Storage);
            };
            if usize::from(chunk.index()) != index
                || chunk.manifest() != reference.manifest()
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
        self.json(
            Method::POST,
            &format!("tasks/{task}/events"),
            Some(&encode(request)?),
            true,
        )
        .await
    }
    /// Acknowledgement of an authenticated offer never grants execution.
    pub async fn received(&mut self, offer: &Offer) -> Result<(), Error> {
        self.verify(&offer.signed, wire::TaskPermit::Offer)?;
        let key = format!("received/{}/{}", offer.task_id(), offer.attempt_id());
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
        if matches!(offer.payload(),wire::TaskPayload::Software(v) if v.start_mode==wire::SoftwareStartMode::UserInitiated)
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
        materials.validate(offer)?;
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
    /// Revalidate a short-lived permit immediately before the bridge submits to local admission.
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
            "DELETE FROM state WHERE key IN(?1,?2)",
            rusqlite::params![format!("binding/{task}"), format!("projection/{task}")],
        )?;
        tx.commit()?;
        Ok(())
    }
}
pub(crate) fn same_input(a: &wire::TaskPayload, b: &wire::TaskPayload) -> bool {
    match (a, b) {
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

// Pure bounded transport; the caller owns all identity, decoding and storage decisions.
async fn json_transport(
    request: reqwest::RequestBuilder,
    max: usize,
) -> Result<(StatusCode, Vec<u8>), Error> {
    let task = tokio::spawn(async move {
        let mut response = request.send().await.map_err(|_| Error::Unavailable)?;
        let status = response.status();
        if !status.is_success() {
            return Ok((status, Vec::new()));
        }
        if !response
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
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unavailable)? {
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
struct CancelTransport(tokio::task::AbortHandle);
impl Drop for CancelTransport {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
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
