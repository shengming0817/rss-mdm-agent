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
    app: Mutex<ExecutionApp<support::TestHost, DeterministicTestRunner, execution_sqlite::Store>>,
    plan: FrozenExecution,
    caller: RequestContext,
}
impl FixtureService {
    pub fn open(path: &Path) -> Result<Arc<Self>, execution_app::Error> {
        let mut host = support::TestHost::new();
        let mut input = host.template.spec().clone();
        input.request.request_id = RequestId::new("ai-unknown").unwrap();
        let plan = FrozenExecution::freeze(input, &test_execution_limits()).unwrap();
        host.template = plan.clone();
        let caller = RequestContext {
            actor: plan.spec().request.actor.clone(),
        };
        let runner = DeterministicTestRunner::new(
            Id::new("test-runner").unwrap(),
            TestScenario::Unknown,
            16,
        )?;
        let existing = path
            .try_exists()
            .map_err(|_| execution_app::Error::Unavailable)?;
        let app = if existing {
            support::reopen_app(path, host, runner, AppConfig::test_defaults())?
        } else {
            support::create_app(path, host, runner, AppConfig::test_defaults())?
        };
        Ok(Arc::new(Self {
            app: Mutex::new(app),
            plan,
            caller,
        }))
    }
    pub fn policy_descriptor(&self) -> serde_json::Value {
        let item: rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem = serde_json::from_value(serde_json::json!({
            "capability":{"id":"00000000-0000-4000-a000-000000000001","objectType":"policy","toolName":"policy_0000000000004000a000000000000001","alias":null,"scope":"00000000-0000-4000-a000-000000000003","instance":"fixture","tenant":"00000000-0000-4000-a000-000000000004","revision":3,"version":"00000000-0000-4000-a000-000000000002","operations":["invoke"],"inputSchema":{"type":"object","properties":{"name":{"type":"string","maxLength":32}},"required":["name"],"additionalProperties":false},"outputSchema":{}},"policyId":"00000000-0000-4000-a000-000000000001","policyRevision":3,"policyVersion":"00000000-0000-4000-a000-000000000002",
            "subject":{"kind":"device"},
            "selfService":{"published": true, "allowAi": true, "riskLevel": 2, "presentation": {"displayName": "Explicit protocol fixture", "description": "Registration proof only", "prerequisites": "Test assembly", "sideEffects": "No execution", "category": "Test", "keywords": ["test"]}},
            "task":{"kind":"script","runAs":"system","resource":{"id":"fixture-resource","version":"v1","variant":"default"},
            "parameterSources":{"name":{"kind":"input"},"path":{"kind":"fixed","value":"/Applications"}},
            "parameterSchema":{"type":"object","properties":{"name":{"type":"string","maxLength":32},"path":{"type":"string"}},"required":["name","path"],"additionalProperties":false}}
        })).unwrap();
        execution_mcp::policy_tool(&item).unwrap()
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
impl ExecutionServicePort for FixtureService {
    async fn task_operation(
        &self,
        input: execution_app::tasks::TaskOperation,
        _: CancellationToken,
    ) -> Result<execution_app::tasks::TaskResponse, ServiceError> {
        use execution_app::tasks::*;
        let scope = "explicit-s1-task".to_string();
        let mut app = self.app.lock().unwrap();
        let projection = |value: ExecutionTaskDetails| {
            let revision = value.revision;
            let request = value.action.request_id.clone();
            TaskDetail {
                reference: TaskRef::Request { request },
                scope: scope.clone(),
                revision: TaskRevision {
                    owner: revision,
                    transfers: vec![],
                },
                recorded_at: None,
                source: TaskSource::Ai,
                task_kind: TaskKind::Policy,
                stage: "test".into(),
                active: true,
                facts: TaskFacts::Execution(Box::new(value)),
                nodes: vec![],
                allowed_actions: vec![TaskAction::Cancel],
                control_reason: None,
                upload_acknowledged: None,
                collection_quality: None,
                collection_complete: None,
                software_state: None,
                policy: None,
                policy_revision: None,
                retry_of: None,
                related: vec![],
                selection: None,
                missing: vec![],
                parameter_summary: Default::default(),
                human_confirmation: false,
            }
        };
        let receipt = |id: Id, value: Option<execution_app::Receipt>| {
            TaskResponse::Control(TaskControlReceipt {
                id,
                disposition: match value.map(|v| v.outcome) {
                    Some(Outcome::Stale) => ControlDisposition::Stale,
                    Some(Outcome::Rejected) => ControlDisposition::Denied,
                    Some(_) => ControlDisposition::Confirmed,
                    None => ControlDisposition::Unknown,
                },
                created: None,
            })
        };
        match input {
            TaskOperation::List(q) => {
                let page = app
                    .tasks(&self.caller, None, usize::from(q.limit))
                    .map_err(failure)?;
                Ok(TaskResponse::Page(TaskPage {
                    items: page.items.into_iter().map(projection).collect(),
                    next: None,
                    scope,
                    as_of: 1000,
                    last_sync: None,
                    remote_available: false,
                    coverage: vec![],
                    retained_from: None,
                    complete: true,
                }))
            }
            TaskOperation::Detail(TaskRef::Request { request }) => {
                Ok(TaskResponse::Detail(Box::new(projection(
                    app.task_details(&self.caller, &request)
                        .map_err(|_| ServiceError::NotFound)?,
                ))))
            }
            TaskOperation::Control(c) => {
                let TaskRef::Request { request } = c.reference else {
                    return Err(ServiceError::Denied);
                };
                if c.scope != scope || c.action != TaskAction::Cancel {
                    return Err(ServiceError::Denied);
                }
                let value = app
                    .cancel_control(&self.caller, &request, &c.id, &c.expected_revision)
                    .map_err(failure)?;
                Ok(receipt(c.id, Some(value)))
            }
            TaskOperation::ControlStatus(c) => {
                let TaskRef::Request { request } = c.reference else {
                    return Err(ServiceError::Denied);
                };
                if c.scope != scope {
                    return Err(ServiceError::Denied);
                }
                Ok(receipt(
                    c.id.clone(),
                    app.control_receipt(&self.caller, &request, &c.id)
                        .map_err(failure)?,
                ))
            }
            _ => Err(ServiceError::Denied),
        }
    }

    fn bind_call(
        self: &Arc<Self>,
        _: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Arc<Self>, ServiceError> {
        Ok(self.clone())
    }
    fn check_binding(&self) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn policy_tools(
        &self,
        _: execution_mcp::PolicyToolsQuery,
        _: CancellationToken,
    ) -> Result<execution_mcp::PolicyToolPage, ServiceError> {
        Ok(execution_mcp::PolicyToolPage {
            tools: vec![self.policy_descriptor()],
            next: None,
        })
    }
    async fn invoke_policy(
        &self,
        selector: rss_mdm_agent_wire::runtime::CapabilitySelector,
        arguments: execution_mcp::PolicyToolInput,
        _: CancellationToken,
    ) -> Result<BackendRequest, ServiceError> {
        if !matches!(selector, rss_mdm_agent_wire::runtime::CapabilitySelector::ToolName { ref name } if name == self.policy_descriptor()["name"].as_str().unwrap())
            || arguments.parameters
                != serde_json::json!({"name":"Acme"})
                    .as_object()
                    .unwrap()
                    .clone()
        {
            return Err(ServiceError::InvalidInput);
        }
        // S1 proves SDK delivery only. Real backend consent/Start are verified by agent-service.
        Err(ServiceError::Denied)
    }
    async fn offers(&self, _: CancellationToken) -> Result<Vec<BackendTask>, ServiceError> {
        let s = self.selection();
        Ok(vec![BackendTask {
            presentation: execution_contract::BackendPresentation {
                title: "Explicit S1 test task".into(),
                summary: BackendTaskSummary::Script {
                    identity: BackendIdentity::System,
                },
                extra: Default::default(),
            },
            task: s.task,
            attempt: s.attempt,
            revision: s.revision,
            request: s.request,
            expires_at: 9_999_999_999,
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
}
