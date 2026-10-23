//! Explicit test-only service exercising transport and immutable backend-reference semantics.
use execution_contract::{
    AttemptId, BackendIdentity, BackendSelection, BackendTask, BackendTaskSummary, Digest, Id,
    RequestId, TaskSubmission,
};
use execution_mcp::*;
use sha2::{Digest as _, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;
pub fn limits() -> McpLimits {
    McpLimits {
        frame_bytes: 64 * 1024,
        response_bytes: 256 * 1024,
        json_depth: 32,
        json_nodes: 8192,
        in_flight: 8,
        session_frames: 4096,
        request_timeout: Duration::from_secs(2),
        io_timeout: Duration::from_secs(5),
    }
}
#[derive(Default)]
pub struct TestStore {
    software: HashMap<(TestNamespace, RequestId), LocalSoftwareResult>,
    queries: HashMap<(TestNamespace, RequestId), execution_contract::QuerySubmit>,
    query_cancellations: HashSet<(TestNamespace, RequestId)>,
    accepted: HashMap<(TestNamespace, RequestId), OperationStatus>,
    inputs: HashMap<(TestNamespace, RequestId), BackendSelection>,
    cancellations: HashSet<(TestNamespace, RequestId)>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TestNamespace {
    pub authority: String,
    pub tenant: String,
    pub actor: String,
    pub device: String,
    pub delegation: String,
}
pub struct TestService {
    pub store: Arc<Mutex<TestStore>>,
    pub namespace: TestNamespace,
    pub bound: AtomicBool,
    pub denied: AtomicBool,
    pub attempts: AtomicUsize,
    pub calls: AtomicUsize,
    pub active_waits: AtomicUsize,
    pub submit_delay_ms: AtomicUsize,
    pub capability_delay_ms: AtomicUsize,
    pub catalog_delay_ms: AtomicUsize,
    pub status_delay_ms: AtomicUsize,
}
struct WaitGuard<'a>(&'a AtomicUsize);
impl Drop for WaitGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl TestService {
    pub fn new() -> Self {
        Self {
            store: Default::default(),
            namespace: TestNamespace {
                authority: "test-authority".into(),
                tenant: "test-tenant".into(),
                actor: "actor-1".into(),
                device: "device-1".into(),
                delegation: "delegation-1".into(),
            },
            bound: AtomicBool::new(true),
            denied: AtomicBool::new(false),
            attempts: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
            active_waits: AtomicUsize::new(0),
            submit_delay_ms: AtomicUsize::new(0),
            capability_delay_ms: AtomicUsize::new(0),
            catalog_delay_ms: AtomicUsize::new(0),
            status_delay_ms: AtomicUsize::new(0),
        }
    }
    fn authorize(&self) -> Result<(), ServiceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.check_binding()?;
        if self.denied.load(Ordering::SeqCst) {
            Err(ServiceError::Denied)
        } else {
            Ok(())
        }
    }
    fn key(&self, id: &RequestId) -> (TestNamespace, RequestId) {
        (self.namespace.clone(), id.clone())
    }
    pub fn software_result(&self, value: LocalSoftwareResult) {
        self.store
            .lock()
            .unwrap()
            .software
            .insert(self.key(&value.prepared.request), value);
    }
    pub fn complete_test_result(&self, id: &str) {
        let mut db = self.store.lock().unwrap();
        let value = db
            .accepted
            .get_mut(&self.key(&RequestId::new(id).unwrap()))
            .unwrap();
        value.phase = OperationPhase::Verified;
        value.assessment = Some(execution_lifecycle::EffectAssessment::Satisfied);
        value.evidence = vec![Id::new("test-result").unwrap()];
    }
    fn capability() -> CapabilityView {
        CapabilityView {
            state: CapabilityState::Supported,
            reasons: vec![Id::new("test-only").unwrap()],
        }
    }
}
impl ExecutionServicePort for TestService {
    async fn collected_data(
        &self,
        input: execution_contract::DataOperation,
        _: CancellationToken,
    ) -> Result<execution_contract::DataResponse, ServiceError> {
        self.authorize()?;
        let delay = if matches!(
            input,
            execution_contract::DataOperation::Submit(_)
                | execution_contract::DataOperation::Cancel(_)
        ) {
            &self.submit_delay_ms
        } else {
            &self.catalog_delay_ms
        };
        tokio::time::sleep(Duration::from_millis(delay.load(Ordering::SeqCst) as u64)).await;
        let cancelling = matches!(input, execution_contract::DataOperation::Cancel(_));
        use execution_contract::*;
        let query = QueryRef {
            id: Id::new("native:fixture").unwrap(),
            revision: Digest::new("b".repeat(64)).unwrap(),
        };
        let mut store = self.store.lock().unwrap();
        let state = |request: RequestId, store: &TestStore| QueryStatus {
            query: query.clone(),
            cancel_requested: store.query_cancellations.contains(&self.key(&request)),
            phase: QueryPhase::Completed,
            request,
            preparation: None,
        };
        match input {
            DataOperation::Directory(_) => Ok(DataResponse::Directory(QueryPage {
                policy_next: None,
                items: vec![QueryDescriptor {
                    reference: query,
                    title: "Explicit query fixture".into(),
                    kind: QueryKind::Native,
                    domains: vec!["fixture".into()],
                    scope: DataScope::Device,
                    input_schema: serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
                    policy: None,
                    available: true,
                    reason: None,
                }],
                next: None,
            })),
            DataOperation::Submit(submit) => {
                if submit.query != query || !submit.parameters.is_empty() {
                    return Err(ServiceError::InvalidInput);
                }
                let key = self.key(&submit.request);
                if let Some(previous) = store.queries.get(&key) {
                    if previous != &submit {
                        return Err(ServiceError::Conflict);
                    }
                } else {
                    store.queries.insert(key, submit.clone());
                    self.attempts.fetch_add(1, Ordering::SeqCst);
                }
                Ok(DataResponse::Status(state(submit.request, &store)))
            }
            DataOperation::Status(input) | DataOperation::Cancel(input) => {
                if !store.queries.contains_key(&self.key(&input.request)) {
                    return Err(ServiceError::NotFound);
                }
                // The transport fixture records cancellation independently from collection.
                if cancelling {
                    store.query_cancellations.insert(self.key(&input.request));
                }
                Ok(DataResponse::Status(state(input.request, &store)))
            }
            DataOperation::Read(input) => {
                let DataSelection::Request { request } = input.selection else {
                    return Err(ServiceError::InvalidInput);
                };
                if !store.queries.contains_key(&self.key(&request)) {
                    return Err(ServiceError::NotFound);
                }
                Ok(DataResponse::Read(CollectedData {
                    query,
                    scope: DataScope::Device,
                    request: Some(request),
                    collected_at: Some(100),
                    quality: DataQuality::Complete,
                    enumeration_complete: true,
                    mapping_version: Some(1),
                    fields: Default::default(),
                    rows: vec![serde_json::json!({"fixture":true})],
                    stale: false,
                    next: None,
                }))
            }
        }
    }

