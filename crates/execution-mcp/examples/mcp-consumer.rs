//! Explicit TEST authority and in-memory service. No runner, database or platform effect.
//! The same file runs as an isolated consumer and as a real stdio fixture subprocess.
use execution_contract::{
    AttemptId, Authority, Digest, ExactArtifactRef, FrozenPlan, Id, PlanId, PlanLimits, PlanSpec,
    RequestId, VersionedRef,
};
use execution_mcp::*;
use service_catalog::{decode_catalog, CatalogLimits, CatalogRef, FrozenCatalog, ParameterLimits};
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
        catalog: CatalogLimits {
            max_bytes: 64 * 1024,
            max_depth: 32,
            max_nodes: 8192,
            max_string_bytes: 8192,
            max_collection_items: 128,
        },
        parameters: ParameterLimits {
            max_bytes: 8192,
            max_string_bytes: 4096,
            max_parameters: 64,
        },
    }
}
fn plan_limits() -> PlanLimits {
    PlanLimits {
        max_input_bytes: 64 * 1024,
        max_depth: 32,
        max_nodes: 8192,
        max_string_bytes: 8192,
        max_collection_items: 128,
        max_timeout_ms: 10000,
        max_output_bytes: 65536,
        max_stdin_bytes: 8192,
        max_attempts: 4,
    }
}
#[derive(Default)]
pub struct TestStore {
    plans: HashMap<(TestNamespace, RequestId), FrozenPlan>,
    candidates: HashMap<(TestNamespace, RequestId), ExactArtifactRef>,
    accepted: HashMap<(TestNamespace, RequestId), OperationStatus>,
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
    pub catalog: FrozenCatalog,
    template: PlanSpec,
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
    pub fn new(catalog: &[u8], plan: &[u8]) -> Self {
        Self {
            store: Arc::new(Mutex::new(TestStore::default())),
            namespace: TestNamespace {
                authority: "test-authority".into(),
                tenant: "test-tenant".into(),
                actor: "actor-1".into(),
                device: "device-1".into(),
                delegation: "delegation-1".into(),
            },
            catalog: decode_catalog(catalog, &limits().catalog).unwrap(),
            template: serde_json::from_slice(plan).unwrap(),
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
        let key = self.key(&RequestId::new(id).unwrap());
        let mut db = self.store.lock().unwrap();
        let operation = db.accepted.get_mut(&key).unwrap();
        operation.phase = OperationPhase::TestCompleted;
        operation.evidence = vec![Id::new("test-result").unwrap()];
    }
    fn capability() -> CapabilityView {
        CapabilityView {
            state: CapabilityState::Supported,
            reasons: vec![Id::new("test-only").unwrap()],
        }
    }
    fn freeze(
        &self,
        id: RequestId,
        selection: Option<CatalogCandidate>,
        candidate: Option<ExactArtifactRef>,
    ) -> Result<PlanPreview, ServiceError> {
        let mut spec = self.template.clone();
        spec.plan_id = PlanId::new(id.as_str()).map_err(|_| ServiceError::InvalidInput)?;
        spec.request.request_id = id.clone();
        spec.request.authority = Authority::Test {
            id: Id::new(&self.namespace.authority).map_err(|_| ServiceError::Unbound)?,
        };
        if let Some(selected) = selection {
            if selected.selection.availability(100) != service_catalog::CatalogAvailability::Listed
            {
                return Err(ServiceError::Expired);
            }
            spec.request.parameters = selected.selection.parameters().clone();
            spec.request.operation.action = selected.selection.operation().action.clone();
            spec.request.operation.resource =
                selected.selection.operation().resource.reference.clone();
        }
        if let Some(candidate) = candidate {
            spec.launch.artifact = candidate;
        }
        let plan =
            FrozenPlan::freeze(spec, &plan_limits()).map_err(|_| ServiceError::InvalidInput)?;
        let reference = PlanRef {
            plan_id: plan.spec().plan_id.clone(),
            digest: plan.digest().clone(),
        };
        let mut db = self.store.lock().unwrap();
        let key = self.key(&id);
        if let Some(existing) = db.plans.get(&key) {
            if existing.digest() != plan.digest() {
                return Err(ServiceError::Conflict);
            }
        } else {
            db.plans.insert(key, plan);
        }
        Ok(PlanPreview {
            operation_request_id: id,
            plan: reference,
            capability: Self::capability(),
        })
    }
}
impl ExecutionServicePort for TestService {
    fn check_binding(&self) -> Result<(), ServiceError> {
        if self.bound.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(ServiceError::Unbound)
        }
    }
    async fn catalog(
        &self,
        reference: Option<CatalogRef>,
        _: CancellationToken,
    ) -> Result<FrozenCatalog, ServiceError> {
        self.authorize()?;
        tokio::time::sleep(Duration::from_millis(
            self.catalog_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        if reference.is_some_and(|r| r != self.catalog.reference()) {
            return Err(ServiceError::Expired);
        }
        Ok(self.catalog.clone())
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
    async fn preview(
        &self,
        request: PreviewRequest,
        _: CancellationToken,
    ) -> Result<PlanPreview, ServiceError> {
        self.authorize()?;
        match request {
            PreviewRequest::Catalog(selection) => self.freeze(
                selection.operation_request_id.clone(),
                Some(*selection),
                None,
            ),
            PreviewRequest::Candidate {
                operation_request_id,
                candidate,
            } => {
                if self
                    .store
                    .lock()
                    .unwrap()
                    .candidates
                    .get(&self.key(&operation_request_id))
                    != Some(&candidate)
                {
                    return Err(ServiceError::NotFound);
                }
                self.freeze(operation_request_id, None, Some(candidate))
            }
        }
    }
    async fn propose(
        &self,
        request: CandidateRequest,
        _: CancellationToken,
    ) -> Result<CandidateReceipt, ServiceError> {
        self.authorize()?;
        let (id, bytes) = match request {
            CandidateRequest::Catalog(selection) => (
                selection.operation_request_id,
                serde_json::to_vec(selection.selection.reference()).unwrap(),
            ),
            CandidateRequest::Script(draft) => (
                draft.operation_request_id().clone(),
                serde_json::to_vec(&(draft.source_utf8(), draft.interpreter())).unwrap(),
            ),
        };
        let candidate = ExactArtifactRef {
            resource: VersionedRef {
                id: Id::new(id.as_str()).unwrap(),
                revision: Id::new("1").unwrap(),
            },
            sha256: Digest::new(format!("{:x}", Sha256::digest(bytes))).unwrap(),
        };
        let mut db = self.store.lock().unwrap();
        let key = self.key(&id);
        if let Some(existing) = db.candidates.get(&key) {
            if existing != &candidate {
                return Err(ServiceError::Conflict);
            }
        } else {
            db.candidates.insert(key, candidate.clone());
        }
        Ok(CandidateReceipt {
            operation_request_id: id,
            candidate,
        })
    }
    async fn submit(
        &self,
        request: SubmitRequest,
        _: CancellationToken,
    ) -> Result<OperationStatus, ServiceError> {
        self.authorize()?;
        let status = {
            let mut db = self.store.lock().unwrap();
            let key = self.key(&request.operation_request_id);
            if let Some(existing) = db.accepted.get(&key) {
                if existing.plan != request.plan {
                    return Err(ServiceError::Conflict);
                }
                existing.clone()
            } else {
                let plan = db.plans.get(&key).ok_or(ServiceError::NotFound)?;
                if plan.spec().plan_id != request.plan.plan_id
                    || plan.digest() != &request.plan.digest
                {
                    return Err(ServiceError::Conflict);
                }
                self.attempts.fetch_add(1, Ordering::SeqCst);
                let status = OperationStatus {
                    operation_request_id: request.operation_request_id,
                    plan: request.plan,
                    phase: OperationPhase::Accepted,
                    attempt_id: Some(AttemptId::new("test-attempt").unwrap()),
                    evidence: vec![],
                };
                db.accepted.insert(key, status.clone());
                status
            }
        };
        // Fault seam: committed acceptance is retained even if this future is dropped.
        self.active_waits.fetch_add(1, Ordering::SeqCst);
        let _wait = WaitGuard(&self.active_waits);
        tokio::time::sleep(Duration::from_millis(
            self.submit_delay_ms.load(Ordering::SeqCst) as u64,
        ))
        .await;
        Ok(status)
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
        let disposition = if operation.phase == OperationPhase::TestCompleted {
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
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let server = args.get(1).is_some_and(|a| a == "--test-server");
    let offset = if server { 2 } else { 1 };
    let catalog = std::fs::read(&args[offset]).unwrap();
    let plan = std::fs::read(&args[offset + 1]).unwrap();
    if server {
        // No env-derived subscriber: even RUST_LOG=trace cannot dump SDK request bodies.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let adapter =
                ExecutionMcp::new(Arc::new(TestService::new(&catalog, &plan)), limits()).unwrap();
            let _closed = adapter
                .serve(
                    tokio::io::stdin(),
                    tokio::io::stdout(),
                    CancellationToken::new(),
                )
                .await;
        });
    } else {
        run_process(&args[offset], &args[offset + 1]);
    }
}
pub fn run_process(catalog: &str, plan: &str) {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
        sync::mpsc,
    };
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--test-server", catalog, plan])
            .env("RUST_LOG", "trace")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut input = child.0.stdin.take().unwrap();
    let output = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::sync_channel(16);
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if tx.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let mut rpc = |id: u64, method: &str, params: serde_json::Value| {
        writeln!(
            input,
            "{}",
            serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )
        .unwrap();
        input.flush().unwrap();
        let line = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("bounded process reply");
        assert!(line.len() <= limits().response_bytes);
        serde_json::from_str::<serde_json::Value>(&line).unwrap()
    };
    let hello = rpc(
        1,
        "initialize",
        serde_json::json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"isolated-test","version":"1"}}),
    );
    assert_eq!(hello["result"]["protocolVersion"], "2025-11-25");
    // The notification is sent separately because it has no response.
    writeln!(
        input,
        "{}",
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    writeln!(
        input,
        "{}",
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})
    )
    .unwrap();
    input.flush().unwrap();
    let tools: serde_json::Value =
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 7);
    writeln!(input, "{}", serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"execution_capabilities","arguments":{}}})).unwrap();
    input.flush().unwrap();
    let answer: serde_json::Value =
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
    assert_eq!(
        answer["result"]["structuredContent"]["result"]["reasons"][0],
        "test-only"
    );
    let mut tool = |id: u64, name: &str, args: serde_json::Value| {
        writeln!(input, "{}", serde_json::json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap();
        input.flush().unwrap();
        let line = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("bounded process reply");
        assert!(line.len() <= limits().response_bytes);
        let r: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(r["result"]["isError"], false, "{r}");
        r["result"]["structuredContent"]["result"].clone()
    };
    let directory = tool(4, "execution_catalog", serde_json::json!({}));
    let selection = serde_json::json!({"operationRequestId":"process-operation","catalog":directory["catalog"],"itemId":"diagnostics","variantId":"network-check","arguments":{"host":"example.invalid"}});
    let candidate = tool(
        5,
        "execution_propose",
        serde_json::json!({"catalog":{"selection":selection}}),
    );
    assert_eq!(candidate["operationRequestId"], "process-operation");
    let preview = tool(
        6,
        "execution_preview",
        serde_json::json!({"catalog":{"selection":selection}}),
    );
    let submit =
        serde_json::json!({"operationRequestId":"process-operation","plan":preview["plan"]});
    let accepted = tool(7, "execution_submit", submit.clone());
    assert_eq!(accepted["phase"], "accepted");
    assert_eq!(tool(8, "execution_submit", submit), accepted);
    assert_eq!(
        tool(
            9,
            "execution_status",
            serde_json::json!({"operationRequestId":"process-operation"})
        ),
        accepted
    );
    let cancel = tool(
        10,
        "execution_cancel",
        serde_json::json!({"operationRequestId":"process-operation"}),
    );
    assert_eq!(cancel["disposition"], "requested");
    assert_eq!(cancel["operation"], accepted);
    drop(input);
    child.0.wait().unwrap();
    reader.join().unwrap();
    use std::io::Read;
    let mut logs = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut logs)
        .unwrap();
    assert!(
        logs.is_empty(),
        "fixture logs must exclude raw SDK payloads"
    );
}
