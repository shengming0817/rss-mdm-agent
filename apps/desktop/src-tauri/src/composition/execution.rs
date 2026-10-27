//! Production IPC consumer. The desktop owns neither an execution journal nor a runner.
use crate::self_service as ui;
use execution_app::{Error, ExecutionStatus, TaskPhase};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply, Request, ServiceClient};
use execution_mcp as mcp;
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
fn task_ui_error(error: Error) -> ui::ServiceError {
    eprintln!("RSS_TASK_FAILURE code={error:?}");
    let (code, message) = match error {
        Error::Denied | Error::NotFound | Error::Unbound => {
            ("task_denied", "原任务不可访问或授权已变化，操作未获准")
        }
        Error::Conflict => ("task_stale", "任务或策略版本已变化，操作未提交，请重新读取"),
        Error::InvalidInput => ("task_input", "任务请求无效，操作未提交"),
        Error::Unsupported => ("task_denied", "原 owner 当前阶段不支持此操作"),
        _ => ("task_unknown", "控制结果未知，请查询原操作"),
    };
    ui::error(code, message)
}
fn mcp_error(error: Error) -> mcp::ServiceError {
    eprintln!("RSS_EXECUTION_MCP_FAILURE code={error:?}");
    match error {
        Error::Denied | Error::Unbound => mcp::ServiceError::Denied,
        Error::NotFound => mcp::ServiceError::NotFound,
        Error::Conflict => mcp::ServiceError::Conflict,
        Error::InvalidInput => mcp::ServiceError::InvalidInput,
        Error::Unsupported => mcp::ServiceError::Unsupported,
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
    pub async fn entry(&self, input: execution_ipc::entry::EntryOperation) -> ui::Result<Reply> {
        if !matches!(self.origin, ClientOrigin::Desktop {}) {
            return Err(ui::error("denied", "入口需要当前真实用户"));
        }
        self.request(Request::Entry { input })
            .await
            .map_err(task_ui_error)
    }
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
    // Runtime serializes identity transitions. Their protected native handshake
    // must remain available when business reads fill the existing bounded queue.
    async fn context_request(&self, request: Request) -> Result<Reply, Error> {
        let handle = self.clone();
        let method = if matches!(&request, Request::BindContext { .. }) {
            "bind"
        } else {
            "revoke"
        };
        let reply = tokio::task::spawn_blocking(move || {
            #[cfg(test)]
            if let Some(send) = &handle.context_transport {
                return handle.exchange(request, |request| send(request));
            }
            let client = ServiceClient::installed()?;
            handle.exchange(request, |request| client.request(request))
        })
        .await
        .map_err(|_| Error::OutcomeUnknown)?
        .inspect_err(|error| {
            eprintln!("RSS_EXECUTION_CONTEXT_FAILURE method={method} code={error:?}")
        })?;
        match reply {
            Reply::Rejected => {
                eprintln!("RSS_EXECUTION_CONTEXT_FAILURE method={method} code=Denied");
                Err(Error::Denied)
            }
            Reply::Unavailable => {
                eprintln!("RSS_EXECUTION_CONTEXT_FAILURE method={method} code=OutcomeUnknown");
                Err(Error::OutcomeUnknown)
            }
            value => Ok(value),
        }
    }
    pub async fn revoke_context(&self) -> ui::Result<()> {
        if !self.context_possible.load(Ordering::Acquire) {
            return Ok(());
        }
        match self
            .context_request(Request::RevokeContext {})
            .await
            .map_err(bad)?
        {
            Reply::ContextRevoked => {
                self.context_possible.store(false, Ordering::Release);
                Ok(())
            }
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn bind_context(
        &self,
        value: execution_ipc::host::NativeContextSession,
        expected: Option<execution_contract::Id>,
    ) -> ui::Result<execution_contract::NativeContextRef> {
        self.current().map_err(bad)?;
        let reply = self
            .context_request(Request::BindContext { value, expected })
            .await
            .map_err(bad)?;
        self.current().map_err(bad)?;
        match reply {
            Reply::ContextBound { value } => Ok(value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn current_user_status(&self) -> ui::Result<CurrentUserSync> {
        match self
            .request(Request::CurrentUserStatus {})
            .await
            .map_err(bad)?
        {
            Reply::CurrentUserSync { value } => Ok(value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn recover_current_user(&self) -> ui::Result<CurrentUserSync> {
        match self
            .request(Request::RetireEnterpriseContext {
                generation: None,
                proof: None,
                recover: true,
            })
            .await
            .map_err(bad)?
        {
            Reply::CurrentUserSync { value } => Ok(value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
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
    async fn software_request(
        &self,
        input: LocalSoftwareOperation,
    ) -> Result<Reply, mcp::ServiceError> {
        let reply = self
            .request(Request::LocalSoftware {
                context: self.context_ref().map_err(|_| mcp::ServiceError::Unbound)?,
                origin: self.origin.clone(),
                input,
            })
            .await
            .map_err(mcp_error)?;
        software_reply(reply)
    }
    pub async fn software_history(
        &self,
        input: SoftwareHistoryQuery,
    ) -> ui::Result<execution_app::LocalSoftwareHistory> {
        match self
            .software_request(LocalSoftwareOperation::History(input))
            .await
            .map_err(software_ui_error)?
        {
            Reply::SoftwareHistory { value } => Ok(value),
            _ => Err(bad(Error::Unavailable)),
        }
    }
    pub async fn device_diagnostics(&self) -> ui::Result<serde_json::Value> {
        let mdm = match self.request(Request::MdmStatus {}).await.map_err(bad)? {
            Reply::MdmStatus { value } => value,
            _ => return Err(bad(Error::Unavailable)),
        };
        let registration = match self
            .request(Request::RegistrationStatus {})
            .await
            .map_err(bad)?
        {
            Reply::RegistrationStatus { value } => value,
            _ => return Err(bad(Error::Unavailable)),
        };
        Ok(
            serde_json::json!({"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"available":true,"mdmState":mdm.state,"mdmObservedAt":mdm.observed_at,"registrationPhase":registration.phase,"credential":registration.credential,"registered":registration.registered}),
        )
    }
    pub async fn network_check(
        &self,
        input: NetworkCheckOperation,
    ) -> ui::Result<execution_app::NetworkCheckResponse> {
        if !matches!(self.origin, ClientOrigin::Desktop {}) {
            return Err(bad(Error::Denied));
        }
        match self
            .request(Request::NetworkCheck {
                context: self.context_ref()?,
                origin: self.origin.clone(),
                input,
            })
            .await
            .map_err(bad)?
        {
            Reply::NetworkDefaults { target } => {
                Ok(execution_app::NetworkCheckResponse::Defaults(target))
            }
            Reply::NetworkCheck { value } => {
                Ok(execution_app::NetworkCheckResponse::Status(Box::new(value)))
            }
            Reply::DataFailure { reason } => Err(ui::error(
                match reason {
                    DataError::InvalidInput => "network_invalid_input",
                    DataError::NotFound => "network_not_found",
                    DataError::Denied => "network_denied",
                    DataError::Conflict => "network_conflict",
                    _ => "network_unavailable",
                },
                "连接检查未完成，请核对目标或查询原请求",
            )),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn system_settings(
        &self,
        input: execution_contract::SettingsOperation,
    ) -> ui::Result<execution_contract::SettingsResponse> {
        if !matches!(self.origin, ClientOrigin::Desktop {}) {
            return Err(bad(Error::Denied));
        }
        let reply = self
            .request(Request::Settings {
                context: self.context_ref()?,
                origin: self.origin.clone(),
                input,
            })
            .await
            .map_err(bad)?;
        match software_reply(reply).map_err(software_ui_error)? {
            Reply::Settings { value } => Ok(value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn software_prepare(
        &self,
        input: LocalSoftwareChoice,
    ) -> ui::Result<LocalSoftwarePrepared> {
        match self
            .software_request(LocalSoftwareOperation::Prepare(input))
            .await
            .map_err(software_ui_error)?
        {
            Reply::SoftwarePrepared { value } => Ok(value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn software_submit(
        &self,
        input: LocalSoftwareSubmission,
    ) -> ui::Result<execution_app::LocalSoftwareStatus> {
        match self
            .software_request(LocalSoftwareOperation::Submit(input))
            .await
            .map_err(software_ui_error)?
        {
            Reply::SoftwareStatus { value } => Ok(value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn software_status(
        &self,
        input: QueryRequest,
    ) -> ui::Result<execution_app::LocalSoftwareStatus> {
        match self
            .software_request(LocalSoftwareOperation::Status(input))
            .await
            .map_err(software_ui_error)?
        {
            Reply::SoftwareStatus { value } => Ok(value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    async fn read_software(&self, input: SoftwareRead) -> Result<SoftwarePage, mcp::ServiceError> {
        let reply = self
            .request(Request::SoftwareRead {
                context: self.context_ref().map_err(|_| mcp::ServiceError::Unbound)?,
                origin: self.origin.clone(),
                input,
            })
            .await
            .map_err(mcp_error)?;
        match software_reply(reply)? {
            Reply::SoftwarePage { value } => Ok(value),
            _ => Err(mcp::ServiceError::Unavailable),
        }
    }
    pub async fn software_read(&self, input: SoftwareRead) -> ui::Result<SoftwarePage> {
        self.read_software(input).await.map_err(software_ui_error)
    }
    pub async fn device_access(
        &self,
        input: execution_ipc::device_access::Operation,
    ) -> ui::Result<execution_ipc::device_access::Response> {
        if !matches!(self.origin, ClientOrigin::Desktop {}) {
            return Err(bad(Error::Denied));
        }
        #[cfg(target_os = "macos")]
        if matches!(
            input,
            execution_ipc::device_access::Operation::Begin(_)
                | execution_ipc::device_access::Operation::Confirm { .. }
                | execution_ipc::device_access::Operation::PrepareMac { .. }
        ) {
            let present = tauri::async_runtime::spawn_blocking(|| {
                unsafe extern "C" {
                    fn rss_device_access_presence() -> i32;
                }
                // SAFETY: synchronous native UI gate returns no credentials or retained pointers.
                unsafe { rss_device_access_presence() == 1 }
            })
            .await
            .map_err(|_| bad(Error::Denied))?;
            if !present {
                return Err(bad(Error::Denied));
            }
        }
        match self
            .request(Request::DeviceAccess {
                context: self.context_ref()?,
                origin: self.origin.clone(),
                input,
            })
            .await
            .map_err(bad)?
        {
            Reply::DeviceAccess { value } => Ok(value),
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
    pub async fn invoke_policy(&self, input: PolicyInvocation) -> ui::Result<BackendRequest> {
        match self
            .request(Request::InvokePolicy {
                context: self.context_ref()?,
                choice: input.choice,

                origin: self.origin.clone(),
            })
            .await
            .map_err(bad)?
        {
            Reply::PolicyRequest { value } => Ok(*value),
            _ => Err(bad(Error::OutcomeUnknown)),
        }
    }
    pub async fn policy_resolve(
        &self,
        selector: rss_mdm_agent_wire::runtime::CapabilitySelector,
    ) -> ui::Result<rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem> {
        self.resolve_policy(selector)
            .await
            .map_err(|_| bad(Error::Denied))
    }
    async fn resolve_policy(
        &self,
        selector: rss_mdm_agent_wire::runtime::CapabilitySelector,
    ) -> Result<rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem, mcp::ServiceError> {
        match self
            .request(Request::ResolvePolicy {
                context: self.context_ref().map_err(|_| mcp::ServiceError::Denied)?,
                selector,
                origin: self.origin.clone(),
            })
            .await
            .map_err(|_| mcp::ServiceError::Unavailable)?
        {
            Reply::PolicyResolved { value } => Ok(*value),
            _ => Err(mcp::ServiceError::Denied),
        }
    }
    async fn policy_call(
        &self,
        selector: rss_mdm_agent_wire::runtime::CapabilitySelector,
        input: mcp::PolicyToolInput,
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
        let invocation = PolicyInvocation {
            choice: PolicyChoice {
                request,
                policy_id: Id::new(self.resolve_policy(selector).await?.policy_id.to_string())
                    .map_err(|_| mcp::ServiceError::InvalidInput)?,
                parameters: input.parameters.into_iter().collect(),
            },
        };
        if draft {
            self.prepare_policy(invocation).await
        } else {
            self.invoke_policy(invocation).await
        }
        .map_err(|_| mcp::ServiceError::OutcomeUnknown)
    }
    pub async fn prepare_policy(&self, input: PolicyInvocation) -> ui::Result<BackendRequest> {
        match self
            .request(Request::PreparePolicy {
                context: self.context_ref()?,
                choice: input.choice,

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
                origin: self.origin.clone(),
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
    pub async fn offers_ui(&self, _: ()) -> ui::Result<Vec<BackendTask>> {
        self.offers().await
    }
    pub async fn offers(&self) -> ui::Result<Vec<BackendTask>> {
        match self.request(Request::Offers {}).await.map_err(bad)? {
            Reply::Offers { available } => Ok(available),
            _ => Err(bad(Error::Unavailable)),
        }
    }
    async fn task_request(
        &self,
        input: execution_app::tasks::TaskOperation,
    ) -> Result<execution_app::tasks::TaskResponse, Error> {
        match self
            .request(Request::Task {
                input,
                origin: self.origin.clone(),
            })
            .await?
        {
            Reply::Task { value } => Ok(value),
            Reply::DataFailure { reason } => {
                eprintln!("RSS_TASK_RESPONSE_FAILURE reason={reason:?}");
                Err(match reason {
                    DataError::InvalidInput => Error::InvalidInput,
                    DataError::Conflict => Error::Conflict,
                    DataError::Unsupported => Error::Unsupported,
                    DataError::Denied | DataError::NotFound | DataError::Expired => Error::Denied,
                    _ => Error::OutcomeUnknown,
                })
            }
            _ => {
                eprintln!("RSS_TASK_RESPONSE_FAILURE reason=UnexpectedReply");
                Err(Error::OutcomeUnknown)
            }
        }
    }
    pub async fn task_list(
        &self,
        input: execution_app::tasks::TaskQuery,
    ) -> ui::Result<execution_app::tasks::TaskPage> {
        match self
            .task_request(execution_app::tasks::TaskOperation::List(input))
            .await
            .map_err(task_ui_error)?
        {
            execution_app::tasks::TaskResponse::Page(value) => Ok(value),
            _ => Err(bad(Error::InvalidInput)),
        }
    }
    pub async fn task_detail(
        &self,
        input: execution_app::tasks::TaskRef,
    ) -> ui::Result<execution_app::tasks::TaskDetail> {
        match self
            .task_request(execution_app::tasks::TaskOperation::Detail(input))
            .await
            .map_err(task_ui_error)?
        {
            execution_app::tasks::TaskResponse::Detail(value) => Ok(*value),
            _ => Err(bad(Error::InvalidInput)),
        }
    }
    pub async fn task_control(
        &self,
        input: execution_app::tasks::TaskControl,
    ) -> ui::Result<execution_app::tasks::TaskControlReceipt> {
        match self
            .task_request(execution_app::tasks::TaskOperation::Control(input))
            .await
            .map_err(task_ui_error)?
        {
            execution_app::tasks::TaskResponse::Control(value) => Ok(value),
            _ => Err(bad(Error::InvalidInput)),
        }
    }
    pub async fn task_control_status(
        &self,
        input: execution_app::tasks::TaskControlQuery,
    ) -> ui::Result<execution_app::tasks::TaskControlReceipt> {
        match self
            .task_request(execution_app::tasks::TaskOperation::ControlStatus(input))
            .await
            .map_err(task_ui_error)?
        {
            execution_app::tasks::TaskResponse::Control(value) => Ok(value),
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
                TaskPhase::Waiting | TaskPhase::ConfirmationRequired => {
                    mcp::OperationPhase::Waiting
                }
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
fn local_software_result(value: execution_app::LocalSoftwareStatus) -> mcp::LocalSoftwareResult {
    mcp::LocalSoftwareResult {
        detected: value.detected,
        prepared: value.prepared,
        operation: operation(value.execution),
    }
}
impl mcp::ExecutionServicePort for ExecutionHandle {
    async fn task_operation(
        &self,
        input: execution_app::tasks::TaskOperation,
        _: CancellationToken,
    ) -> Result<execution_app::tasks::TaskResponse, mcp::ServiceError> {
        self.task_request(input).await.map_err(mcp_error)
    }

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
    async fn offers(&self, _: CancellationToken) -> Result<Vec<BackendTask>, mcp::ServiceError> {
        match self.request(Request::Offers {}).await.map_err(mcp_error)? {
            Reply::Offers { available } => Ok(available),
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
    async fn software_read(
        &self,
        input: SoftwareRead,
        _: CancellationToken,
    ) -> Result<SoftwarePage, mcp::ServiceError> {
        self.read_software(input).await
    }
    async fn software_prepare(
        &self,
        input: mcp::LocalSoftwareProposal,
        _: CancellationToken,
    ) -> Result<LocalSoftwarePrepared, mcp::ServiceError> {
        let ClientOrigin::Ai { tool_call, .. } = &self.origin else {
            return Err(mcp::ServiceError::Denied);
        };
        let request = RequestId::new(
            uuid::Uuid::parse_str(tool_call.as_str())
                .map_err(|_| mcp::ServiceError::InvalidInput)?
                .to_string(),
        )
        .map_err(|_| mcp::ServiceError::InvalidInput)?;
        match self
            .software_request(LocalSoftwareOperation::Prepare(LocalSoftwareChoice {
                request,
                software: input.software,
                action: input.action,
            }))
            .await?
        {
            Reply::SoftwarePrepared { value } => Ok(value),
            _ => Err(mcp::ServiceError::OutcomeUnknown),
        }
    }
    async fn software_status(
        &self,
        input: QueryRequest,
        _: CancellationToken,
    ) -> Result<mcp::LocalSoftwareResult, mcp::ServiceError> {
        let value = match self
            .software_request(LocalSoftwareOperation::Status(input))
            .await?
        {
            Reply::SoftwareStatus { value } => value,
            _ => return Err(mcp::ServiceError::Unavailable),
        };
        Ok(local_software_result(value))
    }
    async fn policy_tools(
        &self,
        input: mcp::PolicyToolsQuery,
        _: CancellationToken,
    ) -> Result<mcp::PolicyToolPage, mcp::ServiceError> {
        if !matches!(self.origin, ClientOrigin::Ai { .. }) {
            return Err(mcp::ServiceError::Denied);
        }
        let page = self
            .policy_directory(ui::PolicyDirectoryQuery { after: input.after })
            .await
            .map_err(|_| mcp::ServiceError::Unavailable)?;
        let mut tools = Vec::new();
        for item in page.items {
            match mcp::policy_tool(
                &serde_json::from_value(
                    serde_json::to_value(item).map_err(|_| mcp::ServiceError::InvalidInput)?,
                )
                .map_err(|_| mcp::ServiceError::InvalidInput)?,
            ) {
                Ok(tool) => tools.push(tool),
                Err(mcp::ServiceError::InvalidInput) => (), // Unsupported Resource schemas are not executable tools.
                Err(error) => return Err(error),
            }
        }
        Ok(mcp::PolicyToolPage {
            tools,
            next: page.next_cursor,
        })
    }
    async fn invoke_policy(
        &self,
        selector: rss_mdm_agent_wire::runtime::CapabilitySelector,
        input: mcp::PolicyToolInput,
        _: CancellationToken,
    ) -> Result<BackendRequest, mcp::ServiceError> {
        self.policy_call(selector, input, false).await
    }
    async fn prepare_operation(
        &self,
        input: mcp::OperationProposal,
        _: CancellationToken,
    ) -> Result<BackendRequest, mcp::ServiceError> {
        let item = self.resolve_policy(input.selector).await?;
        self.policy_call(
            rss_mdm_agent_wire::runtime::CapabilitySelector::Id { id: item.policy_id },
            mcp::PolicyToolInput {
                parameters: input.parameters,
            },
            true,
        )
        .await
    }

    async fn capabilities(
        &self,
        _: CancellationToken,
    ) -> Result<mcp::CapabilityView, mcp::ServiceError> {
        self.request(Request::Offers {}).await.map_err(mcp_error)?;
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
}

fn software_reply(reply: Reply) -> Result<Reply, mcp::ServiceError> {
    if let Reply::DataFailure { reason } = reply {
        return Err(match reason {
            DataError::InvalidInput => mcp::ServiceError::InvalidInput,
            DataError::Denied => mcp::ServiceError::Denied,
            DataError::NotFound => mcp::ServiceError::NotFound,
            DataError::Conflict => mcp::ServiceError::Conflict,
            DataError::Expired => mcp::ServiceError::Expired,
            DataError::Unsupported => mcp::ServiceError::Unsupported,
            DataError::InProgress => mcp::ServiceError::InProgress,
            DataError::Capacity => mcp::ServiceError::Limit,
            DataError::Unavailable => mcp::ServiceError::Unavailable,
        });
    }
    Ok(reply)
}
fn software_ui_error(error: mcp::ServiceError) -> ui::ServiceError {
    ui::error(
        match error {
            mcp::ServiceError::NotFound => "software_not_found",
            mcp::ServiceError::Conflict => "software_conflict",
            mcp::ServiceError::Denied | mcp::ServiceError::Unbound => "software_denied",
            mcp::ServiceError::Expired => "software_expired",
            mcp::ServiceError::Unsupported => "software_unsupported",
            _ => "software_unavailable",
        },
        "软件操作尚未完成；请核对所选软件或查询原请求",
    )
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
            retry_of: None,
            retry_expected_revision: None,
            recorded_at_unix_ms: 1,
            cancellation_control: None,
            cancellation_expected_revision: None,
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

#[cfg(test)]
mod local_software_error_tests {
    use super::*;
    #[test]
    fn verified_detection_facts_survive_the_native_mcp_projection() {
        use serde_json::json;
        let mut value: execution_app::LocalSoftwareStatus = serde_json::from_value(json!({
            "detected":null,
            "prepared":{
                "request":"local-detection","digest":"a".repeat(64),
                "choice":{"request":"local-detection","software":{"query":{"id":"native:apps","revision":"b".repeat(64)},"installation":"app"},"action":"detect"},
                "name":"App","initiator":{"kind":"human","osSession":{"device":"device","account":{"platform":"macos","subject":"uid:501"},"session":"login"}},
                "identity":"system","targetVersion":null,"startBeforeUnixMs":1000
            },
            "execution":{
                "operationRequestId":"local-detection","contentDigest":"a".repeat(64),
                "phase":"executionEnded","mode":"test","process":null,"software":null,
                "attemptId":null,"attempts":1,"cancelRequested":false,"admission":null,
                "dispatchCause":null,"stopOutcome":null,"assessment":null,"evidence":[]
            }
        })).unwrap();
        for detected in [
            Some(SoftwareState::Present {
                version: PackageValue::new("1.2.3").unwrap(),
            }),
            Some(SoftwareState::Absent {}),
            Some(SoftwareState::Unknown {
                reason: SoftwareDetectionFailure::Unavailable,
            }),
            None,
        ] {
            value.detected = detected.clone();
            let projected = local_software_result(value.clone());
            assert_eq!(projected.detected, detected);
            assert_eq!(projected.prepared.request, value.prepared.request);
            assert_eq!(
                projected.operation.operation_request_id,
                value.execution.operation_request_id
            );
            assert_eq!(
                projected.operation.phase,
                mcp::OperationPhase::ExecutionEnded
            );
        }
    }

    #[test]
    fn definitive_software_absence_survives_the_mcp_boundary() {
        assert!(matches!(
            software_reply(Reply::DataFailure {
                reason: DataError::NotFound
            }),
            Err(mcp::ServiceError::NotFound)
        ));
        assert!(matches!(
            software_reply(Reply::DataFailure {
                reason: DataError::Conflict
            }),
            Err(mcp::ServiceError::Conflict)
        ));
        assert!(matches!(
            software_reply(Reply::DataFailure {
                reason: DataError::Denied
            }),
            Err(mcp::ServiceError::Denied)
        ));
    }
}

#[cfg(test)]
mod context_admission_tests {
    use super::*;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn identity_binding_remains_available_when_all_business_read_slots_are_occupied() {
        let release = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate = release.clone();
        let handle = ExecutionHandle::new().with_context_transport(move |request| match request {
            Request::ServiceStatus {} => {
                let (lock, wake) = &*gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = wake.wait(released).unwrap();
                }
                Ok(Reply::Unavailable)
            }
            Request::RevokeContext {} => Ok(Reply::ContextRevoked),
            Request::BindContext { value, .. } => Ok(Reply::ContextBound {
                value: NativeContextRef {
                    id: Id::new("bound-context").unwrap(),
                    generation: value.generation,
                },
            }),
            _ => Err(Error::InvalidInput),
        });
        let mut reads = Vec::new();
        for _ in 0..16 {
            let read = handle.clone();
            reads.push(tokio::spawn(async move {
                read.request(Request::ServiceStatus {}).await
            }));
        }
        let saturated = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while handle.permits.available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await;
        let generation = Id::new("next-generation").unwrap();
        let bound = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            handle.bind_context(
                execution_ipc::host::NativeContextSession::device(generation.clone()),
                None,
            ),
        )
        .await;
        // Always release blocked native reads before asserting; test failures must reap owners too.
        *release.0.lock().unwrap() = true;
        release.1.notify_all();
        for read in reads {
            let _ = read.await;
        }
        assert!(saturated.is_ok());
        assert_eq!(bound.unwrap().unwrap().generation, generation);
        assert!(handle.context_possible.load(Ordering::Acquire));
        handle.revoke_context().await.unwrap();
        assert!(!handle.context_possible.load(Ordering::Acquire));
    }
}