    async fn software_read(
        &self,
        _: execution_contract::SoftwareRead,
        _: CancellationToken,
    ) -> Result<execution_contract::SoftwarePage, ServiceError> {
        self.capabilities(CancellationToken::new()).await?;
        Err(ServiceError::Denied)
    }
    async fn software_prepare(
        &self,
        _: LocalSoftwareProposal,
        _: CancellationToken,
    ) -> Result<execution_contract::LocalSoftwarePrepared, ServiceError> {
        self.prepare_operation(serde_json::from_value(serde_json::json!({"scene":"policy.parameters.v1","tool":"policy_fixture","locator":{"after":null},"parameters":{}})).unwrap(),CancellationToken::new()).await?;
        Err(ServiceError::Denied)
    }
    async fn software_status(
        &self,
        input: execution_contract::QueryRequest,
        _: CancellationToken,
    ) -> Result<LocalSoftwareResult, ServiceError> {
        self.capabilities(CancellationToken::new()).await?;
        self.store
            .lock()
            .unwrap()
            .software
            .get(&self.key(&input.request))
            .cloned()
            .ok_or(ServiceError::NotFound)
    }
    async fn software_cancel(
        &self,
        input: execution_contract::QueryRequest,
        wait: CancellationToken,
    ) -> Result<LocalSoftwareResult, ServiceError> {
        self.software_status(input, wait).await
    }
    async fn invoke_policy(
        &self,
        _: &str,
        _: PolicyToolInput,
        _: CancellationToken,
    ) -> Result<execution_contract::BackendRequest, ServiceError> {
        self.authorize()?;
        tokio::time::sleep(Duration::from_millis(
            self.submit_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        Err(ServiceError::Denied)
    }
    fn bind_call(
        self: &Arc<Self>,
        _: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Arc<Self>, ServiceError> {
        self.check_binding()?;
        Ok(self.clone())
    }
    fn check_binding(&self) -> Result<(), ServiceError> {
        if self.bound.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(ServiceError::Unbound)
        }
    }
    async fn prepare_operation(
        &self,
        _: OperationProposal,
        _: CancellationToken,
    ) -> Result<execution_contract::BackendRequest, ServiceError> {
        self.authorize()?;
        tokio::time::sleep(Duration::from_millis(
            self.submit_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        Err(ServiceError::Denied)
    }
    async fn policy_tools(
        &self,
        _: execution_mcp::PolicyToolsQuery,
        _: CancellationToken,
    ) -> Result<execution_mcp::PolicyToolPage, ServiceError> {
        self.capabilities(CancellationToken::new()).await?;
        Ok(execution_mcp::PolicyToolPage {
            tools: Vec::new(),
            next: None,
        })
    }
    async fn policy_status(
        &self,
        _: OperationRequest,
        _: CancellationToken,
    ) -> Result<serde_json::Value, ServiceError> {
        self.capabilities(CancellationToken::new()).await?;
        Err(ServiceError::Denied)
    }
    async fn observe_software(
        &self,
        _: execution_contract::SoftwareReadSelection,
        _: CancellationToken,
    ) -> Result<execution_contract::BackendRequest, ServiceError> {
        self.capabilities(CancellationToken::new()).await?;
        Err(ServiceError::Denied)
    }
    async fn tasks(&self, _: CancellationToken) -> Result<Vec<BackendTask>, ServiceError> {
        self.authorize()?;
        tokio::time::sleep(Duration::from_millis(
            self.catalog_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        Ok(vec![BackendTask {
            presentation: execution_contract::BackendPresentation {
                title: "Test-only backend offer".into(),
                summary: BackendTaskSummary::Script {
                    identity: BackendIdentity::System,
                },
                extra: Default::default(),
            },
            task: Id::new("test-task").unwrap(),
            attempt: Id::new("test-attempt").unwrap(),
            revision: Digest::new("a".repeat(64)).unwrap(),
            request: RequestId::new("test-request").unwrap(),
            expires_at: 9999999999,
            user_initiated: true,
        }])
    }
    async fn capabilities(&self, _: CancellationToken) -> Result<CapabilityView, ServiceError> {
        self.authorize()?;
        self.active_waits.fetch_add(1, Ordering::SeqCst);
        let _wait = WaitGuard(&self.active_waits);
        tokio::time::sleep(Duration::from_millis(
            self.capability_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        Ok(Self::capability())
    }
    async fn execute(
        &self,
        request: BackendSelection,
        _: CancellationToken,
    ) -> Result<TaskSubmission, ServiceError> {
        self.authorize()?;
        {
            let mut db = self.store.lock().unwrap();
            let key = self.key(&request.request);
            if let Some(old) = db.inputs.get(&key) {
                if old != &request {
                    return Err(ServiceError::Conflict);
                }
            } else {
                if request.revision.as_str() != "a".repeat(64) {
                    return Err(ServiceError::Expired);
                }
                self.attempts.fetch_add(1, Ordering::SeqCst);
                let digest = Digest::new(format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&request).unwrap())
                ))
                .unwrap();
                db.inputs.insert(key.clone(), request.clone());
                db.accepted.insert(
                    key,
                    OperationStatus {
                        mode: execution_lifecycle::ExecutionMode::Test,
                        process: None,
                        assessment: None,
                        cancel_requested: false,
                        operation_request_id: request.request.clone(),
                        content_digest: digest,
                        phase: OperationPhase::Accepted,
                        attempt_id: Some(AttemptId::new("test-attempt").unwrap()),
                        evidence: vec![],
                    },
                );
            }
        }
        self.active_waits.fetch_add(1, Ordering::SeqCst);
        let _guard = WaitGuard(&self.active_waits);
        tokio::time::sleep(Duration::from_millis(
            self.submit_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        Ok(TaskSubmission {
            request: request.request,
            confirmation_required: true,
        })
    }
    async fn status(
        &self,
        request: OperationRequest,
        _: CancellationToken,
    ) -> Result<OperationStatus, ServiceError> {
        self.authorize()?;
        tokio::time::sleep(Duration::from_millis(
            self.status_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        self.store
            .lock()
            .unwrap()
            .accepted
            .get(&self.key(&request.operation_request_id))
            .cloned()
            .ok_or(ServiceError::NotFound)
    }
    async fn cancel(
        &self,
        request: OperationRequest,
        _: CancellationToken,
    ) -> Result<CancelResult, ServiceError> {
        let id = request.operation_request_id.clone();
        let operation = self.status(request, CancellationToken::new()).await?;
        let disposition = if operation.phase == OperationPhase::Verified {
            CancelDisposition::AlreadyTerminal
        } else {
            self.store
                .lock()
                .unwrap()
                .cancellations
                .insert(self.key(&id));
            CancelDisposition::Requested
        };
        Ok(CancelResult {
            disposition,
            operation,
        })
    }
}
