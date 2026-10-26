use super::request_id;
use crate::error::app_error as map_app_error;
use agent_client::protocol::*;
use agent_client::{Client, Clock, Error, IdentityProvider, Materials, Offer, Start};
use rss_mdm_agent_wire::runtime as wire;
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
impl ExecutionBridge<agent_client::CredentialRedactor> {
    pub(crate) fn protect_user_session(
        &mut self,
        value: &std::sync::Arc<zeroize::Zeroizing<String>>,
    ) -> Result<(), Error> {
        self.policy.protect_user_session(value)
    }
}
impl<P: OutputPolicy> ExecutionBridge<P> {
    /// Select one stable consumer and independently trusted output policy.
    pub fn new(consumer: Id, policy: P) -> Self {
        Self { consumer, policy }
    }
    pub(crate) fn protect_status(
        &self,
        mut status: wire::SelfServiceStatus,
    ) -> Result<wire::SelfServiceStatus, Error> {
        fn protect<P: OutputPolicy>(
            policy: &P,
            value: &mut serde_json::Value,
        ) -> Result<(), Error> {
            match value {
                serde_json::Value::String(text) => *text = policy.redact(text)?,
                serde_json::Value::Array(values) => {
                    for value in values {
                        protect(policy, value)?;
                    }
                }
                serde_json::Value::Object(values) => {
                    let old = std::mem::take(values);
                    for (key, mut value) in old {
                        protect(policy, &mut value)?;
                        let key = policy.redact(&key)?;
                        if values.insert(key, value).is_some() {
                            return Err(Error::Untrusted);
                        }
                    }
                }
                _ => (),
            }
            Ok(())
        }
        if let Some(wire::SelfServiceResult::Script { evidence: result }) = &mut status.result {
            let original = result.output.clone();
            protect(&self.policy, &mut result.output)?;
            if result.output != original {
                result.schema_valid = false;
                result.trusted = false;
            }
            let d = &result.diagnostics;
            result.diagnostics = wire::TaskDiagnostics::new(
                self.policy.redact(d.stdout())?,
                self.policy.redact(d.stderr())?,
                d.duration_ms(),
                d.executed_at(),
                d.failure(),
            )
            .map_err(|_| Error::Protocol)?;
        }
        Ok(status)
    }
    /// Check materials and current host prerequisites BEFORE requesting a remote Start.
    /// Scripts and ordered software each bind to one immutable local intent.
    pub fn prepare<H: AppHost, R: RunnerPort, J: execution_app::JournalPort>(
        &self,
        offer: &Offer,
        materials: &Materials,
        _app: &ExecutionApp<H, R, J>,
        caller: &RequestContext,
        plan: &FrozenExecution,
    ) -> Result<PreparedExecution, Error> {
        materials.validate_offer(offer)?;
        let (device, platform, run_as, timeout, output) = match offer.payload() {
            wire::TaskPayload::DeviceAccess(v) => {
                if plan.spec().launch.interpreter.profile.id.as_str()
                    != crate::access_worker::PROFILE
                    || plan.spec().request.operation.resource.revision.as_str()
                        != super::plan::hex(&v.definition_digest)
                {
                    return Err(Error::Untrusted);
                }
                (
                    &v.device_id,
                    v.platform,
                    wire::ExecutionIdentity::System,
                    3_600_000,
                    crate::access_worker::OUTPUT_BYTES,
                )
            }
            wire::TaskPayload::Unmanage(_) => return Err(Error::Unsupported),
            wire::TaskPayload::Credential(v) => {
                if plan.spec().launch.interpreter.profile.id.as_str()
                    != crate::credential_worker::PROFILE
                    || plan.spec().request.operation.resource.revision.as_str()
                        != super::plan::hex(&v.definition_digest)
                {
                    return Err(Error::Untrusted);
                }
                (
                    &v.device_id,
                    v.platform,
                    wire::ExecutionIdentity::System,
                    3_600_000,
                    crate::credential_worker::OUTPUT_BYTES,
                )
            }
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
            wire::TaskPayload::Collection(v) => {
                if !matches!(
                    plan.spec().execution,
                    execution_contract::ExecutionSpec::Process {}
                ) || plan.spec().launch.interpreter.profile.id.as_str()
                    != crate::collection_worker::PROFILE
                    || plan.spec().request.operation.resource.revision.as_str()
                        != super::plan::hex(&v.resource_digest)
                {
                    return Err(Error::Untrusted);
                }
                (
                    &v.device_id,
                    v.platform,
                    v.execution.identity,
                    u64::from(v.plan.spec().budget.timeout_seconds) * 1000,
                    u64::from(v.plan.spec().budget.output_bytes),
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
                    timeout,
                    output,
                )
            }
            wire::TaskPayload::Enrollment(v) => {
                if plan.spec().launch.interpreter.profile.id.as_str()
                    != crate::enrollment_worker::PROFILE
                {
                    return Err(Error::Untrusted);
                }
                (
                    &v.device_id,
                    v.platform,
                    wire::ExecutionIdentity::LoggedInUser,
                    crate::enrollment_worker::TIMEOUT_MS,
                    crate::enrollment_worker::OUTPUT_BYTES,
                )
            }
            wire::TaskPayload::Script(_) => return Err(Error::Untrusted),
        };
        let actual_platform = match platform {
            wire::TaskPlatform::Macos => execution_contract::Platform::Macos,
            wire::TaskPlatform::Windows => execution_contract::Platform::Windows,
        };
        let p = plan.spec();
        if p.request.request_id != request_id(offer)? {
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
        Ok(PreparedExecution {
            plan: plan.clone(),
            caller: caller.clone(),
            input: offer.payload().clone(),
        })
    }
    /// Submit a host-adapted immutable plan only with a currently valid Start.
    /// Preparation/adaptation must occur before requesting Start; this does no network I/O.
    pub fn dispatch<
        S: IdentityProvider,
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
        if let wire::TaskPayload::DeviceAccess(task) = start.payload() {
            super::access::verify_input(plan, task)?;
        }
        if let wire::TaskPayload::Credential(task) = start.payload() {
            super::credential::verify_input(plan, task)?;
        }
        if let wire::TaskPayload::Script(task) = start.payload() {
            super::plan::verify_script_projection(plan, task)?;
        }
        if let wire::TaskPayload::Collection(task) = start.payload() {
            super::collection::verify_input(plan, task)?;
        }
        if !start.matches_payload(&input) {
            return Err(Error::Untrusted);
        }
        let expiry = u64::try_from(start.payload().expires_at())
            .map_err(|_| Error::Clock)?
            .checked_mul(1000)
            .ok_or(Error::Clock)?;
        if plan.spec().validity.expires_at_unix_ms > expiry {
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
        S: IdentityProvider,
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
        S: IdentityProvider,
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
    pub async fn send_abandonment<S: IdentityProvider, C: Clock>(
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
        S: IdentityProvider,
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
        S: IdentityProvider,
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
        S: IdentityProvider,
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
        S: IdentityProvider,
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
        let retained = client.retained_offer(task)?;
        if let (wire::TaskPayload::Credential(spec), Some(attempt)) =
            (retained.payload(), binding.local_attempt())
        {
            let plan = app
                .service_frozen_input(binding.request())
                .map_err(map_app_error)?;
            let root = crate::credential_worker::plan_root(&plan)?;
            if crate::credential_worker::stored_result(&root, spec)?.is_some() {
                app.service_assess(
                    binding.request(),
                    attempt,
                    super::plan::reference(spec.task_id.to_string(), "credential-result")?,
                    &super::credential::EffectEvidence {
                        task: spec,
                        attempt,
                    },
                )
                .map_err(map_app_error)?;
            }
        }
        if let (wire::TaskPayload::DeviceAccess(spec), Some(attempt)) =
            (retained.payload(), binding.local_attempt())
        {
            let plan = app
                .service_frozen_input(binding.request())
                .map_err(map_app_error)?;
            let root = crate::access_worker::plan_root(&plan)?;
            if crate::access_worker::stored_result(&root, spec)?.is_some() {
                app.service_assess(
                    binding.request(),
                    attempt,
                    super::plan::reference(spec.task_id.to_string(), "access-result")?,
                    &super::access::EffectEvidence {
                        task: spec,
                        attempt,
                    },
                )
                .map_err(map_app_error)?;
            }
        }
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
    pub async fn send_delivery<S: IdentityProvider, C: Clock>(
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
        S: IdentityProvider,
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
        S: IdentityProvider,
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
        if let wire::TaskPayload::DeviceAccess(spec) = payload {
            if let Some(terminal) = evidence.terminal {
                return Ok(Some(access_terminal(spec, terminal)));
            }
            let stopped = evidence.terminal.is_some()
                || evidence.process.as_ref().is_some_and(|p| p.finished);
            if !stopped {
                return Ok(None);
            }
            let quiescent = evidence.terminal.is_some()
                || evidence.process.as_ref().is_some_and(|p| p.quiescent);
            let result = if quiescent {
                crate::access_worker::stored_result(
                    &crate::access_worker::plan_root(&evidence.input)?,
                    spec,
                )?
            } else {
                None
            };
            let Some(result) = result else {
                return Ok(None);
            };
            if !crate::access_worker::complete(spec, &result.effect)
                && !matches!(result.effect, wire::AccessEffect::Failed { .. })
            {
                return Ok(None);
            }
            return Ok(Some(wire::TaskEvent::DeviceAccessResult(result)));
        }
        if let Some(terminal) = evidence.terminal {
            if let wire::TaskPayload::Credential(spec) = payload {
                return Ok(Some(wire::TaskEvent::CredentialResult(
                    crate::credential_worker::failure_with_materials(
                        &crate::credential_worker::plan_root(&evidence.input)?,
                        spec,
                        wire::CredentialFailure::RecoveryState,
                    )?,
                )));
            }
            if terminal == execution_app::DeliveryTerminal::Cancelled {
                return Ok(Some(wire::TaskEvent::Cancelled));
            }
            if matches!(payload, wire::TaskPayload::Collection(_)) {
                return Ok(None);
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
                wire::TaskPayload::DeviceAccess(_) => return Err(Error::Unsupported),
                wire::TaskPayload::Script(_) => wire::TaskEvent::Result(wire::TaskResult::new(
                    None,
                    wire::OutputQuality::Failed,
                    serde_json::Value::Null,
                    diagnostics,
                    i64::try_from(evidence.observed_at_unix_ms / 1000).map_err(|_| Error::Clock)?,
                )?),
                wire::TaskPayload::Software(spec) => {
                    wire::TaskEvent::SoftwareResult(software_result(
                        spec,
                        None,
                        |i, p| software_encoding(&evidence.input, i, p),
                        &self.policy,
                        evidence.observed_at_unix_ms,
                    )?)
                }
                wire::TaskPayload::Enrollment(_) => wire::TaskEvent::EnrollmentResult {
                    outcome: wire::EnrollmentEntryOutcome::Failed,
                },
                _ => return Err(Error::Unsupported),
            }));
        }
        let Some(process) = evidence.process.as_ref() else {
            return Ok(None);
        };
        if !process.finished {
            return Ok(None);
        }
        if !matches!(payload, wire::TaskPayload::Credential(_))
            && process.end == ProcessEnd::Cancelled
            && process.quiescent
        {
            return Ok(Some(wire::TaskEvent::Cancelled));
        }
        if let wire::TaskPayload::Credential(spec) = payload {
            let root = crate::credential_worker::plan_root(&evidence.input)?;
            let result = if process.quiescent {
                crate::credential_worker::stored_result(&root, spec)?
            } else {
                None
            };
            let result = match result {
                Some(value) => value,
                None => crate::credential_worker::failure_with_materials(
                    &root,
                    spec,
                    wire::CredentialFailure::RecoveryState,
                )?,
            };
            return Ok(Some(wire::TaskEvent::CredentialResult(result)));
        }
        if let wire::TaskPayload::Software(spec) = payload {
            let Some(progress) = evidence.software_progress.as_ref() else {
                return Ok(None);
            };
            return Ok(Some(wire::TaskEvent::SoftwareResult(software_result(
                spec,
                Some(progress.as_ref()),
                |i, p| software_encoding(&evidence.input, i, p),
                &self.policy,
                evidence.observed_at_unix_ms,
            )?)));
        }
        if matches!(payload, wire::TaskPayload::Enrollment(_)) {
            let outcome = if process.end == ProcessEnd::Exited
                && process.quiescent
                && process.quality == OutputQuality::Complete
                && process.exit_code == Some(0)
            {
                serde_json::from_slice(&process.stdout).map_err(|_| Error::Protocol)?
            } else {
                wire::EnrollmentEntryOutcome::Unknown
            };
            return Ok(Some(wire::TaskEvent::EnrollmentResult { outcome }));
        }
        if let wire::TaskPayload::Collection(spec) = payload {
            if process.end != ProcessEnd::Exited
                || !process.quiescent
                || process.quality != OutputQuality::Complete
                || process.exit_code != Some(0)
            {
                return Ok(None); // Retain unknown evidence; no native result was durably captured.
            }
            let mut result: wire::NativeCollectionResult =
                serde_json::from_slice(&process.stdout).map_err(|_| Error::Protocol)?;
            let now =
                i64::try_from(evidence.observed_at_unix_ms / 1000).map_err(|_| Error::Clock)?;
            result.validate_for(&spec.plan, now)?;
            for (id, query) in &mut result.queries {
                let definition = &spec.plan.spec().queries[id];
                // Do not rewrite typed values or identities into redacted but apparently valid facts.
                let text = serde_json::to_string(&query.rows).map_err(|_| Error::Protocol)?;
                if self.policy.redact(&text)? != text {
                    *query = crate::collection_worker::failure(
                        definition,
                        wire::NativeQueryStatus::Denied,
                        query.collected_at,
                    );
                } else if query.status == wire::NativeQueryStatus::Complete
                    && !query.complete_for(definition, now)
                {
                    query.status = wire::NativeQueryStatus::Invalid;
                    query.enumeration_complete = false;
                }
            }
            result.validate_for(&spec.plan, now)?;
            return Ok(Some(wire::TaskEvent::CollectionResult(result)));
        }
        let (stdout, malformed, diagnostics) = project_diagnostics(
            &self.policy,
            process,
            evidence.input.spec().launch.output,
            evidence.observed_at_unix_ms,
        )?;
        let failure = diagnostics.failure();
        Ok(Some(match payload {
            wire::TaskPayload::DeviceAccess(_) => return Err(Error::Unsupported),
            wire::TaskPayload::Script(_) => {
                let output = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
                let quality = match process.quality {
                    OutputQuality::Complete
                        if process.exit_code == Some(0) && failure.is_none() =>
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
                    i64::try_from(evidence.observed_at_unix_ms / 1000).map_err(|_| Error::Clock)?,
                )?)
            }
            _ => return Err(Error::Unsupported),
        }))
    }
    /// Explicit terminal cleanup after every journal event has been confirmed.
    pub fn finish<
        S: IdentityProvider,
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
        // Script Result is process evidence, not an independent effect assessment.
        // Only a closed activity may release its retained association and materials.
        let script_closed = matches!(
            client.retained_offer(task)?.payload(),
            wire::TaskPayload::Script(_)
        ) && status.phase == TaskPhase::ExecutionEnded
            && status
                .process
                .as_ref()
                .is_some_and(|p| p.finished && p.quiescent);
        if (!settled(&status) && !script_closed)
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
        if let wire::TaskPayload::Credential(spec) = client.retained_offer(task)?.payload() {
            let input = app
                .frozen_input(caller, binding.request())
                .map_err(map_app_error)?;
            crate::credential_worker::retire(&crate::credential_worker::plan_root(&input)?, spec)?;
        }
        if let wire::TaskPayload::DeviceAccess(spec) = client.retained_offer(task)?.payload() {
            let input = app
                .frozen_input(caller, binding.request())
                .map_err(map_app_error)?;
            crate::access_worker::retire(&input, spec)?;
        }
        client.release_result(ack)
    }
}
// A trusted pre-dispatch terminal is evidence itself; there can be no worker result file.
fn access_terminal(
    spec: &wire::AccessTaskSpec,
    terminal: execution_app::DeliveryTerminal,
) -> wire::TaskEvent {
    if terminal == execution_app::DeliveryTerminal::Cancelled {
        wire::TaskEvent::Cancelled
    } else {
        wire::TaskEvent::DeviceAccessResult(crate::access_worker::report(
            spec,
            wire::AccessEffect::Failed {
                reason: wire::AccessFailure::NativeFailure,
            },
        ))
    }
}
/// Shared proof of safe terminal state; Unknown and an unquiesced attempt remain retained.
pub(crate) fn settled(status: &execution_app::ExecutionStatus) -> bool {
    (status.phase == TaskPhase::Verified
        && matches!(
            status.assessment,
            Some(
                execution_lifecycle::EffectAssessment::Satisfied
                    | execution_lifecycle::EffectAssessment::NoEffect
            )
        ))
        || matches!(
            status.phase,
            TaskPhase::Cancelled | TaskPhase::FailedBeforeDispatch
        )
        || (status.phase == TaskPhase::AdmissionDenied && status.attempt_id.is_none())
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
 ) -> Result<execution_contract::OutputSpec, Error> {
    let invocation = input
        .spec()
        .execution
        .software_program()
        .and_then(|program| program.invocation(index, phase))
        .ok_or(Error::Protocol)?;
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
        let commands = execution_contract::software_commands(&step.action);
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
            step_id: step.step_id,
            index: index.try_into().map_err(|_| Error::Capacity)?,
            step_digest: step.digest()?,
            target: step.target.clone(),
            package: step.action.package.clone(),
            identity: observed_identity(&step.action),
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
    assess_software_result(&result, spec)?;
    Ok(result)
}

