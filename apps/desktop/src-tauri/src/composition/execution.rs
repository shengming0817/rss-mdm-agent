//! Production IPC consumer. The desktop owns neither an execution journal nor a runner.
use crate::self_service as ui;
use execution_app::{BackendTaskView, Error, ExecutionStatus, TaskPhase};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply, Request, ServiceClient};
use execution_mcp as mcp;
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
}
impl ExecutionHandle {
    pub fn new() -> Self {
        Self {
            users: None,
            generation: None,
            origin: ClientOrigin::Desktop {},
            permits: Arc::new(tokio::sync::Semaphore::new(16)),
            wire: Arc::new(Mutex::new(())),
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
        let wire = self.wire.clone();
        let bound = self.clone();
        let reply = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            // One short-lived native connection at a time. The existing admission semaphore
            // bounds both waiting and active transport; these handles own no business queue.
            let _wire = wire.lock().map_err(|_| Error::Unavailable)?;
            bound.current()?;
            let client = ServiceClient::installed().inspect_err(|error| {
                eprintln!("RSS_EXECUTION_IPC_FAILURE stage=installation code={error:?}");
            })?;
            client.request(request).inspect_err(|error| {
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
            if value.offer.request == input.request
                && value.offer.task == input.task
                && value.offer.attempt == input.attempt
                && value.offer.revision == input.revision =>
        {
            if matches!(
                value.state,
                BackendRequestState::Failed | BackendRequestState::Cancelled
            ) {
                return Err(Error::Denied);
            }
            Ok(TaskSubmission {
                request: value.offer.request,
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
        operation_request_id: value.offer.request,
        content_digest: value.offer.revision,
        phase: match value.state {
            BackendRequestState::Failed => mcp::OperationPhase::Failed,
            BackendRequestState::Cancelled => mcp::OperationPhase::Cancelled,
            _ => mcp::OperationPhase::Waiting,
        },
        attempt_id: None,
        evidence: vec![],
    }
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
            offer: BackendTask {
                request: selection.request.clone(),
                task: selection.task.clone(),
                attempt: selection.attempt.clone(),
                revision: selection.revision.clone(),
                title: "original".into(),
                expires_at: 100,
                user_initiated: true,
                summary: BackendTaskSummary::Script {
                    identity: BackendIdentity::System,
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
            match field {
                0 => value.offer.request = RequestId::new("other-request").unwrap(),
                1 => value.offer.task = Id::new("other-task").unwrap(),
                2 => value.offer.attempt = Id::new("other-attempt").unwrap(),
                _ => value.offer.revision = Digest::new("b".repeat(64)).unwrap(),
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
