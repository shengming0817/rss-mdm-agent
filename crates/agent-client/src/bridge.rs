use crate::{
    client::same_input,
    store::{decode, encode, hash},
    wire, Client, Clock, Error, Materials, SecretProvider, Start,
};
use execution_app::{
    AppHost, ExecutionApp, ExecutionStatus, RequestContext, RunnerPort, TaskPhase,
};
use execution_contract::{EventId, FrozenExecution, Id, OutputQuality, ProcessEnd, RequestId};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
#[derive(Serialize, Deserialize)]
pub(crate) struct Binding {
    task: Uuid,
    attempt: Uuid,
    pub(crate) request: RequestId,
    digest: execution_contract::Digest,
    actor: execution_contract::ActorId,
    local_attempt: Option<execution_contract::AttemptId>,
}
/// Frozen result delivery prepared synchronously from the authoritative journal.
pub struct PendingDelivery {
    task: Uuid,
    key: String,
    request: wire::TaskEventRequest,
    events: Vec<EventId>,
    binding: Binding,
    accepted: bool,
}
/// An unsubmitted offer whose absence was checked against the authoritative journal.
pub struct PendingAbandonment {
    task: Uuid,
    key: String,
    event: wire::TaskEventRequest,
    request: RequestId,
    device: execution_contract::DeviceId,
    accepted: bool,
}
impl PendingAbandonment {
    /// Original request checked against the journal, including before an execution exists.
    pub fn request_id(&self) -> &RequestId {
        &self.request
    }
}
/// Host-adapted, preflighted input bound to an exact verified offer. Not dispatch authority.
pub struct PreparedExecution {
    plan: FrozenExecution,
    caller: RequestContext,
    input: wire::TaskPayload,
}
/// Trusted host output protection. This policy is not supplied by network/UI DTOs.
pub trait OutputPolicy {
    /// Remove secrets before structured output and diagnostic streams become wire values.
    fn redact(&self, text: &str) -> Result<String, Error>;
}
/// Redacts the communication owner's actual credentials while preserving task output.
/// The secret list is private and never serialized or printed.
pub struct CredentialRedactor(pub(crate) Vec<wire::Secret>);
impl OutputPolicy for CredentialRedactor {
    fn redact(&self, text: &str) -> Result<String, Error> {
        let mut value = text.to_owned();
        for secret in &self.0 {
            value = value.replace(secret.expose(), "[redacted]");
        }
        Ok(value)
    }
}
/// Concrete bridge into the existing execution service, never a second executor/journal.
pub struct ExecutionBridge<P> {
    consumer: Id,
    policy: P,
}
impl<P: OutputPolicy> ExecutionBridge<P> {
    /// Select one stable consumer and independently trusted output policy.
    pub fn new(consumer: Id, policy: P) -> Self {
        Self { consumer, policy }
    }
    /// Check materials and current host prerequisites BEFORE requesting a remote Start.
    /// Scripts and ordered software each bind to one immutable local intent.
    pub fn prepare<H: AppHost, R: RunnerPort>(
        &self,
        offer: &crate::Offer,
        materials: &Materials,
        app: &ExecutionApp<H, R>,
        caller: &RequestContext,
        plan: &FrozenExecution,
    ) -> Result<PreparedExecution, Error> {
        materials.validate(offer)?;
        let (device, platform, run_as, timeout, output) = match offer.payload() {
            wire::TaskPayload::Script(v)
                if matches!(
                    plan.spec().execution,
                    execution_contract::ExecutionSpec::Process {}
                ) =>
            {
                (
                    &v.device_id,
                    v.platform,
                    v.run_as,
                    u64::from(v.timeout_seconds) * 1000,
                    u64::from(v.output_bytes),
                )
            }
            wire::TaskPayload::Software(v) => {
                let program = plan
                    .spec()
                    .execution
                    .software_program()
                    .ok_or(Error::Untrusted)?;
                let expected = v
                    .definition_digest
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                if program.definition_digest.as_str() != expected
                    || program.steps.len() != v.steps.len()
                {
                    return Err(Error::Untrusted);
                }
                let mut timeout = 0u64;
                let mut output = 0u64;
                for step in &v.steps {
                    let command = match v.intent {
                        wire::SoftwareTaskIntent::Install => Some(&step.action.install),
                        wire::SoftwareTaskIntent::Uninstall => {
                            Some(step.action.uninstall.as_ref().ok_or(Error::Unsupported)?)
                        }
                        wire::SoftwareTaskIntent::Detect => None,
                    };
                    if let Some(c) = command {
                        timeout = timeout.saturating_add(u64::from(c.timeout_seconds) * 1000);
                        output = output.saturating_add(u64::from(c.output_bytes));
                    }
                    if let wire::SoftwareTaskDetection::Script { command } = &step.action.detect {
                        timeout = timeout.saturating_add(u64::from(command.timeout_seconds) * 1000);
                        output = output.saturating_add(u64::from(command.output_bytes));
                    }
                }
                (
                    &v.device_id,
                    v.platform,
                    wire::ExecutionIdentity::System,
                    timeout.clamp(1000, 86_400_000),
                    output.clamp(1024, 1_048_576),
                )
            }
            _ => return Err(Error::Unsupported),
        };
        let actual_platform = match platform {
            wire::TaskPlatform::Macos => execution_contract::Platform::Macos,
            wire::TaskPlatform::Windows => execution_contract::Platform::Windows,
        };
        let p = plan.spec();
        if p.request.request_id != offer.request_id()? {
            return Err(Error::Untrusted);
        }
        let matches_run_as = matches!(
            (&p.run_as, run_as),
            (
                execution_contract::RunAs::System { .. },
                wire::ExecutionIdentity::System
            ) | (
                execution_contract::RunAs::User { .. },
                wire::ExecutionIdentity::LoggedInUser
            )
        );
        if p.request.target.device.as_str() != device
            || p.request.target.platform != actual_platform
            || !matches_run_as
            || p.budget.total_timeout_ms > timeout
            || p.budget.total_output_bytes > output
        {
            return Err(Error::Untrusted);
        }
        app.prepare_execution(caller, plan)?;
        Ok(PreparedExecution {
            plan: plan.clone(),
            caller: caller.clone(),
            input: offer.payload().clone(),
        })
    }
    /// Submit a host-adapted immutable plan only with a currently valid Start.
    /// Preparation/adaptation must occur before requesting Start; this does no network I/O.
    pub fn dispatch<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        start: Start,
        materials: &Materials,
        app: &mut ExecutionApp<H, R>,
        prepared: PreparedExecution,
    ) -> Result<ExecutionStatus, Error> {
        let PreparedExecution {
            plan,
            caller,
            input,
        } = prepared;
        let plan = &plan;
        let caller = &caller;
        client.validate_start(&start)?;
        if !same_input(&input, start.payload()) {
            return Err(Error::Untrusted);
        }
        let expiry = u64::try_from(start.payload().expires_at())
            .map_err(|_| Error::Clock)?
            .checked_mul(1000)
            .ok_or(Error::Clock)?;
        // Start expiry bounds the first dispatch. The signed runtime budget remains available
        // after admission; treating the short Start window as that budget truncates valid work.
        if plan.spec().validity.expires_at_unix_ms
            > expiry
                .checked_add(plan.spec().budget.total_timeout_ms)
                .ok_or(Error::Clock)?
        {
            return Err(Error::Expired);
        }
        let original: wire::TaskPayload = decode(&materials.input)?;
        if !same_input(&original, start.payload())
            || materials.task != start.payload().task_id()
            || materials.attempt != start.payload().attempt_id()
        {
            return Err(Error::Untrusted);
        }
        for file in materials.files() {
            file.verify()?;
        }
        if caller.actor != plan.spec().request.actor {
            return Err(Error::Denied);
        }
        let mut binding = Binding {
            task: start.payload().task_id(),
            attempt: start.payload().attempt_id(),
            request: plan.spec().request.request_id.clone(),
            digest: plan.digest().clone(),
            actor: caller.actor.clone(),
            local_attempt: None,
        };
        let key = format!("binding/{}", binding.task);
        if let Some(old) = client.store.get::<Binding>(&key)? {
            if old.attempt != binding.attempt
                || old.request != binding.request
                || old.digest != binding.digest
                || old.actor != binding.actor
            {
                return Err(Error::Conflict);
            }
            binding.local_attempt = old.local_attempt;
        } else {
            client.store.put(&key, &binding)?;
        }
        // The only first dispatch is still created by local admission and the execution journal.
        let status = app.request_execution(caller, plan)?;
        if binding.local_attempt.is_some() && binding.local_attempt != status.attempt_id {
            return Err(Error::Conflict);
        }
        binding.local_attempt = status.attempt_id.clone();
        client.store.put(&key, &binding)?;
        Ok(status)
    }
    /// Repair only the transport association after an interrupted commit hand-off.
    /// The journal remains the authority for whether an attempt exists; this never dispatches.
    pub fn recover_binding<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &ExecutionApp<H, R>,
    ) -> Result<bool, Error> {
        let key = format!("binding/{task}");
        let mut binding: Binding = client.store.get(&key)?.ok_or(Error::Conflict)?;
        let device = execution_contract::DeviceId::new(client.registration()?.device_id)
            .map_err(|_| Error::Identity)?;
        if !app.has_service_execution(&binding.request, &device)? {
            if binding.local_attempt.is_some() {
                return Err(Error::Conflict);
            }
            return Ok(false);
        }
        if binding.local_attempt.is_some() {
            return Ok(true);
        }
        let caller = RequestContext {
            actor: binding.actor.clone(),
        };
        let plan = app.frozen_input(&caller, &binding.request)?;
        let offer = client.stored_offer(task)?;
        if plan.digest() != &binding.digest
            || binding.task != task
            || binding.attempt != offer.attempt_id()
            || binding.request != offer.request_id()?
        {
            return Err(Error::Conflict);
        }
        let status = app.status(&caller, &binding.request)?;
        if binding.local_attempt.is_some() && binding.local_attempt != status.attempt_id {
            return Err(Error::Conflict);
        }
        if binding.local_attempt != status.attempt_id {
            binding.local_attempt = status.attempt_id;
            client.store.put(&key, &binding)?;
        }
        Ok(true)
    }
    /// Settle an offer that never entered this authoritative journal. Cancellation ACK
    /// precedes releasing associations; any existing local execution remains journal-owned.
    pub fn prepare_abandonment<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &ExecutionApp<H, R>,
    ) -> Result<PendingAbandonment, Error> {
        let offer = client.stored_offer(task)?;
        let request = offer.request_id()?;
        let device = execution_contract::DeviceId::new(client.registration()?.device_id)
            .map_err(|_| Error::Unsupported)?;
        if let Some(binding) = client.store.get::<Binding>(&format!("binding/{task}"))? {
            if binding.task != task
                || binding.attempt != offer.attempt_id()
                || binding.request != request
            {
                return Err(Error::Conflict);
            }
        }
        if app.has_service_execution(&request, &device)? {
            return Err(Error::Conflict);
        }
        let key = format!("abandon/{task}/{}", offer.attempt_id());
        let event = client.event_request(
            &key,
            task,
            offer.attempt_id(),
            wire::TaskEvent::Cancelled,
            None,
        )?;
        let accepted: bool = client.store.conn.query_row(
            "SELECT accepted FROM requests WHERE key=?1",
            [&key],
            |r| r.get(0),
        )?;
        Ok(PendingAbandonment {
            task,
            key,
            event,
            request,
            device,
            accepted,
        })
    }
    /// Send only transport work, leaving the execution owner free to reconcile and cancel.
    pub async fn send_abandonment<S: SecretProvider, C: Clock>(
        &self,
        client: &mut Client<S, C>,
        pending: &mut PendingAbandonment,
    ) -> Result<(), Error> {
        if !pending.accepted {
            let ack = client.send_event(pending.task, &pending.event).await?;
            if ack.permit().is_some() {
                return Err(Error::Protocol);
            }
            client.store.conn.execute(
                "UPDATE requests SET accepted=1 WHERE key=?1",
                [&pending.key],
            )?;
            pending.accepted = true;
        }
        Ok(())
    }
    /// Release transport state only after the same journal still proves there is no execution.
    pub fn finish_abandonment<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        pending: PendingAbandonment,
        app: &ExecutionApp<H, R>,
    ) -> Result<(), Error> {
        if !pending.accepted || app.has_service_execution(&pending.request, &pending.device)? {
            return Err(Error::Conflict);
        }
        client.release(pending.task)
    }
    /// Convenience for callers that do not own a concurrent execution loop.
    pub async fn abandon<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &ExecutionApp<H, R>,
    ) -> Result<(), Error> {
        let mut pending = self.prepare_abandonment(client, task, app)?;
        self.send_abandonment(client, &mut pending).await?;
        self.finish_abandonment(client, pending, app)
    }
    /// Reconcile one exact cancellation against its existing journal; absence never proves stop.
    pub fn cancel<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &Client<S, C>,
        cancel: &wire::TaskCancellation,
        app: &mut ExecutionApp<H, R>,
        caller: &RequestContext,
    ) -> Result<ExecutionStatus, Error> {
        let binding: Binding = client
            .store
            .get(&format!("binding/{}", cancel.task_id()))?
            .ok_or(Error::Conflict)?;
        if binding.attempt != cancel.attempt_id() || binding.actor != caller.actor {
            return Err(Error::Denied);
        }
        app.cancel(caller, &binding.request)?;
        Ok(app.reconcile(&binding.request)?)
    }
    /// Send one frozen result and precisely confirm its source events after durable remote ACK.
    /// Lost confirmations never cause another HTTP result request or runner dispatch.
    pub fn prepare_delivery<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &mut ExecutionApp<H, R>,
        limit: usize,
    ) -> Result<Option<PendingDelivery>, Error> {
        client.active()?;
        client.now()?;
        if !self.recover_binding(client, task, app)? {
            return Err(Error::Conflict);
        }
        let binding: Binding = client
            .store
            .get(&format!("binding/{task}"))?
            .ok_or(Error::Conflict)?;
        let pending:Option<(String,Vec<u8>,String,bool)>=client.store.conn.query_row(
            "SELECT key,CASE WHEN typeof(body)='blob' AND length(body)<=1114112 THEN body END,source,accepted FROM requests WHERE task=?1 AND source IS NOT NULL ORDER BY rowid LIMIT 1",
            [task.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let (key, request, events, accepted) = if let Some((key, body, source, accepted)) = pending
        {
            (
                key,
                decode::<wire::TaskEventRequest>(&body)?,
                decode::<Vec<EventId>>(source.as_bytes())?,
                accepted,
            )
        } else {
            let items = app.service_delivery(&binding.request, &self.consumer, limit)?;
            if items.is_empty() {
                return Ok(None);
            }
            if items.iter().any(|v| v.input.digest() != &binding.digest) {
                return Err(Error::Conflict);
            }
            let offer_body:Vec<u8>=client.store.conn.query_row(
                "SELECT CASE WHEN typeof(body)='blob' AND length(body)<=16777216 THEN body END FROM tasks WHERE id=?1",[task.to_string()],|r|r.get(0))?;
            let offer: wire::SignedTask = decode(&offer_body)?;
            if offer.payload.attempt_id() != binding.attempt {
                return Err(Error::Conflict);
            }
            let candidate = items.iter().rev().find(|v| {
                v.current_attempt == binding.local_attempt
                    && (v.terminal.is_some() || v.process.as_ref().is_some_and(|p| p.finished))
            });
            let Some(candidate) = candidate else {
                return Ok(None);
            };
            let Some(event) = self.project(&offer.payload, candidate)? else {
                return Ok(None);
            };
            let events: Vec<_> = items.iter().map(|v| v.receipt.event_id.clone()).collect();
            // V4 accepts one terminal result per attempt. Later independent local facts
            // remain in the execution journal but cannot replace an acknowledged wire result.
            if client
                .store
                .get::<String>(&format!("projection/{task}"))?
                .is_some()
            {
                for id in events {
                    app.service_confirm(&binding.request, &self.consumer, &id)?;
                }
                return Ok(None);
            }
            let key = format!("result/{task}/{}", candidate.receipt.event_id.as_str());
            let source = String::from_utf8(encode(&events)?).map_err(|_| Error::Protocol)?;
            let request =
                client.event_request(&key, task, binding.attempt, event, Some(&source))?;
            (key, request, events, false)
        };
        if request.attempt_id() != binding.attempt {
            return Err(Error::Conflict);
        }
        // Authorization is checked immediately before handing the frozen request to transport.
        app.service_delivery(&binding.request, &self.consumer, 1)?;
        Ok(Some(PendingDelivery {
            task,
            key,
            request,
            events,
            accepted,
            binding,
        }))
    }
    /// Perform bounded HTTP delivery without borrowing the execution application.
    pub async fn send_delivery<S: SecretProvider, C: Clock>(
        &self,
        client: &mut Client<S, C>,
        pending: &mut PendingDelivery,
    ) -> Result<(), Error> {
        if !pending.accepted {
            let ack = client.send_event(pending.task, &pending.request).await?;
            if ack.permit().is_some() {
                return Err(Error::Protocol);
            }
            let tx = client.store.conn.unchecked_transaction()?;
            tx.execute(
                "UPDATE requests SET accepted=1 WHERE key=?1",
                [&pending.key],
            )?;
            tx.execute(
                "INSERT INTO state VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET body=excluded.body",
                params![
                    format!("projection/{}", pending.task),
                    encode(&hash(&encode(pending.request.event())?))?
                ],
            )?;
            tx.commit()?;
            pending.accepted = true;
        }
        Ok(())
    }
    /// Confirm original journal receipts after the durable HTTP acknowledgement.
    pub fn finish_delivery<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        pending: PendingDelivery,
        app: &mut ExecutionApp<H, R>,
    ) -> Result<usize, Error> {
        if !pending.accepted {
            return Err(Error::Conflict);
        }
        for id in &pending.events {
            app.service_confirm(&pending.binding.request, &self.consumer, id)?;
        }
        client
            .store
            .conn
            .execute("DELETE FROM requests WHERE key=?1", [pending.key])?;
        Ok(1)
    }
    /// Deliver synchronously for consumers without a concurrent execution-owner loop.
    pub async fn flush<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &mut ExecutionApp<H, R>,
        limit: usize,
    ) -> Result<usize, Error> {
        let Some(mut pending) = self.prepare_delivery(client, task, app, limit)? else {
            return Ok(0);
        };
        self.send_delivery(client, &mut pending).await?;
        self.finish_delivery(client, pending, app)
    }
    fn project(
        &self,
        payload: &wire::TaskPayload,
        evidence: &execution_sqlite::DeliveryEvidence,
    ) -> Result<Option<wire::TaskEvent>, Error> {
        if let Some(terminal) = evidence.terminal {
            if terminal == execution_sqlite::DeliveryTerminal::Cancelled {
                return Ok(Some(wire::TaskEvent::Cancelled));
            }
            let diagnostics = wire::TaskDiagnostics::new(
                String::new(),
                String::new(),
                0,
                i64::try_from(evidence.observed_at_unix_ms / 1000)
                    .map_err(|_| Error::Clock)?
                    .max(1),
                Some(wire::TaskFailure::LaunchFailed),
            )?;
            return Ok(Some(match payload {
                wire::TaskPayload::Script(_) => wire::TaskEvent::Result(wire::TaskResult::new(
                    None,
                    wire::OutputQuality::Failed,
                    serde_json::Value::Null,
                    diagnostics,
                )?),
                wire::TaskPayload::Software(spec) => {
                    wire::TaskEvent::SoftwareResult(wire::SoftwareTaskResult {
                        intent: spec.intent,
                        installer_exit_code: None,
                        detection: wire::SoftwareDetectionState::Unknown,
                        definition_digest: spec.definition_digest,
                        observed_version: None,
                        evidence_digest: [0; 32],
                        reboot_required: false,
                        diagnostics,
                    })
                }
                _ => return Err(Error::Unsupported),
            }));
        }
        let Some(process) = evidence.process.as_ref() else {
            return Ok(None);
        };
        if !process.finished {
            return Ok(None);
        }
        if process.end == ProcessEnd::Cancelled && process.quiescent {
            return Ok(Some(wire::TaskEvent::Cancelled));
        }
        let output_spec = evidence.input.spec().launch.output;
        let stdout = decode_stream(&process.stdout, output_spec.stdout);
        let stderr = decode_stream(&process.stderr, output_spec.stderr);
        let malformed = stdout.is_none() || stderr.is_none();
        let stdout = self
            .policy
            .redact(stdout.as_deref().unwrap_or("stdout decoding failed"))?;
        let stderr = self
            .policy
            .redact(stderr.as_deref().unwrap_or("stderr decoding failed"))?;
        let failure = match process.end {
            ProcessEnd::Rejected => Some(wire::TaskFailure::LaunchFailed),
            ProcessEnd::TimedOut => Some(wire::TaskFailure::TimedOut),
            ProcessEnd::OutputLimit => Some(wire::TaskFailure::OutputLimit),
            _ if process.exit_code.is_some_and(|v| v != 0) => Some(wire::TaskFailure::NonZeroExit),
            _ if malformed || process.quality == OutputQuality::Failed => {
                Some(wire::TaskFailure::CaptureFailed)
            }
            _ => None,
        };
        let diagnostics = wire::TaskDiagnostics::new(
            bound(stdout.clone()),
            bound(stderr),
            0,
            i64::try_from(evidence.observed_at_unix_ms / 1000)
                .map_err(|_| Error::Clock)?
                .max(1),
            failure,
        )?;
        Ok(Some(match payload {
            wire::TaskPayload::Script(_) => {
                let output = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
                let quality = match process.quality {
                    OutputQuality::Complete
                        if process.exit_code == Some(0)
                            && failure.is_none()
                            && process.quiescent =>
                    {
                        wire::OutputQuality::Complete
                    }
                    OutputQuality::Truncated if failure == Some(wire::TaskFailure::OutputLimit) => {
                        wire::OutputQuality::Truncated
                    }
                    _ if malformed || process.quality == OutputQuality::Failed => {
                        wire::OutputQuality::Failed
                    }
                    _ => wire::OutputQuality::Partial,
                };
                wire::TaskEvent::Result(wire::TaskResult::new(
                    process.exit_code,
                    quality,
                    output,
                    diagnostics,
                )?)
            }
            wire::TaskPayload::Software(spec) => {
                let Some(progress) = evidence.software_progress.as_ref() else {
                    return Ok(None);
                };
                let observed = progress.checkpoints.iter().rev().find_map(|c| match c {
                    execution_contract::SoftwareCheckpoint::End {
                        detected: Some(state),
                        ..
                    } => Some(state),
                    _ => None,
                });
                let (detection, version) = if progress.complete(&evidence.input) {
                    match observed {
                        Some(execution_contract::SoftwareState::Present { version }) => (
                            wire::SoftwareDetectionState::Present,
                            Some(version.as_str().to_owned()),
                        ),
                        Some(execution_contract::SoftwareState::Absent {}) => {
                            (wire::SoftwareDetectionState::Absent, None)
                        }
                        _ => (wire::SoftwareDetectionState::Unknown, None),
                    }
                } else {
                    (wire::SoftwareDetectionState::Unknown, None)
                };
                let mutation =
                    progress
                        .checkpoints
                        .iter()
                        .rev()
                        .find_map(|checkpoint| match checkpoint {
                            execution_contract::SoftwareCheckpoint::End {
                                phase: execution_contract::SoftwarePhase::Mutation,
                                process: Some(facts),
                                ..
                            } => Some(facts),
                            _ => None,
                        });
                let result = wire::SoftwareTaskResult {
                    intent: spec.intent,
                    installer_exit_code: mutation.and_then(|f| f.exit_code),
                    detection,
                    definition_digest: spec.definition_digest,
                    observed_version: version,
                    evidence_digest: Sha256::digest(encode(progress.as_ref())?).into(),
                    reboot_required: mutation
                        .is_some_and(|f| matches!(f.exit_code, Some(3010 | 1641))),
                    diagnostics,
                };
                result.validate()?;
                wire::TaskEvent::SoftwareResult(result)
            }
            _ => return Err(Error::Unsupported),
        }))
    }
    /// Explicit terminal cleanup after every journal event has been confirmed.
    pub fn finish<S: SecretProvider, C: Clock, H: AppHost, R: RunnerPort>(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &ExecutionApp<H, R>,
        caller: &RequestContext,
    ) -> Result<(), Error> {
        let binding: Binding = client
            .store
            .get(&format!("binding/{task}"))?
            .ok_or(Error::Conflict)?;
        if binding.actor != caller.actor {
            return Err(Error::Denied);
        }
        let status = app.status(caller, &binding.request)?;
        if status.phase == TaskPhase::Verified
            && !matches!(
                status.assessment,
                Some(
                    execution_lifecycle::EffectAssessment::Satisfied
                        | execution_lifecycle::EffectAssessment::NoEffect
                )
            )
        {
            return Err(Error::Conflict);
        }
        let terminal = matches!(
            status.phase,
            TaskPhase::Verified | TaskPhase::Cancelled | TaskPhase::FailedBeforeDispatch
        ) || (status.phase == TaskPhase::AdmissionDenied
            && status.attempt_id.is_none());
        if !terminal
            || !app
                .service_delivery(&binding.request, &self.consumer, 1)?
                .is_empty()
        {
            return Err(Error::Conflict);
        }
        client.release(task)
    }
}
fn bound(mut value: String) -> String {
    value.retain(|v| v != '\0');
    let mut length = value.len().min(wire::MAX_TASK_DIAGNOSTIC_BYTES);
    while !value.is_char_boundary(length) {
        length -= 1;
    }
    value.truncate(length);
    value
}

// Strict decoding follows the frozen stream encoding; original evidence is never rewritten.
// A diagnostic marker permits failure delivery without inventing replacement text/results.
fn decode_stream(bytes: &[u8], encoding: execution_contract::TextEncoding) -> Option<String> {
    match encoding {
        execution_contract::TextEncoding::Utf8 => {
            std::str::from_utf8(bytes).ok().map(str::to_owned)
        }
        execution_contract::TextEncoding::Utf16Le if bytes.len().is_multiple_of(2) => {
            String::from_utf16(
                &bytes
                    .chunks_exact(2)
                    .map(|v| u16::from_le_bytes([v[0], v[1]]))
                    .collect::<Vec<_>>(),
            )
            .ok()
        }
        _ => None,
    }
}
