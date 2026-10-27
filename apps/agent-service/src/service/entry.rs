//! CLI and Deep Link consume the original business owners; no directory or journal lives here.
use super::*;
use execution_contract::*;
use execution_ipc::{
    entry::{EntryContext, EntryOperation},
    host::{ClientOrigin, NativeContextSession, Reply, Request},
};

impl<S: IdentityProvider> DeviceService<S> {
    fn entry_namespace(&self) -> Option<EntryNamespace> {
        let cfg = self.client.configuration();
        self.client.registration().ok().map(|b| EntryNamespace {
            instance: cfg.service_id.to_string(),
            tenant: cfg.tenant.to_string(),
            device: b.device_id,
            registration: b.registration_id.to_string(),
            generation: b.generation,
        })
    }
    async fn attach_entry(
        &mut self,
        command: &Command,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<NativeContextRef, Error> {
        if let Ok(bound) = self.core.context(command) {
            return Ok(bound.context.clone());
        }
        // Only an absent context may be initialized. A revoked/expired user context is
        // never downgraded to a device context by a short-lived CLI process.
        if self.core.contexts.contains_key(&policy::peer_key(command)?) {
            return Err(Error::Identity);
        }
        let generation = plan::id(uuid::Uuid::new_v4().to_string())?;
        let request = Request::BindContext {
            value: NativeContextSession::device(generation),
            expected: None,
        };
        match self.policy_operation(command, &request, commands).await? {
            Reply::ContextBound { value } => Ok(value),
            _ => Err(Error::Identity),
        }
    }
    pub(super) async fn entry_operation(
        &mut self,
        command: &Command,
        input: &EntryOperation,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        let origin = ClientOrigin::Desktop {};
        if matches!(input, EntryOperation::Checkin) {
            // The ordinary owner drive performs claim/report/renewal; no second loop.
            self.next_network = std::time::Instant::now();
            return Ok(Reply::CheckinRequested {});
        }
        let context = self.attach_entry(command, commands).await?;
        let bound = self.core.context(command)?.clone();
        match input {
            EntryOperation::Discover { after } => {
                let local = [LocalTool::Network, LocalTool::Settings, LocalTool::Software]
                    .into_iter()
                    .map(|name| LocalToolDescriptor {
                        name,
                        input_schema: name.schema(),
                        node: name.node().into(),
                    })
                    .collect();
                let (policies, remote_failure) = match self
                    .policy_directory(&bound, &origin, after.clone(), commands)
                    .await
                {
                    Ok(value) => (Some(value), None),
                    Err(error) => match data::failure(error) {
                        Reply::DataFailure { reason } => (None, Some(reason)),
                        _ => return Err(Error::Unavailable),
                    },
                };
                self.core.context(command)?;
                Ok(Reply::EntryCapabilities {
                    local,
                    remote_available: policies.is_some(),
                    policies,
                    remote_failure,
                })
            }
            EntryOperation::Preview { intent } => {
                intent.validate().map_err(|_| Error::Protocol)?;
                if intent
                    .namespace
                    .as_ref()
                    .is_some_and(|n| Some(n) != self.entry_namespace().as_ref())
                {
                    return Err(Error::Identity);
                }
                match &intent.selector {
                    EntrySelector::Capability { selector } => {
                        let current = self
                            .resolve_policy(&bound, &origin, selector.clone(), commands)
                            .await?;
                        policy::operation_parameters_supported(&current)?;
                        if intent
                            .version
                            .as_ref()
                            .is_some_and(|v| *v != current.policy_version.to_string())
                            || intent
                                .revision
                                .is_some_and(|v| v != current.policy_revision)
                        {
                            return Err(Error::Conflict);
                        }
                        if !intent.parameters.is_empty() {
                            policy::parameters(&current, &intent.parameters)?;
                        }
                        self.core.context(command)?;
                        Ok(Reply::PolicyResolved {
                            value: Box::new(current),
                        })
                    }
                    _ => Err(Error::Unsupported),
                }
            }
            EntryOperation::Attach => Ok(Reply::EntryContext {
                value: EntryContext {
                    context: (!command.request.is_machine()).then_some(context),
                    namespace: self.entry_namespace(),
                },
            }),
            EntryOperation::Data { input } => {
                let mut input = input.clone();
                if command.request.is_machine() {
                    self.check_machine_data(command, &mut input)?;
                }
                self.data_command(command, &context, &origin, &input, commands)
                    .await
            }
            EntryOperation::Resolve { selector } => match selector {
                EntrySelector::LocalTool { name } => Ok(Reply::LocalTool {
                    value: LocalToolDescriptor {
                        name: *name,
                        input_schema: name.schema(),
                        node: name.node().into(),
                    },
                }),
                EntrySelector::Node { id } if PAGE_NODES.contains(&id.as_str()) => {
                    Ok(Reply::EntryNodes {
                        value: vec![id.clone()],
                    })
                }
                EntrySelector::Capability { selector } => Ok(Reply::PolicyResolved {
                    value: Box::new(
                        self.resolve_policy(&bound, &origin, selector.clone(), commands)
                            .await?,
                    ),
                }),
                EntrySelector::Request { request } => self.core.task_operation(
                    command,
                    &execution_app::tasks::TaskOperation::Detail(
                        execution_app::tasks::TaskRef::Request {
                            request: request.clone(),
                        },
                    ),
                    &origin,
                ),
                EntrySelector::Sampling { query } => self.core.task_operation(
                    command,
                    &execution_app::tasks::TaskOperation::Detail(
                        execution_app::tasks::TaskRef::Sampling {
                            query: query.clone(),
                        },
                    ),
                    &origin,
                ),
                EntrySelector::Activity { id } => {
                    let scope = self.core.task_scope(command)?;
                    let cache = network(
                        self.client.management_detail(&scope, id),
                        &mut self.core,
                        commands,
                    )
                    .await?;
                    self.core.context(command)?;
                    self.core.management = Some(cache);
                    self.core.task_operation(
                        command,
                        &execution_app::tasks::TaskOperation::Detail(
                            execution_app::tasks::TaskRef::Activity { id: id.clone() },
                        ),
                        &origin,
                    )
                }
                EntrySelector::Batch { id } => {
                    let value = network(
                        self.client.management_page(Some(id), None, None, 64),
                        &mut self.core,
                        commands,
                    )
                    .await?;
                    self.core.context(command)?;
                    Ok(Reply::Activities { value })
                }
                EntrySelector::Task { task, attempt } => {
                    let request = self
                        .core
                        .app
                        .service_remote_request(&plan::id(task)?, &plan::id(attempt)?)
                        .map_err(map_app_error)?
                        .ok_or(Error::NotFound)?;
                    self.core.task_operation(
                        command,
                        &execution_app::tasks::TaskOperation::Detail(
                            execution_app::tasks::TaskRef::Request { request },
                        ),
                        &origin,
                    )
                }
                EntrySelector::Event { .. } => Err(Error::Unsupported),
                _ => Err(Error::Protocol),
            },
            EntryOperation::Activities {
                batch,
                related,
                after,
                limit,
            } => {
                let value = network(
                    self.client.management_page(
                        batch.as_deref(),
                        related.as_deref(),
                        after.as_deref(),
                        *limit,
                    ),
                    &mut self.core,
                    commands,
                )
                .await?;
                self.core.context(command)?;
                Ok(Reply::Activities { value })
            }
            EntryOperation::Invoke { intent, request } => {
                intent.validate().map_err(|_| Error::Protocol)?;
                if intent.action != EntryAction::Invoke
                    || intent
                        .namespace
                        .as_ref()
                        .is_some_and(|n| Some(n) != self.entry_namespace().as_ref())
                {
                    return Err(Error::Identity);
                }
                match &intent.selector {
                    EntrySelector::LocalTool { name } => {
                        if command.request.is_machine() {
                            return Err(Error::Denied);
                        }
                        let mut operation = serde_json::to_value(&intent.parameters)
                            .map_err(|_| Error::Protocol)?;
                        let fields = operation.as_object_mut().ok_or(Error::Protocol)?;
                        let kind = fields
                            .get("kind")
                            .and_then(|v| v.as_str())
                            .ok_or(Error::Protocol)?
                            .to_owned();
                        let original = match name {
                            LocalTool::Software if kind != "history" => fields
                                .get_mut("input")
                                .and_then(|v| v.as_object_mut())
                                .ok_or(Error::Protocol)?,
                            _ => fields,
                        };
                        let mutating = match name {
                            LocalTool::Network => kind == "run",
                            LocalTool::Settings => {
                                matches!(kind.as_str(), "refresh" | "change" | "openNative")
                            }
                            LocalTool::Software => kind == "prepare",
                        };
                        if mutating {
                            if original
                                .get("request")
                                .is_some_and(|v| v.as_str() != Some(request.as_str()))
                            {
                                return Err(Error::Conflict);
                            }
                            original.insert("request".into(), serde_json::json!(request));
                        }
                        let request = match name {
                            LocalTool::Network => Request::NetworkCheck {
                                context: context.clone(),
                                origin: origin.clone(),
                                input: serde_json::from_value(operation)
                                    .map_err(|_| Error::Protocol)?,
                            },
                            LocalTool::Settings => Request::Settings {
                                context: context.clone(),
                                origin: origin.clone(),
                                input: serde_json::from_value(operation)
                                    .map_err(|_| Error::Protocol)?,
                            },
                            LocalTool::Software => Request::LocalSoftware {
                                context: context.clone(),
                                origin: origin.clone(),
                                input: serde_json::from_value(operation)
                                    .map_err(|_| Error::Protocol)?,
                            },
                        };
                        self.policy_operation(command, &request, commands).await
                    }
                    EntrySelector::Capability { selector } => {
                        let previous = self
                            .core
                            .app
                            .backend_request(&self.core.caller(), request)
                            .map_err(map_app_error)?;
                        let mut expected = None;
                        let choice = if let Some(previous) = previous {
                            let PreparedAction::Policy {
                                choice,
                                address,
                                definition,
                                policy_revision,
                                ..
                            } = previous.action
                            else {
                                return Err(Error::Conflict);
                            };
                            if serde_json::to_value(address).map_err(|_| Error::Protocol)?
                                != serde_json::to_value(&intent.selector)
                                    .map_err(|_| Error::Protocol)?
                            {
                                return Err(Error::Conflict);
                            }
                            if choice.parameters != intent.parameters
                                || intent
                                    .version
                                    .as_ref()
                                    .is_some_and(|v| v != definition.revision.as_str())
                                || intent.revision.is_some_and(|v| v != policy_revision)
                            {
                                return Err(Error::Conflict);
                            }
                            if selector
                                .policy_id()?
                                .is_some_and(|id| id.to_string() != choice.policy_id.as_str())
                            {
                                return Err(Error::Conflict);
                            }
                            choice
                        } else {
                            let current = self
                                .resolve_policy(&bound, &origin, selector.clone(), commands)
                                .await?;
                            policy::operation_parameters_supported(&current)?;
                            if intent
                                .version
                                .as_ref()
                                .is_some_and(|v| *v != current.policy_version.to_string())
                                || intent
                                    .revision
                                    .is_some_and(|v| v != current.policy_revision)
                            {
                                return Err(Error::Conflict);
                            }
                            expected = Some((
                                current.policy_version,
                                current.policy_revision,
                                intent.selector.clone(),
                            ));
                            PolicyChoice {
                                request: request.clone(),
                                policy_id: plan::id(current.policy_id.to_string())?,
                                parameters: intent.parameters.clone(),
                            }
                        };
                        self.invoke_task_policy(
                            command, &bound, &origin, &choice, false, None, expected, commands,
                        )
                        .await
                    }
                    EntrySelector::Event { name } if intent.parameters.is_empty() => {
                        self.invoke_entry_event(command, &bound, name, request, commands)
                            .await
                    }
                    _ => Err(Error::Unsupported),
                }
            }
            EntryOperation::Sync => {
                let scope = self.core.task_scope(command)?;
                let cache = network(
                    self.client.sync_management(&scope),
                    &mut self.core,
                    commands,
                )
                .await?;
                self.core.context(command)?;
                self.core.management = Some(cache);
                self.core.management_online = true;
                self.core.task_operation(
                    command,
                    &execution_app::tasks::TaskOperation::List(execution_app::tasks::TaskQuery {
                        view: execution_app::tasks::TaskView::Management,
                        limit: 64,
                        ..Default::default()
                    }),
                    &origin,
                )
            }
            EntryOperation::Refresh { request, query } => {
                let mut input = DataOperation::Submit(QuerySubmit {
                    request: request.clone(),
                    query: query.clone(),
                    parameters: Default::default(),
                });
                if command.request.is_machine() {
                    self.check_machine_data(command, &mut input)?;
                }
                self.data_command(command, &context, &origin, &input, commands)
                    .await
            }
            EntryOperation::Checkin => unreachable!(),
        }
    }
    fn check_machine_data(
        &mut self,
        command: &Command,
        input: &mut DataOperation,
    ) -> Result<(), Error> {
        self.core.data.install_local()?;
        let definition = match input {
            DataOperation::Directory(query) => {
                query.refresh_policies = false;
                if query.policy_cursor.is_some() {
                    return Err(Error::Denied);
                }
                return Ok(());
            }
            DataOperation::Submit(query) => {
                if !query.parameters.is_empty() {
                    return Err(Error::Denied);
                }
                let definition = self
                    .core
                    .data
                    .local_definition(&query.query, &command.subject)?;
                if definition.read.execution.identity != wire::ExecutionIdentity::System {
                    return Err(Error::Denied);
                }
                definition.descriptor
            }
            DataOperation::Read(read) => match &read.selection {
                DataSelection::Query { query } => self
                    .core
                    .data
                    .directory(&command.subject)?
                    .into_iter()
                    .find(|v| v.reference == *query)
                    .ok_or(Error::NotFound)?,
                DataSelection::Request { request } => {
                    let association = self.core.data.association(
                        request,
                        &command.subject,
                        command.binding.as_ref().ok_or(Error::Denied)?,
                    )?;
                    self.core
                        .data
                        .directory(&command.subject)?
                        .into_iter()
                        .find(|v| v.reference == association.query)
                        .ok_or(Error::NotFound)?
                }
            },
            DataOperation::Status(query) => {
                let association = self.core.data.association(
                    &query.request,
                    &command.subject,
                    command.binding.as_ref().ok_or(Error::Denied)?,
                )?;
                self.core
                    .data
                    .directory(&command.subject)?
                    .into_iter()
                    .find(|v| v.reference == association.query)
                    .ok_or(Error::NotFound)?
            }
        };
        if !matches!(definition.scope, DataScope::Device)
            || matches!(definition.kind, QueryKind::Policy)
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    async fn invoke_entry_event(
        &mut self,
        command: &Command,
        bound: &policy::NativeContext,
        name: &wire::PolicyEventName,
        request: &RequestId,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        let caller = self.core.caller();
        let trigger = policy::trigger(&self.core, command, &ClientOrigin::Desktop {})?;
        let namespace = self.entry_namespace().ok_or(Error::Identity)?;
        let mut record = if let Some(old) = self
            .core
            .app
            .backend_request(&caller, request)
            .map_err(map_app_error)?
        {
            let PreparedAction::Event {
                input,
                namespace: original,
                ..
            } = &old.action
            else {
                return Err(Error::Conflict);
            };
            if input.event.as_str() != name.as_str()
                || *original != namespace
                || old.trigger != trigger
            {
                return Err(Error::Conflict);
            }
            old
        } else {
            let input = wire::PolicyEventRequest {
                operation_id: uuid::Uuid::parse_str(request.as_str())
                    .map_err(|_| Error::Protocol)?,
                event: name.clone(),
                source: wire::PolicyEventSource::Human,
                execution_context: Some(self.client.execution_context()?),
            };
            let record = BackendRequest {
                retry_of: None,
                retry_expected_revision: None,
                recorded_at_unix_ms: self.core.host.clock.millis()?,
                cancellation_control: None,
                cancellation_expected_revision: None,
                action: PreparedAction::Event {
                    request: request.clone(),
                    input,
                    namespace,
                    receipt: None,
                    batch: None,
                    expires_at_unix_ms: bound
                        .expires_at_unix_ms
                        .min(self.core.host.clock.millis()?.saturating_add(5 * 60 * 1000)),
                },
                trigger,
                risk: None,
                confirmation: None,
                revision: 1,
                state: BackendRequestState::Ready,
                failure: None,
            };
            self.core
                .app
                .record_backend_request(&caller, None, &record)
                .map_err(map_app_error)?;
            record
        };
        let PreparedAction::Event {
            receipt,
            batch,
            expires_at_unix_ms,
            ..
        } = &record.action
        else {
            return Err(Error::Protocol);
        };
        if receipt.is_some() || batch.is_some() {
            return Ok(Reply::PolicyRequest {
                value: Box::new(record),
            });
        }
        if !matches!(
            record.state,
            BackendRequestState::Ready | BackendRequestState::Submitting
        ) || self.core.host.clock.millis()? >= *expires_at_unix_ms
        {
            return Err(Error::Expired);
        }
        if record.state == BackendRequestState::Submitting {
            // Observe the producer's original request before considering exact replay.
            // A query failure is unknown, never permission to issue another mutation.
            let batch = network(
                self.client.event_batch(input_id(&record)?),
                &mut self.core,
                commands,
            )
            .await?;
            self.core.context(command)?;
            if let Some(batch) = batch {
                let expected = record.clone();
                if let PreparedAction::Event { batch: saved, .. } = &mut record.action {
                    *saved = Some(plan::id(batch.to_string())?);
                }
                record.state = BackendRequestState::Observed;
                record.revision += 1;
                self.core
                    .app
                    .record_backend_request(&caller, Some(&expected), &record)
                    .map_err(map_app_error)?;
                return Ok(Reply::PolicyRequest {
                    value: Box::new(record),
                });
            }
        }
        if record.state == BackendRequestState::Ready {
            record = self
                .core
                .transition(request, BackendRequestState::Submitting, None)?
                .ok_or(Error::Conflict)?;
        }
        let PreparedAction::Event { input, .. } = &record.action else {
            return Err(Error::Protocol);
        };
        // Replays use the exact frozen request; the producer returns its original batch.
        let receipt = network(
            self.client.invoke_policy_event(input),
            &mut self.core,
            commands,
        )
        .await?;
        self.core.context(command)?;
        let expected = record.clone();
        let PreparedAction::Event { receipt: saved, .. } = &mut record.action else {
            return Err(Error::Protocol);
        };
        *saved = Some(receipt);
        record.revision += 1;
        record.state = BackendRequestState::Observed;
        self.core
            .app
            .record_backend_request(&caller, Some(&expected), &record)
            .map_err(map_app_error)?;
        Ok(Reply::PolicyRequest {
            value: Box::new(record),
        })
    }
}

fn input_id(record: &BackendRequest) -> Result<uuid::Uuid, Error> {
    match &record.action {
        PreparedAction::Event { input, .. } => Ok(input.operation_id),
        _ => Err(Error::Protocol),
    }
}
