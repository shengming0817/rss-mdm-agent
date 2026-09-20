use super::authority::{id, S1Host};
use crate::self_service::{self as ui, fixtures, selection};
use execution_app::*;
use execution_contract::*;
use execution_mcp as mcp;
use service_catalog::{DisplayDecision, DisplayStatus, FrozenCatalog, SelectedOperation};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub fn now() -> Result<u64, Error> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Clock)?
            .as_millis(),
    )
    .map_err(|_| Error::Clock)
}
pub fn catalog() -> ui::Result<FrozenCatalog> {
    fixtures::catalog(0)
}
fn bad(error: Error) -> ui::ServiceError {
    let code = match error {
        Error::OutcomeUnknown => "outcomeUnknown",
        Error::ConfirmationUnknown => "confirmationUnknown",
        Error::Denied | Error::Unbound => "denied",
        Error::Conflict => "conflict",
        Error::InvalidInput => "invalidInput",
        _ => "execution",
    };
    ui::error(code, "受控执行操作未确认；请查询原任务")
}
fn mcp_error(e: Error) -> mcp::ServiceError {
    match e {
        Error::Unsupported => mcp::ServiceError::Unsupported,
        Error::NotFound => mcp::ServiceError::NotFound,
        Error::Denied | Error::Unbound => mcp::ServiceError::Denied,
        Error::Conflict => mcp::ServiceError::Conflict,
        Error::InvalidInput => mcp::ServiceError::InvalidInput,
        Error::OutcomeUnknown => mcp::ServiceError::OutcomeUnknown,
        _ => mcp::ServiceError::Unavailable,
    }
}
pub const BINDING: &str = "s1-desktop-test-actor-device-r1";
type App = ExecutionApp<S1Host, S1Runner>;
type Job = Box<dyn FnOnce(&mut Owner) + Send>;
#[derive(Clone)]
pub struct ExecutionHandle {
    caller: Option<RequestContext>,
    origin: Option<Initiator>,
    sender: mpsc::SyncSender<Job>,
    stopped: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}
pub struct Owner {
    app: App,
    host: S1Host,
    catalog: FrozenCatalog,
    started: std::collections::BTreeMap<RequestId, u64>,
}
#[derive(Clone)]
struct S1Runner {
    complete: DeterministicTestRunner,
    wait: DeterministicTestRunner,
    unknown: DeterministicTestRunner,
}
impl S1Runner {
    fn select(&self, p: &FrozenPlan) -> &DeterministicTestRunner {
        match p.spec().request.operation.resource.id.as_str() {
            "fixture-maintenance" => &self.wait,
            "fixture-unknown" => &self.unknown,
            _ => &self.complete,
        }
    }
}
impl RunnerPort for S1Runner {
    fn id(&self) -> Id {
        id("s1-runner")
    }
    fn mode(&self) -> execution_lifecycle::ExecutionMode {
        execution_lifecycle::ExecutionMode::Test
    }
    fn dispatch(&self, permit: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
        self.select(permit.plan()).dispatch(permit)
    }
    fn stop(&self, p: &FrozenPlan, a: &AttemptId) -> Result<(), Error> {
        self.select(p).stop(p, a)
    }
    fn observe(
        &self,
        p: &FrozenPlan,
        a: &AttemptId,
        stage: ObservationStage,
        time: u64,
    ) -> Result<Option<execution_lifecycle::ObservationFacts>, Error> {
        self.select(p).observe(p, a, stage, time)
    }
}
impl ExecutionHandle {
    /// A request view only: clones the shared sender and never creates execution resources.
    pub fn for_caller(&self, actor: &str) -> Result<Self, Error> {
        let mut call = self.clone();
        call.caller = Some(RequestContext {
            actor: ActorId::new(actor).map_err(|_| Error::Denied)?,
        });
        call.origin = None;
        Ok(call)
    }

