//! Production IPC consumer. The desktop owns neither an execution journal nor a runner.
use crate::self_service as ui;
use execution_app::{BackendTaskView, Error, ExecutionStatus, TaskPhase};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply, Request, ServiceClient};
use execution_mcp as mcp;
use sha2::Digest as _;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

pub fn now() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Clock)?
        .as_millis()
        .try_into()
        .map_err(|_| Error::Clock)
}
fn bad(_: Error) -> ui::ServiceError {
    ui::error("execution_unavailable", "执行服务未确认操作，请查询原任务")
}
fn relation_error(phase: CurrentUserSyncPhase) -> ui::ServiceError {
    match phase {
        CurrentUserSyncPhase::Conflict => ui::error(
            "binding_conflict",
            "当前使用者发生冲突，请明确清理后重新登录",
        ),
        CurrentUserSyncPhase::Invalidated => {
            ui::error("binding_invalidated", "设备身份已变化，请重新登录")
        }
        CurrentUserSyncPhase::CleanupPending => {
            ui::error("cleanup_pending", "退出清理尚未确认，请重试原操作")
        }
        _ => ui::error("binding_unknown", "登录确认尚未完成，请重试原操作"),
    }
}
fn mcp_error(error: Error) -> mcp::ServiceError {
    eprintln!("RSS_EXECUTION_MCP_FAILURE code={error:?}");
    match error {
        Error::Denied | Error::Unbound => mcp::ServiceError::Denied,
        Error::NotFound => mcp::ServiceError::NotFound,
        Error::Conflict => mcp::ServiceError::Conflict,
        Error::InvalidInput => mcp::ServiceError::InvalidInput,
        Error::OutcomeUnknown => mcp::ServiceError::OutcomeUnknown,
        _ => mcp::ServiceError::Unavailable,
    }
}
#[derive(Clone)]
pub struct ExecutionHandle {
    users: Option<Arc<Mutex<super::users::Users>>>,
    generation: Option<String>,
    origin: ClientOrigin,
    permits: Arc<tokio::sync::Semaphore>,
    wire: Arc<Mutex<()>>,
    context_possible: Arc<AtomicBool>,
    context_initialized: Arc<AtomicBool>,
    #[cfg(test)]
    context_transport: Option<ContextTransport>,
}
#[cfg(test)]
type ContextTransport = Arc<dyn Fn(Request) -> Result<Reply, Error> + Send + Sync>;
impl ExecutionHandle {
    pub fn new() -> Self {
        Self {
            users: None,
            generation: None,
            origin: ClientOrigin::Desktop {},
            permits: Arc::new(tokio::sync::Semaphore::new(16)),
            wire: Arc::new(Mutex::new(())),
            context_possible: Arc::new(AtomicBool::new(false)),
            context_initialized: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            context_transport: None,
        }
    }
    pub fn with_trusted_users(mut self, users: Arc<Mutex<super::users::Users>>) -> Self {
        self.users = Some(users);
        self
    }
    #[cfg(test)]
    pub(super) fn with_context_transport(
        mut self,
        send: impl Fn(Request) -> Result<Reply, Error> + Send + Sync + 'static,
    ) -> Self {
        self.context_transport = Some(Arc::new(send));
        self
    }
    fn exchange(
        &self,
        request: Request,
        send: impl Fn(Request) -> Result<Reply, Error>,
    ) -> Result<Reply, Error> {
        if matches!(&request, Request::BindContext { .. }) {
            // Once transport can begin, retain uncertainty until a separate revoke ACK.
            self.context_possible.store(true, Ordering::Release);
            if !self.context_initialized.load(Ordering::Acquire) {
                match send(Request::RevokeContext {})? {
                    Reply::ContextRevoked => {
                        self.context_initialized.store(true, Ordering::Release)
                    }
                    _ => return Err(Error::OutcomeUnknown),
                }
            }
        }
        send(request)
    }
    pub fn for_caller(&self, actor: &str) -> Result<Self, Error> {
        let current = self
            .users
            .as_ref()
            .ok_or(Error::Unbound)?
            .lock()
            .map_err(|_| Error::Unavailable)?
            .current()
            .map_err(|_| Error::Unbound)?;
        if current.user.user_id.as_str() != actor {
            return Err(Error::Denied);
        }
        let mut view = self.clone();
        view.generation = Some(current.generation.to_string());
        Ok(view)
    }
    fn current(&self) -> Result<(), Error> {
        if let Some(generation) = &self.generation {
            self.users
                .as_ref()
                .ok_or(Error::Unbound)?
                .lock()
                .map_err(|_| Error::Unavailable)?
                .require(generation)
                .map_err(|_| Error::Unbound)?;
        }
        Ok(())
    }
    async fn request(&self, request: Request) -> Result<Reply, Error> {
        self.current()?;
        let permit = self
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Capacity)?;
        let wire = self.wire.clone();
        let bound = self.clone();
        let reply = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            // One short-lived native connection at a time. The existing admission semaphore
            // bounds both waiting and active transport; these handles own no business queue.
            let _wire = wire.lock().map_err(|_| Error::Unavailable)?;
            bound.current()?;
            #[cfg(test)]
            if let Some(send) = &bound.context_transport {
                return bound.exchange(request, |request| send(request));
            }
            let client = ServiceClient::installed().inspect_err(|error| {
                eprintln!("RSS_EXECUTION_IPC_FAILURE stage=installation code={error:?}");
            })?;
            // An unavailable protected installation has not attempted a handoff.
            bound
                .exchange(request, |request| client.request(request))
                .inspect_err(|error| {
                    eprintln!("RSS_EXECUTION_IPC_FAILURE stage=request code={error:?}");
                })
        })
        .await
        .map_err(|_| Error::OutcomeUnknown)??;
        self.current()?;
        match reply {
            Reply::Rejected => Err(Error::Denied),
            Reply::Unavailable => Err(Error::OutcomeUnknown),
            reply => Ok(reply),
        }
    }
    pub async fn revoke_context(&self) -> ui::Result<()> {
        if !self.context_possible.load(Ordering::Acquire) {
            return Ok(());
        }
        let handle = self.clone();
        tokio::task::spawn_blocking(move || {
            // This control request never takes the business semaphore or transport lock.
            #[cfg(test)]
            let reply = match &handle.context_transport {
                Some(send) => send(Request::RevokeContext {})?,
                None => ServiceClient::installed()?.request(Request::RevokeContext {})?,
            };
            #[cfg(not(test))]
            let reply = ServiceClient::installed()?.request(Request::RevokeContext {})?;
            match reply {
                Reply::ContextRevoked => {
                    handle.context_possible.store(false, Ordering::Release);
                    Ok(())
                }
                _ => Err(Error::OutcomeUnknown),
            }
        })
        .await
        .map_err(|_| bad(Error::OutcomeUnknown))?
        .map_err(bad)
    }
    pub async fn bind_context(
        &self,
        value: execution_ipc::host::NativeContextSession,
        expected: Option<execution_contract::Id>,
    ) -> ui::Result<execution_contract::NativeContextRef> {
        match self
            .request(Request::BindContext { value, expected })
            .await
            .map_err(bad)?
        {
            Reply::ContextBound { value } => Ok(value),
            Reply::CurrentUserSync { value } => Err(relation_error(value.phase)),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn retire_enterprise(
        &self,
        generation: Option<Id>,
        proof: Option<zeroize::Zeroizing<String>>,
        recover: bool,
    ) -> ui::Result<()> {
        match self
            .request(Request::RetireEnterpriseContext {
                generation,
                proof,
                recover,
            })
            .await
            .map_err(bad)?
        {
            Reply::CurrentUserSync { value }
                if matches!(
                    value.phase,
                    CurrentUserSyncPhase::Unbound | CurrentUserSyncPhase::Invalidated
                ) =>
            {
                Ok(())
            }
            Reply::CurrentUserSync { value } => Err(relation_error(value.phase)),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn independent_login_allowed(&self) -> bool {
        #[cfg(test)]
        if self.context_transport.is_some() {
            return false;
        }
        tokio::task::spawn_blocking(|| {
            matches!(
                ServiceClient::inspect(),
                execution_ipc::host::ServiceView::NotInstalled
                    | execution_ipc::host::ServiceView::Connected {
                        status: execution_ipc::host::ServiceStatus {
                            readiness: execution_ipc::host::Readiness::RegistrationRequired,
                            ..
                        }
                    }
            )
        })
        .await
        .unwrap_or(false)
    }
    fn context_ref(&self) -> ui::Result<execution_contract::NativeContextRef> {
        let snapshot = self
            .users
            .as_ref()
            .ok_or_else(|| bad(Error::Unbound))?
            .lock()
            .map_err(|_| bad(Error::Unavailable))?
            .current()?;
        if self.generation.as_deref() != Some(snapshot.generation.as_str()) {
            return Err(bad(Error::Unbound));
        }
        Ok(execution_contract::NativeContextRef {
            id: execution_contract::Id::new(
                snapshot
                    .execution_binding
                    .as_ref()
                    .ok_or_else(|| bad(Error::Unbound))?
                    .as_str(),
            )
            .map_err(|_| bad(Error::Unbound))?,
            generation: execution_contract::Id::new(snapshot.generation.as_str())
                .map_err(|_| bad(Error::Unbound))?,
        })
    }
    async fn request_data(&self, input: DataOperation) -> Result<DataResponse, mcp::ServiceError> {
        let context = self.context_ref().map_err(|_| mcp::ServiceError::Unbound)?;
        match self
            .request(Request::Data {
                context,
                origin: self.origin.clone(),
                input,
            })
            .await
            .map_err(mcp_error)?
        {
            Reply::Data { value } => Ok(value),
            Reply::DataFailure { reason } => Err(match reason {
                DataError::InvalidInput => mcp::ServiceError::InvalidInput,
                DataError::Denied => mcp::ServiceError::Denied,
                DataError::NotFound => mcp::ServiceError::NotFound,
                DataError::Conflict => mcp::ServiceError::Conflict,
                DataError::Expired => mcp::ServiceError::Expired,
                DataError::Unsupported => mcp::ServiceError::Unsupported,
                DataError::InProgress => mcp::ServiceError::InProgress,
                DataError::Capacity => mcp::ServiceError::Limit,
                DataError::Unavailable => mcp::ServiceError::Unavailable,
            }),
            _ => Err(mcp::ServiceError::Unavailable),
        }
    }
    pub async fn query_directory(&self, input: QueryDirectory) -> ui::Result<QueryPage> {
        match self
            .request_data(DataOperation::Directory(input))
            .await
            .map_err(data_error)?
        {
            DataResponse::Directory(value) => Ok(value),
            _ => Err(bad(Error::Unavailable)),
        }
    }
    pub async fn data_read(&self, input: DataRead) -> ui::Result<CollectedData> {
        match self
            .request_data(DataOperation::Read(input))
            .await
            .map_err(data_error)?
        {
            DataResponse::Read(value) => Ok(value),
            _ => Err(bad(Error::Unavailable)),
        }
    }
    pub async fn query_submit(&self, input: QuerySubmit) -> ui::Result<QueryStatus> {
        match self
            .request_data(DataOperation::Submit(input))
            .await
            .map_err(data_error)?
        {
            DataResponse::Status(value) => Ok(value),
            _ => Err(bad(Error::Unavailable)),
        }
    }
    pub async fn query_status(&self, input: QueryRequest) -> ui::Result<QueryStatus> {
        match self
            .request_data(DataOperation::Status(input))
            .await
            .map_err(data_error)?
        {
            DataResponse::Status(value) => Ok(value),
            _ => Err(bad(Error::Unavailable)),
        }
    }
    pub async fn query_cancel(&self, input: QueryRequest) -> ui::Result<QueryStatus> {
        match self
            .request_data(DataOperation::Cancel(input))
            .await
            .map_err(data_error)?
        {
            DataResponse::Status(value) => Ok(value),
            _ => Err(bad(Error::Unavailable)),
        }
    }
    pub async fn policy_directory(
        &self,
        input: ui::PolicyDirectoryQuery,
    ) -> ui::Result<rss_mdm_agent_wire::runtime::SelfServiceDirectoryResponse> {
        match self
            .request(Request::PolicyDirectory {
                context: self.context_ref()?,
                after: input.after,
                origin: self.origin.clone(),
            })
            .await
            .map_err(bad)?
        {
            Reply::PolicyDirectory { value } => Ok(value),
            _ => Err(bad(Error::Denied)),
        }
    }
    pub async fn invoke_policy(&self, choice: PolicyChoice) -> ui::Result<BackendRequest> {
        match self
            .request(Request::InvokePolicy {
                context: self.context_ref()?,
                choice,
                origin: self.origin.clone(),
            })
            .await
            .map_err(bad)?
        {
            Reply::PolicyRequest { value } => Ok(*value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    async fn policy_call(
        &self,
        name: &str,
        args: serde_json::Map<String, serde_json::Value>,
        draft: bool,
    ) -> Result<BackendRequest, mcp::ServiceError> {
        let ClientOrigin::Ai { tool_call, .. } = &self.origin else {
            return Err(mcp::ServiceError::Denied);
        };
        let request = RequestId::new(
            uuid::Uuid::parse_str(tool_call.as_str())
                .map_err(|_| mcp::ServiceError::InvalidInput)?
                .to_string(),
        )
        .map_err(|_| mcp::ServiceError::InvalidInput)?;
        let mut after = None;
        for _ in 0..16 {
            let page = self
                .policy_directory(ui::PolicyDirectoryQuery { after })
                .await
                .map_err(|_| mcp::ServiceError::Unavailable)?;
            for item in page.items {
                if mcp::policy_tool_name(&item) == name {
                    mcp::policy_tool(&item)?;
                    let choice = PolicyChoice {
                        request,
                        policy_id: Id::new(item.policy_id.to_string())
                            .map_err(|_| mcp::ServiceError::InvalidInput)?,
                        policy_revision: item.policy_revision,
                        policy_version: Id::new(item.policy_version.to_string())
                            .map_err(|_| mcp::ServiceError::InvalidInput)?,
                        parameters: args.into_iter().collect(),
                    };
                    return if draft {
                        self.prepare_policy(choice).await
                    } else {
                        self.invoke_policy(choice).await
                    }
                    .map_err(|_| mcp::ServiceError::OutcomeUnknown);
                }
            }
            after = page
                .next_cursor
                .map(|id| Id::new(id.to_string()))
                .transpose()
                .map_err(|_| mcp::ServiceError::InvalidInput)?;
            if after.is_none() {
                break;
            }
        }
        Err(mcp::ServiceError::Denied)
    }
    pub async fn prepare_policy(&self, choice: PolicyChoice) -> ui::Result<BackendRequest> {
        match self
            .request(Request::PreparePolicy {
                context: self.context_ref()?,
                choice,
                origin: self.origin.clone(),
            })
            .await
            .map_err(bad)?
        {
            Reply::PolicyRequest { value } => Ok(*value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn submit_policy(
        &self,
        input: execution_contract::PolicySubmission,
    ) -> ui::Result<BackendRequest> {
        match self
            .request(Request::SubmitPolicy {
                context: self.context_ref()?,
                input,
            })
            .await
            .map_err(bad)?
        {
            Reply::PolicyRequest { value } => Ok(*value),
            Reply::OperationRejected { reason } => Err(ui::error(
                match reason {
                    OperationRejection::InvalidParameters => "operation_invalid_parameters",
                    OperationRejection::Conflict => "operation_conflict",
                    OperationRejection::Expired => "operation_expired",
                    OperationRejection::Denied => "operation_denied",
                    OperationRejection::Unsupported => "operation_unsupported",
                },
                "操作未提交，请核对原准备和当前参数",
            )),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn confirm_policy(&self, input: ui::ActionRef) -> ui::Result<BackendRequest> {
        match self
            .request(Request::ConfirmPolicy {
                context: self.context_ref()?,
                request: input.request_id,
            })
            .await
            .map_err(bad)?
        {
            Reply::PolicyRequest { value } => Ok(*value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn policy_status(&self, input: ui::ActionRef) -> ui::Result<ui::PolicyStatus> {
        match self
            .request(Request::PolicyStatus {
                context: self.context_ref()?,
                request: input.request_id,
            })
            .await
            .map_err(bad)?
        {
            Reply::PolicyRequest { value } => Ok(ui::PolicyStatus::Preparation { value }),
            Reply::PolicyStatus { value } => Ok(ui::PolicyStatus::Run {
                value: Box::new(value),
            }),
            Reply::PolicyAbsent => Ok(ui::PolicyStatus::Absent),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn observe_software(&self, choice: SoftwareReadChoice) -> ui::Result<BackendRequest> {
        match self
            .request(Request::ObserveSoftware {
                choice,
                origin: self.origin.clone(),
            })
            .await
            .map_err(bad)?
        {
            Reply::PolicyRequest { value } => Ok(*value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn close(&self) {
        self.permits.close();
    }
    pub async fn snapshot(&self, query: ui::SnapshotQuery) -> ui::Result<ui::Snapshot> {
        match self
            .request(Request::Tasks { after: query.after })
            .await
            .map_err(bad)?
        {
            Reply::Tasks {
                value,
                available,
                preparations,
            } => Ok(ui::Snapshot {
                available,
                preparations,
                selected: match query.selected {
                    Some(request) => self.details(request).await.ok(),
                    None => None,
                },
                requests: value.items,
                next: value.next,
            }),
            _ => Err(bad(Error::InvalidInput)),
        }
    }
    async fn select(&self, input: BackendSelection) -> Result<TaskSubmission, Error> {
        let expected = input.clone();
        let reply = self
            .request(Request::StartTask {
                request: input.request,
                task: input.task,
                attempt: input.attempt,
                revision: input.revision,
                origin: self.origin.clone(),
            })
            .await?;
        selection_reply(&expected, reply)
    }
    pub async fn execute_ui(&self, input: BackendSelection) -> ui::Result<TaskSubmission> {
        if !matches!(self.origin, ClientOrigin::Desktop {}) {
            return Err(bad(Error::Denied));
        }
        self.select(input).await.map_err(bad)
    }
    pub async fn confirm_ui(&self, input: BackendSelection) -> ui::Result<TaskSubmission> {
        if !matches!(self.origin, ClientOrigin::Desktop {}) {
            return Err(bad(Error::Denied));
        }
        let expected = input.clone();
        let reply = self
            .request(Request::ConfirmTask { selection: input })
            .await
            .map_err(bad)?;
        selection_reply(&expected, reply).map_err(bad)
    }
    pub async fn cancel_ui(&self, input: ui::ActionRef) -> ui::Result<BackendTaskView> {
        self.cancel_task(input.request_id).await.map_err(bad)
    }
    pub async fn details(&self, request: RequestId) -> Result<BackendTaskView, Error> {
        match self.request(Request::Details { request }).await? {
            Reply::Details { value } => Ok(BackendTaskView::Execution { value }),
            Reply::Pending { value } => Ok(BackendTaskView::Pending { value }),
            _ => Err(Error::InvalidInput),
        }
    }
    pub async fn cancel_task(&self, request: RequestId) -> Result<BackendTaskView, Error> {
        match self
            .request(Request::Cancel {
                request: request.clone(),
            })
            .await?
        {
            Reply::Status { .. } => self.details(request).await,
            Reply::Pending { value } => Ok(BackendTaskView::Pending { value }),
            _ => Err(Error::OutcomeUnknown),
        }
    }
}
impl Default for ExecutionHandle {
    fn default() -> Self {
        Self::new()
    }
}
fn operation(s: ExecutionStatus) -> mcp::OperationStatus {
    mcp::OperationStatus {
        mode: s.mode,
        process: s.process,
        assessment: s.assessment,
        cancel_requested: s.cancel_requested,
        operation_request_id: s.operation_request_id,
        content_digest: s.content_digest,
        phase: if s.admission == Some(execution_app::AdmissionStatus::Denied) {
            mcp::OperationPhase::Failed
        } else {
            match s.phase {
                TaskPhase::Accepted => mcp::OperationPhase::Accepted,
                TaskPhase::Waiting
                | TaskPhase::ApprovalRequired
                | TaskPhase::ConfirmationRequired => mcp::OperationPhase::Waiting,
                TaskPhase::Running => mcp::OperationPhase::Running,
                TaskPhase::OutcomeUnknown => mcp::OperationPhase::OutcomeUnknown,
                TaskPhase::ExecutionEnded => mcp::OperationPhase::ExecutionEnded,
                TaskPhase::Verified => mcp::OperationPhase::Verified,
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
        let current = self
            .users
            .as_ref()
            .ok_or(mcp::ServiceError::Unbound)?
            .lock()
            .map_err(|_| mcp::ServiceError::Unavailable)?
            .current()
            .map_err(|_| mcp::ServiceError::Unbound)?;
        let mut view = self
            .for_caller(current.user.user_id.as_str())
            .map_err(mcp_error)?;
        view.origin = super::origin::bind(&current, metadata)?;
        Ok(Arc::new(view))
    }
    fn check_binding(&self) -> Result<(), mcp::ServiceError> {
        self.current().map_err(mcp_error)?;
        if self.users.is_none() {
            return Err(mcp::ServiceError::Unbound);
        }
        Ok(())
    }
    async fn tasks(&self, _: CancellationToken) -> Result<Vec<BackendTask>, mcp::ServiceError> {
        match self
            .request(Request::Tasks { after: None })
            .await
            .map_err(mcp_error)?
        {
            Reply::Tasks { available, .. } => Ok(available),
            _ => Err(mcp::ServiceError::Unavailable),
        }
    }
    async fn collected_data(
        &self,
        mut input: DataOperation,
        _: CancellationToken,
    ) -> Result<DataResponse, mcp::ServiceError> {
        let ClientOrigin::Ai { tool_call, .. } = &self.origin else {
            return Err(mcp::ServiceError::Denied);
        };
        if let DataOperation::Submit(submit) = &mut input {
            // Submission identity belongs to the authenticated Host, never to model arguments.
            submit.request = RequestId::new(
                uuid::Uuid::parse_str(tool_call.as_str())
                    .map_err(|_| mcp::ServiceError::InvalidInput)?
                    .to_string(),
            )
            .map_err(|_| mcp::ServiceError::InvalidInput)?;
        }
        self.request_data(input).await
    }
    async fn observe_software(
        &self,
        input: SoftwareReadSelection,
        _: CancellationToken,
    ) -> Result<BackendRequest, mcp::ServiceError> {
        let ClientOrigin::Ai { tool_call, .. } = &self.origin else {
            return Err(mcp::ServiceError::Denied);
        };
        let request = RequestId::new(
            uuid::Uuid::parse_str(tool_call.as_str())
                .map_err(|_| mcp::ServiceError::InvalidInput)?
                .to_string(),
        )
        .map_err(|_| mcp::ServiceError::InvalidInput)?;
        self.observe_software(SoftwareReadChoice { request, input })
            .await
            .map_err(|_| mcp::ServiceError::Unavailable)
    }
    async fn policy_tools(
        &self,
        _: CancellationToken,
    ) -> Result<Vec<serde_json::Value>, mcp::ServiceError> {
        if !matches!(self.origin, ClientOrigin::Ai { .. }) {
            return Err(mcp::ServiceError::Denied);
        }
        let mut after = None;
        let mut tools = Vec::new();
        for _ in 0..16 {
            let page = self
                .policy_directory(ui::PolicyDirectoryQuery { after })
                .await
                .map_err(|_| mcp::ServiceError::Unavailable)?;
            for item in page.items {
                match mcp::policy_tool(&item) {
                    Ok(tool) => tools.push(tool),
                    Err(mcp::ServiceError::InvalidInput) => {
                        eprintln!(
                            "RSS_POLICY_TOOL_UNSUPPORTED name={}",
                            mcp::policy_tool_name(&item)
                        );
                    }
                    Err(error) => return Err(error),
                }
            }
            after = page
                .next_cursor
                .map(|id| Id::new(id.to_string()))
                .transpose()
                .map_err(|_| mcp::ServiceError::InvalidInput)?;
            if after.is_none() {
                return Ok(tools);
            }
            if tools.len() >= 128 {
                return Err(mcp::ServiceError::Limit);
            }
        }
        Err(mcp::ServiceError::Limit)
    }
    async fn invoke_policy(
        &self,
        name: &str,
        args: serde_json::Map<String, serde_json::Value>,
        _: CancellationToken,
    ) -> Result<BackendRequest, mcp::ServiceError> {
        self.policy_call(name, args, false).await
    }
    async fn prepare_operation(
        &self,
        input: mcp::OperationProposal,
        _: CancellationToken,
    ) -> Result<BackendRequest, mcp::ServiceError> {
        self.policy_call(&input.tool, input.parameters, true).await
    }

    async fn policy_status(
        &self,
        request: mcp::OperationRequest,
        _: CancellationToken,
    ) -> Result<serde_json::Value, mcp::ServiceError> {
        let value = self
            .policy_status(ui::ActionRef {
                request_id: request.operation_request_id,
            })
            .await
            .map_err(|_| mcp::ServiceError::Unavailable)?;
        if matches!(value, ui::PolicyStatus::Absent) {
            return Err(mcp::ServiceError::NotFound);
        }
        serde_json::to_value(value).map_err(|_| mcp::ServiceError::Unavailable)
    }
    async fn capabilities(
        &self,
        _: CancellationToken,
    ) -> Result<mcp::CapabilityView, mcp::ServiceError> {
        self.request(Request::Tasks { after: None })
            .await
            .map_err(mcp_error)?;
        Ok(mcp::CapabilityView {
            state: mcp::CapabilityState::Supported,
            reasons: vec![Id::new("backend-task-selection").expect("constant")],
        })
    }
    async fn execute(
        &self,
        request: BackendSelection,
        _: CancellationToken,
    ) -> Result<TaskSubmission, mcp::ServiceError> {
        if !matches!(self.origin, ClientOrigin::Ai { .. }) {
            return Err(mcp::ServiceError::Denied);
        }
        self.select(request).await.map_err(mcp_error)
    }
    async fn status(
        &self,
        request: mcp::OperationRequest,
        _: CancellationToken,
    ) -> Result<mcp::OperationStatus, mcp::ServiceError> {
        match self
            .request(Request::Status {
                request: request.operation_request_id,
            })
            .await
            .map_err(mcp_error)?
        {
            Reply::Status { value } => Ok(operation(value)),
            Reply::Pending { value } => Ok(pending_operation(*value)),
            _ => Err(mcp::ServiceError::Unavailable),
        }
    }
    async fn cancel(
        &self,
        request: mcp::OperationRequest,
        _: CancellationToken,
    ) -> Result<mcp::CancelResult, mcp::ServiceError> {
        Ok(mcp::CancelResult {
            disposition: mcp::CancelDisposition::Requested,
            operation: match self
                .cancel_task(request.operation_request_id)
                .await
                .map_err(mcp_error)?
            {
                BackendTaskView::Execution { value } => operation(value.status),
                BackendTaskView::Pending { value } => pending_operation(*value),
            },
        })
    }
}

fn selection_reply(input: &BackendSelection, reply: Reply) -> Result<TaskSubmission, Error> {
    match reply {
        Reply::Queued {
            request,
            confirmation_required,
            ..
        } if request == input.request => Ok(TaskSubmission {
            request,
            confirmation_required,
        }),
        Reply::Pending { value }
            if value.action.offer().ok_or(Error::InvalidInput)?.request == input.request
                && value.action.offer().ok_or(Error::InvalidInput)?.task == input.task
                && value.action.offer().ok_or(Error::InvalidInput)?.attempt == input.attempt
                && value.action.offer().ok_or(Error::InvalidInput)?.revision == input.revision =>
        {
            if matches!(
                value.state,
                BackendRequestState::Failed | BackendRequestState::Cancelled
            ) {
                return Err(Error::Denied);
            }
            Ok(TaskSubmission {
                request: value.action.request().clone(),
                confirmation_required: value.state == BackendRequestState::AwaitingConfirmation,
            })
        }
        _ => Err(Error::OutcomeUnknown),
    }
}

fn pending_operation(value: execution_contract::BackendRequest) -> mcp::OperationStatus {
    use execution_contract::BackendRequestState;
    mcp::OperationStatus {
        mode: execution_lifecycle::ExecutionMode::Real,
        process: None,
        assessment: None,
        cancel_requested: value.state == BackendRequestState::Cancelled,
        operation_request_id: value.action.request().clone(),
        content_digest: value
            .action
            .offer()
            .map(|o| o.revision.clone())
            .unwrap_or_else(|| {
                Digest::new(format!(
                    "{:x}",
                    sha2::Sha256::digest(
                        serde_json::to_vec(&value.action).expect("typed preparation")
                    )
                ))
                .expect("digest")
            }),
        phase: match value.state {
            BackendRequestState::Observed => mcp::OperationPhase::ExecutionEnded,
            BackendRequestState::Failed => mcp::OperationPhase::Failed,
            BackendRequestState::Cancelled => mcp::OperationPhase::Cancelled,
            _ => mcp::OperationPhase::Waiting,
        },
        attempt_id: None,
        evidence: vec![],
    }
}

/// Ask the protected installation owner to start this configured login's helper.
pub fn install_user_helper() -> Result<(), Error> {
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    #[derive(serde::Deserialize)]
    struct Pin {
        version: u32,
        ipc_version: u8,
        service: installation_security::Artifact,
        helper_work_roots: BTreeMap<String, PathBuf>,
    }
    let path = installation_security::deployment_path().map_err(|_| Error::Configuration)?;
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(Error::Unavailable),
        Ok(_) => (),
    }
    let pin: Pin = serde_json::from_slice(
        &installation_security::read_protected(&path).map_err(|_| Error::Denied)?,
    )
    .map_err(|_| Error::Configuration)?;
    if pin.version != execution_ipc::host::DEPLOYMENT_VERSION
        || pin.ipc_version != execution_ipc::host::IPC_VERSION
    {
        return Err(Error::Configuration);
    }
    let subject = execution_ipc::host::current_subject()?;
    if !pin.helper_work_roots.contains_key(&subject) {
        return Ok(());
    }
    pin.service
        .verify(&pin.service.path)
        .map_err(|_| Error::Denied)?;
    installation_security::Target::current()
        .map_err(|_| Error::Denied)?
        .verify_image(&pin.service.path)
        .map_err(|_| Error::Denied)?;
    let mut child = Command::new(&pin.service.path)
        .arg("--installation")
        .arg("install")
        .arg("--scope")
        .arg("user")
        .arg("--binary")
        .arg(&pin.service.path)
        .arg("--config")
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Error::Unavailable)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().map_err(|_| Error::Unavailable)? {
            return if status.success() {
                Ok(())
            } else {
                Err(Error::Unavailable)
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn data_error(error: mcp::ServiceError) -> ui::ServiceError {
    ui::error(
        match error {
            mcp::ServiceError::InvalidInput => "data_invalid_input",
            mcp::ServiceError::Denied => "data_denied",
            mcp::ServiceError::NotFound => "data_not_found",
            mcp::ServiceError::Conflict => "data_conflict",
            mcp::ServiceError::InProgress => "data_in_progress",
            mcp::ServiceError::Expired => "data_expired",
            mcp::ServiceError::Unsupported => "data_unsupported",
            mcp::ServiceError::Limit => "data_capacity",
            _ => "data_unavailable",
        },
        "本地数据操作未完成，请根据原请求状态处理",
    )
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn lost_selection_reply_recovers_original_request_and_confirmation() {
        let selection = BackendSelection {
            request: RequestId::new("original-request").unwrap(),
            task: Id::new("original-task").unwrap(),
            attempt: Id::new("original-attempt").unwrap(),
            revision: Digest::new("a".repeat(64)).unwrap(),
        };
        let original = BackendRequest {
            action: PreparedAction::Offered {
                offer: BackendTask {
                    request: selection.request.clone(),
                    task: selection.task.clone(),
                    attempt: selection.attempt.clone(),
                    revision: selection.revision.clone(),
                    presentation: execution_contract::BackendPresentation {
                        title: "original".into(),
                        summary: BackendTaskSummary::Script {
                            identity: BackendIdentity::System,
                        },
                        extra: Default::default(),
                    },
                    expires_at: 100,
                    user_initiated: true,
                },
            },
            trigger: BackendTrigger::Automatic {},
            risk: None,
            confirmation: None,
            revision: 1,
            state: BackendRequestState::AwaitingConfirmation,
            failure: None,
        };
        for state in [
            BackendRequestState::AwaitingConfirmation,
            BackendRequestState::Ready,
            BackendRequestState::Submitting,
            BackendRequestState::Failed,
            BackendRequestState::Cancelled,
        ] {
            let value = BackendRequest {
                state,
                ..original.clone()
            };
            let result = selection_reply(
                &selection,
                Reply::Pending {
                    value: Box::new(value),
                },
            );
            if matches!(
                state,
                BackendRequestState::Failed | BackendRequestState::Cancelled
            ) {
                assert_eq!(result.unwrap_err(), Error::Denied);
                continue;
            }
            let submission = result.unwrap();
            assert_eq!(submission.request, selection.request);
            assert_eq!(
                submission.confirmation_required,
                state == BackendRequestState::AwaitingConfirmation
            );
        }
        for field in 0..4 {
            let mut value = original.clone();
            let PreparedAction::Offered { offer } = &mut value.action else {
                unreachable!()
            };
            match field {
                0 => offer.request = RequestId::new("other-request").unwrap(),
                1 => offer.task = Id::new("other-task").unwrap(),
                2 => offer.attempt = Id::new("other-attempt").unwrap(),
                _ => offer.revision = Digest::new("b".repeat(64)).unwrap(),
            }
            assert!(selection_reply(
                &selection,
                Reply::Pending {
                    value: Box::new(value)
                }
            )
            .is_err());
        }
    }
}
