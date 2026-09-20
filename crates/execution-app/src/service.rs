use crate::{
    host::{capabilities, Host, ObservationEvidence},
    *,
};
use execution_contract::{AttemptId, Authority, EventId, FrozenPlan, RequestId};
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
    /// Explicit bootstrap/open modes. S1 rejects production without touching storage; it never
    /// selects a fixture authority as fallback. Open never creates or repairs a missing database.
    pub fn start(
        path: &Path,
        startup: Startup,
        host: H,
        runner: R,
        config: AppConfig,
    ) -> Result<Self, Error> {
        let config = Configuration::new(config)?;
        let binding = host.service_binding()?;
        if startup == Startup::Production {
            return Err(if matches!(binding.authority, Authority::Test { .. }) {
                Error::Unbound
            } else {
                Error::Unsupported
            });
        }
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
                    OpenOutcome::NewerSchema { found, supported } => {
                        return Err(Error::NewerSchema { found, supported })
                    }
                }
            }
            Startup::Production => unreachable!(),
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
        plan: Option<&'a FrozenPlan>,
    ) -> Host<'a, H> {
        Host::new(&self.host, &self.binding, &self.config, context).with_plan(plan)
    }
    fn check_binding(
        &self,
        context: Option<&RequestContext>,
        plan: &FrozenPlan,
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
        self.check_binding(context, execution.plan())?;
        Ok(execution)
    }
    /// Persist a preview without submitting it or obtaining dispatch authority.
    pub fn register_plan(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        plan: &FrozenPlan,
    ) -> Result<ExecutionStatus, Error> {
        let context = Some(caller);
        self.check_binding(context, plan)?;
        if request != &plan.spec().request.request_id {
            return Err(Error::Conflict);
        }
        let op = operation(plan, "preview", "")?;
        self.store.open_execution(
            &op,
            plan,
            &Host::new(&self.host, &self.binding, &self.config, context).with_plan(Some(plan)),
        )?;
        self.status_for(context, request, ExecutionAccess::Submission)
    }
    /// Submit an immutable plan under its original business request ID. Replays return current
    /// durable state without consuming approval, checking new execution policy or dispatching.
    pub fn submit(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        plan: &FrozenPlan,
    ) -> Result<ExecutionStatus, Error> {
        let context = Some(caller);
        self.check_binding(context, plan)?;
        if request != &plan.spec().request.request_id {
            return Err(Error::Conflict);
        }
        let operation = operation(plan, "register", "")?;
        let host =
            Host::new(&self.host, &self.binding, &self.config, context).with_plan(Some(plan));
        let result = self.store.open_execution(&operation, plan, &host)?;
        if matches!(result, CommitOutcome::AlreadyCommitted(_)) {
            return self.status_for(context, request, ExecutionAccess::Submission);
        }
        self.advance(caller, request, &CommandId::initial_attempt())
    }
    /// Explicit owner action, distinct from submit replay. Every new attempt rechecks capabilities,
    /// C07 and C08 against current trusted facts and atomically consumes all required approvals.
    pub fn advance(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        command: &CommandId,
    ) -> Result<ExecutionStatus, Error> {
        let context = Some(caller);
        let config = self.config.active()?;
        let mut execution = self.load(context, request, ExecutionAccess::Execute)?;
        let op = operation(execution.plan(), "begin", command.as_str())?;
        if let Some(receipt) = self.store.execution_receipt(
            &Scope::from_plan(execution.plan()),
            &op,
            &self.adapter(context, None),
        )? {
            return if receipt.outcome == Outcome::Stale {
                Err(Error::Conflict)
            } else {
                self.status_for(context, request, ExecutionAccess::Execute)
            };
        }
        capabilities(&self.host, execution.plan(), config)?;
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
        let scope = Scope::from_plan(execution.plan());
        let revision = self
            .store
            .trust_revision(&scope, &self.adapter(context, Some(execution.plan())))?;
        let refresh = operation(
            execution.plan(),
            "trust",
            &format!("{}:{revision:?}", command.as_str()),
        )?;
        let host = Host::new(&self.host, &self.binding, &self.config, context)
            .with_plan(Some(execution.plan()));
        self.store
            .refresh_trust(&refresh, &scope, revision, &host)?;
        let bindings = self.host.approval_bindings(execution.plan())?;
        let attempt = AttemptId::new(key(execution.plan(), "attempt", command.as_str())?)
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
            .and_then(|config| capabilities(&self.host, execution.plan(), config));
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
                    .authorize_runner(execution.plan(), Access::Execute)
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
                action,
                plan: execution.plan().clone(),
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
        let plan = execution.plan();
        let scope = Scope::from_plan(plan);
        let op = if matches!(command, Command::BeginAttempt { .. }) {
            operation(plan, stage, identity)?
        } else {
            revision_operation(execution, stage, identity)?
        };
        let host =
            Host::new(&self.host, &self.binding, &self.config, context).with_plan(Some(plan));
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
        let plan = execution.plan();
        let identity = serde_json::to_string(&facts.evidence).map_err(|_| Error::InvalidInput)?;
        let op = revision_operation(execution, "observe", &identity)?;
        let event = ObservationEvent {
            id: EventId::new(op.as_str()).map_err(|_| Error::InvalidInput)?,
            expected_revision: execution.snapshot().revision,
            attempt_id: attempt_id.clone(),
            evidence: facts.evidence.clone(),
        };
        let host =
            Host::new(&self.host, &self.binding, &self.config, context).with_plan(Some(plan));
        Ok(self.store.apply_observation(
            &op,
            &Scope::from_plan(plan),
            &event,
            &host,
            &ObservationEvidence(facts),
        )?)
    }
    /// Reconcile facts only, at most termination plus assessment. Missing runner memory stays
    /// uncertain and never produces a new attempt or a synthetic successful observation.
    pub fn reconcile(&mut self, request: &RequestId) -> Result<ExecutionStatus, Error> {
        let context = None;
        let mut stop_error = None;
        for _ in 0..2 {
            let mut execution = self.load(context, request, ExecutionAccess::RunnerFact)?;
            if execution.snapshot().attempt.is_none() {
                break;
            }
            let directive = execution
                .directive(self.host.reliable_now()?)
                .map_err(|_| Error::Clock)?;
            if matches!(directive, Directive::StopRunner(_)) {
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
            self.authorize_runner(execution.plan(), Access::RunnerFact)?;
            let Some(facts) = self.runner.observe(
                execution.plan(),
                &attempt.id,
                stage,
                self.host.reliable_now()?,
            )?
            else {
                if attempt.termination.is_none()
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
        self.authorize_runner(execution.plan(), Access::RunnerFact)?;
        let attempt = execution
            .snapshot()
            .attempt
            .as_ref()
            .ok_or(Error::Conflict)?;
        let outcome = if self.runner.stop(execution.plan(), &attempt.id).is_ok() {
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
    fn authorize_runner(&self, plan: &FrozenPlan, access: Access) -> Result<(), Error> {
        let context = None;
        self.adapter(context, Some(plan)).authorize(AccessRequest {
            access,
            scope: &Scope::from_plan(plan),
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
    pub fn frozen_plan(
        &self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<FrozenPlan, Error> {
        let context = Some(caller);
        Ok(self
            .load(context, request, ExecutionAccess::Result)?
            .plan()
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
        self.check_binding(context, execution.plan())?;
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
                Phase::Verified => TaskPhase::TestCompleted,
                Phase::FailedBeforeDispatch => TaskPhase::FailedBeforeDispatch,
                Phase::Cancelled => TaskPhase::Cancelled,
            }
        };
        let status = ExecutionStatus {
            submitted: self.store.has_execution_receipt(
                &Scope::from_plan(execution.plan()),
                &operation(execution.plan(), "register", "")?,
                access,
                &self.adapter(context, Some(execution.plan())),
            )?,
            operation_request_id: request.clone(),
            plan_id: s.plan_id.clone(),
            plan_digest: s.plan_digest.clone(),
            phase,
            mode: s.attempt.as_ref().map_or(ExecutionMode::Test, |a| a.mode),
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
            plan: crate::FrozenPlanSummary::from_plan(execution.plan()),
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
pub(crate) fn key(plan: &FrozenPlan, stage: &str, identity: &str) -> Result<String, Error> {
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
    plan: &FrozenPlan,
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
    operation(execution.plan(), stage, &identity)
}
