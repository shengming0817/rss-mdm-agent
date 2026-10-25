//! Authenticated data access and explicit queries, sharing the existing execution journal.
use super::*;
use crate::collected_data::{digest, Association};
use execution_contract::*;
use execution_ipc::host::{ClientOrigin, Reply, Request};

impl Core {
    pub(super) fn local_data(
        &mut self,
        command: &Command,
        context: &NativeContextRef,
        origin: &ClientOrigin,
        input: &DataOperation,
    ) -> Result<Reply, Error> {
        self.data_context(command, context)?;
        if matches!(origin, ClientOrigin::Ai { context: ai, .. } if ai != context) {
            return Err(Error::Identity);
        }
        let binding = command.binding.as_ref().ok_or(Error::Denied)?;
        let value = match input {
            DataOperation::Read(read) => {
                self.data.authorize_read(read, &command.subject, binding)?;
                Ok(DataResponse::Read(self.data.read(
                    read,
                    &command.subject,
                    binding,
                    self.host.clock.millis()? / 1000,
                )?))
            }
            DataOperation::Directory(query) if query.policy_page.is_none() => {
                self.maintain_data()?;
                let items = self.local_directory(command)?;
                Ok(DataResponse::Directory(directory_page(items, query)?))
            }
            _ => Err(Error::Protocol),
        }?;
        Ok(Reply::Data { value })
    }
    pub(super) fn local_directory(
        &mut self,
        command: &Command,
    ) -> Result<Vec<QueryDescriptor>, Error> {
        self.data.install_local()?;
        self.data.install_user(
            &command.subject,
            login_binding_id(command.binding.as_ref().ok_or(Error::Denied)?),
        )?;
        let items = self.data.directory(&command.subject)?;
        Ok(items)
    }

    pub(crate) fn periodic_data(
        &mut self,
        c: &wire::TelemetryConfiguration,
        e: &ProcessEvidence,
        at: i64,
        normalized: Option<std::collections::BTreeMap<wire::FieldKey, wire::TelemetryReading>>,
    ) -> Result<(), Error> {
        self.data.periodic(c, e, at, normalized)
    }