#[cfg(test)]
mod result_tests {
    use super::*;
    use execution_contract::{
        AttemptId, Digest, Id, PackageValue, ProcessEvidence, ProcessFailureKind, ProcessScope,
        SoftwareCheckpoint as C, SoftwarePhase as P, SoftwareProgress, SoftwareState as S,
    };
    #[test]
    fn access_pre_dispatch_terminals_preserve_cancellation_and_failure() {
        let id = Uuid::new_v4();
        let task = wire::AccessTaskSpec {
            wire_version: 1,
            tenant_id: id,
            device_id: "device".into(),
            registration_id: id,
            generation: 1,
            platform: wire::TaskPlatform::Windows,
            architecture: wire::TaskArchitecture::X86_64,
            task_id: id,
            attempt_id: id,
            permit: wire::TaskPermit::Start,
            expires_at: 10,
            deadline: 20,
            binding_id: id,
            instance_id: id,
            principal_id: id,
            binding_generation: 1,
            factor_generation: 1,
            revision: 1,
            definition_digest: [7; 32],
            action: wire::AccessAction::Apply {
                account: wire::AccessAccount::Windows {
                    sid: "S-1-12-1-1-2-3-4".into(),
                    authority: wire::AccessWindowsAuthority::Entra {
                        tenant_id: id,
                        object_id: Uuid::from_bytes_le([
                            1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0,
                        ]),
                    },
                },
                configuration: wire::AccessConfiguration {
                    windows: wire::AccessWindowsConfiguration::Whfb,
                    mac: None,
                    enabled: true,
                    self_enrollment: true,
                    role: None,
                },
            },
            windows: Some(wire::AccessWindowsPlan::Whfb {
                retire: false,
                previous_key: None,
            }),
            authorization: wire::TaskAuthorization::Policy {
                policy_id: id,
                policy_version: id,
            },
        };
        task.validate().unwrap();
        let input = std::sync::Arc::new(crate::backend::test_support::local::plan());
        let mut evidence = execution_app::DeliveryEvidence {
            receipt: execution_app::Receipt {
                sequence: 1,
                event_id: execution_contract::EventId::new("terminal").unwrap(),
                operation_id: execution_app::OperationRequestId::new("terminal-op").unwrap(),
                scope: execution_app::Scope::from_input(&input),
                kind: execution_app::OperationKind::Execution,
                outcome: execution_app::Outcome::Cancelled,
                admission: None,
                revision: 1,
                attempt_id: None,
                occurred_at_unix_ms: 1000,
            },
            input,
            process: None,
            software_progress: None,
            current_attempt: None,
            terminal: Some(execution_app::DeliveryTerminal::Cancelled),
            observed_at_unix_ms: 1000,
        };
        let bridge = ExecutionBridge::new(Id::new("terminal-consumer").unwrap(), Redact);
        let payload = wire::TaskPayload::DeviceAccess(Box::new(task.clone()));
        // The input deliberately has no native worker root or result file.
        assert!(matches!(
            bridge.project(&payload, &evidence).unwrap(),
            Some(wire::TaskEvent::Cancelled)
        ));
        evidence.terminal = Some(execution_app::DeliveryTerminal::NeverDispatched);
        let Some(wire::TaskEvent::DeviceAccessResult(report)) =
            bridge.project(&payload, &evidence).unwrap()
        else {
            panic!("missing failure")
        };
        evidence.terminal = None;
        assert!(bridge.project(&payload, &evidence).unwrap().is_none());
        assert_eq!(report.binding_id, task.binding_id);
        assert_eq!(report.definition_digest, task.definition_digest);
        assert!(matches!(
            report.effect,
            wire::AccessEffect::Failed {
                reason: wire::AccessFailure::NativeFailure
            }
        ));
        report.validate_for(task.platform).unwrap();
    }
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
        let _budget = crate::scenario_budget::enter();
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
        assert!(
            matches!(
                result.steps[1].before,
                wire::SoftwareDetectionObservation::Unknown { .. }
            ) && matches!(
                result.steps[1].after,
                wire::SoftwareDetectionObservation::Unknown { .. }
            )
        );
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
        assert!(matches!(
            result.steps[1].after,
            wire::SoftwareDetectionObservation::Unknown { .. }
        ));
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
