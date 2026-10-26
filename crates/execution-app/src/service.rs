use crate::{
    host::{check_budget, Host, ObservationEvidence},
    *,
};
use execution_contract::{AttemptId, Authority, EventId, FrozenExecution, RequestId};
use execution_contract::{Id, SoftwareProgress};
use execution_lifecycle::{
    Command, CommandEvent, Directive, DispatchAction, DispatchCause, DispatchState, Execution,
    ExecutionMode, Observation, ObservationEvent, ObservationFacts, Phase, Preparation,
    StopOutcome, StopReason,
};
use sha2::{Digest as _, Sha256};

/// Service-owned bounded execution operations. Dropping a client, window or model future does
/// not own this object. The host schedules reconciliation independently of those clients.
/// No method implicitly retries dispatch; a new attempt requires an explicit new command.
pub struct ExecutionApp<H, R, J: JournalPort> {
    pub(crate) journal: J,
    pub(crate) host: H,
    runner: R,
    pub(crate) binding: ServiceBinding,
    pub(crate) config: AppConfig,
}
impl<H: AppHost, R: RunnerPort, J: JournalPort> ExecutionApp<H, R, J> {
    /// Stable caller-authorized preparation history, independently paged from execution facts.
    pub fn backend_request_page(
        &self,
        caller: &RequestContext,
        after: Option<&RequestId>,
        watermark: u64,
        limit: usize,
    ) -> Result<Vec<execution_contract::BackendRequest>, Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Denied);
        }
        Ok(self.journal.backend_request_page(
            &self.host.binding_for_actor(&caller.actor)?.authority,
            &caller.actor,
            after,
            watermark,
            limit,
            &self.adapter(Some(caller)),
        )?)
    }
    /// Bounded pre-execution history for the same authenticated actor.
    pub fn backend_requests(
        &self,
        caller: &RequestContext,
    ) -> Result<Vec<execution_contract::BackendRequest>, Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Unbound);
        }
        Ok(self.journal.backend_requests(
            &self.host.binding_for_actor(&caller.actor)?.authority,
            &caller.actor,
            &Host::new(&self.host, &self.binding, &self.config, Some(caller)),
        )?)
    }
    /// Read an authenticated caller's preparation record from the sole journal.
    pub fn backend_request(
        &self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<Option<execution_contract::BackendRequest>, Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Unbound);
        }
        let scope = Scope {
            authority: self.host.binding_for_actor(&caller.actor)?.authority,
            actor: caller.actor.clone(),
            request_id: request.clone(),
        };
        Ok(self.journal.backend_request(
            &scope,
            &Host::new(&self.host, &self.binding, &self.config, Some(caller)),
        )?)
    }
    /// Persist preparation only. This never performs admission or dispatches a runner.
    pub fn record_backend_request(
        &mut self,
        caller: &RequestContext,
        expected: Option<&execution_contract::BackendRequest>,
        next: &execution_contract::BackendRequest,
    ) -> Result<(), Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Unbound);
        }
        let scope = Scope {
            authority: self.host.binding_for_actor(&caller.actor)?.authority,
            actor: caller.actor.clone(),
            request_id: next.action.request().clone(),
        };
        let host = Host::new(&self.host, &self.binding, &self.config, Some(caller));
        Ok(self
            .journal
            .record_backend_request(&scope, expected, next, &host)?)
    }
    /// Attach an already-opened trusted journal. Storage creation/opening belongs to the
    /// product composition root, which validates production inputs before touching storage.
    pub fn new(journal: J, host: H, runner: R, config: AppConfig) -> Result<Self, Error> {
        config.validate(journal.input_limits())?;
        let binding = host.service_binding()?;
        let expected_mode = if matches!(binding.authority, Authority::Test { .. }) {
            ExecutionMode::Test
        } else {
            host.reliable_now()?;
            ExecutionMode::Real
        };
        if journal.authority() != &binding.authority || runner.mode() != expected_mode {
            return Err(Error::Unbound);
        }
        Ok(Self {
            journal,
            host,
            runner,
            binding,
            config,
        })
    }
    /// Replace only independently verified business bindings under the same physical owner.
    /// Existing inputs, journal identity, runner, budgets and attempts remain unchanged.
    pub fn replace_host(&mut self, host: H) -> Result<(), Error> {
        if host.service_binding()? != self.binding {
            return Err(Error::Unbound);
        }
        host.reliable_now()?;
        self.host = host;
        Ok(())
    }
    pub(crate) fn adapter<'a>(&'a self, context: Option<&'a RequestContext>) -> Host<'a, H> {
        Host::new(&self.host, &self.binding, &self.config, context)
    }
    fn check_binding(
        &self,
        context: Option<&RequestContext>,
        plan: &FrozenExecution,
    ) -> Result<(), Error> {
        let actual = self.host.service_binding()?;
        let p = &plan.spec().request;
        let selected = self.host.binding_for_authority(&p.authority)?;
        if actual != self.binding
            || p.authority != selected.authority
            || context.is_some_and(|caller| p.actor != caller.actor)
            || p.target.device != selected.device
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    pub(crate) fn load(
        &self,
        context: Option<&RequestContext>,
        request: &RequestId,
        access: ExecutionAccess<'_>,
    ) -> Result<Execution, Error> {
        let execution = self
            .journal
            .execution_by_request(request, access, &self.adapter(context))?
            .execution;
        self.check_binding(context, execution.input())?;
        Ok(execution)
    }
    /// Accept a single immutable execution. Exact replays continue only before the first intent.
    /// Check current host binding, creation access, static limits without
    /// opening a journal task or consuming approval. Actual submission still repeats its gates.
    pub fn prepare_execution(
        &self,
        caller: &RequestContext,
        plan: &FrozenExecution,
    ) -> Result<(), Error> {
        self.check_binding(Some(caller), plan)?;
        let scope = Scope::from_input(plan);
        self.host.authorize(
            caller,
            AccessRequest {
                access: Access::Create,
                scope: &scope,
                consumer: None,
                interaction: None,
            },
        )?;
        check_budget(plan, self.config)?;
        Ok(())
    }
    /// Retain an exact preparation in the existing journal without admission, an attempt or dispatch.
    /// The trusted product must authorize the original intent before calling this method.
    /// A later explicit submission rechecks all gates against the retained immutable input.
    pub fn stage_execution(
        &mut self,
        caller: &RequestContext,
        plan: &FrozenExecution,
    ) -> Result<ExecutionStatus, Error> {
        self.prepare_execution(caller, plan)?;
        let op = operation(plan, "prepare", "")?;
        let host = Host::new(&self.host, &self.binding, &self.config, Some(caller));
        self.journal.open_execution(&op, plan, &host)?;
        self.status_for(
            Some(caller),
            &plan.spec().request.request_id,
            ExecutionAccess::Submission,
        )
    }
    /// Accept a single immutable execution. Exact replays continue only before the first intent.
    pub fn request_execution(
        &mut self,
        caller: &RequestContext,
        plan: &FrozenExecution,
    ) -> Result<ExecutionStatus, Error> {
        self.check_binding(Some(caller), plan)?;
        let request = &plan.spec().request.request_id;
        let op = operation(plan, "request", "")?;
        let host = Host::new(&self.host, &self.binding, &self.config, Some(caller));
        self.journal.open_execution(&op, plan, &host)?;
        let current = self.status_for(Some(caller), request, ExecutionAccess::Submission)?;
        if current.attempts > 0 || current.cancel_requested || current.admission.is_some() {
            return Ok(current);
        }
        let attempt = AttemptId::new(key(plan, "attempt", CommandId::initial_attempt().as_str())?)
            .map_err(|_| Error::InvalidInput)?;
        let gate = if matches!(
            plan.spec().request.initiator,
            execution_contract::Initiator::Ai { .. }
        ) {
            match self.host.admit(plan, &attempt, self.host.reliable_now()?) {
                Ok(gate) => gate.execution_gate,
                Err(crate::JournalError::Denied) => crate::ExecutionGate::Blocked,
                Err(error) => return Err(error.into()),
            }
        } else {
            crate::ExecutionGate::Direct
        };
        if gate == crate::ExecutionGate::Confirmation {
            let spec = crate::execution_confirmation(plan);
            self.open_interaction(
                caller,
                request,
                spec.id.clone(),
                spec.kind,
                spec.expires_at_unix_ms,
            )?;
            let state = self.interaction(caller, request, &spec.id)?;
            match &state.snapshot().status {
                execution_interaction::Status::Answered {
                    response: execution_interaction::Response::Confirmation { accepted: true },
                    ..
                } if self.host.reliable_now()? < spec.expires_at_unix_ms => {}
                execution_interaction::Status::Pending
                    if self.host.reliable_now()? < spec.expires_at_unix_ms =>
                {
                    return self.status_for(Some(caller), request, ExecutionAccess::Submission)
                }
                _ => return self.cancel(caller, request),
            }
        }
        self.advance(caller, request, &CommandId::initial_attempt())
    }
    /// Confirm the exact persisted action through a trusted caller; never change its origin.
    pub fn confirm_execution(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        digest: &execution_contract::Digest,
        accepted: bool,
    ) -> Result<ExecutionStatus, Error> {
        let input = self.frozen_input(caller, request)?;
        if input.digest() != digest {
            return Err(Error::Conflict);
        }
        let spec = crate::execution_confirmation(&input);
        let command = CommandId::new(if accepted {
            "execute-confirm"
        } else {
            "execute-decline"
        })?;
        self.respond(
            caller,
            request,
            &command,
            &spec.id,
            &execution_interaction::Command::Answer {
                id: execution_interaction::Reference::new(key(
                    &input,
                    "confirmation-answer",
                    command.as_str(),
                )?)
                .map_err(|_| Error::InvalidInput)?,
                response: execution_interaction::Response::Confirmation { accepted },
            },
        )?;
        self.request_execution(caller, &input)
    }
    /// Explicit owner retry after a completed attempt; never an initial submission stage.
    pub fn retry_execution(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        command: &CommandId,
    ) -> Result<ExecutionStatus, Error> {
        if self
            .status_for(Some(caller), request, ExecutionAccess::Execute)?
            .attempts
            == 0
            && !matches!(
                self.frozen_input(caller, request)?.spec().request.initiator,
                execution_contract::Initiator::Policy { .. }
            )
        {
            return Err(Error::Conflict);
        }
        self.advance(caller, request, command)
    }
    /// Explicit owner action, distinct from submit replay. Every new attempt rechecks admission,
    /// the current product permit inside the same transaction as the attempt.
    pub(crate) fn advance(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        command: &CommandId,
    ) -> Result<ExecutionStatus, Error> {
        let context = Some(caller);
        let config = self.config;
        let mut execution = self.load(context, request, ExecutionAccess::Execute)?;
        let result =
            if let Some(replay) = self.committed_attempt(caller, execution.input(), command)? {
                replay
            } else {
                match self.commit_attempt(caller, &mut execution, command, config) {
                    Err(Error::Conflict) => self
                        .committed_attempt(caller, execution.input(), command)?
                        .ok_or(Error::Conflict)?,
                    result => result?,
                }
            };
        if !matches!(
            result.receipt().outcome,
            Outcome::Changed | Outcome::Duplicate | Outcome::Rejected
        ) {
            return Err(Error::Conflict);
        }
        if let CommitOutcome::Applied {
            first_dispatch: Some(action),
            ..
        } = result
        {
            self.dispatch(request, action)?;
        }
        self.status_for(context, request, ExecutionAccess::Execute)
    }
    /// A receipt proves that this exact durable command was received, even if a later command
    /// replaced the current attempt. It grants no dispatch action or fresh approval consumption.
    fn committed_attempt(
        &self,
        caller: &RequestContext,
        input: &FrozenExecution,
        command: &CommandId,
    ) -> Result<Option<CommitOutcome>, Error> {
        let scope = Scope::from_input(input);
        let op = operation(input, "begin", command.as_str())?;
        let attempt = AttemptId::new(key(input, "attempt", command.as_str())?)
            .map_err(|_| Error::InvalidInput)?;
        let Some(receipt) =
            self.journal
                .execution_receipt(&scope, &op, &self.adapter(Some(caller)))?
        else {
            return Ok(None);
        };
        if receipt.scope != scope
            || receipt.operation_id != op
            || receipt.kind != OperationKind::Execution
            || receipt.attempt_id.as_ref() != Some(&attempt)
            || !matches!(
                receipt.outcome,
                Outcome::Changed | Outcome::Duplicate | Outcome::Rejected
            )
        {
            return Err(Error::Conflict);
        }
        Ok(Some(CommitOutcome::AlreadyCommitted(receipt)))
    }
    /// Advance only to the durable intent boundary. Conflict resolution belongs to advance;
    /// dispatch stays outside this stage so an error after commit cannot be masked as a replay.
    fn commit_attempt(
        &mut self,
        caller: &RequestContext,
        execution: &mut Execution,
        command: &CommandId,
        config: AppConfig,
    ) -> Result<CommitOutcome, Error> {
        let context = Some(caller);
        check_budget(execution.input(), config)?;
        if execution.snapshot().attempt.is_none()
            && execution.snapshot().preparation != Preparation::Prepared
        {
            self.command(
                context,
                execution,
                "prepare",
                command.as_str(),
                Command::Prepare,
            )?;
            *execution = self.load(
                context,
                &execution.input().spec().request.request_id,
                ExecutionAccess::Execute,
            )?;
        }
        if !matches!(
            execution
                .directive(self.host.reliable_now()?)
                .map_err(|_| Error::Clock)?,
            Directive::Ready | Directive::RetryEligible
        ) {
            return Err(Error::Conflict);
        }
        let attempt = AttemptId::new(key(execution.input(), "attempt", command.as_str())?)
            .map_err(|_| Error::InvalidInput)?;
        self.command(
            context,
            execution,
            "begin",
            command.as_str(),
            Command::BeginAttempt {
                attempt_id: attempt,
                runner: self.runner.id(),
                mode: self.runner.mode(),
            },
        )
    }
    fn dispatch(&mut self, request: &RequestId, action: DispatchAction) -> Result<(), Error> {
        let context = None;
        let execution = self.load(context, request, ExecutionAccess::RunnerFact)?;
        check_budget(execution.input(), self.config)?;
        // Re-read persisted stop/revision facts immediately before first delivery.
        let execution = self.load(context, request, ExecutionAccess::RunnerFact)?;
        let attempt = action.attempt_id().clone();
        let current = execution.snapshot();
        let gate = if current.cancel_requested {
            Some(DispatchCause::Cancelled)
        } else if current.revision != action.committed_revision() {
            Some(DispatchCause::StaleRevision)
        } else if action.runner() != &self.runner.id() || action.mode() != self.runner.mode() {
            Some(DispatchCause::RunnerMismatch)
        } else {
            match self
                .host
                .reliable_now()
                .ok()
                .and_then(|now| execution.directive(now).ok())
            {
                None => Some(DispatchCause::ClockUnavailable),
                Some(Directive::Reconcile) => self
                    .authorize_runner(execution.input(), Access::Execute)
                    .err()
                    .map(|_| DispatchCause::AuthorityUnavailable),
                Some(
                    Directive::StopRunner(StopReason::Limit(reason))
                    | Directive::BudgetExhausted(reason),
                ) => Some(DispatchCause::Limit(reason)),
                Some(Directive::StopRunner(StopReason::Cancelled)) => {
                    Some(DispatchCause::Cancelled)
                }
                Some(_) => Some(DispatchCause::LifecycleChanged),
            }
        };
        let cause = if let Some(cause) = gate {
            drop(action);
            Some(cause)
        } else {
            match self.runner.dispatch(AuthorizedDispatch {
                issued: std::time::Instant::now(),
                allowance: execution
                    .allowance(self.host.reliable_now()?)
                    .map_err(|_| Error::Clock)?,
                action,
                plan: execution.input().clone(),
            }) {
                Ok(DispatchOutcome::Accepted) => None,
                Ok(DispatchOutcome::NeverDispatched) => {
                    Some(execution_lifecycle::DispatchCause::RunnerRejected)
                }
                Ok(DispatchOutcome::OutcomeUnknown) => {
                    Some(execution_lifecycle::DispatchCause::DeliveryUnknown)
                }
                Err(_) => Some(execution_lifecycle::DispatchCause::RunnerError),
            }
        };
        let command = match cause {
            None => Command::Dispatched {
                attempt_id: attempt.clone(),
            },
            Some(cause) => Command::DispatchUnconfirmed {
                attempt_id: attempt.clone(),
                cause,
            },
        };
        self.command(
            context,
            &execution,
            "dispatch-result",
            attempt.as_str(),
            command,
        )?;
        Ok(())
    }
    fn command(
        &mut self,
        context: Option<&RequestContext>,
        execution: &Execution,
        stage: &str,
        identity: &str,
        command: Command,
    ) -> Result<CommitOutcome, Error> {
        let plan = execution.input();
        let scope = Scope::from_input(plan);
        let op = if matches!(command, Command::BeginAttempt { .. }) {
            operation(plan, stage, identity)?
        } else {
            revision_operation(execution, stage, identity)?
        };
        let host = Host::new(&self.host, &self.binding, &self.config, context);
        let event = CommandEvent {
            id: EventId::new(op.as_str()).map_err(|_| Error::InvalidInput)?,
            expected_revision: execution.snapshot().revision,
            command,
        };
        Ok(self.journal.apply_command(&op, &scope, &event, &host)?)
    }
    fn observation(
        &mut self,
        execution: &Execution,
        attempt_id: &AttemptId,
        facts: &ObservationFacts,
    ) -> Result<CommitOutcome, Error> {
        let context = None;
        let plan = execution.input();
        let identity = serde_json::to_string(&facts.evidence).map_err(|_| Error::InvalidInput)?;
        let op = revision_operation(execution, "observe", &identity)?;
        let event = ObservationEvent {
            id: EventId::new(op.as_str()).map_err(|_| Error::InvalidInput)?,
            expected_revision: execution.snapshot().revision,
            attempt_id: attempt_id.clone(),
            evidence: facts.evidence.clone(),
        };
        let host = Host::new(&self.host, &self.binding, &self.config, context);
        Ok(self.journal.apply_observation(
            &op,
            &Scope::from_input(plan),
            &event,
            &host,
            &ObservationEvidence(facts),
        )?)
    }
    fn collect_software(&mut self, execution: &Execution) -> Result<(), Error> {
        let Some(active) = &execution.snapshot().attempt else {
            return Ok(());
        };
        let scope = Scope::from_input(execution.input());
        if let Some(previous) =
            self.journal
                .software_progress(&scope, &active.id, &self.adapter(None))?
        {
            if let Some(facts) = self
                .runner
                .recover_software_progress(execution.input(), &previous)?
            {
                let host = Host::new(&self.host, &self.binding, &self.config, None);
                self.journal
                    .record_software_progress(&scope, &facts, &host)?;
            }
        }
        if !execution.snapshot().cancel_requested && active.termination.is_none() {
            if let Some(progress) =
                self.journal
                    .software_progress(&scope, &active.id, &self.adapter(None))?
            {
                if progress.resumable(execution.input()) {
                    if let Ok(allowance) = execution.allowance(self.host.reliable_now()?) {
                        if allowance.remaining_timeout_ms > 0
                            && allowance.remaining_output_bytes > 0
                        {
                            self.runner.resume_software(crate::SoftwareResume {
                                plan: execution.input().clone(),
                                progress,
                                allowance,
                            })?;
                        }
                    }
                }
            }
        }
        if let Some(progress) =
            self.journal
                .software_progress(&scope, &active.id, &self.adapter(None))?
        {
            if progress.cleanup_allowance(execution.input()).is_some() {
                self.runner
                    .resume_software_cleanup(crate::SoftwareCleanupResume {
                        plan: execution.input().clone(),
                        progress,
                    })?;
            }
        }
        let host = Host::new(&self.host, &self.binding, &self.config, None);
        if let Some(facts) = self
            .runner
            .software_progress(execution.input(), &active.id)?
        {
            let committed = self
                .journal
                .record_software_progress(&scope, &facts, &host)?;
            let receipt = CommittedSoftwareProgress::from_committed(&facts, committed)?;
            self.runner.acknowledge_software_progress(receipt)?;
        }
        Ok(())
    }
    fn capture_for(
        &mut self,
        request: &RequestId,
        execution: &mut Execution,
    ) -> Result<(Option<execution_contract::ProcessEvidence>, bool), Error> {
        self.authorize_runner(execution.input(), Access::RunnerFact)?;
        let active = execution
            .snapshot()
            .attempt
            .as_ref()
            .ok_or(Error::Conflict)?
            .clone();
        self.collect_software(execution)?;
        let live = self.runner.evidence(execution.input(), &active.id)?;
        let had_live = live.is_some();
        let capture = match live {
            Some(facts) => Some(facts),
            None => {
                let stored = self.journal.runner_evidence(
                    &Scope::from_input(execution.input()),
                    &active.id,
                    &self.adapter(None),
                )?;
                if stored.as_ref().is_some_and(|f| f.finished) {
                    stored
                } else {
                    let progress = self.journal.software_progress(
                        &Scope::from_input(execution.input()),
                        &active.id,
                        &self.adapter(None),
                    )?;
                    progress
                        .filter(|progress| progress.complete(execution.input()))
                        .and_then(|progress| {
                            let process = |physical_only: bool| {
                                progress.checkpoints.iter().rev().find_map(|c| match c {
                                    execution_contract::SoftwareCheckpoint::End {
                                        phase,
                                        process: Some(facts),
                                        ..
                                    } if !physical_only
                                        || (!phase.is_observation()
                                            && !matches!(
                                                phase,
                                                execution_contract::SoftwarePhase::Attach
                                                    | execution_contract::SoftwarePhase::Cleanup
                                            )) =>
                                    {
                                        Some(facts)
                                    }
                                    _ => None,
                                })
                            };
                            // Restore installer/reboot facts, using detector capture only when
                            // the complete sequence required no physical mutation.
                            process(true).or_else(|| process(false)).map(|facts| {
                                let mut facts = *facts.clone();
                                facts.total_output_bytes = progress.output_bytes;
                                facts
                            })
                        })
                        .or(stored)
                }
            }
        };
        if let Some(facts) = &capture {
            let host = Host::new(&self.host, &self.binding, &self.config, None);
            self.journal
                .record_process(&Scope::from_input(execution.input()), facts, &host)?;
            if execution
                .snapshot()
                .attempt
                .as_ref()
                .is_some_and(|attempt| attempt.termination.is_none())
                && facts.total_output_bytes > active.output_bytes
            {
                let result = self.command(
                    None,
                    execution,
                    "output",
                    &facts.total_output_bytes.to_string(),
                    Command::Output {
                        attempt_id: active.id.clone(),
                        total_bytes: facts.total_output_bytes,
                    },
                )?;
                if !matches!(
                    result.receipt().outcome,
                    Outcome::Changed | Outcome::Duplicate
                ) {
                    return Err(Error::Conflict);
                }
                *execution = self.load(None, request, ExecutionAccess::RunnerFact)?;
            }
            if facts.finished {
                self.collect_software(execution)?;
                self.runner.acknowledge_capture(execution.input(), facts)?;
            }
        }
        // While the attempt is active, step output is also charged without a whole capture.
        // After termination, bounded recovery output stays in the append-only software journal;
        // it cannot revise the immutable original process exit or lifecycle output total.
        if let Some(progress) = self.journal.software_progress(
            &Scope::from_input(execution.input()),
            &active.id,
            &self.adapter(None),
        )? {
            let charged = execution
                .snapshot()
                .attempt
                .as_ref()
                .ok_or(Error::Conflict)?
                .output_bytes;
            if execution
                .snapshot()
                .attempt
                .as_ref()
                .is_some_and(|attempt| attempt.termination.is_none())
                && progress.output_bytes > charged
            {
                let result = self.command(
                    None,
                    execution,
                    "output",
                    &progress.output_bytes.to_string(),
                    Command::Output {
                        attempt_id: active.id.clone(),
                        total_bytes: progress.output_bytes,
                    },
                )?;
                if !matches!(
                    result.receipt().outcome,
                    Outcome::Changed | Outcome::Duplicate
                ) {
                    return Err(Error::Conflict);
                }
                *execution = self.load(None, request, ExecutionAccess::RunnerFact)?;
            }
        }
        Ok((capture, had_live))
    }
    /// Resume an accepted request only before its first intent. The service independently
    /// authorizes the persisted subject; an existing attempt is never dispatched again.
    pub fn resume_initial(&mut self, request: &RequestId) -> Result<ExecutionStatus, Error> {
        let execution = self.load(None, request, ExecutionAccess::RunnerFact)?;
        let current = self.status_for(None, request, ExecutionAccess::RunnerFact)?;
        if current.attempts > 0 || current.cancel_requested || current.admission.is_some() {
            return Ok(current);
        }
        let input = execution.input();
        self.host.authorize_service(crate::AccessRequest {
            access: crate::Access::Execute,
            scope: &Scope::from_input(input),
            consumer: None,
            interaction: None,
        })?;
        let caller = RequestContext {
            actor: input.spec().request.actor.clone(),
        };
        let spec = crate::execution_confirmation(input);
        match self
            .journal
            .interaction(&Scope::from_input(input), &spec.id, &self.adapter(None))
        {
            Ok(state) if self.host.reliable_now()? >= spec.expires_at_unix_ms => {
                if matches!(
                    state.snapshot().status,
                    execution_interaction::Status::Pending
                ) {
                    self.respond(
                        &caller,
                        request,
                        &CommandId::new("confirmation-expiry")?,
                        &spec.id,
                        &execution_interaction::Command::CheckExpiry {},
                    )?;
                }
                return self.cancel(&caller, request);
            }
            Ok(_) | Err(crate::JournalError::NotFound) => {}
            Err(e) => return Err(e.into()),
        }
        self.request_execution(&caller, input)
    }
    /// Record product-owned effect evidence for the exact quiescent attempt. This trusted
    /// service port is not exposed to IPC/UI; the verifier must authenticate independent
    /// business evidence. It cannot replace termination or manufacture a new attempt.
    pub fn service_assess(
        &mut self,
        request: &RequestId,
        attempt: &AttemptId,
        reference: execution_contract::VersionedRef,
        verifier: &impl execution_lifecycle::ObservationVerifier,
    ) -> Result<bool, Error> {
        let execution = self.load(None, request, ExecutionAccess::RunnerFact)?;
        let active = execution
            .snapshot()
            .attempt
            .as_ref()
            .ok_or(Error::Conflict)?;
        if &active.id != attempt {
            return Err(Error::Conflict);
        }
        if active.termination.is_none() {
            return Ok(false);
        }
        if active.assessment.as_ref().is_some_and(|o| {
            matches!(
                o.observation,
                Observation::Effect {
                    assessment: execution_lifecycle::EffectAssessment::Satisfied
                        | execution_lifecycle::EffectAssessment::NoEffect
                }
            )
        }) {
            return Ok(false);
        }
        self.authorize_runner(execution.input(), Access::RunnerFact)?;
        let evidence = execution_contract::EvidenceRef {
            reference,
            runner: active.runner.clone(),
            kind: if active.mode == ExecutionMode::Real {
                execution_contract::EvidenceKind::StateObserved
            } else {
                execution_contract::EvidenceKind::TestResult
            },
        };
        let now = self.host.reliable_now()?;
        let facts = verifier
            .verify(execution.input(), attempt, &evidence, now)
            .map_err(|_| Error::Unavailable)?;
        execution_lifecycle::ObservationVerifier::verify(
            &ObservationEvidence(&facts),
            execution.input(),
            attempt,
            &evidence,
            now,
        )
        .map_err(|_| Error::InvalidInput)?;
        if !matches!(facts.observation, Observation::Effect { .. }) {
            return Err(Error::InvalidInput);
        }
        if active
            .assessment
            .as_ref()
            .is_some_and(|old| old.observation == facts.observation)
        {
            return Ok(false);
        }
        let result = self.observation(&execution, attempt, &facts)?;
        if result.receipt().outcome == Outcome::Rejected {
            return Err(Error::InvalidInput);
        }
        Ok(true)
    }
    /// Reconcile facts only, at most termination plus assessment. Missing runner memory stays
    /// uncertain and never produces a new attempt or a synthetic successful observation.
    pub fn reconcile(&mut self, request: &RequestId) -> Result<ExecutionStatus, Error> {
        let context = None;
        let mut stop_error = None;
        for _ in 0..2 {
            let mut execution = self.load(context, request, ExecutionAccess::RunnerFact)?;
            if execution.snapshot().attempt.is_none()
                || execution.snapshot().attempt.as_ref().is_some_and(|a| {
                    a.assessment.as_ref().is_some_and(|o| {
                        matches!(
                            o.observation,
                            Observation::Effect {
                                assessment: execution_lifecycle::EffectAssessment::Satisfied
                                    | execution_lifecycle::EffectAssessment::NoEffect
                                    | execution_lifecycle::EffectAssessment::NotSatisfied
                            }
                        )
                    })
                })
            {
                self.collect_software(&execution)?;
                break;
            }
            let (capture, had_live) = self.capture_for(request, &mut execution)?;
            let directive = execution
                .directive(self.host.reliable_now()?)
                .map_err(|_| Error::Clock)?;
            if matches!(directive, Directive::StopRunner(_))
                && !capture.as_ref().is_some_and(|f| f.finished)
            {
                // A stop response is not a termination fact. Even persistence failure must not
                // hide trusted observations that can release the reserved terminal capacity.
                if let Err(error) = self.stop_and_record(&execution) {
                    stop_error = Some(error);
                }
                execution = self.load(context, request, ExecutionAccess::RunnerFact)?;
            }
            let Some(attempt) = execution.snapshot().attempt.as_ref() else {
                break;
            };
            if attempt.assessment.as_ref().is_some_and(|o| {
                !matches!(
                    o.observation,
                    Observation::Effect {
                        assessment: execution_lifecycle::EffectAssessment::Unknown
                    }
                )
            }) {
                break;
            }
            let stage = if attempt.termination.is_some() {
                ObservationStage::Assessment
            } else {
                ObservationStage::Termination
            };
            self.authorize_runner(execution.input(), Access::RunnerFact)?;
            let now = self.host.reliable_now()?;
            let observed = self
                .runner
                .observe(execution.input(), &attempt.id, stage, now)?;
            let program = self
                .journal
                .software_progress(
                    &Scope::from_input(execution.input()),
                    &attempt.id,
                    &self.adapter(None),
                )?
                .filter(|progress| {
                    progress.complete(execution.input())
                        || (progress.closed(execution.input())
                            && (capture.as_ref().is_some_and(|facts| facts.finished)
                                || matches!(
                                    progress.checkpoints.last(),
                                    Some(execution_contract::SoftwareCheckpoint::CleanupEnd {
                                        resources_closed: true,
                                        ..
                                    })
                                ))
                            && (stage == ObservationStage::Termination
                                || attempt.assessment.is_none()))
                })
                .map(|progress| ObservationFacts {
                    request_id: execution.input().spec().request.request_id.clone(),
                    content_digest: execution.input().digest().clone(),
                    attempt_id: attempt.id.clone(),
                    observed_at_unix_ms: now,
                    evidence: execution_contract::EvidenceRef {
                        reference: execution_contract::VersionedRef {
                            id: execution_contract::Id::new(attempt.id.as_str())
                                .expect("attempt id"),
                            revision: execution_contract::Id::new("software-sequence")
                                .expect("constant"),
                        },
                        runner: attempt.runner.clone(),
                        kind: if attempt.mode == ExecutionMode::Test {
                            execution_contract::EvidenceKind::TestResult
                        } else {
                            execution_contract::EvidenceKind::StateObserved
                        },
                    },
                    observation: if stage == ObservationStage::Termination {
                        Observation::Quiescent {}
                    } else {
                        Observation::Effect {
                            assessment: if progress.complete(execution.input()) {
                                execution_lifecycle::EffectAssessment::Satisfied
                            } else {
                                execution_lifecycle::EffectAssessment::Unknown
                            },
                        }
                    },
                });
            let stored =
                if stage == ObservationStage::Termination && attempt.mode == ExecutionMode::Real {
                    capture.as_ref().and_then(|facts| {
                        crate::host::process_observation(execution.input(), facts, now)
                    })
                } else {
                    None
                };
            let Some(facts) = observed.or(stored).or(program) else {
                if attempt.termination.is_none()
                    && (!had_live || capture.as_ref().is_some_and(|f| f.finished))
                    && !matches!(attempt.dispatch, DispatchState::Unknown { .. })
                {
                    self.command(
                        context,
                        &execution,
                        "recover",
                        attempt.id.as_str(),
                        Command::Recover,
                    )?;
                }
                break;
            };
            let result = self.observation(&execution, &attempt.id, &facts)?;
            if result.receipt().outcome == Outcome::Rejected {
                return Err(Error::InvalidInput);
            }
        }
        let status = self.status_for(context, request, ExecutionAccess::RunnerFact)?;
        match stop_error {
            Some(error) => Err(error),
            None => Ok(status),
        }
    }
    /// Persist a cancellation request under Execute only. The independently scheduled owner
    /// reconcile requests stop and records runner facts; this response is not termination evidence.
    pub fn cancel(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<ExecutionStatus, Error> {
        let context = Some(caller);
        for _ in 0..3 {
            let execution = self.load(context, request, ExecutionAccess::Execute)?;
            if !execution.snapshot().cancel_requested {
                let result = self.command(context, &execution, "cancel", "", Command::Cancel)?;
                if result.receipt().outcome == Outcome::Rejected {
                    return Err(Error::Conflict);
                }
                // Return only after a current read confirms durable cancellation.
                continue;
            }
            return self.status_for(context, request, ExecutionAccess::Execute);
        }
        Err(Error::Conflict)
    }
    /// Apply cancellation with the caller-observed version and stable original receipt identity.
    pub fn cancel_control(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        id: &Id,
        expected_revision: u64,
    ) -> Result<Receipt, Error> {
        let execution = self.load(Some(caller), request, ExecutionAccess::Execute)?;
        let op = OperationRequestId::new(key(execution.input(), "task-control", id.as_str())?)
            .map_err(|_| Error::InvalidInput)?;
        let scope = Scope::from_input(execution.input());
        let event = CommandEvent {
            id: EventId::new(op.as_str()).map_err(|_| Error::InvalidInput)?,
            expected_revision,
            command: Command::Cancel,
        };
        let host = Host::new(&self.host, &self.binding, &self.config, Some(caller));
        Ok(self
            .journal
            .apply_command(&op, &scope, &event, &host)?
            .receipt()
            .clone())
    }
    /// Read one original cancellation receipt under current owner permission.
    pub fn control_receipt(
        &self,
        caller: &RequestContext,
        request: &RequestId,
        id: &Id,
    ) -> Result<Option<Receipt>, Error> {
        let execution = self.load(Some(caller), request, ExecutionAccess::Execute)?;
        let op = OperationRequestId::new(key(execution.input(), "task-control", id.as_str())?)
            .map_err(|_| Error::InvalidInput)?;
        Ok(self.journal.execution_receipt(
            &Scope::from_input(execution.input()),
            &op,
            &self.adapter(Some(caller)),
        )?)
    }
    fn stop_and_record(&mut self, execution: &Execution) -> Result<(), Error> {
        let context = None;
        self.authorize_runner(execution.input(), Access::RunnerFact)?;
        let attempt = execution
            .snapshot()
            .attempt
            .as_ref()
            .ok_or(Error::Conflict)?;
        let outcome = if self.runner.stop(execution.input(), &attempt.id).is_ok() {
            StopOutcome::Acknowledged
        } else {
            StopOutcome::Failed
        };
        if attempt.stop_outcome == Some(outcome) {
            return Ok(());
        }
        let result = self.command(
            context,
            execution,
            "stop-result",
            attempt.id.as_str(),
            Command::StopReported {
                attempt_id: attempt.id.clone(),
                outcome,
            },
        )?;
        if matches!(result.receipt().outcome, Outcome::Stale | Outcome::Rejected) {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    fn authorize_runner(&self, plan: &FrozenExecution, access: Access) -> Result<(), Error> {
        let context = None;
        self.adapter(context).authorize(AccessRequest {
            access,
            scope: &Scope::from_input(plan),
            consumer: None,
            interaction: None,
        })?;
        Ok(())
    }
    /// Current authenticated read. Test provenance is never upgraded to real execution success.
    pub fn status(
        &self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<ExecutionStatus, Error> {
        let context = Some(caller);
        self.status_for(context, request, ExecutionAccess::Result)
    }
    /// Authorized coherent task details from one protected record; no privileged audit read.
    pub fn task_details(
        &self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<crate::ExecutionTaskDetails, Error> {
        let context = Some(caller);
        self.details_for(context, request, ExecutionAccess::Result)
    }
    /// Authorized append-only software checkpoints for the original attempt.
    pub fn task_software_progress(
        &self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<Option<SoftwareProgress>, Error> {
        let execution = self.load(Some(caller), request, ExecutionAccess::Result)?;
        let Some(attempt) = &execution.snapshot().attempt else {
            return Ok(None);
        };
        Ok(self.journal.software_progress(
            &Scope::from_input(execution.input()),
            &attempt.id,
            &self.adapter(Some(caller)),
        )?)
    }
    /// Trusted Rust composition read of the original plan. Contains private launch inputs;
    /// UI/model transports must expose only task_details, never serialize this value.
    pub fn frozen_input(
        &self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<FrozenExecution, Error> {
        let context = Some(caller);
        Ok(self
            .load(context, request, ExecutionAccess::Result)?
            .input()
            .clone())
    }
    /// Device-owner read of retained native process evidence. Never serialize raw bytes to UI/model.
    pub fn service_process_evidence(
        &self,
        request: &RequestId,
    ) -> Result<Option<execution_contract::ProcessEvidence>, Error> {
        let execution = self.load(None, request, ExecutionAccess::RunnerFact)?;
        let Some(attempt) = &execution.snapshot().attempt else {
            return Ok(None);
        };
        Ok(self.journal.runner_evidence(
            &Scope::from_input(execution.input()),
            &attempt.id,
            &self.adapter(None),
        )?)
    }
    /// Sole device-owner read of the original frozen recovery input. No UI/model transport
    /// may expose this value; access is checked against the current service binding.
    pub fn service_frozen_input(&self, request: &RequestId) -> Result<FrozenExecution, Error> {
        Ok(self
            .load(None, request, ExecutionAccess::RunnerFact)?
            .input()
            .clone())
    }
    /// Capture a connection-local checkpoint for the exclusive device-owner GC.
    pub fn service_material_checkpoint(&self) -> Result<J::MaterialCheckpoint, Error> {
        Ok(self.journal.material_checkpoint()?)
    }
    /// Continue only proofs whose safety has not been weakened by a journal mutation.
    pub fn service_material_proof_current(
        &self,
        checkpoint: J::MaterialCheckpoint,
    ) -> Result<bool, Error> {
        Ok(self.journal.material_proof_current(checkpoint)?)
    }
    /// Complete authenticated owner scan, without restoring UI details or a second input.
    pub fn service_material_page(
        &self,
        after: Option<&RequestId>,
        limit: usize,
        consumer: &execution_contract::Id,
    ) -> Result<crate::ServiceMaterialPage, Error> {
        let items = self
            .journal
            .material_page(after, limit, consumer, &self.adapter(None))?;
        for item in &items {
            self.check_binding(None, &item.input)?;
        }
        let next = if items.len() == limit {
            items
                .last()
                .map(|i| i.input.spec().request.request_id.clone())
        } else {
            None
        };
        Ok(crate::ServiceMaterialPage { items, next })
    }
    /// Authenticate a newly associated request while a complete-journal scan is in progress.
    pub fn service_material_fact(
        &self,
        request: &RequestId,
        consumer: &execution_contract::Id,
    ) -> Result<crate::ServiceMaterialFact, Error> {
        let fact = self
            .journal
            .material_fact(request, consumer, &self.adapter(None))?;
        self.check_binding(None, &fact.input)?;
        Ok(fact)
    }
    /// Fold receipt-backed local changes into a proof without resetting its history cursor.
    pub fn service_material_changes(
        &self,
        checkpoint: J::MaterialCheckpoint,
        limit: usize,
        consumer: &execution_contract::Id,
    ) -> Result<crate::ServiceMaterialChanges<J::MaterialCheckpoint>, Error> {
        let changes =
            self.journal
                .material_changes(checkpoint, limit, consumer, &self.adapter(None))?;
        for item in &changes.items {
            self.check_binding(None, &item.input)?;
        }
        Ok(changes)
    }
    /// Run one bounded filesystem step behind the authoritative journal's writer fence.
    pub fn with_service_material_proof<T>(
        &self,
        checkpoint: J::MaterialCheckpoint,
        action: impl FnOnce() -> Result<T, Error>,
    ) -> Result<T, Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Denied);
        }
        self.journal.with_material_proof(checkpoint, action)
    }
    /// Service-only exact remote/local association from the original frozen execution owner.
    pub fn service_remote_request(
        &self,
        task: &Id,
        attempt: &Id,
    ) -> Result<Option<RequestId>, Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Denied);
        }
        Ok(self
            .journal
            .remote_request(task, attempt, &self.adapter(None))?)
    }
    /// Exact active-resource lookup for the sole service owner, independent of historical task volume.
    pub fn service_resource_owner(&self, key: &str) -> Result<Option<RequestId>, Error> {
        if key.is_empty() || key.len() > 256 || self.host.service_binding()? != self.binding {
            return Err(Error::Denied);
        }
        let owner = self.journal.resource_owner(key, &self.adapter(None))?;
        if let Some(request) = &owner {
            self.service_frozen_input(request)?;
        }
        Ok(owner)
    }
    /// Internal device-owner recovery page; never exposed as a UI/model operation.
    pub fn service_tasks(
        &self,
        after: Option<&RequestId>,
        limit: usize,
    ) -> Result<crate::TaskPage, Error> {
        if !(1..=128).contains(&limit) {
            return Err(Error::Configuration);
        }
        let requests = self
            .journal
            .service_requests(after, limit, &self.adapter(None))?;
        let page = crate::ExecutionRequestPage {
            next: (requests.len() == limit)
                .then(|| requests.last().expect("nonempty page").clone()),
            requests,
        };
        let items = page
            .requests
            .iter()
            .map(|id| self.details_for(None, id, ExecutionAccess::RunnerFact))
            .collect::<Result<_, _>>()?;
        Ok(crate::TaskPage {
            items,
            next: page.next,
        })
    }
    /// Capture the original journal's discovery boundaries without creating a task.
    pub fn service_task_watermarks(&self) -> Result<(u64, u64), Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Denied);
        }
        Ok(self.journal.task_watermarks()?)
    }
    /// Stable original-task discovery, ordered by first durable receipt rather than mutable activity.
    pub fn service_task_page(
        &self,
        before: Option<u64>,
        limit: usize,
    ) -> Result<crate::RecentTaskPage, Error> {
        if !(1..=128).contains(&limit) || self.host.service_binding()? != self.binding {
            return Err(Error::Denied);
        }
        let page = self
            .journal
            .task_requests(before, limit, &self.adapter(None))?;
        let items = page
            .items
            .into_iter()
            .map(|record| {
                Ok(crate::RecentTask {
                    recorded_at_unix_ms: record.recorded_at_unix_ms,
                    details: self.details_for(
                        None,
                        &record.request,
                        ExecutionAccess::RunnerFact,
                    )?,
                })
            })
            .collect::<Result<_, Error>>()?;
        Ok(crate::RecentTaskPage {
            items,
            next: page.next.map(|v| v.to_string()),
        })
    }
    /// Authorized bounded task page for this exact actor and device.
    pub fn tasks(
        &self,
        caller: &RequestContext,
        after: Option<&RequestId>,
        limit: usize,
    ) -> Result<crate::TaskPage, Error> {
        let context = Some(caller);
        if self.host.service_binding()? != self.binding {
            return Err(Error::Denied);
        }
        let page = self.journal.execution_requests(
            &caller.actor,
            &self.host.binding_for_actor(&caller.actor)?.device,
            after,
            limit,
            &self.adapter(context),
        )?;
        let items = page
            .requests
            .iter()
            .map(|id| self.task_details(caller, id))
            .collect::<Result<_, _>>()?;
        Ok(crate::TaskPage {
            items,
            next: page.next,
        })
    }
    fn status_for(
        &self,
        context: Option<&RequestContext>,
        request: &RequestId,
        access: ExecutionAccess<'_>,
    ) -> Result<ExecutionStatus, Error> {
        self.details_for(context, request, access)
            .map(|details| details.status)
    }
    fn details_for(
        &self,
        context: Option<&RequestContext>,
        request: &RequestId,
        access: ExecutionAccess<'_>,
    ) -> Result<crate::ExecutionTaskDetails, Error> {
        let record = self
            .journal
            .execution_by_request(request, access, &self.adapter(context))?;
        let execution = record.execution;
        self.check_binding(context, execution.input())?;
        let s = execution.snapshot();
        let assessment = s
            .attempt
            .as_ref()
            .and_then(|a| a.assessment.as_ref())
            .and_then(|o| {
                if let Observation::Effect { assessment } = o.observation {
                    Some(assessment)
                } else {
                    None
                }
            });
        let phase = if s.cancel_requested
            && (s.attempt.is_none()
                || assessment == Some(execution_lifecycle::EffectAssessment::NoEffect))
        {
            TaskPhase::Cancelled
        } else {
            match execution.phase() {
                Phase::Received | Phase::Prepared | Phase::Waiting => match record.admission {
                    Some(AdmissionStatus::Denied) => TaskPhase::AdmissionDenied,
                    _ => TaskPhase::Waiting,
                },
                Phase::Starting => TaskPhase::Accepted,
                Phase::Running => TaskPhase::Running,
                Phase::OutcomeUnknown => TaskPhase::OutcomeUnknown,
                Phase::ExecutionEnded => TaskPhase::ExecutionEnded,
                Phase::Verified
                    if assessment == Some(execution_lifecycle::EffectAssessment::Unknown) =>
                {
                    TaskPhase::OutcomeUnknown
                }
                Phase::Verified => TaskPhase::Verified,
                Phase::FailedBeforeDispatch => TaskPhase::FailedBeforeDispatch,
                Phase::Cancelled => TaskPhase::Cancelled,
            }
        };
        let phase = if s.attempt.is_none() && !s.cancel_requested && record.admission.is_none() {
            match self.journal.execution_confirmation_state(
                execution.input(),
                access,
                &self.adapter(context),
            )? {
                Some(i)
                    if matches!(i.snapshot().status, execution_interaction::Status::Pending) =>
                {
                    TaskPhase::ConfirmationRequired
                }
                _ => phase,
            }
        } else {
            phase
        };
        let status = ExecutionStatus {
            process: record.process,
            software: record.software,
            operation_request_id: request.clone(),
            content_digest: s.content_digest.clone(),
            phase,
            mode: s.attempt.as_ref().map_or(self.runner.mode(), |a| a.mode),
            attempt_id: s.attempt.as_ref().map(|a| a.id.clone()),
            attempts: s.attempts,
            cancel_requested: s.cancel_requested,
            admission: record.admission,
            dispatch_cause: s.attempt.as_ref().and_then(|a| a.dispatch_cause),
            stop_outcome: s.attempt.as_ref().and_then(|a| a.stop_outcome),
            assessment,
            evidence: s
                .attempt
                .iter()
                .flat_map(|a| a.termination.iter().chain(a.assessment.iter()))
                .map(|o| o.evidence.clone())
                .collect(),
        };
        Ok(crate::ExecutionTaskDetails {
            recorded_at_unix_ms: s.opened_at_unix_ms,
            revision: s.revision,
            status,
            action: crate::FrozenExecutionSummary::from_input(execution.input()),
        })
    }
}
pub(crate) fn key(plan: &FrozenExecution, stage: &str, identity: &str) -> Result<String, Error> {
    let bytes = serde_json_canonicalizer::to_vec(&(
        "execution-app/v1",
        &plan.spec().request.authority,
        &plan.spec().request.request_id,
        stage,
        identity,
    ))
    .map_err(|_| Error::InvalidInput)?;
    Ok(format!("app-{:x}", Sha256::digest(bytes)))
}
pub(crate) fn operation(
    plan: &FrozenExecution,
    stage: &str,
    identity: &str,
) -> Result<OperationRequestId, Error> {
    Ok(OperationRequestId::new(key(plan, stage, identity)?)?)
}