    pub(super) fn submit_local(
        &mut self,
        command: &Command,
        frozen: &FrozenExecution,
    ) -> Result<execution_app::ExecutionStatus, Error> {
        let fences = self.fences.clone();
        let scope = policy::SessionKey::new(
            command.subject.clone(),
            command.binding.clone().ok_or(Error::Denied)?,
        );
        let _admission = fences.admit(&scope, command.fence_epoch)?;
        *self
            .host
            .local_read
            .lock()
            .map_err(|_| Error::Unavailable)? =
            Some(crate::local_execution::ReadPermit::new(frozen.clone())?);
        let result = self
            .app
            .request_execution(&self.local_caller(), frozen)
            .map_err(map_app_error);
        *self
            .host
            .local_read
            .lock()
            .map_err(|_| Error::Unavailable)? = None;
        result
    }
    pub(crate) fn maintain_data(&mut self) -> Result<(), Error> {
        let mut active = std::collections::BTreeSet::new();
        for request in self.data.input_requests()? {
            if self
                .app
                .has_service_execution(&request, &self.request_device(&request)?)
                .map_err(map_app_error)?
            {
                let status = self
                    .app
                    .status(&self.request_caller(&request)?, &request)
                    .map_err(map_app_error)?;
                if !matches!(
                    status.phase,
                    execution_app::TaskPhase::Cancelled
                        | execution_app::TaskPhase::AdmissionDenied
                        | execution_app::TaskPhase::FailedBeforeDispatch
                        | execution_app::TaskPhase::ExecutionEnded
                        | execution_app::TaskPhase::Verified
                ) {
                    active.insert(request);
                }
            }
        }
        let policies = self
            .telemetry
            .as_ref()
            .map(|t| t.data_sources())
            .transpose()?
            .unwrap_or_default();
        self.data
            .maintain(&self.data_contexts(), &active, &policies)
    }
    /// Recover only protected, hash-verified worker definitions. Never infer one from result keys.
    pub(crate) fn remember_data(&mut self, input: &FrozenExecution) -> Result<(), Error> {
        if input.spec().launch.interpreter.profile.id.as_str() != crate::collection_worker::PROFILE
        {
            return Ok(());
        }
        let request = &input.spec().request.request_id;
        if self
            .app
            .has_service_execution(request, &input.spec().request.target.device)
            .map_err(map_app_error)?
        {
            let status = self
                .app
                .status(&self.request_caller(request)?, request)
                .map_err(map_app_error)?;
            if status.process.is_none()
                && matches!(
                    status.phase,
                    execution_app::TaskPhase::Cancelled
                        | execution_app::TaskPhase::AdmissionDenied
                        | execution_app::TaskPhase::FailedBeforeDispatch
                )
            {
                return Ok(());
            }
        }
        let path = crate::recovery::content(
            &self.config,
            &input.spec().run_as,
            &input.spec().launch.artifact.sha256,
        );
        let bytes = match execution_runner::staging::read_worker_input(
            &path,
            crate::collection_worker::INPUT_BYTES,
        ) {
            Ok(v) => v,
            Err(_) => return Ok(()), // Historical GC is an unavailable definition, never a guessed plan.
        };
        use sha2::{Digest as _, Sha256};
        if format!("{:x}", Sha256::digest(&bytes)) != input.spec().launch.artifact.sha256.as_str() {
            return Err(Error::Untrusted);
        }
        let worker: crate::collection_worker::Input =
            serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
        worker.read.validate()?;
        let remote = match &worker.read.source {
            crate::collection_worker::ReadSource::Task { task, .. } => {
                Some(plan::enterprise_binding(
                    self.data.service_id,
                    task.tenant_id,
                    task.registration_id,
                    task.generation,
                    &task.device_id,
                )?)
            }
            crate::collection_worker::ReadSource::Periodic {
                configuration: c, ..
            } => Some(plan::enterprise_binding(
                self.data.service_id,
                c.tenant_id,
                c.registration_id,
                c.generation,
                &c.device_id,
            )?),
            crate::collection_worker::ReadSource::Local { .. } => None,
        };
        if remote.as_ref().is_some_and(|b| {
            b.authority != input.spec().request.authority
                || b.device != input.spec().request.target.device
        }) {
            return Err(Error::Untrusted);
        }
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
        if let Some(InputValue::Literal { value }) =
            input.spec().request.parameters.get(plan::SCRIPT_PROJECTION)
        {
            let task: wire::TaskSpec =
                serde_json::from_value(value.clone()).map_err(|_| Error::Protocol)?;
            let b = plan::enterprise_binding(
                self.data.service_id,
                task.tenant_id,
                task.registration_id,
                task.generation,
                &task.device_id,
            )?;
            if b.authority != input.spec().request.authority
                || b.device != input.spec().request.target.device
            {
                return Err(Error::Untrusted);
            }
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
                    self.data.capture_script(
                        &task,
                        request,
                        process,
                        evidence.observed_at_unix_ms,
                    )?;
                }
                self.app
                    .service_confirm(request, &consumer, &evidence.receipt.event_id)
                    .map_err(map_app_error)?;
            }
            return Ok(());
        }
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
                let completion: CollectionCompletion =
                    serde_json::from_slice(&process.stderr).map_err(|_| Error::Protocol)?;
                if completion.input_sha256 != input.spec().launch.artifact.sha256.as_str() {
                    return Err(Error::Untrusted);
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
            .status(&self.request_caller(request)?, request)
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
        if is_local(input) {
            return self.core.local_data(command, context, origin, input);
        }
        let binding = command.binding.as_ref().ok_or(Error::Denied)?;
        // Native context binds the local login. Cached device facts remain readable offline;
        // Policy operations below still use the stricter live enterprise context gate.
        let bound = self.core.data_context(command, context)?.clone();
        if matches!(input, DataOperation::Directory(_)) {
            self.core.maintain_data()?;
        }
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
                let mut items = self.core.local_directory(command)?;
                let mut policy_next = None;
                if let Some(locator) = &query.policy_page {
                    let page = self
                        .policy_directory(&bound, origin, locator.after.clone(), commands)
                        .await?;
                    policy_next = page
                        .next_cursor
                        .map(|next| {
                            Ok::<_, Error>(PolicyLocator {
                                after: Some(plan::id(next.to_string())?),
                            })
                        })
                        .transpose()?;
                    for item in page.items {
                        if matches!(item.task, wire::SelfServiceTask::Script { .. }) {
                            self.core.maintain_data()?;
                            let descriptor = self.core.data.publish(
                                item,
                                locator.clone(),
                                &command.subject,
                                binding,
                                context,
                            )?;
                            items.retain(|item| item.reference.id != descriptor.reference.id);
                            items.push(descriptor);
                        }
                    }
                    self.core.maintain_data()?;
                }
                items.retain(|item| {
                    item.kind != QueryKind::Policy
                        || !item.available
                        || self
                            .core
                            .data
                            .published(&item.reference, &command.subject, binding, context)
                            .is_ok_and(|published| published.is_some())
                });
                let mut page = directory_page(items, query)?;
                page.policy_next = policy_next;
                DataResponse::Directory(page)
            }
            DataOperation::Read(_) => return Err(Error::Protocol),
            DataOperation::Submit(submit) => {
                let parsed =
                    uuid::Uuid::parse_str(submit.request.as_str()).map_err(|_| Error::Protocol)?;
                if parsed.is_nil() || parsed.to_string() != submit.request.as_str() {
                    return Err(Error::Protocol);
                }
                let history = self.core.data.history()?;
                let mut retained = history.len();
                for (old, _) in history {
                    if retained < self.core.data.history_limit() {
                        break;
                    }
                    if old == submit.request {
                        continue;
                    }
                    let terminal = if self
                        .core
                        .app
                        .has_service_execution(&old, &self.core.request_device(&old)?)
                        .map_err(map_app_error)?
                    {
                        matches!(
                            self.core
                                .app
                                .status(&self.core.request_caller(&old)?, &old)
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
                        .has_service_execution(&original, &self.core.request_device(&original)?)
                        .map_err(map_app_error)?
                    {
                        let status = self
                            .core
                            .app
                            .status(&self.core.request_caller(&original)?, &original)
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
                if let Some((item, locator)) =
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
                        locator,
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
                    let d = self
                        .core
                        .data
                        .local_definition(&submit.query, &command.subject)?;
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
                        .has_service_execution(&submit.request, &self.core.host.root.device)
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
                        device: self.core.host.root.device.clone(),
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
                        &self.core.host.root,
                        &self.core.host.local_actor,
                        (&self.core.config, &mut self.core.material_root),
                        helper,
                    )?;
                    self.core.remember_data(&frozen)?;
                    self.core
                        .host
                        .materials
                        .register(&frozen, artifacts)
                        .map_err(map_app_error)?;
                    self.core.submit_local(command, &frozen)?;
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
                        if self
                            .core
                            .app
                            .has_service_execution(
                                &query.request,
                                &self.core.request_device(&query.request)?,
                            )
                            .map_err(map_app_error)?
                        {
                            self.core.capture_data(&query.request)?;
                        }
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
                            .cancel(&self.core.local_caller(), &query.request)
                            .map_err(map_app_error)?;
                    }
                    self.core
                        .app
                        .reconcile(&query.request)
                        .map_err(map_app_error)?;
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

pub(super) fn is_local(input: &DataOperation) -> bool {
    matches!(
        input,
        DataOperation::Read(_)
            | DataOperation::Directory(QueryDirectory {
                policy_page: None,
                ..
            })
    )
}
pub(super) fn failure(error: Error) -> Reply {
    Reply::DataFailure {
        reason: match error {
            Error::Denied | Error::Identity | Error::Untrusted => DataError::Denied,
            Error::Protocol | Error::Configuration => DataError::InvalidInput,
            Error::NotFound => DataError::NotFound,
            Error::Conflict => DataError::Conflict,
            Error::Expired => DataError::Expired,
            Error::Unsupported => DataError::Unsupported,
            Error::Capacity => DataError::Capacity,
            _ => DataError::Unavailable,
        },
    }
}
fn directory_page(
    mut items: Vec<QueryDescriptor>,
    query: &QueryDirectory,
) -> Result<QueryPage, Error> {
    if !(1..=128).contains(&query.limit) {
        return Err(Error::Capacity);
    }
    items.sort_by(|a, b| a.reference.id.cmp(&b.reference.id));
    items.retain(|d| query.after.as_ref().is_none_or(|a| d.reference.id > *a));
    let next = (items.len() > query.limit as usize)
        .then(|| items[query.limit as usize - 1].reference.id.clone());
    items.truncate(query.limit as usize);
    let mut page = QueryPage {
        items,
        next,
        policy_next: None,
    };
    while serde_json::to_vec(&page)
        .map_err(|_| Error::Protocol)?
        .len()
        > 48 * 1024
    {
        page.items.pop();
        let last = page.items.last().ok_or(Error::Capacity)?;
        page.next = Some(last.reference.id.clone());
    }
    Ok(page)
}
