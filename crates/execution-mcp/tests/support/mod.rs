//! Explicit test-only service exercising transport and immutable backend-reference semantics.
use execution_contract::{
    AttemptId, BackendSelection, BackendTask, Digest, Id, RequestId, TaskSubmission,
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
    async fn tasks(&self, _: CancellationToken) -> Result<Vec<BackendTask>, ServiceError> {
        self.authorize()?;
        tokio::time::sleep(Duration::from_millis(
            self.catalog_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        Ok(vec![BackendTask {
            task: Id::new("test-task").unwrap(),
            attempt: Id::new("test-attempt").unwrap(),
            revision: Digest::new("a".repeat(64)).unwrap(),
            request: RequestId::new("test-request").unwrap(),
            title: "Test-only backend offer".into(),
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
