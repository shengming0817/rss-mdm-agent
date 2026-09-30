//! Test-only host using the real journal and explicitly marked deterministic runner.
#[path = "../../../../../crates/execution-app/tests/support/mod.rs"]
mod support;
use execution_app::*;
use execution_contract::*;
use execution_mcp::*;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

pub struct FixtureService {
    app: Mutex<ExecutionApp<support::TestHost, DeterministicTestRunner>>,
    plan: FrozenExecution,
    caller: RequestContext,
}
impl FixtureService {
    pub fn open(path: &Path) -> Result<Arc<Self>, execution_app::Error> {
        let mut host = support::TestHost::new();
        let mut input = host.template.spec().clone();
        input.request.request_id = RequestId::new("ai-unknown").unwrap();
        let plan = FrozenExecution::freeze(input, &test_store_limits().input).unwrap();
        host.template = plan.clone();
        let caller = RequestContext {
            actor: plan.spec().request.actor.clone(),
        };
        let runner = DeterministicTestRunner::new(
            Id::new("test-runner").unwrap(),
            TestScenario::Unknown,
            16,
        )?;
        let mode = if path
            .try_exists()
            .map_err(|_| execution_app::Error::Unavailable)?
        {
            Startup::OpenTest
        } else {
            Startup::CreateTest
        };
        let app = ExecutionApp::start(path, mode, host, runner, AppConfig::test_defaults(1))?;
        Ok(Arc::new(Self {
            app: Mutex::new(app),
            plan,
            caller,
        }))
    }
    pub fn selection(&self) -> BackendSelection {
        BackendSelection {
            request: self.plan.spec().request.request_id.clone(),
            task: Id::new("fixture-task").unwrap(),
            attempt: Id::new("fixture-attempt").unwrap(),
            revision: self.plan.digest().clone(),
        }
    }
    pub fn details(&self, request: &RequestId) -> Result<ExecutionStatus, execution_app::Error> {
        self.app.lock().unwrap().status(&self.caller, request)
    }
}
fn failure(_: execution_app::Error) -> ServiceError {
    ServiceError::Unavailable
}
fn project(status: ExecutionStatus) -> OperationStatus {
    OperationStatus {
        mode: status.mode,
        process: status.process,
        assessment: status.assessment,
        cancel_requested: status.cancel_requested,
        operation_request_id: status.operation_request_id,
        content_digest: status.content_digest,
        phase: OperationPhase::OutcomeUnknown,
        attempt_id: status.attempt_id,
        evidence: status
            .evidence
            .into_iter()
            .map(|e| e.reference.id)
            .collect(),
    }
}
impl ExecutionServicePort for FixtureService {
    fn bind_call(
        self: &Arc<Self>,
        _: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Arc<Self>, ServiceError> {
        Ok(self.clone())
    }
    fn check_binding(&self) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn tasks(&self, _: CancellationToken) -> Result<Vec<BackendTask>, ServiceError> {
        let s = self.selection();
        Ok(vec![BackendTask {
            summary: BackendTaskSummary::Script {
                identity: BackendIdentity::System,
            },
            task: s.task,
            attempt: s.attempt,
            revision: s.revision,
            request: s.request,
            title: "Explicit S1 test task".into(),
            expires_at: i64::MAX,
            user_initiated: true,
        }])
    }
    async fn capabilities(&self, _: CancellationToken) -> Result<CapabilityView, ServiceError> {
        Ok(CapabilityView {
            state: CapabilityState::Supported,
            reasons: vec![Id::new("test-only").unwrap()],
        })
    }
    async fn execute(
        &self,
        selection: BackendSelection,
        _: CancellationToken,
    ) -> Result<TaskSubmission, ServiceError> {
        if selection != self.selection() {
            return Err(ServiceError::Conflict);
        }
        self.app
            .lock()
            .unwrap()
            .request_execution(&self.caller, &self.plan)
            .map_err(failure)?;
        Ok(TaskSubmission {
            request: selection.request,
            confirmation_required: true,
        })
    }
    async fn status(
        &self,
        request: OperationRequest,
        _: CancellationToken,
    ) -> Result<OperationStatus, ServiceError> {
        self.details(&request.operation_request_id)
            .map(project)
            .map_err(|_| ServiceError::NotFound)
    }
    async fn cancel(
        &self,
        request: OperationRequest,
        _: CancellationToken,
    ) -> Result<CancelResult, ServiceError> {
        let status = self
            .app
            .lock()
            .unwrap()
            .cancel(&self.caller, &request.operation_request_id)
            .map_err(failure)?;
        Ok(CancelResult {
            disposition: CancelDisposition::Requested,
            operation: project(status),
        })
    }
}
