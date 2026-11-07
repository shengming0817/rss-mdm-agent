//! CLI and Deep Link consume the original business owners; no directory or journal lives here.
use super::*;
use execution_app::tasks::{TaskFacts, TaskOperation, TaskRef, TaskResponse};
use execution_contract::*;
use execution_ipc::{
    entry::{EntryContext, EntryOperation, OriginalStep},
    host::{ClientOrigin, NativeContextSession, Reply, Request},
};
use rss_mdm_agent_wire::activities::{ActivityPhase, ManagementActivity};

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
            EntryOperation::Resolve { intent, after } => {
                intent.validate().map_err(|_| Error::Protocol)?;
                let namespace = self.entry_namespace();
                if intent.action != EntryAction::View
                    || (intent.namespace.is_some() && intent.namespace != namespace)
                    || (after.is_some() && !matches!(intent.selector, EntrySelector::Batch { .. }))
                {
                    return Err(Error::Denied);
                }
                let reply = self
                    .resolve_entry(command, intent, after.as_deref(), commands)
                    .await?;
                self.check_entry_scope(command, &context, namespace.as_ref())?;
                Ok(reply)
            }
            EntryOperation::Activities {
                related,
                after,
                limit,
            } => {
                let namespace = self.entry_namespace().ok_or(Error::Identity)?;
                let value = network(
                    self.client
                        .management_page(None, related.as_deref(), after.as_deref(), *limit),
                    &mut self.core,
                    commands,
                )
                .await?;
                self.check_entry_scope(command, &context, Some(&namespace))?;
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
    fn check_entry_scope(
        &self,
        command: &Command,
        context: &NativeContextRef,
        namespace: Option<&EntryNamespace>,
    ) -> Result<(), Error> {
        if &self.core.context(command)?.context != context
            || self.entry_namespace().as_ref() != namespace
        {
            return Err(Error::Identity);
        }
        Ok(())
    }

    async fn resolve_entry(
        &mut self,
        command: &Command,
        intent: &EntryIntent,
        after: Option<&str>,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        // Supplementary step evidence shares the entry's interactive IPC budget.
        let step_deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(2400);
        let reference = match &intent.selector {
            EntrySelector::LocalTool { name } => {
                return Ok(Reply::LocalTool {
                    value: LocalToolDescriptor {
                        name: *name,
                        input_schema: name.schema(),
                        node: name.node().into(),
                    },
                });
            }
            EntrySelector::Node { id } => {
                return Ok(Reply::EntryNodes {
                    value: vec![id.clone()],
                });
            }
            EntrySelector::Event { name } => return Ok(Reply::EntryEvent { name: name.clone() }),
            EntrySelector::Batch { id } => {
                let value = network(
                    self.client.management_page(Some(id), None, after, 64),
                    &mut self.core,
                    commands,
                )
                .await?;
                return Ok(Reply::Activities { value });
            }
            EntrySelector::Request { request } => TaskRef::Request {
                request: request.clone(),
            },
            EntrySelector::Sampling { query } => TaskRef::Sampling {
                query: query.clone(),
            },
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
                TaskRef::Activity { id: id.clone() }
            }
            EntrySelector::Task { task, attempt } => {
                if let Some(request) = self.entry_task_request(task, attempt)? {
                    TaskRef::Request { request }
                } else if let Some(step) = intent.step.as_deref() {
                    let task = uuid::Uuid::parse_str(task).map_err(|_| Error::Protocol)?;
                    let attempt = uuid::Uuid::parse_str(attempt).map_err(|_| Error::Protocol)?;
                    let fact = self
                        .entry_attempt_activity(task, attempt, step_deadline, commands)
                        .await?
                        .ok_or(Error::NotFound)?;
                    let step = activity_step(&fact, step);
                    let reference = TaskRef::Activity {
                        id: fact.id.clone(),
                    };
                    // This exact read must not replace the regular management timeline cache.
                    let mut projection = agent_client::ManagementCache::default();
                    projection.scope = self.core.task_scope(command)?;
                    projection.items.insert(fact.id.clone(), fact);
                    let previous = self.core.management.replace(projection);
                    let reply = self.core.task_operation(
                        command,
                        &TaskOperation::Detail(reference),
                        &ClientOrigin::Desktop {},
                    );
                    self.core.management = previous;
                    let Reply::Task {
                        value: TaskResponse::Detail(value),
                    } = reply?
                    else {
                        return Err(Error::Protocol);
                    };
                    return Ok(Reply::EntryStep { value, step });
                } else {
                    return Err(Error::NotFound);
                }
            }
            _ => return Err(Error::Unsupported),
        };
        let origin = ClientOrigin::Desktop {};
        let reply =
            self.core
                .task_operation(command, &TaskOperation::Detail(reference), &origin)?;
        let Some(step) = intent.step.as_deref() else {
            return Ok(reply);
        };
        let Reply::Task {
            value: TaskResponse::Detail(mut detail),
        } = reply
        else {
            return Err(Error::Protocol);
        };
        let mut original_activity = None;
        if let TaskFacts::Management(fact) = &detail.facts {
            let namespace = self.entry_namespace().ok_or(Error::Identity)?;
            if let Some((task, attempt)) = fact.links.task.zip(fact.links.attempt) {
                if !fact.target.as_ref().is_some_and(|target| {
                    target.device == namespace.device
                        && target.registration.to_string() == namespace.registration
                        && u64::try_from(target.generation).ok() == Some(namespace.generation)
                }) {
                    return Err(Error::Identity);
                }
                original_activity = Some(fact.clone());
                if let Some(request) =
                    self.entry_task_request(&task.to_string(), &attempt.to_string())?
                {
                    match self.core.task_operation(
                        command,
                        &TaskOperation::Detail(TaskRef::Request { request }),
                        &origin,
                    ) {
                        Ok(Reply::Task {
                            value: TaskResponse::Detail(value),
                        }) => detail = value,
                        Ok(_) => return Err(Error::Protocol),
                        Err(Error::Denied | Error::NotFound) => {}
                        Err(error) => return Err(error),
                    }
                }
            }
        }
        let step = if matches!(intent.selector, EntrySelector::Request { .. }) {
            detail
                .nodes
                .iter()
                .find(|node| node.id == step)
                .map(|node| OriginalStep {
                    id: step.into(),
                    node: Some(node.id.clone()),
                })
        } else {
            self.resolve_entry_step(
                &detail,
                original_activity.as_deref(),
                step,
                step_deadline,
                commands,
            )
            .await?
        };
        Ok(Reply::EntryStep {
            value: detail,
            step,
        })
    }

    fn entry_task_request(&self, task: &str, attempt: &str) -> Result<Option<RequestId>, Error> {
        self.core
            .app
            .service_remote_request(&plan::id(task)?, &plan::id(attempt)?)
            .map_err(map_app_error)
    }

    async fn resolve_entry_step(
        &mut self,
        detail: &execution_app::tasks::TaskDetail,
        activity: Option<&ManagementActivity>,
        step: &str,
        deadline: tokio::time::Instant,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Option<OriginalStep>, Error> {
        let observed = activity.and_then(|fact| activity_step(fact, step));
        let pair = tasks::remote_pair(&detail.facts)
            .or_else(|| activity.and_then(|fact| fact.links.task.zip(fact.links.attempt)));
        let Some((task, attempt)) = pair else {
            return Ok(observed);
        };
        let local = match &detail.facts {
            TaskFacts::Execution(facts) => match &facts.action.execution {
                execution_app::ExecutionSummary::SoftwareProgram { steps, .. } => Some((
                    steps.len(),
                    facts.status.attempt_id.as_ref().map(|id| id.as_str()),
                    facts.action.operation.resource.revision.as_str(),
                )),
                _ => return Ok(observed),
            },
            _ => None,
        };
        let selection = |ids: &[String]| match local {
            Some((count, attempt, _)) => {
                entry_step_selection(ids, count, attempt, step, &detail.nodes)
            }
            None => original_step(ids, step),
        };
        if let Some(ids) =
            self.client
                .retained_attempt_steps(task, attempt, local.map(|(_, _, digest)| digest))?
        {
            return Ok(selection(&ids).or(observed));
        }
        if let Some(value) = activity
            .filter(|fact| matches!(fact.phase, ActivityPhase::Offered | ActivityPhase::Result))
            .and_then(|fact| selection(&fact.links.steps))
        {
            return Ok(Some(value));
        }
        let fact = self
            .entry_attempt_activity(task, attempt, deadline, commands)
            .await;
        match fact {
            Ok(fact) => Ok(fact
                .and_then(|fact| selection(&fact.links.steps))
                .or(observed)),
            Err(Error::Unavailable | Error::Network) => Ok(observed),
            Err(error) => Err(error),
        }
    }

    async fn entry_attempt_activity(
        &mut self,
        task: uuid::Uuid,
        attempt: uuid::Uuid,
        deadline: tokio::time::Instant,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Option<ManagementActivity>, Error> {
        tokio::time::timeout_at(
            deadline,
            network(
                self.client.attempt_activity(task, attempt),
                &mut self.core,
                commands,
            ),
        )
        .await
        .map_err(|_| Error::Unavailable)?
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

fn activity_step(fact: &ManagementActivity, step: &str) -> Option<OriginalStep> {
    fact.links.task.zip(fact.links.attempt)?;
    original_step(&fact.links.steps, step)
}

fn original_step(ids: &[String], step: &str) -> Option<OriginalStep> {
    let mut matches = ids.iter().filter(|id| id.as_str() == step);
    matches.next()?;
    matches.next().is_none().then(|| OriginalStep {
        id: step.into(),
        node: None,
    })
}

fn entry_step_selection(
    original: &[String],
    frozen_count: usize,
    local_attempt: Option<&str>,
    step: &str,
    nodes: &[execution_app::tasks::TaskNode],
) -> Option<OriginalStep> {
    if original.len() != frozen_count {
        return None;
    }
    let mut matches = original
        .iter()
        .enumerate()
        .filter(|(_, id)| id.as_str() == step);
    let (index, _) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let node = local_attempt.and_then(|attempt| {
        let id = format!("step/{attempt}/{index}");
        nodes
            .iter()
            .any(|node| node.id == id && node.parent.is_none())
            .then_some(id)
    });
    Some(OriginalStep {
        id: step.into(),
        node,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_step_keeps_frozen_identity_without_inventing_a_checkpoint() {
        use execution_app::tasks::TaskNode;
        let nodes = [TaskNode {
            id: "step/local-attempt/1".into(),
            parent: None,
            stage: "software-step".into(),
            bytes: None,
            total: None,
            attempt: Some("local-attempt".into()),
            reason: None,
            exit_code: None,
            detected: None,
            quiescent: None,
        }];
        let original = ["prerequisite".into(), "install".into()];
        assert_eq!(
            entry_step_selection(&original, 2, Some("local-attempt"), "install", &nodes),
            Some(OriginalStep {
                id: "install".into(),
                node: Some(nodes[0].id.clone())
            }),
        );
        for (attempt, step) in [
            (Some("local-attempt"), "prerequisite"),
            (Some("other-attempt"), "install"),
            (None, "install"),
        ] {
            assert_eq!(
                entry_step_selection(&original, 2, attempt, step, &nodes),
                Some(OriginalStep {
                    id: step.into(),
                    node: None
                })
            );
        }
        assert!(entry_step_selection(&original, 2, None, "unknown", &nodes).is_none());
        assert!(entry_step_selection(&original, 3, None, "install", &nodes).is_none());
    }
}