    pub fn start(path: &Path) -> Result<Self, Error> {
        let host = S1Host::new();
        let runner = S1Runner {
            complete: DeterministicTestRunner::new(id("s1-runner"), TestScenario::Complete, 128)?,
            wait: DeterministicTestRunner::new(id("s1-runner"), TestScenario::Wait, 128)?,
            unknown: DeterministicTestRunner::new(id("s1-runner"), TestScenario::Unknown, 128)?,
        };
        let startup = if path.try_exists().map_err(|_| Error::Storage)? {
            Startup::OpenTest
        } else {
            Startup::CreateTest
        };
        let app = ExecutionApp::start(
            path,
            startup,
            host.clone(),
            runner,
            AppConfig::test_defaults(1),
        )?;
        let mut owner = Owner {
            app,
            host,
            catalog: catalog().map_err(|_| Error::Configuration)?,
            started: Default::default(),
        };
        // Fresh runner has no pre-crash facts. Reconcile original attempts; never re-dispatch.
        let mut after = None;
        loop {
            let page = owner.app.service_tasks(after.as_ref(), 128)?;
            for task in page.items {
                if matches!(
                    task.status.phase,
                    TaskPhase::Accepted
                        | TaskPhase::Running
                        | TaskPhase::ExecutionEnded
                        | TaskPhase::OutcomeUnknown
                ) {
                    owner.app.reconcile(&task.status.operation_request_id)?;
                }
            }
            after = page.next;
            if after.is_none() {
                break;
            }
        }
        let (sender, receiver) = mpsc::sync_channel::<Job>(32);
        let stopped = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let stopping = stopped.clone();
        let completion = finished.clone();
        std::thread::Builder::new()
            .name("s1-execution-owner".into())
            .spawn(move || {
                while !stopping.load(Ordering::Acquire) {
                    let received = match owner.next_tick() {
                        Some(delay) => receiver.recv_timeout(delay),
                        None => receiver
                            .recv()
                            .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
                    };
                    match received {
                        Ok(job) => job(&mut owner),
                        Err(mpsc::RecvTimeoutError::Timeout) => (),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    owner.tick();
                }
                drop(owner);
                completion.store(true, Ordering::Release);
            })
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            caller: None,
            origin: None,
            sender,
            stopped,
            finished,
        })
    }
    pub async fn close(&self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.sender.try_send(Box::new(|_| {}));
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            while !self.finished.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await;
    }
    pub async fn call<T: Send + 'static>(
        &self,
        action: impl FnOnce(&mut Owner, &RequestContext) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(Error::Unavailable);
        }
        let caller = self.caller.clone().ok_or(Error::Unbound)?;
        let (tx, rx) = oneshot::channel();
        self.sender
            .try_send(Box::new(move |owner| {
                let _ = tx.send(action(owner, &caller));
            }))
            .map_err(|_| Error::Capacity)?;
        tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .map_err(|_| Error::OutcomeUnknown)?
            .map_err(|_| Error::OutcomeUnknown)?
    }
    pub async fn snapshot(&self, query: ui::SnapshotQuery) -> ui::Result<ui::Snapshot> {
        self.call(move |o, caller| o.snapshot(caller, query))
            .await
            .map_err(bad)
    }
    pub async fn preview_ui(&self, draft: ui::Draft) -> ui::Result<ui::PlanView> {
        self.call(move |o, caller| {
            if draft.instance_id != BINDING {
                return Err(Error::Denied);
            }
            let selected =
                selection::select(&o.catalog, &draft).map_err(|_| Error::InvalidInput)?;
            let p = o.preview(caller, &draft.request_id, selected, fixtures::human())?;
            o.plan_view(&p, draft.revision)
        })
        .await
        .map_err(bad)
    }
    pub async fn submit_ui(&self, input: ui::Submission) -> ui::Result<ui::RequestView> {
        self.call(move |o, caller| {
            o.check_submission(caller, &input)?;
            o.submit(caller, &input.request_id)?;
            o.request_view(caller, &input.request_id)
        })
        .await
        .map_err(bad)
    }
    pub async fn cancel_ui(&self, input: ui::Submission) -> ui::Result<ui::RequestView> {
        self.call(move |o, caller| {
            o.check_submission(caller, &input)?;
            o.app.cancel(caller, &input.request_id)?;
            o.app.reconcile(&input.request_id)?;
            o.request_view(caller, &input.request_id)
        })
        .await
        .map_err(bad)
    }
    pub async fn approve_ui(&self, input: ui::Submission) -> ui::Result<ui::RequestView> {
        self.call(move |o, caller| {
            let p = o.check_submission(caller, &input)?;
            let current = o.app.status(caller, &input.request_id)?;
            if current.attempts > 0 {
                return o.request_view(caller, &input.request_id);
            }
            if current.admission != Some(execution_sqlite::AdmissionStatus::ApprovalRequired) {
                return Err(Error::Denied);
            }
            o.host.approve(caller, &p)?;
            o.app.advance(
                caller,
                &input.request_id,
                &CommandId::new(format!("approve-{}", p.digest().as_str()))?,
            )?;
            o.started.insert(input.request_id.clone(), now()?);
            o.request_view(caller, &input.request_id)
        })
        .await
        .map_err(bad)
    }
    pub async fn respond_ui(&self, input: ui::Reply) -> ui::Result<ui::RequestView> {
        self.call(move |o, caller| {
            if input.instance_id != BINDING {
                return Err(Error::Denied);
            }
            let command = match input.answer {
                ui::Answer::Cancel {} => execution_interaction::Command::Cancel {
                    id: input.command_id.clone(),
                },
                ui::Answer::Confirmation { accepted } => execution_interaction::Command::Answer {
                    id: input.command_id.clone(),
                    response: execution_interaction::Response::Confirmation { accepted },
                },
                _ => return Err(Error::InvalidInput),
            };
            o.app.respond(
                caller,
                &input.request_id,
                &CommandId::new(input.command_id.as_str())?,
                &execution_interaction::Reference::new(input.interaction_id)
                    .map_err(|_| Error::InvalidInput)?,
                &command,
            )?;
            o.request_view(caller, &input.request_id)
        })
        .await
        .map_err(bad)
    }
    pub async fn details(&self, request: RequestId) -> Result<ExecutionTaskDetails, Error> {
        self.call(move |o, caller| o.app.task_details(caller, &request))
            .await
    }
    pub async fn cancel_task(&self, request: RequestId) -> Result<ExecutionStatus, Error> {
        self.call(move |o, caller| {
            o.app.cancel(caller, &request)?;
            o.app.reconcile(&request)
        })
        .await
    }
}
impl Owner {
    fn next_tick(&self) -> Option<Duration> {
        let now = now().unwrap_or(0);
        self.started
            .values()
            .min()
            .map(|at| Duration::from_millis(at.saturating_add(1500).saturating_sub(now)))
    }
    fn tick(&mut self) {
        let Ok(time) = now() else {
            return;
        };
        let due: Vec<_> = self
            .started
            .iter()
            .filter(|(_, at)| time >= at.saturating_add(1500))
            .map(|(id, _)| id.clone())
            .collect();
        for request in due {
            self.started.remove(&request);
            if self.app.reconcile(&request).is_err() {
                self.started.insert(request, time);
            }
        }
    }
    fn preview(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
        selection: SelectedOperation,
        origin: Initiator,
    ) -> Result<FrozenPlan, Error> {
        let previous = match self.app.frozen_plan(caller, request) {
            Ok(p) => Some(p),
            Err(Error::NotFound) => None,
            Err(e) => return Err(e),
        };
        let time = previous
            .as_ref()
            .map(|p| p.spec().validity.not_before_unix_ms)
            .unwrap_or(now()?);
        let p = fixtures::freeze(
            &selection,
            request,
            format!("plan-{}", fixtures::digest(request.as_str().as_bytes())),
            time,
            &origin,
            &caller.actor,
        )
        .map_err(|_| Error::InvalidInput)?;
        if previous
            .as_ref()
            .is_some_and(|old| old.digest() != p.digest())
        {
            return Err(Error::Conflict);
        }
        self.host.validate(&p)?;
        self.app.register_plan(caller, request, &p)?;
        Ok(p)
    }
    fn check_origin(
        &self,
        caller: &RequestContext,
        request: &RequestId,
        origin: &Initiator,
    ) -> Result<(), Error> {
        let p = self.app.frozen_plan(caller, request)?;
        if !super::origin::same_conversation(origin, &p.spec().request.initiator) {
            return Err(Error::Denied);
        }
        Ok(())
    }
    fn check_submission(
        &self,
        caller: &RequestContext,
        input: &ui::Submission,
    ) -> Result<FrozenPlan, Error> {
        if input.instance_id != BINDING {
            return Err(Error::Denied);
        }
        let p = self.app.frozen_plan(caller, &input.request_id)?;
        if p.spec().plan_id != input.plan_id || p.digest() != &input.digest {
            return Err(Error::Conflict);
        }
        Ok(p)
    }
    fn submit(
        &mut self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<ExecutionStatus, Error> {
        let p = self.app.frozen_plan(caller, request)?;
        let status = self.app.submit(caller, request, &p)?;
        if status.attempts > 0
            && p.spec().request.operation.resource.id.as_str() != "fixture-maintenance"
        {
            self.started.entry(request.clone()).or_insert(now()?);
        }
        if p.spec().request.operation.resource.id.as_str() == "fixture-restart" {
            self.app.open_interaction(
                caller,
                request,
                execution_interaction::Reference::new(format!(
                    "notice-{}",
                    p.spec().plan_id.as_str()
                ))
                .map_err(|_| Error::InvalidInput)?,
                execution_interaction::Kind::UserConfirmation {
                    purpose: execution_interaction::ConfirmationPurpose::Continue,
                },
                p.spec().validity.expires_at_unix_ms,
            )?;
        }
        Ok(status)
    }
    fn plan_view(&self, p: &FrozenPlan, revision: u32) -> Result<ui::PlanView, Error> {
        let spec = p.spec();
        let item = self
            .catalog
            .snapshot()
            .items
            .iter()
            .find(|i| i.operations[0].resource.reference == spec.request.operation.resource)
            .ok_or(Error::NotFound)?;
        Ok(ui::PlanView {
            authority: spec.request.authority.clone(),
            actor: spec.request.actor.clone(),
            initiator: spec.request.initiator.clone(),
            request_id: spec.request.request_id.clone(),
            revision,
            plan_id: spec.plan_id.clone(),
            digest: p.digest().clone(),
            item_id: item.id.clone(),
            title: item.name.clone(),
            action: spec.request.operation.action.clone(),
            resource: item.operations[0].resource.clone(),
            target: fixtures::TARGET.into(),
            run_as: "S1 测试用户 fixture-user".into(),
            network: "测试执行器，无网络访问".into(),
            data_scope: "测试参数仅在受保护本地存储中处理，不修改设备".into(),
            permission: "权限与批准由 Rust 受控执行服务裁决".into(),
            parameters: item.operations[0]
                .parameters
                .values()
                .map(|field| ui::ParameterSummary {
                    label: field.title.clone(),
                    state: "已校验；值不回显",
                })
                .collect(),
            expires_at_unix_ms: spec.validity.expires_at_unix_ms,
        })
    }
    fn request_view(
        &self,
        caller: &RequestContext,
        request: &RequestId,
    ) -> Result<ui::RequestView, Error> {
        let p = self.app.frozen_plan(caller, request)?;
        let s = self.app.status(caller, request)?;
        let (status, message) = if s.admission == Some(execution_sqlite::AdmissionStatus::Denied) {
            (
                ui::RequestStatus::Stopped,
                "Rust 已拒绝执行；确认不能覆盖拒绝",
            )
        } else if s.admission == Some(execution_sqlite::AdmissionStatus::ApprovalRequired)
            && s.attempts == 0
        {
            (
                ui::RequestStatus::Approval,
                "等待可信测试管理员批准精确计划",
            )
        } else {
            match s.phase {
                TaskPhase::TestCompleted => (
                    ui::RequestStatus::Complete,
                    "S1 测试执行完成；没有安装软件或修改设备",
                ),
                TaskPhase::Cancelled
                | TaskPhase::AdmissionDenied
                | TaskPhase::FailedBeforeDispatch => (ui::RequestStatus::Stopped, "测试任务已停止"),
                TaskPhase::OutcomeUnknown => (
                    ui::RequestStatus::UnknownEffect,
                    "执行结果未知；只核实原请求，不自动重跑",
                ),
                _ => (
                    ui::RequestStatus::Waiting,
                    "测试任务已登记或正在运行；AI 状态不代表执行结果",
                ),
            }
        };
        let mut interactions = Vec::new();
        if p.spec().request.operation.resource.id.as_str() == "fixture-restart" {
            let reference = execution_interaction::Reference::new(format!(
                "notice-{}",
                p.spec().plan_id.as_str()
            ))
            .map_err(|_| Error::InvalidInput)?;
            if let Ok(i) = self.app.interaction(caller, request, &reference) {
                let snapshot = i.snapshot();
                interactions.push(ui::InteractionView {
                    id: reference.as_str().into(),
                    kind: snapshot.spec.kind.clone(),
                    status: match snapshot.status {
                        execution_interaction::Status::Pending => ui::InteractionStatus::Pending,
                        execution_interaction::Status::Answered { .. } => {
                            ui::InteractionStatus::Answered
                        }
                        execution_interaction::Status::Cancelled { .. } => {
                            ui::InteractionStatus::Cancelled
                        }
                        execution_interaction::Status::Expired { .. } => {
                            ui::InteractionStatus::Expired
                        }
                    },
                    message: "确认已阅读测试提示；不会重启设备，也不形成业务批准",
                    expires_at_unix_ms: snapshot.spec.expires_at_unix_ms,
                    options: vec![],
                });
            }
        }
        Ok(ui::RequestView {
            plan: self.plan_view(&p, 1)?,
            status,
            message,
            interactions,
        })
    }
    fn snapshot(
        &self,
        caller: &RequestContext,
        query: ui::SnapshotQuery,
    ) -> Result<ui::Snapshot, Error> {
        if query.request_ids.len() > 3 {
            return Err(Error::InvalidInput);
        }
        let catalog = self
            .catalog
            .snapshot()
            .items
            .iter()
            .map(|item| {
                let op = &item.operations[0];
                let projection = self
                    .catalog
                    .projection(&item.id, &op.id, &fixtures::PARAMETERS)
                    .map_err(|_| Error::Configuration)?;
                Ok(ui::CatalogView {
                    catalog: self.catalog.reference(),
                    item_id: item.id.clone(),
                    variant_id: op.id.clone(),
                    kind: item.kind,
                    name: item.name.clone(),
                    description: item.description.clone(),
                    category: item.category.clone(),
                    resource: op.resource.clone(),
                    fields: projection.fields().clone(),
                    input_schema: projection.input_schema().clone(),
                    display: DisplayStatus {
                        visibility: DisplayDecision::Allowed,
                        requestability: DisplayDecision::Allowed,
                        executability: if item.id.as_str() == "blocked" {
                            DisplayDecision::Blocked
                        } else {
                            DisplayDecision::Unknown
                        },
                    },
                    reason: "S1 测试目录；最终裁决由提交时 Rust 校验产生".into(),
                    availability: if item.state == service_catalog::PublicationState::Withdrawn {
                        ui::Availability::Withdrawn
                    } else {
                        ui::Availability::Listed
                    },
                })
            })
            .collect::<Result<_, Error>>()?;
        let page = self.app.tasks(caller, query.after.as_ref(), 128)?;
        let requests = page
            .items
            .iter()
            .filter(|task| task.status.submitted)
            .map(|task| self.request_view(caller, &task.status.operation_request_id))
            .collect::<Result<_, _>>()?;
        let mut referenced_requests = Vec::new();
        for request in query.request_ids {
            match self.app.status(caller, &request) {
                Ok(status) if status.submitted => {
                    referenced_requests.push(self.request_view(caller, &request)?)
                }
                Ok(_) | Err(Error::NotFound) => (),
                Err(error) => return Err(error),
            }
        }
        Ok(ui::Snapshot {
            mode: ui::ServiceMode::S1,
            instance_id: BINDING.into(),
            target_label: fixtures::TARGET,
            catalog,
            requests,
            next: page.next,
            referenced_requests,
        })
    }
}
fn operation(s: ExecutionStatus) -> mcp::OperationStatus {
    mcp::OperationStatus {
        submitted: s.submitted,
        cancel_requested: s.cancel_requested,
        operation_request_id: s.operation_request_id,
        plan: mcp::PlanRef {
            plan_id: s.plan_id,
            digest: s.plan_digest,
        },
        phase: if s.admission == Some(execution_sqlite::AdmissionStatus::Denied) {
            mcp::OperationPhase::Failed
        } else {
            match s.phase {
                TaskPhase::Accepted => mcp::OperationPhase::Accepted,
                TaskPhase::Waiting | TaskPhase::ApprovalRequired => mcp::OperationPhase::Waiting,
                TaskPhase::Running => mcp::OperationPhase::Running,
                TaskPhase::OutcomeUnknown => mcp::OperationPhase::OutcomeUnknown,
                TaskPhase::ExecutionEnded => mcp::OperationPhase::ExecutionEnded,
                TaskPhase::TestCompleted => mcp::OperationPhase::TestCompleted,
                TaskPhase::AdmissionDenied | TaskPhase::FailedBeforeDispatch => {
                    mcp::OperationPhase::Failed
                }
                TaskPhase::Cancelled => mcp::OperationPhase::Cancelled,
            }
        },
        attempt_id: s.attempt_id,
        evidence: s.evidence.into_iter().map(|e| e.reference.id).collect(),
    }
}
impl mcp::ExecutionServicePort for ExecutionHandle {
    fn bind_call(
        self: &Arc<Self>,
        metadata: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Arc<Self>, mcp::ServiceError> {
        let actor = super::origin::AiBinding::principal(metadata)?;
        if self
            .caller
            .as_ref()
            .is_some_and(|caller| caller.actor.as_str() != actor)
        {
            return Err(mcp::ServiceError::Denied);
        }
        let mut call = self.for_caller(&actor).map_err(mcp_error)?;
        call.origin = Some(super::origin::AiBinding::for_user(&actor)?.bind(metadata)?);
        Ok(Arc::new(call))
    }
    fn check_binding(&self) -> Result<(), mcp::ServiceError> {
        Ok(())
    }
    async fn catalog(
        &self,
        reference: Option<service_catalog::CatalogRef>,
        _: CancellationToken,
    ) -> Result<FrozenCatalog, mcp::ServiceError> {
        let c = catalog().map_err(|_| mcp::ServiceError::Unavailable)?;
        if reference.is_some_and(|r| r != c.reference()) {
            return Err(mcp::ServiceError::Expired);
        }
        Ok(c)
    }
    async fn capabilities(
        &self,
        _: CancellationToken,
    ) -> Result<mcp::CapabilityView, mcp::ServiceError> {
        Ok(mcp::CapabilityView {
            state: mcp::CapabilityState::Supported,
            reasons: vec![id("s1-test-runner-only")],
        })
    }
    async fn preview(
        &self,
        request: mcp::PreviewRequest,
        _: CancellationToken,
    ) -> Result<mcp::PlanPreview, mcp::ServiceError> {
        let origin = self.origin.clone().ok_or(mcp::ServiceError::Denied)?;
        self.call(move |o, caller| {
            let p = match request {
                mcp::PreviewRequest::Catalog(c) => {
                    o.preview(caller, &c.operation_request_id, c.selection, origin.clone())?
                }
                mcp::PreviewRequest::Candidate {
                    operation_request_id,
                    candidate,
                } => {
                    o.check_origin(caller, &operation_request_id, &origin)?;
                    let p = o.app.frozen_plan(caller, &operation_request_id)?;
                    if p.spec().launch.artifact != candidate {
                        return Err(Error::Conflict);
                    }
                    p
                }
            };
            Ok(mcp::PlanPreview {
                operation_request_id: p.spec().request.request_id.clone(),
                plan: mcp::PlanRef {
                    plan_id: p.spec().plan_id.clone(),
                    digest: p.digest().clone(),
                },
                capability: mcp::CapabilityView {
                    state: mcp::CapabilityState::Supported,
                    reasons: vec![id("s1-test-runner-only")],
                },
            })
        })
        .await
        .map_err(mcp_error)
    }
    async fn propose(
        &self,
        request: mcp::CandidateRequest,
        _: CancellationToken,
    ) -> Result<mcp::CandidateReceipt, mcp::ServiceError> {
        let origin = self.origin.clone().ok_or(mcp::ServiceError::Denied)?;
        self.call(move |o, caller| {
            let mcp::CandidateRequest::Catalog(c) = request else {
                return Err(Error::Unsupported);
            };
            let p = o.preview(caller, &c.operation_request_id, c.selection, origin.clone())?;
            Ok(mcp::CandidateReceipt {
                operation_request_id: c.operation_request_id,
                candidate: p.spec().launch.artifact.clone(),
            })
        })
        .await
        .map_err(mcp_error)
    }
    async fn submit(
        &self,
        request: mcp::SubmitRequest,
        _: CancellationToken,
    ) -> Result<mcp::OperationStatus, mcp::ServiceError> {
        let origin = self.origin.clone().ok_or(mcp::ServiceError::Denied)?;
        self.call(move |o, caller| {
            o.check_origin(caller, &request.operation_request_id, &origin)?;
            let p = o.app.frozen_plan(caller, &request.operation_request_id)?;
            if p.spec().plan_id != request.plan.plan_id || p.digest() != &request.plan.digest {
                return Err(Error::Conflict);
            }
            o.submit(caller, &request.operation_request_id)
                .map(operation)
        })
        .await
        .map_err(mcp_error)
    }
    async fn status(
        &self,
        request: mcp::OperationRequest,
        _: CancellationToken,
    ) -> Result<mcp::OperationStatus, mcp::ServiceError> {
        let origin = self.origin.clone().ok_or(mcp::ServiceError::Denied)?;
        self.call(move |o, caller| {
            o.check_origin(caller, &request.operation_request_id, &origin)?;
            o.app
                .status(caller, &request.operation_request_id)
                .map(operation)
        })
        .await
        .map_err(mcp_error)
    }
    async fn cancel(
        &self,
        request: mcp::OperationRequest,
        _: CancellationToken,
    ) -> Result<mcp::CancelResult, mcp::ServiceError> {
        let origin = self.origin.clone().ok_or(mcp::ServiceError::Denied)?;
        self.call(move |o, caller| {
            o.check_origin(caller, &request.operation_request_id, &origin)?;
            o.app.cancel(caller, &request.operation_request_id)?;
            Ok(mcp::CancelResult {
                disposition: mcp::CancelDisposition::Requested,
                operation: operation(o.app.reconcile(&request.operation_request_id)?),
            })
        })
        .await
        .map_err(mcp_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_preserves_unknown_outcomes_and_deterministic_rejections() {
        for (error, code) in [
            (Error::OutcomeUnknown, "outcomeUnknown"),
            (Error::ConfirmationUnknown, "confirmationUnknown"),
            (Error::Denied, "denied"),
            (Error::Conflict, "conflict"),
            (Error::InvalidInput, "invalidInput"),
        ] {
            assert_eq!(serde_json::to_value(bad(error)).unwrap()["code"], code);
        }
    }

    async fn fixture() -> (ExecutionHandle, std::path::PathBuf) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "rss-task-pages-{}-{}-{}",
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let root = root.canonicalize().unwrap();
        (
            ExecutionHandle::start(&root.join("execution.sqlite"))
                .unwrap()
                .for_caller("fixture-actor")
                .unwrap(),
            root,
        )
    }

    fn add(
        o: &mut Owner,
        caller: &RequestContext,
        index: usize,
        submitted: bool,
        origin: Initiator,
    ) -> Result<(), Error> {
        let draft = ui::Draft {
            instance_id: BINDING.into(),
            request_id: RequestId::new(format!("page-{index:03}")).unwrap(),
            revision: 1,
            catalog: o.catalog.reference(),
            item_id: id("office"),
            variant_id: id("test"),
            fields: Default::default(),
        };
        let selected = selection::select(&o.catalog, &draft).map_err(|_| Error::InvalidInput)?;
        o.preview(caller, &draft.request_id, selected, origin)?;
        if submitted {
            o.submit(caller, &draft.request_id)?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn preview_only_page_keeps_continuation_to_submitted_tasks() {
        let (handle, root) = fixture().await;
        handle
            .call(|o, caller| {
                for i in 0..130 {
                    add(o, caller, i, i >= 128, fixtures::human())?;
                }
                let first = serde_json::to_value(o.snapshot(caller, ui::SnapshotQuery::default())?)
                    .unwrap();
                assert_eq!(first["requests"].as_array().unwrap().len(), 0);
                assert_eq!(first["next"], "page-127");
                let second = o.snapshot(
                    caller,
                    ui::SnapshotQuery {
                        after: Some(RequestId::new("page-127").unwrap()),
                        request_ids: vec![],
                    },
                )?;
                assert_eq!(second.requests.len(), 2);
                assert_eq!(second.requests[0].plan.request_id.as_str(), "page-128");
                assert!(second.next.is_none());
                Ok(())
            })
            .await
            .unwrap();
        handle.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn approval_projection_contains_the_frozen_actor_and_origin() {
        let (handle, root) = fixture().await;
        handle
            .call(|o, caller| {
                let human = fixtures::human();
                let ai = Initiator::Ai {
                    provider: id("codex"),
                    os_session: fixtures::os_session(),
                    config: VersionedRef {
                        id: id("test-config"),
                        revision: id("1"),
                    },
                    conversation: id("conversation-test"),
                    tool_call: id("call-test"),
                };
                for (index, origin) in [human, ai].into_iter().enumerate() {
                    add(o, caller, index, true, origin.clone())?;
                    let request = RequestId::new(format!("page-{index:03}")).unwrap();
                    let p = o.app.frozen_plan(caller, &request)?;
                    let view = serde_json::to_value(o.request_view(caller, &request)?).unwrap();
                    assert_eq!(view["status"], "approval");
                    assert_eq!(
                        view["plan"]["actor"],
                        serde_json::to_value(&p.spec().request.actor).unwrap()
                    );
                    assert_eq!(
                        view["plan"]["authority"],
                        serde_json::to_value(&p.spec().request.authority).unwrap()
                    );
                    assert_eq!(
                        view["plan"]["initiator"],
                        serde_json::to_value(origin).unwrap()
                    );
                    assert_eq!(view["plan"]["digest"], p.digest().as_str());
                }
                Ok(())
            })
            .await
            .unwrap();
        handle.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn pages_cover_all_tasks_and_refresh_off_page_selection_without_dispatch() {
        let (handle, root) = fixture().await;
        handle
            .call(|o, caller| {
                for i in 0..130 {
                    add(o, caller, i, true, fixtures::human())?;
                }
                let first = o.snapshot(caller, ui::SnapshotQuery::default())?;
                assert_eq!(first.requests.len(), 128);
                let selected = first.requests[0].plan.request_id.clone();
                let second = o.snapshot(
                    caller,
                    ui::SnapshotQuery {
                        after: first.next,
                        request_ids: vec![selected.clone()],
                    },
                )?;
                assert_eq!(second.requests.len(), 2);
                assert!(second.next.is_none());
                assert_eq!(second.referenced_requests.len(), 1);
                assert_eq!(second.referenced_requests[0].plan.request_id, selected);
                assert_eq!(o.app.status(caller, &selected)?.attempts, 0);
                assert!(o
                    .snapshot(
                        caller,
                        ui::SnapshotQuery {
                            after: None,
                            request_ids: vec![selected; 4]
                        }
                    )
                    .is_err());
                Ok(())
            })
            .await
            .unwrap();
        handle.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn one_service_preserves_running_tasks_across_request_callers() {
        let (alice, root) = fixture().await;
        alice
            .call(|owner, caller| {
                let draft = ui::Draft {
                    instance_id: BINDING.into(),
                    request_id: RequestId::new("alice-running").unwrap(),
                    revision: 1,
                    catalog: owner.catalog.reference(),
                    item_id: id("maintenance"),
                    variant_id: id("test"),
                    fields: Default::default(),
                };
                let selected =
                    selection::select(&owner.catalog, &draft).map_err(|_| Error::InvalidInput)?;
                owner.preview(caller, &draft.request_id, selected, fixtures::human())?;
                owner.submit(caller, &draft.request_id)?;
                Ok(())
            })
            .await
            .unwrap();
        let bob = alice.for_caller("bob").unwrap();
        assert!(std::sync::Arc::ptr_eq(&alice.finished, &bob.finished));
        for index in 0..100 {
            let view = alice.for_caller(&format!("user-{index}")).unwrap();
            assert!(std::sync::Arc::ptr_eq(&alice.finished, &view.finished));
        }
        assert!(bob
            .details(RequestId::new("alice-running").unwrap())
            .await
            .is_err());
        bob.call(|owner, caller| {
            assert!(owner.app.tasks(caller, None, 128)?.items.is_empty());
            Ok(())
        })
        .await
        .unwrap();
        alice
            .call(|owner, caller| {
                let request = RequestId::new("alice-running").unwrap();
                let plan = owner.app.frozen_plan(caller, &request)?;
                assert_eq!(plan.spec().request.actor.as_str(), "fixture-actor");
                assert_eq!(
                    owner.app.status(caller, &request)?.phase,
                    TaskPhase::Running
                );
                assert_eq!(
                    owner
                        .app
                        .frozen_plan(caller, &request)?
                        .spec()
                        .request
                        .actor
                        .as_str(),
                    "fixture-actor"
                );
                Ok(())
            })
            .await
            .unwrap();
        alice.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
}
