use crate::{
    host::{capabilities, Host},
    *,
};
use execution_contract::{AttemptId, Authority, EventId, FrozenPlan, RequestId};
use execution_lifecycle::{
    Command, Directive, DispatchAction, DispatchState, Event, Execution, ExecutionMode,
    Observation, ObservationFacts, Phase, Preparation,
};
use execution_sqlite::{CommitOutcome, OpenOutcome, OperationRequestId, Outcome, Scope, Store};
use sha2::{Digest as _, Sha256};
use std::path::Path;

/// Service-owned bounded execution operations. Dropping a client, window or model future does
/// not own this object. The host schedules reconciliation independently of those clients.
/// No method implicitly retries dispatch; a new attempt requires an explicit new command.
pub struct ExecutionApp<H, R> {
    pub(crate) store: Store,
    pub(crate) host: H,
    runner: R,
    pub(crate) binding: Binding,
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
        let binding = host.binding()?;
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
                    OpenOutcome::NewerSchema { .. } => return Err(Error::Storage),
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
    pub(crate) fn adapter<'a>(&'a self, plan: Option<&'a FrozenPlan>) -> Host<'a, H> {
        Host {
            inner: &self.host,
            binding: &self.binding,
            config: self.config.active().ok(),
            plan,
            observation: None,
        }
    }
    fn check_binding(&self, plan: &FrozenPlan) -> Result<(), Error> {
        let actual = self.host.binding()?;
        let p = &plan.spec().request;
        if actual != self.binding
            || p.authority != actual.authority
            || p.actor != actual.actor
            || p.target.device != actual.device
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    pub(crate) fn load(&self, request: &RequestId) -> Result<Execution, Error> {
        let execution = self
            .store
            .execution_by_request(request, &self.adapter(None))?;
        self.check_binding(execution.plan())?;
        Ok(execution)
    }
    /// Submit an immutable plan under its original business request ID. Replays return current
    /// durable state without consuming approval, checking new execution policy or dispatching.
    pub fn submit(
        &mut self,
        request: &RequestId,
        plan: &FrozenPlan,
    ) -> Result<ExecutionStatus, Error> {
        self.check_binding(plan)?;
        if request != &plan.spec().request.request_id {
            return Err(Error::Conflict);
        }
        let operation = operation(plan, "register", "")?;
        let host = Host {
            inner: &self.host,
            binding: &self.binding,
            config: self.config.active().ok(),
            plan: Some(plan),
            observation: None,
        };
        let result = self.store.open_execution(&operation, plan, &host)?;
        if matches!(result, CommitOutcome::AlreadyCommitted(_)) {
            return self.status(request);
        }
        self.advance(request, &RequestId::new("initial").expect("static ID"))
    }
    /// Explicit owner action, distinct from submit replay. Every new attempt rechecks capabilities,
    /// C07 and C08 against current trusted facts and atomically consumes all required approvals.
    pub fn advance(
        &mut self,
        request: &RequestId,
        command: &RequestId,
    ) -> Result<ExecutionStatus, Error> {
        let config = self.config.active()?;
        let mut execution = self.load(request)?;
        let op = operation(execution.plan(), "begin", command.as_str())?;
        if let Some(receipt) = self.store.receipt(
            &Scope::from_plan(execution.plan()),
            &op,
            &self.adapter(None),
        )? {
            return if receipt.outcome == Outcome::Rejected {
                Err(Error::AdmissionRejected)
            } else {
                self.status(request)
            };
        }
        capabilities(&self.host, execution.plan(), config)?;
        if execution.snapshot().attempt.is_none()
            && execution.snapshot().preparation != Preparation::Prepared
        {
            self.event(
                &execution,
                "prepare",
                command.as_str(),
                Command::Prepare,
                &[],
                None,
            )?;
            execution = self.load(request)?;
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
            .trust_revision(&scope, &self.adapter(Some(execution.plan())))?;
        let refresh = operation(
            execution.plan(),
            "trust",
            &format!("{}:{revision:?}", command.as_str()),
        )?;
        let host = Host {
            inner: &self.host,
            binding: &self.binding,
            config: Some(config),
            plan: Some(execution.plan()),
            observation: None,
        };
        self.store
            .refresh_trust(&refresh, &scope, revision, &host)?;
        let bindings = self.host.approval_bindings(execution.plan())?;
        let attempt = AttemptId::new(key(execution.plan(), "attempt", command.as_str())?)
            .map_err(|_| Error::InvalidInput)?;
        let result = self.event(
            &execution,
            "begin",
            command.as_str(),
            Command::BeginAttempt {
                attempt_id: attempt,
                runner: self.runner.id(),
                mode: self.runner.mode(),
            },
            &bindings,
            None,
        )?;
        if result.receipt().outcome == Outcome::Rejected {
            return Err(Error::AdmissionRejected);
        }
        if let CommitOutcome::Applied {
            first_dispatch: Some(action),
            ..
        } = result
        {
            self.dispatch(request, action)?;
        }
        self.status(request)
    }
    fn dispatch(&mut self, request: &RequestId, action: DispatchAction) -> Result<(), Error> {
        let execution = self.load(request)?;
        let attempt = action.attempt_id().clone();
        let current = execution.snapshot();
        let gated = current.revision == action.committed_revision()
            && !current.cancel_requested
            && action.runner() == &self.runner.id()
            && action.mode() == self.runner.mode()
            && execution
                .directive(self.host.reliable_now()?)
                .map_err(|_| Error::Clock)?
                == Directive::Reconcile
            && capabilities(&self.host, execution.plan(), self.config.active()?).is_ok();
        let outcome = if gated {
            self.runner.dispatch(AuthorizedDispatch {
                action,
                plan: execution.plan().clone(),
            })
        } else {
            drop(action);
            Ok(DispatchOutcome::OutcomeUnknown)
        };
        let command = if matches!(outcome, Ok(DispatchOutcome::Accepted)) {
            Command::Dispatched {
                attempt_id: attempt.clone(),
            }
        } else {
            Command::Recover
        };
        self.event(
            &execution,
            "dispatch-result",
            attempt.as_str(),
            command,
            &[],
            None,
        )?;
        Ok(())
    }
    fn event(
        &mut self,
        execution: &Execution,
        stage: &str,
        identity: &str,
        command: Command,
        bindings: &[execution_approval::ProfileApproval],
        observation: Option<&ObservationFacts>,
    ) -> Result<CommitOutcome, Error> {
        let plan = execution.plan();
        let scope = Scope::from_plan(plan);
        let op = operation(plan, stage, identity)?;
        let host = Host {
            inner: &self.host,
            binding: &self.binding,
            config: self.config.active().ok(),
            plan: Some(plan),
            observation,
        };
        if let Some(receipt) = self.store.receipt(&scope, &op, &host)? {
            return Ok(CommitOutcome::AlreadyCommitted(receipt));
        }
        let event = Event {
            id: EventId::new(op.as_str()).map_err(|_| Error::InvalidInput)?,
            expected_revision: execution.snapshot().revision,
            command,
        };
        Ok(self
            .store
            .apply_execution(&op, &scope, &event, bindings, &host)?)
    }
    /// Reconcile facts only, at most termination plus assessment. Missing runner memory stays
    /// uncertain and never produces a new attempt or a synthetic successful observation.
    pub fn reconcile(&mut self, request: &RequestId) -> Result<ExecutionStatus, Error> {
        for _ in 0..2 {
            let execution = self.load(request)?;
            let Some(attempt) = execution.snapshot().attempt.as_ref() else {
                break;
            };
            let directive = execution
                .directive(self.host.reliable_now()?)
                .map_err(|_| Error::Clock)?;
            if matches!(directive, Directive::StopRunner(_)) {
                self.runner.stop(execution.plan(), &attempt.id)?;
            }
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
                    self.event(
                        &execution,
                        "recover",
                        attempt.id.as_str(),
                        Command::Recover,
                        &[],
                        None,
                    )?;
                }
                break;
            };
            let identity =
                serde_json::to_string(&facts.evidence).map_err(|_| Error::InvalidInput)?;
            let result = self.event(
                &execution,
                "observe",
                &identity,
                Command::Observe {
                    attempt_id: attempt.id.clone(),
                    evidence: facts.evidence.clone(),
                },
                &[],
                Some(&facts),
            )?;
            if result.receipt().outcome == Outcome::Rejected {
                return Err(Error::InvalidInput);
            }
        }
        self.status(request)
    }
    /// Persist cancellation before requesting stop. Acknowledgement is not termination evidence.
    pub fn cancel(&mut self, request: &RequestId) -> Result<ExecutionStatus, Error> {
        let execution = self.load(request)?;
        self.event(&execution, "cancel", "", Command::Cancel, &[], None)?;
        if let Some(attempt) = &execution.snapshot().attempt {
            self.runner.stop(execution.plan(), &attempt.id)?;
        }
        self.status(request)
    }
    /// Current authenticated read. Test provenance is never upgraded to real execution success.
    pub fn status(&self, request: &RequestId) -> Result<ExecutionStatus, Error> {
        let execution = self.load(request)?;
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
                Phase::Received | Phase::Prepared | Phase::Waiting => TaskPhase::Waiting,
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
        Ok(ExecutionStatus {
            operation_request_id: request.clone(),
            plan_id: s.plan_id.clone(),
            plan_digest: s.plan_digest.clone(),
            phase,
            mode: s.attempt.as_ref().map_or(ExecutionMode::Test, |a| a.mode),
            attempt_id: s.attempt.as_ref().map(|a| a.id.clone()),
            attempts: s.attempts,
            cancel_requested: s.cancel_requested,
            assessment,
            evidence: s
                .attempt
                .iter()
                .flat_map(|a| a.termination.iter().chain(a.assessment.iter()))
                .map(|o| o.evidence.clone())
                .collect(),
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
    pub fn replace_configuration(&mut self, next: AppConfig) -> Result<(), Error> {
        let actual = self.host.binding()?;
        if actual != self.binding {
            return Err(Error::Denied);
        }
        self.config.replace(next, &actual.actor, |change| {
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
