//! Authenticated data access and explicit queries, sharing the existing execution journal.
use super::*;
use crate::collected_data::{digest, Association};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply, Request};

impl Core {
    pub(crate) fn periodic_data(
        &mut self,
        c: &wire::TelemetryConfiguration,
        e: &ProcessEvidence,
        at: i64,
        normalized: Option<std::collections::BTreeMap<wire::FieldKey, wire::TelemetryReading>>,
    ) -> Result<(), Error> {
        self.data.periodic(c, e, at, normalized)
    }

    /// Recover only protected, hash-verified worker definitions. Never infer one from result keys.
    pub(crate) fn remember_data(&mut self, input: &FrozenExecution) -> Result<(), Error> {
        if input.spec().launch.interpreter.profile.id.as_str() != crate::collection_worker::PROFILE
        {
            return Ok(());
        }
        let path = crate::recovery::content(
            &self.config,
            &input.spec().run_as,
            &input.spec().launch.artifact.sha256,
        );
        let bytes = match execution_runner::staging::read_worker_input(&path, 1024 * 1024) {
            Ok(v) => v,
            Err(_) => return Ok(()), // Historical GC is an unavailable definition, never a guessed plan.
        };
        use sha2::{Digest as _, Sha256};
        if format!("{:x}", Sha256::digest(&bytes)) != input.spec().launch.artifact.sha256.as_str() {
            return Err(Error::Untrusted);
        }
        let worker: crate::collection_worker::Input =
            serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
        self.data
            .remember(&input.spec().request.request_id, worker.read)?;
        Ok(())
    }
    pub(crate) fn capture_data(&mut self, request: &RequestId) -> Result<(), Error> {
        if self.data.captured(request)? {
            return Ok(());
        }
        let input = self
            .app
            .service_frozen_input(request)
            .map_err(map_app_error)?;
        if input.spec().launch.interpreter.profile.id.as_str() != crate::collection_worker::PROFILE
        {
            return Ok(());
        }
        self.remember_data(&input)?;
        let Some(read) = self.data.remembered(request)? else {
            return Ok(());
        };
        let consumer = plan::id("local-collected-data")?;
        for evidence in self
            .app
            .service_delivery(request, &consumer, 1)
            .map_err(map_app_error)?
        {
            if let Some(process) = &evidence.process {
                if !process.finished || !process.quiescent {
                    continue;
                }
                self.data.native(
                    read.clone(),
                    Some(request.clone()),
                    process,
                    evidence.observed_at_unix_ms,
                )?;
            }
            self.app
                .service_confirm(request, &consumer, &evidence.receipt.event_id)
                .map_err(map_app_error)?;
        }
        Ok(())
    }
    fn native_query_status(
        &self,
        request: &RequestId,
        a: &Association,
    ) -> Result<QueryStatus, Error> {
        let status = self
            .app
            .status(&self.caller(), request)
            .map_err(map_app_error)?;
        use execution_app::TaskPhase::*;
        let phase = match status.phase {
            Cancelled => QueryPhase::Cancelled,
            AdmissionDenied | FailedBeforeDispatch => QueryPhase::Failed,
            Running => QueryPhase::Running,
            ExecutionEnded | Verified => {
                if status.process.as_ref().is_some_and(|p| {
                    p.exit_code == Some(0) && p.quiescent && p.quality == OutputQuality::Complete
                }) {
                    QueryPhase::Completed
                } else {
                    QueryPhase::Failed
                }
            }
            OutcomeUnknown => QueryPhase::Unknown,
            ApprovalRequired | ConfirmationRequired => QueryPhase::AwaitingConfirmation,
            _ => QueryPhase::Accepted,
        };
        Ok(QueryStatus {
            request: request.clone(),
            query: a.query.clone(),
            phase,
            cancel_requested: status.cancel_requested,
            preparation: None,
        })
    }
}
impl<S: IdentityProvider> DeviceService<S> {
    pub(super) async fn data_command(
        &mut self,
        command: &Command,
        context: &NativeContextRef,
        origin: &ClientOrigin,
        input: &DataOperation,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        let binding = command.binding.as_ref().ok_or(Error::Denied)?;
        // Native context binds the local login. Cached device facts remain readable offline;
        // Policy operations below still use the stricter live enterprise context gate.
        let bound = self.core.data_context(command, context)?.clone();
        if let ClientOrigin::Ai { context: ai, .. } = origin {
            if ai != context {
                return Err(Error::Identity);
            }
        }
        let value = match input {
            DataOperation::Directory(query) => {
                if !(1..=128).contains(&query.limit) {
                    return Err(Error::Capacity);
                }
                self.core
                    .data
                    .install_user(&command.subject, login_binding_id(binding))?;
                let mut items = self.core.data.directory(&command.subject)?;
                // Directory discovery carries no invocation authority. A failed refresh still
                // permits local native reads; cached Policy availability is never asserted.
                let mut after = None;
                for _ in 0..if query.refresh_policies { 16 } else { 0 } {
                    let page = match self
                        .policy_directory(&bound, origin, after.take(), commands)
                        .await
                    {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    for item in page.items {
                        if matches!(item.task, wire::SelfServiceTask::Script { .. }) {
                            items.push(self.core.data.publish(
                                item,
                                &command.subject,
                                binding,
                                context,
                            )?);
                        }
                    }
                    after = page
                        .next_cursor
                        .map(|id| plan::id(id.to_string()))
                        .transpose()?;
                    if after.is_none() {
                        break;
                    }
                }
                items.sort_by(|a, b| a.reference.id.cmp(&b.reference.id));
                items.retain(|d| query.after.as_ref().is_none_or(|a| d.reference.id > *a));
                let next = (items.len() > query.limit as usize)
                    .then(|| items[query.limit as usize - 1].reference.id.clone());
                items.truncate(query.limit as usize);
                DataResponse::Directory(QueryPage { items, next })
            }
            DataOperation::Read(read) => {
                let policy = match &read.selection {
                    DataSelection::Query { query } => query.id.as_str().starts_with("policy:"),
                    DataSelection::Request { request } => {
                        self.core
                            .data
                            .association(request, &command.subject, binding)?
                            .kind
                            == QueryKind::Policy
                    }
                };
                if policy {
                    self.core.context(command)?;
                }
                self.core
                    .data
                    .authorize_read(read, &command.subject, binding, context)?;
                DataResponse::Read(self.core.data.read(
                    read,
                    &command.subject,
                    binding,
                    self.core.host.clock.millis()? / 1000,
                )?)
            }
            DataOperation::Submit(submit) => {
                let parsed =
                    uuid::Uuid::parse_str(submit.request.as_str()).map_err(|_| Error::Protocol)?;
                if parsed.is_nil() || parsed.to_string() != submit.request.as_str() {
                    return Err(Error::Protocol);
                }
                if !self.core.host.identity_ready() {
                    return Err(Error::Denied);
                }
                let history = self.core.data.history()?;
                let mut retained = history.len();
                for (old, _) in history {
                    if retained < 32 {
                        break;
                    }
                    if old == submit.request {
                        continue;
                    }
                    let terminal = if self
                        .core
                        .app
                        .has_service_execution(&old, &self.core.host.binding.device)
                        .map_err(map_app_error)?
                    {
                        matches!(
                            self.core
                                .app
                                .status(&self.core.caller(), &old)
                                .map_err(map_app_error)?
                                .phase,
                            execution_app::TaskPhase::Cancelled
                                | execution_app::TaskPhase::AdmissionDenied
                                | execution_app::TaskPhase::FailedBeforeDispatch
                                | execution_app::TaskPhase::ExecutionEnded
                                | execution_app::TaskPhase::Verified
                        )
                    } else {
                        self.core
                            .app
                            .backend_request(&self.core.caller(), &old)
                            .map_err(map_app_error)?
                            .is_none_or(|r| {
                                matches!(
                                    r.state,
                                    BackendRequestState::Cancelled
                                        | BackendRequestState::Failed
                                        | BackendRequestState::Observed
                                )
                            })
                    };
                    if terminal {
                        self.core.data.retire(&old)?;
                        retained -= 1;
                    }
                }
                for original in
                    self.core
                        .data
                        .related_requests(&submit.query, &command.subject, binding)?
                {
                    if original == submit.request {
                        continue;
                    }
                    let running = if self
                        .core
                        .app
                        .has_service_execution(&original, &self.core.host.binding.device)
                        .map_err(map_app_error)?
                    {
                        let status = self
                            .core
                            .app
                            .status(&self.core.caller(), &original)
                            .map_err(map_app_error)?;
                        !matches!(
                            status.phase,
                            execution_app::TaskPhase::Cancelled
                                | execution_app::TaskPhase::AdmissionDenied
                                | execution_app::TaskPhase::FailedBeforeDispatch
                                | execution_app::TaskPhase::ExecutionEnded
                                | execution_app::TaskPhase::Verified
                        )
                    } else {
                        self.core
                            .app
                            .backend_request(&self.core.caller(), &original)
                            .map_err(map_app_error)?
                            .is_some_and(|r| {
                                !matches!(
                                    r.state,
                                    BackendRequestState::Failed
                                        | BackendRequestState::Cancelled
                                        | BackendRequestState::Observed
                                )
                            })
                    };
                    if running {
                        return Ok(Reply::DataFailure {
                            reason: DataError::InProgress,
                        });
                    }
                }
                if let Some(item) =
                    self.core
                        .data
                        .published(&submit.query, &command.subject, binding, context)?
                {
                    let choice = PolicyChoice {
                        request: submit.request.clone(),
                        policy_id: plan::id(item.policy_id.to_string())?,
                        policy_revision: item.policy_revision,
                        policy_version: plan::id(item.policy_version.to_string())?,
                        parameters: submit.parameters.clone(),
                    };
                    let a = Association {
                        query: submit.query.clone(),
                        subject: command.subject.clone(),
                        session: binding.clone(),
                        parameters: digest(&submit.parameters)?,
                        kind: QueryKind::Policy,
                        created_at: self.core.host.clock.millis()?,
                        origin: digest(origin)?,
                    };
                    self.core.data.associate(&submit.request, &a)?;
                    let request = Request::InvokePolicy {
                        context: context.clone(),
                        choice,
                        origin: origin.clone(),
                    };
                    let reply = self.policy_operation(command, &request, commands).await?;
                    DataResponse::Status(policy_status(reply, &a, &submit.request)?)
                } else {
                    if !submit.parameters.is_empty() {
                        return Err(Error::Protocol);
                    }
                    let d = self.core.data.definition(&submit.query, &command.subject)?;
                    if !d.descriptor.available {
                        return Err(Error::Unsupported);
                    }
                    let a = Association {
                        query: submit.query.clone(),
                        subject: command.subject.clone(),
                        session: binding.clone(),
                        parameters: digest(&submit.parameters)?,
                        kind: QueryKind::Native,
                        created_at: self.core.host.clock.millis()?,
                        origin: digest(origin)?,
                    };
                    if self
                        .core
                        .app
                        .has_service_execution(&submit.request, &self.core.host.binding.device)
                        .map_err(map_app_error)?
                    {
                        let previous = self.core.data.association(
                            &submit.request,
                            &command.subject,
                            binding,
                        )?;
                        if previous.query != a.query
                            || previous.parameters != a.parameters
                            || previous.origin != a.origin
                        {
                            return Err(Error::Conflict);
                        }
                        return Ok(Reply::Data {
                            value: DataResponse::Status(
                                self.core.native_query_status(&submit.request, &a)?,
                            ),
                        });
                    }
                    self.core.data.associate(&submit.request, &a)?;
                    let now = self.core.host.clock.millis()?;
                    let mut read = d.read;
                    let start_until = i64::try_from(now / 1000 + 30).map_err(|_| Error::Clock)?;
                    read.deadline =
                        start_until + i64::from(read.plan.spec().budget.timeout_seconds);
                    let os_session = OsSessionRef {
                        device: self.core.host.binding.device.clone(),
                        account: OsAccountRef {
                            platform: plan::platform()?,
                            subject: plan::id(&command.subject)?,
                        },
                        session: binding.clone(),
                    };
                    let initiator = match origin {
                        ClientOrigin::Desktop {} => Initiator::Human { os_session },
                        ClientOrigin::Ai {
                            config,
                            conversation,
                            tool_call,
                            ..
                        } => Initiator::Ai {
                            provider: plan::id("local-ai-host")?,
                            os_session,
                            config: config.clone(),
                            conversation: conversation.clone(),
                            tool_call: tool_call.clone(),
                        },
                    };
                    let helper = if read.execution.identity == wire::ExecutionIdentity::LoggedInUser
                    {
                        let helper = self.core.helpers.connect_peer(
                            &command.subject,
                            command.session,
                            binding,
                        )?;
                        if let wire::SoftwareExecutionTarget::User {
                            identity,
                            session_id,
                        } = &mut read.execution.target
                        {
                            if identity != &command.subject {
                                return Err(Error::Denied);
                            }
                            *session_id = login_id(helper.context());
                        }
                        Some(helper)
                    } else {
                        None
                    };
                    let (frozen, artifacts) = backend::collection::compile_read(
                        &read,
                        submit.request.clone(),
                        initiator,
                        Operation {
                            action: plan::id("local-query")?,
                            resource: plan::reference(
                                submit.query.id.as_str(),
                                submit.query.revision.as_str(),
                            )?,
                        },
                        plan::reference("local-native-query", "1")?,
                        start_until,
                        &self.core.host.binding,
                        &self.core.host.actor,
                        (&self.core.config, &mut self.core.material_root),
                        helper,
                    )?;
                    self.core.remember_data(&frozen)?;
                    self.core
                        .host
                        .materials
                        .register(&frozen, artifacts)
                        .map_err(map_app_error)?;
                    *self
                        .core
                        .host
                        .local_query
                        .lock()
                        .map_err(|_| Error::Unavailable)? = Some(frozen.clone());
                    let result = self
                        .core
                        .app
                        .request_execution(&self.core.caller(), &frozen)
                        .map_err(map_app_error);
                    *self
                        .core
                        .host
                        .local_query
                        .lock()
                        .map_err(|_| Error::Unavailable)? = None;
                    result?;
                    DataResponse::Status(self.core.native_query_status(&submit.request, &a)?)
                }
            }
            DataOperation::Status(query) | DataOperation::Cancel(query) => {
                let a = self
                    .core
                    .data
                    .association(&query.request, &command.subject, binding)?;
                if a.kind == QueryKind::Policy {
                    let request = Request::PolicyStatus {
                        context: context.clone(),
                        request: query.request.clone(),
                    };
                    let reply = self.policy_operation(command, &request, commands).await?;
                    if let Reply::PolicyStatus { value } = &reply {
                        self.core.data.policy_result(&a, value)?;
                    }
                    let mut status = policy_status(reply, &a, &query.request)?;
                    if matches!(input, DataOperation::Cancel(_)) {
                        if let Some(pending) =
                            self.core.pending(&query.request, &command.subject)?
                        {
                            status.preparation = self
                                .core
                                .transition(
                                    pending.action.request(),
                                    BackendRequestState::Cancelled,
                                    None,
                                )?
                                .map(|p| QueryPreparationRef {
                                    request: p.action.request().clone(),
                                    revision: p.revision,
                                });
                            status.phase = QueryPhase::Cancelled;
                        } else {
                            self.core
                                .app
                                .cancel(&self.core.caller(), &query.request)
                                .map_err(map_app_error)?;
                            status.cancel_requested = true;
                        }
                    }
                    DataResponse::Status(status)
                } else {
                    if matches!(input, DataOperation::Cancel(_)) {
                        self.core
                            .app
                            .cancel(&self.core.caller(), &query.request)
                            .map_err(map_app_error)?;
                    }
                    self.core.capture_data(&query.request)?;
                    DataResponse::Status(self.core.native_query_status(&query.request, &a)?)
                }
            }
        };
        self.core.data_context(command, context)?;
        Ok(Reply::Data { value })
    }
}
fn policy_status(reply: Reply, a: &Association, request: &RequestId) -> Result<QueryStatus, Error> {
    let record = match reply {
        Reply::PolicyRequest { value } | Reply::Pending { value } => Some(value),
        // Authorized server state is deliberately read through its original Policy endpoint.
        Reply::PolicyStatus { value } => {
            let phase = if matches!(value.state.cancellation, wire::RunCancellation::Confirmed) {
                QueryPhase::Cancelled
            } else {
                match value.state.execution {
                    wire::TaskExecution::NotStarted => QueryPhase::Accepted,
                    wire::TaskExecution::Running | wire::TaskExecution::WaitingReboot => {
                        QueryPhase::Running
                    }
                    wire::TaskExecution::Succeeded => QueryPhase::Completed,
                    wire::TaskExecution::Failed => QueryPhase::Failed,
                    wire::TaskExecution::Unknown => QueryPhase::Unknown,
                }
            };
            return Ok(QueryStatus {
                request: request.clone(),
                query: a.query.clone(),
                phase,
                cancel_requested: !matches!(value.state.cancellation, wire::RunCancellation::None),
                preparation: None,
            });
        }
        _ => return Err(Error::Unavailable),
    };
    let phase = record
        .as_ref()
        .map_or(QueryPhase::Running, |r| match r.state {
            BackendRequestState::AwaitingConfirmation => QueryPhase::AwaitingConfirmation,
            BackendRequestState::Failed => QueryPhase::Failed,
            BackendRequestState::Cancelled => QueryPhase::Cancelled,
            BackendRequestState::Observed => QueryPhase::Completed,
            _ => QueryPhase::Accepted,
        });
    Ok(QueryStatus {
        request: request.clone(),
        query: a.query.clone(),
        phase,
        cancel_requested: false,
        preparation: record.map(|p| QueryPreparationRef {
            request: p.action.request().clone(),
            revision: p.revision,
        }),
    })
}
