use super::request_id;
use crate::error::app_error as map_app_error;
use agent_client::{wire, Client, Clock, Error, Materials, Offer, SecretProvider, Start};
fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>, Error> {
    serde_json_canonicalizer::to_vec(value).map_err(|_| Error::Protocol)
}
use execution_app::{
    AppHost, ExecutionApp, ExecutionStatus, RequestContext, RunnerPort, TaskPhase,
};
use execution_contract::{FrozenExecution, Id, OutputQuality, ProcessEnd, RequestId};
use sha2::{Digest, Sha256};
use uuid::Uuid;
/// Local preparation plus an opaque frozen transport or durable acknowledgement.
pub(crate) struct PendingDelivery {
    result: Option<agent_client::FrozenResult>,
    ack: Option<agent_client::ResultAck>,
}
pub(crate) struct PendingAbandonment {
    transport: Option<agent_client::FrozenAbandonment>,
    ack: Option<agent_client::AbandonmentAck>,
    request: RequestId,
    device: execution_contract::DeviceId,
}
impl PendingAbandonment {
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
impl OutputPolicy for agent_client::CredentialRedactor {
    fn redact(&self, text: &str) -> Result<String, Error> {
        self.redact(text)
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
    pub fn prepare<H: AppHost, R: RunnerPort, J: execution_app::JournalPort>(
        &self,
        offer: &Offer,
        materials: &Materials,
        app: &ExecutionApp<H, R, J>,
        caller: &RequestContext,
        plan: &FrozenExecution,
    ) -> Result<PreparedExecution, Error> {
        materials.validate_offer(offer)?;
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
                let (timeout, output) = super::commands::software_budget(v)?;
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
        if p.request.request_id != request_id(&offer)? {
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
        app.prepare_execution(caller, plan).map_err(map_app_error)?;
        Ok(PreparedExecution {
            plan: plan.clone(),
            caller: caller.clone(),
            input: offer.payload().clone(),
        })
    }
    /// Submit a host-adapted immutable plan only with a currently valid Start.
    /// Preparation/adaptation must occur before requesting Start; this does no network I/O.
    pub fn dispatch<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        start: Start,
        materials: &Materials,
        app: &mut ExecutionApp<H, R, J>,
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
        if !start.matches_payload(&input) {
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
        materials.validate_start(&start)?;
        if caller.actor != plan.spec().request.actor {
            return Err(Error::Denied);
        }
        let binding = client.reserve_association(
            &start,
            &plan.spec().request.request_id,
            plan.digest(),
            &caller.actor,
        )?;
        // The only first dispatch is still created by local admission and the execution journal.
        let status = app.request_execution(caller, plan).map_err(|error| {
            // Closed error variants identify the journal boundary without exposing input.
            eprintln!("agent_dispatch: {error:?}");
            map_app_error(error)
        })?;
        client.bind_local_attempt(&binding, status.attempt_id.as_ref())?;
        Ok(status)
    }
    /// Repair only the transport association after an interrupted commit hand-off.
    /// The journal remains the authority for whether an attempt exists; this never dispatches.
    pub fn recover_binding<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &ExecutionApp<H, R, J>,
    ) -> Result<bool, Error> {
        let binding = client.association(task)?.ok_or(Error::Conflict)?;
        let device = execution_contract::DeviceId::new(client.registration()?.device_id)
            .map_err(|_| Error::Identity)?;
        if !app
            .has_service_execution(binding.request(), &device)
            .map_err(map_app_error)?
        {
            if binding.local_attempt().is_some() {
                return Err(Error::Conflict);
            }
            return Ok(false);
        }
        let caller = RequestContext {
            actor: binding.actor().clone(),
        };
        let plan = app
            .frozen_input(&caller, binding.request())
            .map_err(map_app_error)?;
        let offer = client.retained_offer(task)?;
        if plan.digest() != binding.digest()
            || binding.task() != task
            || binding.attempt() != offer.attempt_id()
            || binding.request() != &request_id(&offer)?
        {
            return Err(Error::Conflict);
        }
        let status = app
            .status(&caller, binding.request())
            .map_err(map_app_error)?;
        client.bind_local_attempt(&binding, status.attempt_id.as_ref())?;
        Ok(true)
    }
    /// Settle an offer that never entered this authoritative journal. Cancellation ACK
    /// precedes releasing associations; any existing local execution remains journal-owned.
    pub fn prepare_abandonment<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &ExecutionApp<H, R, J>,
    ) -> Result<PendingAbandonment, Error> {
        let offer = client.retained_offer(task)?;
        let request = request_id(&offer)?;
        let device = execution_contract::DeviceId::new(client.registration()?.device_id)
            .map_err(|_| Error::Unsupported)?;
        if let Some(binding) = client.association(task)? {
            if binding.task() != task
                || binding.attempt() != offer.attempt_id()
                || binding.request() != &request
            {
                return Err(Error::Conflict);
            }
        }
        if app
            .has_service_execution(&request, &device)
            .map_err(map_app_error)?
        {
            return Err(Error::Conflict);
        }
        Ok(PendingAbandonment {
            transport: Some(client.freeze_abandonment(task)?),
            ack: None,
            request,
            device,
        })
    }
    /// Send only transport work, leaving the execution owner free to reconcile and cancel.
    pub async fn send_abandonment<S: SecretProvider, C: Clock>(
        &self,
        client: &mut Client<S, C>,
        pending: &mut PendingAbandonment,
    ) -> Result<(), Error> {
        if let Some(transport) = pending.transport.take() {
            pending.ack = Some(client.send_abandonment(transport).await?);
        }
        Ok(())
    }
    /// Release transport state only after the same journal still proves there is no execution.
    pub fn finish_abandonment<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        pending: PendingAbandonment,
        app: &ExecutionApp<H, R, J>,
    ) -> Result<(), Error> {
        if app
            .has_service_execution(&pending.request, &pending.device)
            .map_err(map_app_error)?
        {
            return Err(Error::Conflict);
        }
        client.release_abandonment(pending.ack.ok_or(Error::Conflict)?)
    }
    /// Convenience for callers that do not own a concurrent execution loop.
    #[cfg(test)]
    pub async fn abandon<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &ExecutionApp<H, R, J>,
    ) -> Result<(), Error> {
        let mut pending = self.prepare_abandonment(client, task, app)?;
        self.send_abandonment(client, &mut pending).await?;
        self.finish_abandonment(client, pending, app)
    }
    /// Reconcile one exact cancellation against its existing journal; absence never proves stop.
    pub fn cancel<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &Client<S, C>,
        cancel: &wire::TaskCancellation,
        app: &mut ExecutionApp<H, R, J>,
        caller: &RequestContext,
    ) -> Result<ExecutionStatus, Error> {
        let binding = client
            .association(cancel.task_id())?
            .ok_or(Error::Conflict)?;
        if binding.attempt() != cancel.attempt_id() || binding.actor() != &caller.actor {
            return Err(Error::Denied);
        }
        app.cancel(caller, binding.request())
            .map_err(map_app_error)?;
        app.reconcile(binding.request()).map_err(map_app_error)
    }
    /// Send one frozen result and precisely confirm its source events after durable remote ACK.
    /// Lost confirmations never cause another HTTP result request or runner dispatch.
    pub fn prepare_delivery<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &mut ExecutionApp<H, R, J>,
        limit: usize,
    ) -> Result<Option<PendingDelivery>, Error> {
        client.check_delivery()?;
        if !self.recover_binding(client, task, app)? {
            return Err(Error::Conflict);
        }
        let binding = client.association(task)?.ok_or(Error::Conflict)?;
        if let Some(result) = client.pending_result(&binding)? {
            if let Some(ack) = client.frozen_result_ack(&result)? {
                return Ok(Some(PendingDelivery {
                    result: None,
                    ack: Some(ack),
                }));
            }
            app.service_delivery(binding.request(), &self.consumer, 1)
                .map_err(map_app_error)?;
            return Ok(Some(PendingDelivery {
                result: Some(result),
                ack: None,
            }));
        }
        let items = app
            .service_delivery(binding.request(), &self.consumer, limit)
            .map_err(map_app_error)?;
        if items.is_empty() {
            return Ok(None);
        }
        if items.iter().any(|v| v.input.digest() != binding.digest()) {
            return Err(Error::Conflict);
        }
        if let Some(ack) = client.acknowledged_result(&binding)? {
            client.validate_result_ack(&ack)?;
            for item in items {
                app.service_confirm(binding.request(), &self.consumer, &item.receipt.event_id)
                    .map_err(map_app_error)?;
            }
            return Ok(None);
        }
        let offer = client.retained_offer(task)?;
        let candidate = items.iter().rev().find(|v| {
            v.current_attempt.as_ref() == binding.local_attempt()
                && (v.terminal.is_some() || v.process.as_ref().is_some_and(|p| p.finished))
        });
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let Some(event) = self.project(offer.payload(), candidate)? else {
            return Ok(None);
        };
        let events: Vec<_> = items.iter().map(|v| v.receipt.event_id.clone()).collect();
        let result = client.freeze_result(&binding, &candidate.receipt.event_id, &events, event)?;
        Ok(Some(PendingDelivery {
            result: Some(result),
            ack: None,
        }))
    }
    /// Perform bounded HTTP delivery without borrowing the execution application.
    pub async fn send_delivery<S: SecretProvider, C: Clock>(
        &self,
        client: &mut Client<S, C>,
        pending: &mut PendingDelivery,
    ) -> Result<(), Error> {
        if let Some(result) = pending.result.take() {
            pending.ack = Some(client.send_result(result).await?);
        }
        Ok(())
    }
    /// Confirm original journal receipts after the durable HTTP acknowledgement.
    pub fn finish_delivery<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        pending: PendingDelivery,
        app: &mut ExecutionApp<H, R, J>,
    ) -> Result<usize, Error> {
        let ack = pending.ack.ok_or(Error::Conflict)?;
        client.validate_result_ack(&ack)?;
        for id in ack.events() {
            app.service_confirm(ack.association().request(), &self.consumer, id)
                .map_err(map_app_error)?;
        }
        client.retire_result(ack)?;
        Ok(1)
    }
    /// Deliver synchronously for consumers without a concurrent execution-owner loop.
    #[cfg(test)]
    pub async fn flush<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &mut ExecutionApp<H, R, J>,
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
        evidence: &execution_app::DeliveryEvidence,
    ) -> Result<Option<wire::TaskEvent>, Error> {
        if let Some(terminal) = evidence.terminal {
            if terminal == execution_app::DeliveryTerminal::Cancelled {
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
                    wire::TaskEvent::SoftwareResult(software_result(
                        spec,
                        None,
                        |i, p| software_encoding(&evidence.input, i, p, spec.intent),
                        &self.policy,
                        evidence.observed_at_unix_ms,
                    )?)
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
        if let wire::TaskPayload::Software(spec) = payload {
            let Some(progress) = evidence.software_progress.as_ref() else {
                return Ok(None);
            };
            return Ok(Some(wire::TaskEvent::SoftwareResult(software_result(
                spec,
                Some(progress.as_ref()),
                |i, p| software_encoding(&evidence.input, i, p, spec.intent),
                &self.policy,
                evidence.observed_at_unix_ms,
            )?)));
        }
        let (stdout, malformed, diagnostics) = project_diagnostics(
            &self.policy,
            process,
            evidence.input.spec().launch.output,
            evidence.observed_at_unix_ms,
        )?;
        let failure = diagnostics.failure();
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
            _ => return Err(Error::Unsupported),
        }))
    }
    /// Explicit terminal cleanup after every journal event has been confirmed.
    pub fn finish<
        S: SecretProvider,
        C: Clock,
        H: AppHost,
        R: RunnerPort,
        J: execution_app::JournalPort,
    >(
        &self,
        client: &mut Client<S, C>,
        task: Uuid,
        app: &ExecutionApp<H, R, J>,
        caller: &RequestContext,
    ) -> Result<(), Error> {
        let binding = client.association(task)?.ok_or(Error::Conflict)?;
        if binding.actor() != &caller.actor {
            return Err(Error::Denied);
        }
        let status = app
            .status(caller, binding.request())
            .map_err(map_app_error)?;
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
                .service_delivery(binding.request(), &self.consumer, 1)
                .map_err(map_app_error)?
                .is_empty()
        {
            return Err(Error::Conflict);
        }
        let ack = client
            .acknowledged_result(&binding)?
            .ok_or(Error::Conflict)?;
        client.release_result(ack)
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

fn project_diagnostics(
    policy: &impl OutputPolicy,
    process: &execution_contract::ProcessEvidence,
    output_spec: execution_contract::OutputSpec,
    observed_at_ms: u64,
) -> Result<(String, bool, wire::TaskDiagnostics), Error> {
    let stdout = decode_stream(&process.stdout, output_spec.stdout);
    let stderr = decode_stream(&process.stderr, output_spec.stderr);
    let malformed = stdout.is_none() || stderr.is_none();
    let stdout = policy.redact(stdout.as_deref().unwrap_or("stdout decoding failed"))?;
    let stderr = policy.redact(stderr.as_deref().unwrap_or("stderr decoding failed"))?;
    let failure = match process.end {
        ProcessEnd::Rejected => Some(wire::TaskFailure::LaunchFailed),
        ProcessEnd::TimedOut => Some(wire::TaskFailure::TimedOut),
        ProcessEnd::OutputLimit => Some(wire::TaskFailure::OutputLimit),
        _ if process.quality == OutputQuality::Truncated => Some(wire::TaskFailure::OutputLimit),
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
        i64::try_from(observed_at_ms / 1000)
            .map_err(|_| Error::Clock)?
            .max(1),
        failure,
    )?;
    Ok((stdout, malformed, diagnostics))
}
fn software_encoding(
    input: &execution_contract::FrozenExecution,
    index: usize,
    phase: execution_contract::SoftwarePhase,
    intent: wire::SoftwareTaskIntent,
) -> Result<execution_contract::OutputSpec, Error> {
    use execution_contract::{SoftwareDetector, SoftwarePhase as P};
    let step = input
        .spec()
        .execution
        .software_program()
        .and_then(|p| p.steps.get(index))
        .ok_or(Error::Protocol)?;
    let invocation = match phase {
        P::Mutation if intent == wire::SoftwareTaskIntent::Uninstall => {
            step.uninstall.as_ref().ok_or(Error::Protocol)?
        }
        P::Mutation => &step.install,
        P::Upgrade => match &step.upgrade {
            execution_contract::SoftwareUpgrade::InPlace { invocation } => invocation,
            _ => return Err(Error::Protocol),
        },
        P::Removal => step
            .auxiliary
            .get(&phase)
            .or(step.uninstall.as_ref())
            .ok_or(Error::Protocol)?,
        P::Attach | P::Stage | P::Cleanup => step.auxiliary.get(&phase).ok_or(Error::Protocol)?,
        P::Before | P::RemovalAfter | P::After => match &step.detection {
            SoftwareDetector::Script { invocation } => invocation.as_ref(),
            _ => &step.install,
        },
    };
    Ok(invocation.launch.output)
}
fn software_result(
    spec: &wire::SoftwareTaskSpec,
    progress: Option<&execution_contract::SoftwareProgress>,
    encoding: impl Fn(
        usize,
        execution_contract::SoftwarePhase,
    ) -> Result<execution_contract::OutputSpec, Error>,
    policy: &impl OutputPolicy,
    observed_at_ms: u64,
) -> Result<wire::SoftwareTaskResult, Error> {
    use execution_contract::{SoftwareCheckpoint as C, SoftwarePhase as P, SoftwareState as S};
    let time = i64::try_from(observed_at_ms / 1000)
        .map_err(|_| Error::Clock)?
        .max(1);
    let digest = Sha256::digest(encode(&progress)?).into();
    let mut results = Vec::new();
    for (index, step) in spec.steps.iter().enumerate() {
        let checkpoints = progress.map(|p| p.checkpoints.as_slice()).unwrap_or(&[]);
        let observation = |phase| match checkpoints.iter().rev().find_map(|c| match c {
            C::End {
                step,
                phase: p,
                detected,
                ..
            } if *step as usize == index && *p == phase => detected.as_ref(),
            _ => None,
        }) {
            Some(S::Present { version }) => wire::SoftwareDetectionObservation::Present {
                version: version.as_str().into(),
                evidence_sha256: digest,
                observed_at: time,
            },
            Some(S::Absent {}) => wire::SoftwareDetectionObservation::Absent {
                evidence_sha256: digest,
                observed_at: time,
            },
            _ => wire::SoftwareDetectionObservation::Unknown {
                diagnostic: "native observation unavailable".into(),
            },
        };
        let commands = super::commands::software_commands(&step.action)?;
        let policy_for = |phase| match phase {
            P::Upgrade => commands.upgrade.unwrap_or(commands.install),
            P::Removal => commands.uninstall.unwrap_or(commands.install),
            P::Mutation if spec.intent == wire::SoftwareTaskIntent::Uninstall => {
                commands.uninstall.unwrap_or(commands.install)
            }
            _ => commands.install,
        };
        let completed = checkpoints
            .iter()
            .any(|c| matches!(c,C::Complete { step } if *step as usize == index));
        let begun = checkpoints.iter().any(|c| matches!(c,C::Begin {step,phase} if *step as usize == index && !phase.is_observation()));
        let physical: Vec<_> = checkpoints
            .iter()
            .filter_map(|c| match c {
                C::End {
                    step,
                    phase,
                    process: Some(process),
                    quiescent,
                    ..
                } if *step as usize == index && !phase.is_observation() => {
                    Some((*phase, process.as_ref(), *quiescent))
                }
                _ => None,
            })
            .collect();
        let approved = |phase, p: &execution_contract::ProcessEvidence| {
            p.end == ProcessEnd::Exited
                && p.failure_kind == execution_contract::ProcessFailureKind::None
                && p.exit_code.is_some_and(|code| {
                    if matches!(phase, P::Attach | P::Stage | P::Cleanup) {
                        code == 0
                    } else {
                        policy_for(phase).exit_codes.success.contains(&code)
                            || (step.action.reboot == wire::SoftwareTaskReboot::Report
                                && policy_for(phase).exit_codes.reboot.contains(&code))
                    }
                })
        };
        let chosen = physical
            .iter()
            .find(|(phase, p, quiet)| !quiet || !approved(*phase, p))
            .or_else(|| {
                physical
                    .iter()
                    .rev()
                    .find(|(phase, _, _)| matches!(phase, P::Mutation | P::Upgrade | P::Removal))
            })
            .or_else(|| physical.last());
        let selected = chosen.map(|(phase, p, _)| (*phase, *p)).or_else(|| {
            if begun {
                None
            } else {
                checkpoints.iter().rev().find_map(|c| match c {
                    C::End {
                        step,
                        phase,
                        process: Some(p),
                        ..
                    } if *step as usize == index => Some((*phase, p.as_ref())),
                    _ => None,
                })
            }
        });
        let process = chosen.map(|(_, p, _)| *p);
        let duration_ms = checkpoints
            .iter()
            .filter_map(|checkpoint| match checkpoint {
                C::End {
                    step, duration_ms, ..
                }
                | C::CleanupEnd {
                    step, duration_ms, ..
                } if *step as usize == index => Some(*duration_ms),
                _ => None,
            })
            .fold(0u64, u64::saturating_add);
        let mut step_diagnostics = match selected {
            Some((phase, p)) => {
                let original =
                    project_diagnostics(policy, p, encoding(index, phase)?, observed_at_ms)?.2;
                let failure = if original.failure() == Some(wire::TaskFailure::NonZeroExit)
                    && approved(phase, p)
                {
                    None
                } else {
                    original.failure()
                };
                wire::TaskDiagnostics::new(
                    original.stdout().into(),
                    original.stderr().into(),
                    duration_ms,
                    time,
                    failure,
                )?
            }
            None => {
                wire::TaskDiagnostics::new(String::new(), String::new(), duration_ms, time, None)?
            }
        };
        if begun && !completed && step_diagnostics.failure().is_none() {
            step_diagnostics = wire::TaskDiagnostics::new(
                step_diagnostics.stdout().into(),
                step_diagnostics.stderr().into(),
                duration_ms,
                time,
                Some(wire::TaskFailure::CaptureFailed),
            )?;
        }
        let observed_process = if !begun {
            wire::SoftwareProcessObservation::NotRun
        } else if let Some(code) = process
            .filter(|p| p.end == ProcessEnd::Exited)
            .and_then(|p| p.exit_code)
        {
            wire::SoftwareProcessObservation::Exited { code }
        } else {
            wire::SoftwareProcessObservation::Failed {
                failure: step_diagnostics
                    .failure()
                    .unwrap_or(wire::TaskFailure::CaptureFailed),
            }
        };
        let before = observation(P::Before);
        let mut after = observation(P::After);
        if !begun
            && checkpoints
                .iter()
                .any(|c| matches!(c,C::Complete {step} if *step as usize==index))
        {
            after = before.clone();
        }
        results.push(wire::SoftwareStepResult {
            index: index.try_into().map_err(|_| Error::Capacity)?,
            step_digest: step.digest()?,
            target: step.target.clone(),
            package: step.action.package.clone(),
            identity: step.action.observed_identity(),
            before,
            after,
            process: observed_process,
            reboot_required: physical.iter().any(|(phase, p, _)| {
                !matches!(phase, P::Attach | P::Stage | P::Cleanup)
                    && p.exit_code
                        .is_some_and(|code| policy_for(*phase).exit_codes.reboot.contains(&code))
            }),
            diagnostics: step_diagnostics,
        });
    }
    let result = wire::SoftwareTaskResult {
        intent: spec.intent,
        definition_digest: spec.definition_digest,
        steps: results,
    };
    result.validate_for(spec)?;
    Ok(result)
}

#[cfg(test)]
mod result_tests {
    use super::*;
    use execution_contract::{
        AttemptId, Digest, Id, PackageValue, ProcessEvidence, ProcessFailureKind, ProcessScope,
        SoftwareCheckpoint as C, SoftwarePhase as P, SoftwareProgress, SoftwareState as S,
    };
    struct Redact;
    impl OutputPolicy for Redact {
        fn redact(&self, text: &str) -> Result<String, Error> {
            Ok(text.replace("private", "[redacted]"))
        }
    }
    fn encoding(
        index: usize,
        _: execution_contract::SoftwarePhase,
    ) -> Result<execution_contract::OutputSpec, Error> {
        Ok(execution_contract::OutputSpec {
            format: execution_contract::OutputFormat::Text {},
            stdout: if index == 0 {
                execution_contract::TextEncoding::Utf8
            } else {
                execution_contract::TextEncoding::Utf16Le
            },
            stderr: if index == 0 {
                execution_contract::TextEncoding::Utf8
            } else {
                execution_contract::TextEncoding::Utf16Le
            },
        })
    }
    #[tokio::test]
    async fn review_regression_step_results_never_borrow_another_steps_facts() {
        let server = crate::protocol_test_support::Server::new().await;
        let spec = {
            let mut data = server.data.lock().unwrap();
            data.software(2, false);
            let wire::TaskPayload::Software(spec) = data.offer.as_ref().unwrap().payload.clone()
            else {
                panic!("software")
            };
            spec
        };
        let digest = Digest::new("a".repeat(64)).unwrap();
        let attempt = AttemptId::new("attempt").unwrap();
        let runner = Id::new("fixture").unwrap();
        let process = ProcessEvidence {
            content_digest: digest.clone(),
            attempt_id: attempt.clone(),
            runner: runner.clone(),
            scope: ProcessScope::ProcessGroup { owner: 1, group: 1 },
            finished: true,
            exit_code: Some(0),
            end: ProcessEnd::Exited,
            failure_kind: ProcessFailureKind::None,
            quiescent: true,
            stdout: b"step-zero-private".to_vec(),
            stderr: b"step-zero-error".to_vec(),
            total_output_bytes: 0,
            quality: OutputQuality::Complete,
        };
        let mut progress = SoftwareProgress {
            attempt_id: attempt,
            content_digest: digest,
            runner,
            elapsed_ms: 100,
            output_bytes: 0,
            checkpoints: vec![
                C::Begin {
                    step: 0,
                    phase: P::Before,
                },
                C::End {
                    duration_ms: 20,
                    step: 0,
                    phase: P::Before,
                    process: None,
                    detected: Some(S::Absent {}),
                    quiescent: true,
                },
                C::Begin {
                    step: 0,
                    phase: P::Mutation,
                },
                C::End {
                    duration_ms: 20,
                    step: 0,
                    phase: P::Mutation,
                    process: Some(Box::new(process)),
                    detected: None,
                    quiescent: true,
                },
                C::Begin {
                    step: 0,
                    phase: P::After,
                },
                C::End {
                    duration_ms: 20,
                    step: 0,
                    phase: P::After,
                    process: None,
                    detected: Some(S::Present {
                        version: PackageValue::new("1.0").unwrap(),
                    }),
                    quiescent: true,
                },
                C::Complete { step: 0 },
            ],
        };
        let result = software_result(&spec, Some(&progress), encoding, &Redact, 1000).unwrap();
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].diagnostics.duration_ms(), 60);
        assert_eq!(result.steps[1].diagnostics.duration_ms(), 0);
        for (i, step) in result.steps.iter().enumerate() {
            assert_eq!(step.index as usize, i);
            assert_eq!(step.step_digest, spec.steps[i].digest().unwrap());
            assert_eq!(step.package, spec.steps[i].action.package);
        }
        assert!(matches!(
            result.steps[0].before,
            wire::SoftwareDetectionObservation::Absent { .. }
        ));
        assert!(matches!(
            result.steps[0].after,
            wire::SoftwareDetectionObservation::Present { .. }
        ));
        assert_eq!(
            result.steps[0].process,
            wire::SoftwareProcessObservation::Exited { code: 0 }
        );
        assert!(!result.steps[0].reboot_required);
        assert_eq!(result.steps[0].diagnostics.stdout(), "step-zero-[redacted]");
        assert_eq!(result.steps[1].diagnostics.stdout(), "");
        assert_eq!(result.steps[1].diagnostics.failure(), None);
        assert!(result.steps[1].before.is_unknown() && result.steps[1].after.is_unknown());
        assert_eq!(
            result.steps[1].process,
            wire::SoftwareProcessObservation::NotRun
        );
        let mut upgraded = progress.clone();
        for checkpoint in &mut upgraded.checkpoints {
            match checkpoint {
                C::Begin { phase, .. } | C::End { phase, .. } if *phase == P::Mutation => {
                    *phase = P::Upgrade
                }
                _ => (),
            }
        }
        let mut upgrade_spec = spec.clone();
        let wire::SoftwareTaskBehavior::Pkg(native) = &mut upgrade_spec.steps[0].action.behavior
        else {
            unreachable!()
        };
        native.upgrade_invocation.exit_codes.success.insert(42);
        for checkpoint in &mut upgraded.checkpoints {
            if let C::End {
                phase: P::Upgrade,
                process: Some(p),
                ..
            } = checkpoint
            {
                p.exit_code = Some(42);
            }
        }
        let result =
            software_result(&upgrade_spec, Some(&upgraded), encoding, &Redact, 1000).unwrap();
        assert_eq!(
            result.steps[0].process,
            wire::SoftwareProcessObservation::Exited { code: 42 }
        );
        assert_eq!(result.steps[0].diagnostics.failure(), None);
        upgraded
            .checkpoints
            .retain(|c| !matches!(c, C::Complete { step: 0 }));
        upgraded.checkpoints.push(C::Begin {
            step: 0,
            phase: P::Cleanup,
        });
        let result =
            software_result(&upgrade_spec, Some(&upgraded), encoding, &Redact, 1000).unwrap();
        assert_eq!(
            result.steps[0].process,
            wire::SoftwareProcessObservation::Exited { code: 42 }
        );
        assert_eq!(
            result.steps[0].diagnostics.failure(),
            Some(wire::TaskFailure::CaptureFailed)
        );
        progress.checkpoints.extend([
            C::Begin {
                step: 1,
                phase: P::Before,
            },
            C::End {
                duration_ms: 20,
                step: 1,
                phase: P::Before,
                process: None,
                detected: Some(S::Absent {}),
                quiescent: true,
            },
            C::Begin {
                step: 1,
                phase: P::Mutation,
            },
        ]);
        let result = software_result(&spec, Some(&progress), encoding, &Redact, 1000).unwrap();
        assert!(matches!(
            result.steps[1].process,
            wire::SoftwareProcessObservation::Failed { .. }
        ));
        assert!(result.steps[1].after.is_unknown());
        let C::End {
            process: Some(first),
            ..
        } = &progress.checkpoints[3]
        else {
            panic!("mutation")
        };
        let mut timed_out = first.clone();
        timed_out.end = ProcessEnd::TimedOut;
        timed_out.exit_code = None;
        timed_out.stdout = "step-one-private"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        timed_out.stderr = "step-one-error"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        progress.checkpoints.push(C::End {
            duration_ms: 20,
            step: 1,
            phase: P::Mutation,
            process: Some(timed_out),
            detected: None,
            quiescent: true,
        });
        let result = software_result(&spec, Some(&progress), encoding, &Redact, 1000).unwrap();
        assert_eq!(result.steps[0].diagnostics.stdout(), "step-zero-[redacted]");
        assert_eq!(result.steps[0].diagnostics.failure(), None);
        assert_eq!(result.steps[1].diagnostics.stdout(), "step-one-[redacted]");
        assert_eq!(result.steps[1].diagnostics.stderr(), "step-one-error");
        assert_eq!(
            result.steps[1].diagnostics.failure(),
            Some(wire::TaskFailure::TimedOut)
        );
        assert_eq!(
            result.steps[1].process,
            wire::SoftwareProcessObservation::Failed {
                failure: wire::TaskFailure::TimedOut
            }
        );
    }
}
