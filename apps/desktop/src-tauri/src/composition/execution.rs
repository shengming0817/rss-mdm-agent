//! Production IPC consumer. The desktop owns neither an execution journal nor a runner.
use crate::self_service as ui;
use execution_app::{Error, ExecutionStatus, ExecutionTaskDetails, TaskPhase};
use execution_contract::*;
use execution_mcp as mcp;
use execution_runner::host::{ClientOrigin, Reply, Request, ServiceClient};
use std::{
    sync::{Arc, Mutex},
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
fn mcp_error(error: Error) -> mcp::ServiceError {
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
}
impl ExecutionHandle {
    pub fn new() -> Self {
        Self {
            users: None,
            generation: None,
            origin: ClientOrigin::Desktop {},
            permits: Arc::new(tokio::sync::Semaphore::new(16)),
        }
    }
    pub fn with_trusted_users(mut self, users: Arc<Mutex<super::users::Users>>) -> Self {
        self.users = Some(users);
        self
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
        let reply = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            ServiceClient::installed()?.request(request)
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
    pub async fn close(&self) {
        self.permits.close();
    }
    pub async fn snapshot(&self, query: ui::SnapshotQuery) -> ui::Result<ui::Snapshot> {
        match self
            .request(Request::Tasks { after: query.after })
            .await
            .map_err(bad)?
        {
            Reply::Tasks { value, available } => Ok(ui::Snapshot {
                available,
                requests: value.items,
                next: value.next,
            }),
            _ => Err(bad(Error::InvalidInput)),
        }
    }
    async fn select(&self, input: BackendSelection) -> Result<TaskSubmission, Error> {
        match self
            .request(Request::StartTask {
                request: input.request,
                task: input.task,
                attempt: input.attempt,
                revision: input.revision,
                origin: self.origin.clone(),
            })
            .await?
        {
            Reply::Queued {
                request,
                confirmation_required,
                ..
            } => Ok(TaskSubmission {
                request,
                confirmation_required,
            }),
            _ => Err(Error::OutcomeUnknown),
        }
    }
    pub async fn execute_ui(&self, input: BackendSelection) -> ui::Result<TaskSubmission> {
        self.select(input).await.map_err(bad)
    }
    pub async fn cancel_ui(&self, input: ui::ActionRef) -> ui::Result<ExecutionStatus> {
        self.cancel_task(input.request_id).await.map_err(bad)
    }
    pub async fn details(&self, request: RequestId) -> Result<ExecutionTaskDetails, Error> {
        match self.request(Request::Details { request }).await? {
            Reply::Details { value } => Ok(value),
            _ => Err(Error::InvalidInput),
        }
    }
    pub async fn cancel_task(&self, request: RequestId) -> Result<ExecutionStatus, Error> {
        match self.request(Request::Cancel { request }).await? {
            Reply::Status { value } => Ok(value),
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
        phase: if s.admission == Some(execution_sqlite::AdmissionStatus::Denied) {
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
            operation: operation(
                self.cancel_task(request.operation_request_id)
                    .await
                    .map_err(mcp_error)?,
            ),
        })
    }
}
