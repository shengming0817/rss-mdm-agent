//! Runner-owned test assembly. No fixture authority is compiled into production.
use super::*;
use execution_app::{AccessRequest, AdmissionGate, ExecutionGate};
use execution_app::{AppConfig, AppHost, ExecutionApp, RequestContext, ServiceBinding};

pub fn plan() -> FrozenExecution {
    let original = FrozenExecution::freeze(
        decode_execution(
            include_bytes!("../../../execution-contract/tests/fixtures/plan.json"),
            &execution_app::test_execution_limits(),
        )
        .unwrap(),
        &execution_app::test_execution_limits(),
    )
    .unwrap();
    let mut spec = original.spec().clone();
    spec.request.initiator = execution_contract::Initiator::Policy {
        policy: spec.policy.clone(),
    };
    FrozenExecution::freeze(spec, &execution_app::test_execution_limits()).unwrap()
}
#[derive(Clone)]
pub struct TestHost {
    pub template: FrozenExecution,
}
impl TestHost {
    pub fn new() -> Self {
        Self { template: plan() }
    }
    fn now(&self) -> u64 {
        self.template
            .spec()
            .validity
            .not_before_unix_ms
            .saturating_add(1)
            .max(1000)
    }
}

impl AppHost for TestHost {
    fn service_binding(&self) -> Result<ServiceBinding, Error> {
        Ok(ServiceBinding {
            authority: self.template.spec().request.authority.clone(),
            device: self.template.spec().request.target.device.clone(),
        })
    }
    fn authorize(
        &self,
        caller: &RequestContext,
        r: AccessRequest<'_>,
    ) -> Result<(), execution_app::JournalError> {
        if caller.actor != r.scope.actor {
            return Err(execution_app::JournalError::Denied);
        }
        self.authorize_service(r)
    }
    fn authorize_service(&self, r: AccessRequest<'_>) -> Result<(), execution_app::JournalError> {
        if r.scope.authority != self.template.spec().request.authority
            || r.scope.actor != self.template.spec().request.actor
        {
            Err(execution_app::JournalError::Denied)
        } else {
            Ok(())
        }
    }
    fn reliable_now(&self) -> Result<u64, execution_app::JournalError> {
        Ok(self.now())
    }

    fn admit(
        &self,
        plan: &FrozenExecution,
        _: &AttemptId,
        _now: u64,
    ) -> Result<execution_app::AdmissionGate, execution_app::JournalError> {
        if plan.digest() != self.template.digest() {
            return Err(execution_app::JournalError::Denied);
        }
        Ok(AdmissionGate {
            execution_gate: ExecutionGate::Direct,
            valid_until_unix_ms: plan.spec().validity.expires_at_unix_ms,
        })
    }
}
// Explicit fixture-only carrier: retain production's Real/Test authority separation.
#[derive(Clone)]
pub struct TestCarrier(
    pub Arc<NativeRunner>,
    pub Arc<std::sync::atomic::AtomicUsize>,
);
impl RunnerPort for TestCarrier {
    fn software_progress(
        &self,
        p: &FrozenExecution,
        a: &AttemptId,
    ) -> Result<Option<SoftwareProgress>, Error> {
        self.0.software_progress(p, a)
    }
    fn acknowledge_software_progress(
        &self,
        receipt: execution_app::CommittedSoftwareProgress,
    ) -> Result<(), Error> {
        self.0.acknowledge_software_progress(receipt)
    }
    fn id(&self) -> Id {
        self.0.id()
    }
    fn mode(&self) -> ExecutionMode {
        ExecutionMode::Test
    }
    fn dispatch(&self, p: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
        let allowance = p.allowance();
        p.dispatch(|plan, action| {
            if action.mode() != ExecutionMode::Test
                || !matches!(plan.spec().request.authority, Authority::Test { .. })
                || action.runner() != &self.0.id
                || action.content_digest() != plan.digest()
                || action.request_id() != &plan.spec().request.request_id
            {
                return Err(Error::Denied);
            }
            self.1.fetch_add(1, Ordering::SeqCst);
            self.0.launch(plan, action.attempt_id(), allowance)
        })
    }
    fn stop(&self, p: &FrozenExecution, a: &AttemptId) -> Result<(), Error> {
        self.0.stop(p, a)
    }
    fn evidence(
        &self,
        p: &FrozenExecution,
        a: &AttemptId,
    ) -> Result<Option<ProcessEvidence>, Error> {
        self.0.evidence(p, a)
    }
    fn acknowledge_capture(&self, p: &FrozenExecution, f: &ProcessEvidence) -> Result<(), Error> {
        self.0.acknowledge_capture(p, f)
    }
    fn observe(
        &self,
        p: &FrozenExecution,
        a: &AttemptId,
        s: ObservationStage,
        n: u64,
    ) -> Result<Option<ObservationFacts>, Error> {
        self.0.observe(p, a, s, n).map(|facts| {
            facts.map(|mut facts| {
                facts.evidence.kind = EvidenceKind::TestResult;
                facts
            })
        })
    }
}

/// Create a fixture journal, then attach it through the real application port.
pub(super) fn create_app<H: AppHost, R: RunnerPort>(
    path: &std::path::Path,
    host: H,
    runner: R,
    config: AppConfig,
) -> Result<ExecutionApp<H, R, execution_sqlite::Store>, execution_app::Error> {
    let binding = host.service_binding()?;
    let journal = execution_sqlite::Store::initialize_test(
        path,
        binding.authority,
        execution_sqlite::test_store_limits(),
    )?;
    ExecutionApp::new(journal, host, runner, config)
}
/// Reopen a fixture journal without create fallback or first-dispatch reconstruction.
pub(super) fn reopen_app<H: AppHost, R: RunnerPort>(
    path: &std::path::Path,
    host: H,
    runner: R,
    config: AppConfig,
) -> Result<ExecutionApp<H, R, execution_sqlite::Store>, execution_app::Error> {
    let binding = host.service_binding()?;
    let journal = match execution_sqlite::Store::open(
        path,
        &binding.authority,
        execution_sqlite::test_store_limits(),
    )? {
        execution_sqlite::OpenOutcome::Ready(journal) => *journal,
        execution_sqlite::OpenOutcome::UnsupportedSchema { found, supported } => {
            return Err(execution_app::Error::UnsupportedSchema { found, supported });
        }
    };
    ExecutionApp::new(journal, host, runner, config)
}
