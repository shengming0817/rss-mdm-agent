use crate::{
    host::{capabilities, Host, ObservationEvidence},
    *,
};
use execution_contract::{AttemptId, Authority, EventId, FrozenExecution, RequestId};
use execution_lifecycle::{
    Command, CommandEvent, Directive, DispatchAction, DispatchCause, DispatchState, Execution,
    ExecutionMode, Observation, ObservationEvent, ObservationFacts, Phase, Preparation,
    StopOutcome, StopReason,
};
use execution_sqlite::{
    Access, AccessRequest, AdmissionStatus, CommitOutcome, ExecutionAccess, Host as _, OpenOutcome,
    OperationRequestId, Outcome, Scope, Store,
};
use sha2::{Digest as _, Sha256};
use std::path::Path;

/// Service-owned bounded execution operations. Dropping a client, window or model future does
/// not own this object. The host schedules reconciliation independently of those clients.
/// No method implicitly retries dispatch; a new attempt requires an explicit new command.
pub struct ExecutionApp<H, R> {
    pub(crate) store: Store,
    pub(crate) host: H,
    runner: R,
    pub(crate) binding: ServiceBinding,
    pub(crate) config: Configuration,
}
impl<H: AppHost, R: RunnerPort> ExecutionApp<H, R> {
    /// Bounded pre-execution history for the same authenticated actor.
    pub fn backend_requests(
        &self,
        caller: &RequestContext,
    ) -> Result<Vec<execution_contract::BackendRequest>, Error> {
        if self.host.service_binding()? != self.binding {
            return Err(Error::Unbound);
        }
        Ok(self.store.backend_requests(
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
            authority: self.binding.authority.clone(),
            actor: caller.actor.clone(),
            request_id: request.clone(),
        };
        Ok(self.store.backend_request(
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
            authority: self.binding.authority.clone(),
            actor: caller.actor.clone(),
            request_id: next.offer.request.clone(),
        };
        let host = Host::new(&self.host, &self.binding, &self.config, Some(caller));
        Ok(self
            .store
            .record_backend_request(&scope, expected, next, &host)?)
    }
    /// Assemble the production journal with an independently authenticated host and real runner.
    /// The host must verify registration, OS identity and protected policy before returning its
    /// binding. Opening storage does not authorize a task; all ordinary per-operation gates remain.
    pub fn start_production(
        path: &Path,
        startup: ProductionStartup,
        host: H,
        runner: R,
        config: AppConfig,
        limits: execution_sqlite::Limits,
    ) -> Result<Self, Error> {
        let config = Configuration::new(config, limits.input)?;
        let binding = host.service_binding()?;
        if matches!(binding.authority, Authority::Test { .. })
            || runner.mode() != ExecutionMode::Real
        {
            return Err(Error::Unbound);
        }
        host.reliable_now()?;
        let store = match startup {
            ProductionStartup::Create => {
                Store::initialize_production(path, binding.authority.clone(), limits)?
            }
            ProductionStartup::Open => match Store::open(path, &binding.authority, limits)? {
                OpenOutcome::Ready(store) => *store,
                OpenOutcome::UnsupportedSchema { found, supported } => {
                    return Err(Error::UnsupportedSchema { found, supported });
                }
            },
        };
        Ok(Self {
            store,
            host,
            runner,
            binding,
            config,
        })
    }
    /// Explicit bootstrap/open modes. S1 rejects production without touching storage; it never
    /// selects a fixture authority as fallback. Open never creates or repairs a missing database.
    pub fn start(
        path: &Path,
        startup: Startup,
        host: H,
        runner: R,
        config: AppConfig,
    ) -> Result<Self, Error> {
        let config = Configuration::new(config, test_store_limits().input)?;
        let binding = host.service_binding()?;
        if !matches!(binding.authority, Authority::Test { .. })
            || runner.mode() != ExecutionMode::Test
        {
            return Err(Error::Unbound);
        }
        let store = match startup {
            Startup::CreateTest => {
                Store::initialize_test(path, binding.authority.clone(), test_store_limits())?
            }
            Startup::OpenTest => {
                match Store::open(path, &binding.authority, test_store_limits())? {
                    OpenOutcome::Ready(store) => *store,
                    OpenOutcome::UnsupportedSchema { found, supported } => {
                        return Err(Error::UnsupportedSchema { found, supported })
                    }
                }
            }
        };
        Ok(Self {
            store,
            host,
            runner,
            binding,
            config,
        })
    }
    pub(crate) fn adapter<'a>(
        &'a self,
        context: Option<&'a RequestContext>,
        plan: Option<&'a FrozenExecution>,
    ) -> Host<'a, H> {
        Host::new(&self.host, &self.binding, &self.config, context).with_input(plan)
    }
    fn check_binding(
        &self,
        context: Option<&RequestContext>,
        plan: &FrozenExecution,
    ) -> Result<(), Error> {
        let actual = self.host.service_binding()?;
        let p = &plan.spec().request;
        if actual != self.binding
            || p.authority != actual.authority
            || context.is_some_and(|caller| p.actor != caller.actor)
            || p.target.device != actual.device
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
            .store
            .execution_by_request(request, access, &self.adapter(context, None))?
            .execution;
        self.check_binding(context, execution.input())?;
        Ok(execution)
    }
    /// Accept a single immutable execution. Exact replays continue only before the first intent.
    /// Check current host binding, creation access, configuration and capabilities without
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
        capabilities(&self.host, plan, self.config.active()?)?;
        Ok(())
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
        let host =
            Host::new(&self.host, &self.binding, &self.config, Some(caller)).with_input(Some(plan));
        self.store.open_execution(&op, plan, &host)?;
        let current = self.status_for(Some(caller), request, ExecutionAccess::Submission)?;
        if current.attempts > 0 || current.cancel_requested || current.admission.is_some() {
            return Ok(current);
        }
        let attempt = AttemptId::new(key(plan, "attempt", CommandId::initial_attempt().as_str())?)
            .map_err(|_| Error::InvalidInput)?;
        let decision = execution_admission::decide(
            plan,
            &attempt,
            &self.host,
            execution_admission::AdmissionLimits {
                max_rules: self.config.active()?.max_rules,
            },
        );
        if decision.execution_gate() == execution_admission::ExecutionGate::Confirmation {
            let spec = execution_sqlite::execution_confirmation(plan);
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
        match self.advance(caller, request, &CommandId::initial_attempt()) {
            Err(Error::Conflict) => {
                // An identical first submission can commit while preflight is outside the
                // transaction. Return only that exact original intent; never mint a retry.
                let current =
                    self.status_for(Some(caller), request, ExecutionAccess::Submission)?;
                if current.attempt_id.as_ref() == Some(&attempt) {
                    Ok(current)
                } else {
                    Err(Error::Conflict)
                }
            }
            result => result,
        }
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
        let spec = execution_sqlite::execution_confirmation(&input);
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
    /// Explicit owner action, distinct from submit replay. Every new attempt rechecks capabilities,
    /// C07 and C08 against current trusted facts and atomically consumes all required approvals.
    pub(crate) fn advance(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        command: &CommandId,
    ) -> Result<ExecutionStatus, Error> {
        let context = Some(caller);
        let config = self.config.active()?;
        let mut execution = self.load(context, request, ExecutionAccess::Execute)?;
        let op = operation(execution.input(), "begin", command.as_str())?;
        if let Some(receipt) = self.store.execution_receipt(
            &Scope::from_input(execution.input()),
            &op,
            &self.adapter(context, None),
        )? {
            return if receipt.outcome == Outcome::Stale {
                Err(Error::Conflict)
            } else {
                self.status_for(context, request, ExecutionAccess::Execute)
            };
        }
        capabilities(&self.host, execution.input(), config)?;
        if execution.snapshot().attempt.is_none()
            && execution.snapshot().preparation != Preparation::Prepared
        {
            self.command(
                context,
                &execution,
                "prepare",
                command.as_str(),
                Command::Prepare,
                &[],
            )?;
            execution = self.load(context, request, ExecutionAccess::Execute)?;
        }
        if !matches!(
            execution
                .directive(self.host.reliable_now()?)
                .map_err(|_| Error::Clock)?,
            Directive::Ready | Directive::RetryEligible
        ) {
            return Err(Error::Conflict);
        }
        let scope = Scope::from_input(execution.input());
        let revision = self
            .store
            .trust_revision(&scope, &self.adapter(context, Some(execution.input())))?;
        let refresh = operation(
            execution.input(),
            "trust",
            &format!("{}:{revision:?}", command.as_str()),
        )?;
        let host = Host::new(&self.host, &self.binding, &self.config, context)
            .with_input(Some(execution.input()));
        self.store
            .refresh_trust(&refresh, &scope, revision, &host)?;
        let bindings = self.host.approval_bindings(execution.input())?;
        let attempt = AttemptId::new(key(execution.input(), "attempt", command.as_str())?)
            .map_err(|_| Error::InvalidInput)?;
        let result = self.command(
            context,
            &execution,
            "begin",
            command.as_str(),
            Command::BeginAttempt {
                attempt_id: attempt,
                runner: self.runner.id(),
                mode: self.runner.mode(),
            },
            &bindings,
        )?;
        if result.receipt().outcome == Outcome::Rejected {
            return self.status_for(context, request, ExecutionAccess::Execute);
        }
        if result.receipt().outcome == Outcome::Stale {
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
    fn dispatch(&mut self, request: &RequestId, action: DispatchAction) -> Result<(), Error> {
        let context = None;
        let execution = self.load(context, request, ExecutionAccess::RunnerFact)?;
        // Capability verification can take time or overlap another service handle's cancellation.
        // Re-read the protected stop facts and clock after it, immediately before first delivery.
        let capability = self
            .config
            .active()
            .and_then(|config| capabilities(&self.host, execution.input(), config));
        let execution = self.load(context, request, ExecutionAccess::RunnerFact)?;
        let attempt = action.attempt_id().clone();
        let current = execution.snapshot();
        let gate = if let Err(error) = capability {
            Some(match error {
                Error::Clock => DispatchCause::ClockUnavailable,
                Error::Degraded | Error::Configuration => DispatchCause::ConfigurationUnavailable,
                Error::Denied | Error::Unbound => DispatchCause::AuthorityUnavailable,
                _ => DispatchCause::CapabilityUnavailable,
            })
        } else if current.cancel_requested {
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
                ownership: self.store.software_ownership(
                    &Scope::from_input(execution.input()),
                    &self.adapter(None, Some(execution.input())),
                )?,
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
            &[],
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
        bindings: &[execution_approval::ProfileApproval],
    ) -> Result<CommitOutcome, Error> {
        let plan = execution.input();
        let scope = Scope::from_input(plan);
        let op = if matches!(command, Command::BeginAttempt { .. }) {
            operation(plan, stage, identity)?
        } else {
            revision_operation(execution, stage, identity)?
        };
        let host =
            Host::new(&self.host, &self.binding, &self.config, context).with_input(Some(plan));
        let event = CommandEvent {
            id: EventId::new(op.as_str()).map_err(|_| Error::InvalidInput)?,
            expected_revision: execution.snapshot().revision,
            command,
        };
        Ok(self
            .store
            .apply_command(&op, &scope, &event, bindings, &host)?)
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
        let host =
            Host::new(&self.host, &self.binding, &self.config, context).with_input(Some(plan));
        Ok(self.store.apply_observation(
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
        if let Some(previous) = self.store.software_progress(
            &scope,
            &active.id,
            &self.adapter(None, Some(execution.input())),
        )? {
            if let Some(facts) = self
                .runner
                .recover_software_progress(execution.input(), &previous)?
            {
                let host = Host::new(&self.host, &self.binding, &self.config, None)
                    .with_input(Some(execution.input()));
                self.store.record_software_progress(&scope, &facts, &host)?;
            }
        }
        if !execution.snapshot().cancel_requested && active.termination.is_none() {
            if let Some(progress) = self.store.software_progress(
                &scope,
                &active.id,
                &self.adapter(None, Some(execution.input())),
            )? {
                if progress.resumable(execution.input()) {
                    if let Ok(allowance) = execution.allowance(self.host.reliable_now()?) {
                        if allowance.remaining_timeout_ms > 0
                            && allowance.remaining_output_bytes > 0
                        {
                            self.runner.resume_software(crate::SoftwareResume {
                                ownership: self.store.software_ownership(
                                    &scope,
                                    &self.adapter(None, Some(execution.input())),
                                )?,
                                plan: execution.input().clone(),
                                progress,
                                allowance,
                            })?;
                        }
                    }
                }
            }
        }
        if let Some(progress) = self.store.software_progress(
            &scope,
            &active.id,
            &self.adapter(None, Some(execution.input())),
        )? {
            if progress.cleanup_allowance(execution.input()).is_some() {
                self.runner
                    .resume_software_cleanup(crate::SoftwareCleanupResume {
                        plan: execution.input().clone(),
                        progress,
                    })?;
            }
        }
        let host = Host::new(&self.host, &self.binding, &self.config, None)
            .with_input(Some(execution.input()));
        if let Some(facts) = self
            .runner
            .software_progress(execution.input(), &active.id)?
        {
            let receipt = self.store.record_software_progress(&scope, &facts, &host)?;
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
                let stored = self.store.runner_evidence(
                    &Scope::from_input(execution.input()),
                    &active.id,
                    &self.adapter(None, Some(execution.input())),
                )?;
                if stored.as_ref().is_some_and(|f| f.finished) {
                    stored
                } else {
                    let progress = self.store.software_progress(
                        &Scope::from_input(execution.input()),
                        &active.id,
                        &self.adapter(None, Some(execution.input())),
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
            let host = Host::new(&self.host, &self.binding, &self.config, None)
                .with_input(Some(execution.input()));
            self.store
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
                    &[],
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
        if let Some(progress) = self.store.software_progress(
            &Scope::from_input(execution.input()),
            &active.id,
            &self.adapter(None, Some(execution.input())),
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
                    &[],
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
        self.host
            .authorize_service(execution_sqlite::AccessRequest {
                access: execution_sqlite::Access::Execute,
                scope: &Scope::from_input(input),
                consumer: None,
                interaction: None,
            })?;
        let caller = RequestContext {
            actor: input.spec().request.actor.clone(),
        };
        let spec = execution_sqlite::execution_confirmation(input);
        match self.store.interaction(
            &Scope::from_input(input),
            &spec.id,
            &self.adapter(None, None),
        ) {
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
            Ok(_) | Err(execution_sqlite::Error::NotFound) => {}
            Err(e) => return Err(e.into()),
        }
        self.request_execution(&caller, input)
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
                .store
                .software_progress(
                    &Scope::from_input(execution.input()),
                    &attempt.id,
                    &self.adapter(None, Some(execution.input())),
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
            let Some(facts) = program.or(observed).or(stored) else {
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
                        &[],
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
                let result =
                    self.command(context, &execution, "cancel", "", Command::Cancel, &[])?;
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
            &[],
        )?;
        if matches!(result.receipt().outcome, Outcome::Stale | Outcome::Rejected) {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    fn authorize_runner(&self, plan: &FrozenExecution, access: Access) -> Result<(), Error> {
        let context = None;
        self.adapter(context, Some(plan)).authorize(AccessRequest {
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
    /// Internal device-owner recovery page; never exposed as a UI/model operation.
    pub fn service_tasks(
        &self,
        after: Option<&RequestId>,
        limit: usize,
    ) -> Result<crate::TaskPage, Error> {
        let page = self.store.device_execution_requests(
            &self.binding.device,
            after,
            limit,
            &self.adapter(None, None),
        )?;
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
        let page = self.store.execution_requests(
            &caller.actor,
            &self.binding.device,
            after,
            limit,
            &self.adapter(context, None),
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
        let record =
            self.store
                .execution_by_request(request, access, &self.adapter(context, None))?;
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
                    Some(AdmissionStatus::ApprovalRequired) => TaskPhase::ApprovalRequired,
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
            match self.store.execution_confirmation_state(
                execution.input(),
                access,
                &self.adapter(context, None),
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
            status,
            action: crate::FrozenExecutionSummary::from_input(execution.input()),
        })
    }
    /// Current service configuration health.
    pub fn configuration_state(&self) -> ConfigState {
        self.config.state()
    }
    /// Trusted owner notification, not a transport/model operation.
    pub fn configuration_load_failed(&mut self, mandatory_revision: u64) {
        self.config.load_failed(mandatory_revision);
    }
    /// Audit an authenticated administrative change before atomic in-memory activation.
    pub fn replace_configuration(
        &mut self,
        caller: &RequestContext,
        next: AppConfig,
    ) -> Result<(), Error> {
        let actual = self.host.service_binding()?;
        if actual != self.binding {
            return Err(Error::Denied);
        }
        self.config.replace(next, &caller.actor, |change| {
            self.host.configuration_change(change)
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

impl<H: AppHost, R: RunnerPort> ExecutionApp<H, R> {
    /// Service-owned bounded recovery: continue pre-intent requests, reconcile existing attempts.
    pub fn reconcile_page(
        &mut self,
        after: Option<&RequestId>,
        limit: usize,
    ) -> Result<Option<RequestId>, Error> {
        let requests = self
            .store
            .service_requests(after, limit, &self.adapter(None, None))?;
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
    /// Stop live runners on service shutdown. Missing proof remains Unknown in the same journal.
    pub fn stop_active(&mut self, limit: usize) -> Result<(), Error> {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut after = None;
        loop {
            let requests =
                self.store
                    .service_requests(after.as_ref(), limit, &self.adapter(None, None))?;
            for request in &requests {
                if std::time::Instant::now() >= until {
                    return Err(Error::Unavailable);
                }
                let execution = self.load(None, request, ExecutionAccess::RunnerFact)?;
                if let Some(attempt) = execution
                    .snapshot()
                    .attempt
                    .as_ref()
                    .filter(|a| a.termination.is_none())
                {
                    let facts = self.store.runner_evidence(
                        &Scope::from_input(execution.input()),
                        &attempt.id,
                        &self.adapter(None, Some(execution.input())),
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