// A stale receipt does not complete a fact: resubmission at a new revision needs a new operation.
// BeginAttempt alone keeps its immutable identity and does not use this helper.
fn revision_operation(
    execution: &Execution,
    stage: &str,
    identity: &str,
) -> Result<OperationRequestId, Error> {
    let identity = serde_json::to_string(&(identity, execution.snapshot().revision))
        .map_err(|_| Error::InvalidInput)?;
    operation(execution.input(), stage, &identity)
}

impl<H: AppHost, R: RunnerPort, J: JournalPort> ExecutionApp<H, R, J> {
    /// Service-owned bounded recovery: continue pre-intent requests, reconcile existing attempts.
    pub fn reconcile_page(
        &mut self,
        after: Option<&RequestId>,
        limit: usize,
    ) -> Result<Option<RequestId>, Error> {
        let requests = self
            .journal
            .service_requests(after, limit, &self.adapter(None))?;
        for request in &requests {
            self.resume_initial(request)?;
            self.reconcile(request)?;
        }
        Ok(if requests.len() == limit {
            requests.last().cloned()
        } else {
            None
        })
    }
    /// Read durable proof that every accepted attempt is quiescent. Callers must
    /// fence all new dispatch while checking; an unknown effect is not a live process.
    pub fn all_attempts_quiescent(&self, limit: usize) -> Result<bool, Error> {
        if limit == 0 || limit > 128 {
            return Err(Error::InvalidInput);
        }
        let mut after = None;
        loop {
            let requests =
                self.journal
                    .service_requests(after.as_ref(), limit, &self.adapter(None))?;
            for request in &requests {
                let execution = self.load(None, request, ExecutionAccess::RunnerFact)?;
                if let Some(attempt) = execution
                    .snapshot()
                    .attempt
                    .as_ref()
                    .filter(|a| a.termination.is_none())
                {
                    let facts = self.journal.runner_evidence(
                        &Scope::from_input(execution.input()),
                        &attempt.id,
                        &self.adapter(None),
                    )?;
                    if !facts.is_some_and(|f| {
                        f.finished
                            && f.quiescent
                            && f.attempt_id == attempt.id
                            && f.runner == attempt.runner
                            && &f.content_digest == execution.input().digest()
                    }) {
                        return Ok(false);
                    }
                }
            }
            if requests.len() < limit {
                return Ok(true);
            }
            after = requests.last().cloned();
        }
    }
    /// Request stop, await the original physical owners, then retain their final evidence.
    /// Shutdown itself never manufactures termination or effect facts.
    pub fn service_shutdown(&mut self, limit: usize) -> Result<(), Error> {
        let requested = self.stop_active(limit);
        let physical = self.runner.shutdown();
        requested?;
        physical?;
        self.stop_active(limit)
    }
    /// Stop live runners on service shutdown. Missing proof remains Unknown in the same journal.
    pub fn stop_active(&mut self, limit: usize) -> Result<(), Error> {
        self.stop_active_matching(limit, |_| true)
    }
    /// Stop only the product-selected authority domain; all retained journal facts are preserved.
    pub fn stop_active_matching(
        &mut self,
        limit: usize,
        selected: impl Fn(&FrozenExecution) -> bool,
    ) -> Result<(), Error> {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut after = None;
        loop {
            let requests =
                self.journal
                    .service_requests(after.as_ref(), limit, &self.adapter(None))?;
            for request in &requests {
                if std::time::Instant::now() >= until {
                    return Err(Error::Unavailable);
                }
                let execution = self.load(None, request, ExecutionAccess::RunnerFact)?;
                if !selected(execution.input()) {
                    continue;
                }
                if let Some(attempt) = execution
                    .snapshot()
                    .attempt
                    .as_ref()
                    .filter(|a| a.termination.is_none())
                {
                    let facts = self.journal.runner_evidence(
                        &Scope::from_input(execution.input()),
                        &attempt.id,
                        &self.adapter(None),
                    )?;
                    if !facts.is_some_and(|f| f.finished) {
                        self.stop_and_record(&execution)?;
                    }
                    self.reconcile(request)?;
                }
            }
            if requests.len() < limit {
                return Ok(());
            }
            after = requests.last().cloned();
        }
    }
}
