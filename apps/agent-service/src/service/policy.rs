//! Policy self-service is a projection of backend authority, driven by the sole device owner.
use super::*;
use execution_contract::{ExecutionRisk, Id, PolicyAccess, PreparedAction, RiskLevel, RiskSource};
use execution_ipc::host::{ClientOrigin, NativeContextSession, Reply, Request};
use std::collections::{BTreeMap, VecDeque};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SessionKey {
    subject: String,
    binding: Id,
}
impl SessionKey {
    pub(crate) fn new(subject: String, binding: Id) -> Self {
        Self { subject, binding }
    }
    fn command(command: &Command) -> Result<Self, Error> {
        Ok(Self::new(
            command.subject.clone(),
            command.binding.clone().ok_or(Error::Denied)?,
        ))
    }
}
#[derive(Default)]
pub(crate) struct Fences(std::sync::Mutex<BTreeMap<SessionKey, u64>>);
impl Fences {
    pub(super) fn current(&self, session: &SessionKey) -> Result<u64, Error> {
        Ok(*self
            .0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(session)
            .unwrap_or(&0))
    }
    pub(crate) fn admit(
        &self,
        session: &SessionKey,
        expected: u64,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<SessionKey, u64>>, Error> {
        let epochs = self.0.lock().map_err(|_| Error::Unavailable)?;
        if *epochs.get(session).unwrap_or(&0) != expected {
            return Err(Error::Identity);
        }
        Ok(epochs)
    }
    pub(super) fn revoke(&self, session: &SessionKey) -> Result<(), Error> {
        let mut epochs = self.0.lock().map_err(|_| Error::Unavailable)?;
        let epoch = epochs.entry(session.clone()).or_default();
        *epoch = epoch.checked_add(1).ok_or(Error::Capacity)?;
        Ok(())
    }
}
pub(super) struct NativeFence {
    pub(super) policy: execution_ipc::host::PeerPolicy,
    pub(super) fences: Arc<Fences>,
    pub(super) finished: Arc<AtomicBool>,
}
impl execution_ipc::host::SessionFence for NativeFence {
    fn revoke(&self, peer: &execution_ipc::host::Peer) -> Reply {
        if self.finished.load(Ordering::Acquire) {
            return Reply::Unavailable;
        }
        match peer.authenticate(&self.policy).and_then(|subject| {
            let binding = peer.session_binding()?;
            self.fences
                .revoke(&SessionKey::new(subject, binding))
                .map_err(|_| execution_app::Error::Unavailable)
        }) {
            Ok(()) => Reply::ContextRevoked,
            Err(_) => Reply::Rejected,
        }
    }
}
#[derive(Clone)]
pub(super) struct NativeContext {
    session: NativeContextSession,
    scope: SessionKey,
    epoch: u64,
    lease: Option<Arc<zeroize::Zeroizing<String>>>,
    pub(super) context: execution_contract::NativeContextRef,
}
impl std::ops::Deref for NativeContext {
    type Target = NativeContextSession;
    fn deref(&self) -> &Self::Target {
        &self.session
    }
}
pub(super) type Contexts = BTreeMap<String, NativeContext>;
pub(super) type Commands = VecDeque<Command>;
fn peer_key(command: &Command) -> Result<String, Error> {
    Ok(format!(
        "{}:{}:{}",
        command.subject,
        command.session,
        command.binding.as_ref().ok_or(Error::Denied)?.as_str()
    ))
}
fn source(origin: &ClientOrigin) -> wire::SelfServiceCaller {
    match origin {
        ClientOrigin::Desktop {} => wire::SelfServiceCaller::Human,
        ClientOrigin::Ai { .. } => wire::SelfServiceCaller::Ai,
    }
}
fn trigger(core: &Core, command: &Command, origin: &ClientOrigin) -> Result<BackendTrigger, Error> {
    core.require_origin_context(
        &command.subject,
        command.binding.as_ref().ok_or(Error::Denied)?,
        origin,
    )?;
    let os_session = execution_contract::OsSessionRef {
        device: core.host.binding.device.clone(),
        account: execution_contract::OsAccountRef {
            platform: plan::platform()?,
            subject: plan::id(&command.subject)?,
        },
        session: command.binding.clone().ok_or(Error::Denied)?,
    };
    Ok(match origin {
        ClientOrigin::Desktop {} => BackendTrigger::Human { os_session },
        ClientOrigin::Ai {
            config,
            conversation,
            tool_call,
            context,
        } => BackendTrigger::Ai {
            os_session,
            config: config.clone(),
            conversation: conversation.clone(),
            tool_call: tool_call.clone(),
            context: context.clone(),
        },
    })
}
fn uuid(value: &str) -> Result<Uuid, Error> {
    let parsed = Uuid::parse_str(value).map_err(|_| Error::Protocol)?;
    if parsed.is_nil() || parsed.to_string() != value {
        return Err(Error::Protocol);
    }
    Ok(parsed)
}
// Generic operation cards have no secret reference owner. Secret fields remain unsupported;
// local schema references are handled by the existing validator.
fn operation_parameters_supported(directory: &wire::SelfServiceDirectoryItem) -> Result<(), Error> {
    fn unsafe_schema(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(fields) => {
                fields.get("writeOnly") == Some(&serde_json::Value::Bool(true))
                    || fields.get("format").and_then(|v| v.as_str()) == Some("password")
                    || fields.values().any(unsafe_schema)
            }
            serde_json::Value::Array(values) => values.iter().any(unsafe_schema),
            _ => false,
        }
    }
    if let wire::SelfServiceTask::Script {
        parameter_schema, ..
    } = &directory.task
    {
        if unsafe_schema(parameter_schema) {
            return Err(Error::Unsupported);
        }
    }
    Ok(())
}
fn parameters(
    directory: &wire::SelfServiceDirectoryItem,
    inputs: &BTreeMap<String, serde_json::Value>,
) -> Result<BTreeMap<String, serde_json::Value>, Error> {
    let wire::SelfServiceTask::Script {
        parameter_sources,
        parameter_schema,
        ..
    } = &directory.task
    else {
        return if inputs.is_empty() {
            Ok(BTreeMap::new())
        } else {
            Err(Error::Protocol)
        };
    };
    let mut resolved = BTreeMap::new();
    let mut expected = 0;
    for (key, source) in parameter_sources {
        let value = match source {
            wire::ScriptParameterSource::Fixed { value } => value.clone(),
            wire::ScriptParameterSource::Input {} => {
                expected += 1;
                inputs.get(key).ok_or(Error::Protocol)?.clone()
            }
        };
        resolved.insert(key.clone(), value);
    }
    if inputs.len() != expected {
        return Err(Error::Protocol);
    }
    let value = serde_json::to_value(&resolved).map_err(|_| Error::Protocol)?;
    let validator = jsonschema::validator_for(parameter_schema).map_err(|_| Error::Protocol)?;
    if !validator.is_valid(&value) {
        return Err(Error::Protocol);
    }
    Ok(resolved)
}
impl Core {
    pub(super) fn require_origin_context(
        &self,
        subject: &str,
        session: &Id,
        origin: &ClientOrigin,
    ) -> Result<(), Error> {
        if let ClientOrigin::Ai { context, .. } = origin {
            let bound = self
                .contexts
                .iter()
                .find(|(key, value)| {
                    key.starts_with(&format!("{subject}:"))
                        && &value.scope.binding == session
                        && &value.context == context
                })
                .map(|(_, value)| value)
                .ok_or(Error::Identity)?;
            if self.fences.current(&bound.scope)? != bound.epoch {
                return Err(Error::Identity);
            }
            if self.host.clock.millis()? >= bound.expires_at_unix_ms {
                return Err(Error::Expired);
            }
        }
        Ok(())
    }
    pub(super) fn data_contexts(&self) -> Vec<(String, Id, execution_contract::NativeContextRef)> {
        self.contexts
            .values()
            .filter(|c| self.fences.current(&c.scope).ok() == Some(c.epoch))
            .map(|c| {
                (
                    c.scope.subject.clone(),
                    c.scope.binding.clone(),
                    c.context.clone(),
                )
            })
            .collect()
    }
    pub(super) fn data_context(
        &self,
        command: &Command,
        expected: &execution_contract::NativeContextRef,
    ) -> Result<&NativeContext, Error> {
        let bound = self
            .contexts
            .get(&peer_key(command)?)
            .ok_or(Error::Identity)?;
        if self.stopping
            || self.fences.current(&bound.scope)? != bound.epoch
            || &bound.context != expected
        {
            return Err(Error::Identity);
        }
        Ok(bound)
    }
    pub(super) fn context(&self, command: &Command) -> Result<&NativeContext, Error> {
        if self.host.revoked.load(Ordering::Acquire) || self.stopping {
            return Err(Error::Denied);
        }
        let bound = self
            .contexts
            .get(&peer_key(command)?)
            .ok_or(Error::Identity)?;
        if self.fences.current(&bound.scope)? != bound.epoch {
            return Err(Error::Identity);
        }
        if self.host.clock.millis()? >= bound.expires_at_unix_ms {
            return Err(Error::Expired);
        }
        Ok(bound)
    }
    fn policy_context(&self, record: &BackendRequest) -> Result<Option<&NativeContext>, Error> {
        if self.host.revoked.load(Ordering::Acquire) || self.stopping {
            return Err(Error::Denied);
        }
        let login = match &record.trigger {
            BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. } => {
                os_session
            }
            _ => return Err(Error::Identity),
        };
        if login.device != self.host.binding.device || login.account.platform != plan::platform()? {
            return Err(Error::Identity);
        }
        let PreparedAction::Policy { access, .. } = &record.action else {
            return Err(Error::Protocol);
        };
        let session_binding = match access {
            PolicyAccess::Device {} => return Ok(None),
            PolicyAccess::User { session_binding } => session_binding,
        };
        let scope = SessionKey::new(
            login.account.subject.as_str().to_owned(),
            login.session.clone(),
        );
        let current = self
            .contexts
            .values()
            .find(|bound| bound.scope == scope && &bound.context == session_binding)
            .ok_or(Error::Identity)?;
        if self.fences.current(&scope)? != current.epoch {
            return Err(Error::Identity);
        }
        if self.host.clock.millis()? >= current.expires_at_unix_ms {
            return Err(Error::Expired);
        }
        Ok(Some(current))
    }
    fn current_policy_context(
        &self,
        record: &BackendRequest,
    ) -> Result<Option<&NativeContext>, Error> {
        let context = self.policy_context(record)?;
        if self.host.clock.millis()? >= record.action.expires_at_unix_ms() {
            return Err(Error::Expired);
        }
        Ok(context)
    }
    pub(super) fn policy_readable(
        &self,
        request: &RequestId,
        subject: &str,
        binding: Option<&Id>,
    ) -> Result<bool, Error> {
        let Some(record) = self
            .app
            .backend_request(&self.caller(), request)
            .map_err(map_app_error)?
        else {
            let frozen = self
                .app
                .service_frozen_input(request)
                .map_err(map_app_error)?;
            if frozen.spec().request.authority == self.host.root.authority
                && [
                    "local-software-choice",
                    "local-settings-action",
                    "network-check-target",
                ]
                .iter()
                .any(|key| frozen.spec().request.parameters.contains_key(*key))
            {
                crate::account_identity::verify_local_owner(&frozen, &self.host.root, subject)?;
                return Ok(self.contexts.values().any(|current| {
                    current.scope.subject == subject
                        && Some(&current.scope.binding) == binding
                        && self.fences.current(&current.scope).ok() == Some(current.epoch)
                        && self
                            .host
                            .clock
                            .millis()
                            .is_ok_and(|now| now < current.expires_at_unix_ms)
                }));
            }

            return Ok(true);
        };
        if matches!(record.action, PreparedAction::Policy { .. }) {
            return Ok(trigger_subject(&record.trigger) == Some(subject)
                && record.trigger_os_session() == binding
                && self.policy_context(&record).is_ok());
        }
        if let BackendTrigger::Ai {
            os_session,
            context,
            config,
            conversation,
            tool_call,
        } = &record.trigger
        {
            return Ok(os_session.account.subject.as_str() == subject
                && Some(&os_session.session) == binding
                && self
                    .require_origin_context(
                        subject,
                        &os_session.session,
                        &ClientOrigin::Ai {
                            context: context.clone(),
                            config: config.clone(),
                            conversation: conversation.clone(),
                            tool_call: tool_call.clone(),
                        },
                    )
                    .is_ok());
        }
        Ok(true)
    }
    pub(crate) fn policy_fence(
        &self,
        record: &BackendRequest,
    ) -> Result<Option<(Arc<Fences>, SessionKey, u64)>, Error> {
        if matches!(
            record.action,
            PreparedAction::Policy {
                run_id: Some(_),
                ..
            }
        ) {
            return Ok(None);
        }

        if matches!(record.action, PreparedAction::Policy { .. }) {
            return Ok(self
                .current_policy_context(record)?
                .map(|bound| (self.fences.clone(), bound.scope.clone(), bound.epoch)));
        }
        if let BackendTrigger::Ai {
            os_session,
            context,
            config,
            conversation,
            tool_call,
        } = &record.trigger
        {
            self.require_origin_context(
                os_session.account.subject.as_str(),
                &os_session.session,
                &ClientOrigin::Ai {
                    context: context.clone(),
                    config: config.clone(),
                    conversation: conversation.clone(),
                    tool_call: tool_call.clone(),
                },
            )?;
            let bound = self
                .contexts
                .values()
                .find(|bound| &bound.context == context)
                .ok_or(Error::Identity)?;
            return Ok(Some((
                self.fences.clone(),
                bound.scope.clone(),
                bound.epoch,
            )));
        }
        Ok(None)
    }
    pub(crate) fn current_policy_access(&self, record: &BackendRequest) -> Result<(), Error> {
        if self.host.revoked.load(Ordering::Acquire) || self.stopping {
            return Err(Error::Denied);
        }
        if matches!(
            record.action,
            PreparedAction::Policy {
                run_id: Some(_),
                ..
            }
        ) {
            return Ok(());
        }
        if matches!(record.action, PreparedAction::Policy { .. }) {
            self.current_policy_context(record)?;
        } else if let BackendTrigger::Ai {
            os_session,
            context,
            config,
            conversation,
            tool_call,
        } = &record.trigger
        {
            self.require_origin_context(
                os_session.account.subject.as_str(),
                &os_session.session,
                &ClientOrigin::Ai {
                    context: context.clone(),
                    config: config.clone(),
                    conversation: conversation.clone(),
                    tool_call: tool_call.clone(),
                },
            )?;
        }
        Ok(())
    }
    pub(super) fn enqueue_policy(&mut self, command: Command) {
        if let Some(Request::RevokeContext {}) = command.request.as_operation() {
            self.management = None;
            self.management_online = false;
            self.management_clear = true;
            let _ = SessionKey::command(&command).and_then(|scope| self.fences.revoke(&scope));
            if let Ok(key) = peer_key(&command) {
                if let Some(context) = self.contexts.get_mut(&key) {
                    context.lease = None;
                    context.session.clear_credential();
                }
            }
            let _ = command.reply.send(Reply::ContextRevoked);
            return;
        }
        if self.policy_commands.len() >= 32 {
            let _ = command.reply.send(Reply::Unavailable);
        } else {
            self.policy_commands.push_back(command);
        }
    }
}
impl<S: IdentityProvider> DeviceService<S> {
    pub(super) async fn drive_policy(
        &mut self,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Error> {
        // Only this owner performs HTTP or journal IO. Native callbacks merely enqueue.
        if self.core.management_clear {
            self.client.clear_management_cache()?;
            self.core.management_clear = false;
        }
        while let Some(command) = self.core.policy_commands.pop_front() {
            if command.reply.is_closed() {
                continue;
            }
            let result = self.policy_command(&command, commands).await;
            let operation_submit = match command.request.as_operation() {
                Some(Request::SubmitPolicy { input, .. }) => self
                    .core
                    .app
                    .backend_request(&self.core.caller(), &input.request)
                    .is_ok_and(|record| {
                        record.is_none_or(|r| r.state == BackendRequestState::Draft)
                    }),
                _ => false,
            };
            let reply = match result {
                Err(error)
                    if matches!(
                        command.request.as_operation(),
                        Some(
                            Request::Task { .. }
                                | Request::Data { .. }
                                | Request::LocalSoftware { .. }
                                | Request::Settings { .. }
                                | Request::NetworkCheck { .. }
                        )
                    ) =>
                {
                    super::data::failure(error)
                }
                Err(error)
                    if operation_submit
                        && matches!(
                            error,
                            Error::Protocol
                                | Error::Conflict
                                | Error::Expired
                                | Error::Identity
                                | Error::Denied
                                | Error::Unsupported
                        ) =>
                {
                    Reply::OperationRejected {
                        reason: match error {
                            Error::Protocol => {
                                execution_contract::OperationRejection::InvalidParameters
                            }
                            Error::Conflict => execution_contract::OperationRejection::Conflict,
                            Error::Expired => execution_contract::OperationRejection::Expired,
                            Error::Unsupported => {
                                execution_contract::OperationRejection::Unsupported
                            }
                            _ => execution_contract::OperationRejection::Denied,
                        },
                    }
                }
                Ok(reply) => reply,
                Err(Error::Unavailable | Error::Capacity | Error::Expired) => Reply::Unavailable,
                Err(_) => Reply::Rejected,
            };
            let _ = command.reply.send(reply);
        }
        Ok(())
    }
    pub(super) async fn policy_directory(
        &mut self,
        bound: &NativeContext,
        origin: &ClientOrigin,
        after: Option<String>,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<wire::SelfServiceDirectoryResponse, Error> {
        let query = wire::SelfServiceDirectory {
            source: source(origin),
            filter: None,
            sort: None,
            fields: None,
            limit: Some(64),
            cursor: after,
        };
        network(
            self.client
                .self_service_directory(bound.credential().map(|v| v.as_str()), &query),
            &mut self.core,
            commands,
        )
        .await
    }
    async fn resolve_policy(
        &mut self,
        bound: &NativeContext,
        origin: &ClientOrigin,
        selector: wire::CapabilitySelector,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<wire::SelfServiceDirectoryItem, Error> {
        let query = wire::SelfServiceResolve {
            source: source(origin),
            selector,
        };
        let item = network(
            self.client
                .resolve_self_service(bound.credential().map(|v| v.as_str()), &query),
            &mut self.core,
            commands,
        )
        .await?;
        if item.capability.tenant != self.client.configuration().tenant {
            return Err(Error::Untrusted);
        }
        if let wire::SelfServiceSubject::User { user } = &item.subject {
            if Some(user) != bound.user() {
                return Err(Error::Untrusted);
            }
        }
        Ok(item)
    }
    async fn current_policy(
        &mut self,
        bound: &NativeContext,
        origin: &ClientOrigin,
        choice: &execution_contract::PolicyChoice,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<wire::SelfServiceDirectoryItem, Error> {
        self.resolve_policy(
            bound,
            origin,
            wire::CapabilitySelector::Id {
                id: uuid(choice.policy_id.as_str())?,
            },
            commands,
        )
        .await
    }
    async fn retry_task(
        &mut self,
        command: &Command,
        control: &execution_app::tasks::TaskControl,
        origin: &ClientOrigin,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        use execution_app::tasks::*;
        let detail = self.core.task_detail(command, &control.reference)?;
        if control.scope != detail.scope {
            return Err(Error::Denied);
        }
        let TaskRef::Request { request: original } = &control.reference else {
            return Err(Error::Denied);
        };
        let new_request =
            RequestId::new(uuid(control.id.as_str())?.to_string()).map_err(|_| Error::Protocol)?;
        if let Some(previous) = self
            .core
            .app
            .backend_request(&self.core.caller(), &new_request)
            .map_err(map_app_error)?
        {
            if previous.retry_of.as_ref() != Some(original)
                || previous.retry_expected_revision.as_ref() != Some(&control.expected_revision)
            {
                return Err(Error::Conflict);
            }
            return self.core.task_operation(
                command,
                &TaskOperation::ControlStatus(TaskControlQuery {
                    reference: control.reference.clone(),
                    id: control.id.clone(),
                    scope: control.scope.clone(),
                }),
                origin,
            );
        }
        if self
            .core
            .task_control_id_used(command, &control.reference, &control.id)?
        {
            return Err(Error::Conflict);
        }
        if detail.revision != control.expected_revision
            || !detail.allowed_actions.contains(&TaskAction::Retry)
        {
            return Err(Error::Conflict);
        }
        let prior = self
            .core
            .app
            .backend_request(&self.core.caller(), original)
            .map_err(map_app_error)?
            .ok_or(Error::Denied)?;
        let PreparedAction::Policy {
            choice,
            definition,
            policy_revision,
            ..
        } = &prior.action
        else {
            return Err(Error::Unsupported);
        };
        let bound = self.core.context(command)?.clone();
        let current = self
            .current_policy(&bound, origin, choice, commands)
            .await?;
        if current.policy_id.to_string() != definition.id.as_str()
            || current.policy_version.to_string() != definition.revision.as_str()
            || current.policy_revision != *policy_revision
        {
            return Err(Error::Conflict);
        }
        let mut choice = choice.clone();
        choice.request = new_request;
        self.invoke_task_policy(
            command,
            &bound,
            origin,
            &choice,
            false,
            Some((original.clone(), control.expected_revision.clone())),
            commands,
        )
        .await?;
        self.core.task_operation(
            command,
            &TaskOperation::ControlStatus(TaskControlQuery {
                reference: control.reference.clone(),
                id: control.id.clone(),
                scope: control.scope.clone(),
            }),
            origin,
        )
    }
    #[allow(clippy::too_many_arguments)] // The original invocation's actor, immutable choice and retry association are explicit.
    async fn invoke_task_policy(
        &mut self,
        command: &Command,
        bound: &NativeContext,
        origin: &ClientOrigin,
        choice: &execution_contract::PolicyChoice,
        draft: bool,
        retry: Option<(RequestId, execution_app::tasks::TaskRevision)>,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        let retry_of = retry.as_ref().map(|v| v.0.clone());
        if draft && !matches!(origin, ClientOrigin::Ai { .. }) {
            return Err(Error::Denied);
        }
        uuid(choice.request.as_str())?;
        let caller = self.core.caller();
        let bound_trigger = trigger(&self.core, command, origin)?;
        let result = if let Some(previous) = self
            .core
            .app
            .backend_request(&caller, &choice.request)
            .map_err(map_app_error)?
        {
            let PreparedAction::Policy {
                choice: original, ..
            } = &previous.action
            else {
                return Err(Error::Conflict);
            };
            if previous.retry_of != retry_of
                || original != choice
                || previous.trigger != bound_trigger
                || (!draft && previous.state == BackendRequestState::Draft)
            {
                return Err(Error::Conflict);
            }
            self.core.policy_context(&previous)?;
            Reply::PolicyRequest {
                value: Box::new(self.send_policy(previous, bound, commands).await?),
            }
        } else {
            let directory = self.current_policy(bound, origin, choice, commands).await?;
            if !directory.self_service.published
                || (source(origin) == wire::SelfServiceCaller::Ai
                    && !directory.self_service.allow_ai)
            {
                return Err(Error::Denied);
            }
            if matches!(directory.task, wire::SelfServiceTask::Enrollment { .. })
                && self.core.mdm.state != wire::MdmEnrollmentState::Unenrolled
            {
                return Err(Error::Denied);
            }
            if draft {
                operation_parameters_supported(&directory)?;
            }
            if !draft {
                parameters(&directory, &choice.parameters)?;
            } else if choice.parameters.keys().any(|key| {
                let wire::SelfServiceTask::Script {
                    parameter_sources, ..
                } = &directory.task
                else {
                    return true;
                };
                !matches!(
                    parameter_sources.get(key),
                    Some(wire::ScriptParameterSource::Input {})
                )
            }) {
                return Err(Error::Protocol);
            }
            let (access, until) = match &directory.subject {
                wire::SelfServiceSubject::Device {} => {
                    (PolicyAccess::Device {}, bound.expires_at_unix_ms)
                }
                wire::SelfServiceSubject::User { user } if Some(user) == bound.user() => (
                    PolicyAccess::User {
                        session_binding: bound.context.clone(),
                    },
                    bound.expires_at_unix_ms,
                ),
                _ => return Err(Error::Identity),
            };
            let risk = if source(origin) == wire::SelfServiceCaller::Ai {
                Some(ExecutionRisk {
                    level: match directory.self_service.risk_level {
                        1 => RiskLevel::One,
                        2 => RiskLevel::Two,
                        _ => return Err(Error::Denied),
                    },
                    source: RiskSource::Policy {
                        policy: plan::reference(
                            directory.policy_id.to_string(),
                            directory.policy_revision.to_string(),
                        )?,
                    },
                    expires_at_unix_ms: until,
                })
            } else {
                None
            };
            let record = BackendRequest {
                retry_of: retry_of.clone(),
                retry_expected_revision: retry.as_ref().map(|v| v.1.clone()),
                recorded_at_unix_ms: self.core.host.clock.millis()?,
                cancellation_control: None,
                cancellation_expected_revision: None,
                action: PreparedAction::Policy {
                    definition: plan::reference(
                        directory.policy_id.to_string(),
                        directory.policy_version.to_string(),
                    )?,
                    policy_revision: directory.policy_revision,
                    choice: choice.clone(),
                    task: Box::new(directory.task),
                    presentation: directory.self_service.presentation,
                    run_id: None,
                    offer: None,
                    access,
                    expires_at_unix_ms: until,
                },
                trigger: bound_trigger,
                risk: risk.clone(),
                confirmation: None,
                revision: 1,
                state: if draft {
                    BackendRequestState::Draft
                } else if risk.is_some_and(|r| r.level == RiskLevel::Two) {
                    BackendRequestState::AwaitingConfirmation
                } else {
                    BackendRequestState::Ready
                },
                failure: None,
            };
            let current = self.core.context(command)?;
            if current.context != bound.context {
                return Err(Error::Identity);
            }
            self.core
                .app
                .record_backend_request(&caller, None, &record)
                .map_err(map_app_error)?;
            Reply::PolicyRequest {
                value: Box::new(self.send_policy(record, bound, commands).await?),
            }
        };
        Ok(result)
    }
    async fn policy_command(
        &mut self,
        command: &Command,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        let Some(request) = command.request.as_operation() else {
            return Err(Error::Protocol);
        };
        match request {
            Request::RequestRegistration {} => {
                let deployment = self.installation.as_ref().ok_or(Error::Configuration)?;
                self.client
                    .prepare_authorization(deployment.capabilities())?;
                self.next_enrollment = std::time::Instant::now();
                return Ok(Reply::RegistrationStatus {
                    value: crate::registration::project(self.client.enrollment_status()?),
                });
            }
            Request::ImportRegistration { file } => {
                let deployment = self.installation.as_ref().ok_or(Error::Configuration)?;
                self.client.prepare_configuration(
                    file.clone(),
                    crate::installation::installer_format(deployment)?,
                    deployment.capabilities(),
                )?;
                self.next_enrollment = std::time::Instant::now();
                return Ok(Reply::RegistrationStatus {
                    value: crate::registration::project(self.client.enrollment_status()?),
                });
            }
            _ => {}
        }
        if let Request::Task { input, origin } = request {
            if let execution_app::tasks::TaskOperation::Control(control) = input {
                if control.action == execution_app::tasks::TaskAction::Retry {
                    return self.retry_task(command, control, origin, commands).await;
                }
            }
            let remote = matches!(input,execution_app::tasks::TaskOperation::List(q) if q.view == execution_app::tasks::TaskView::Management)
                || matches!(
                    input,
                    execution_app::tasks::TaskOperation::Detail(
                        execution_app::tasks::TaskRef::Activity { .. }
                    )
                );
            if !remote {
                return self.core.task_operation(command, input, origin);
            }
            let scope = self.core.task_scope(command)?;
            // Continuations read the same cached snapshot; a fresh list explicitly advances synchronization.
            if matches!(input,execution_app::tasks::TaskOperation::List(q) if q.cursor.is_some()) {
                return self.core.task_operation(command, input, origin);
            }
            // A failed refresh retains only the same authorized scope's cache and its old coverage.
            self.core.management = self.client.current_management_cache(&scope).ok();
            self.core.management_online = false;
            match network(
                self.client.sync_management(&scope),
                &mut self.core,
                commands,
            )
            .await
            {
                Ok(cache) => {
                    self.core.management = Some(cache);
                    self.core.management_online = true;
                }
                Err(
                    Error::Denied
                    | Error::Identity
                    | Error::CredentialRevoked
                    | Error::CredentialRetired,
                ) => {
                    self.client.clear_management_cache()?;
                    self.core.management = None;
                    return Err(Error::Denied);
                }
                Err(_) => {}
            }
            if let execution_app::tasks::TaskOperation::Detail(
                execution_app::tasks::TaskRef::Activity { id },
            ) = input
            {
                match network(
                    self.client.management_detail(&scope, id),
                    &mut self.core,
                    commands,
                )
                .await
                {
                    Ok(cache) => {
                        self.core.management = Some(cache);
                        self.core.management_online = true;
                    }
                    Err(
                        Error::Denied
                        | Error::Identity
                        | Error::CredentialRevoked
                        | Error::CredentialRetired,
                    ) => {
                        self.core.management = None;
                        self.client.clear_management_cache()?;
                        return Err(Error::Denied);
                    }
                    Err(_) => self.core.management_online = false,
                }
            }
            if command.binding.is_none()
                || self
                    .core
                    .fences
                    .current(&SessionKey::command(command)?)
                    .ok()
                    != Some(command.fence_epoch)
            {
                self.core.management = None;
                self.client.clear_management_cache()?;
                return Err(Error::Denied);
            }
            return self.core.task_operation(command, input, origin);
        }
        if let Request::DeviceAccess {
            context,
            origin,
            input,
        } = request
        {
            return self
                .access_command(command, context, origin, input, commands)
                .await;
        }
        if let Request::NetworkCheck {
            context,
            origin,
            input,
        } = request
        {
            self.core.data_context(command, context)?;
            if !matches!(origin, ClientOrigin::Desktop {}) {
                return Err(Error::Denied);
            }
            if matches!(input, execution_contract::NetworkCheckOperation::Defaults) {
                let target = self
                    .client
                    .connection_status()
                    .ok()
                    .and_then(|s| url::Url::parse(&s.origin).ok())
                    .and_then(|u| {
                        Some(execution_contract::NetworkTarget {
                            host: u.host_str()?.trim_matches(['[', ']']).into(),
                            port: u.port_or_known_default()?,
                        })
                    });
                return Ok(Reply::NetworkDefaults { target });
            }
            return self.core.network_operation(command, context, origin, input);
        }
        if let Request::Settings {
            context,
            origin,
            input,
        } = request
        {
            return self
                .core
                .settings_operation(command, context, origin, input);
        }
        if let Request::LocalSoftware {
            context,
            origin,
            input,
        } = request
        {
            return self
                .core
                .software_operation(command, context, origin, input);
        }
        if let Request::Data {
            context,
            origin,
            input,
        } = request
        {
            return self
                .data_command(command, context, origin, input, commands)
                .await;
        }
        self.policy_operation(command, request, commands).await
    }
    pub(super) async fn policy_operation(
        &mut self,
        command: &Command,
        request: &Request,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        if let Request::CurrentUserStatus {} = request {
            return Ok(Reply::CurrentUserSync {
                value: self.client.current_user_status()?,
            });
        }
        if let Request::RetireEnterpriseContext {
            generation,
            proof,
            recover,
        } = request
        {
            let key = peer_key(command)?;
            self.client.clear_management_cache()?;
            self.core.management = None;
            // Retiring a renderer context and settling its relation are different facts.
            self.core.fences.revoke(&SessionKey::command(command)?)?;
            let stored = self.client.current_user_owner()?;
            if let Some(owner) = stored {
                let own_generation = generation
                    .as_ref()
                    .is_some_and(|g| *g == owner.context.generation);
                if !*recover && (owner.peer != key || !own_generation) {
                    return Ok(Reply::CurrentUserSync {
                        value: Default::default(),
                    });
                }
                if *recover
                    && self.core.contexts.values().any(|c| {
                        c.context == owner.context
                            && c.expires_at_unix_ms > self.core.host.clock.millis().unwrap_or(0)
                            && self
                                .core
                                .fences
                                .current(&c.scope)
                                .is_ok_and(|e| e == c.epoch)
                    })
                {
                    return Err(Error::Conflict);
                }
                let retained = self
                    .core
                    .contexts
                    .get(&key)
                    .and_then(|c| c.credential())
                    .cloned();
                let proof = proof.as_ref().or(retained.as_ref());
                let result = network(
                    self.client
                        .retire_current_user(&owner, proof.map(|v| v.as_str()), *recover),
                    &mut self.core,
                    commands,
                )
                .await;
                if result.is_err() {
                    return Ok(Reply::CurrentUserSync {
                        value: self.client.current_user_status()?,
                    });
                }
            }
            return Ok(Reply::CurrentUserSync {
                value: self.client.current_user_status()?,
            });
        }
        if let Request::BindContext { value, expected } = request {
            if value.expires_at_unix_ms > execution_contract::MAX_SAFE_INTEGER
                || value.user().is_some_and(|user| {
                    user.tenant_id != self.client.configuration().tenant
                        || user.instance_id != self.client.configuration().service_id
                        || user.principal_id.is_nil()
                })
                || value.credential().is_some_and(|secret| {
                    secret.len() != 64
                        || !secret
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
                || value.expires_at_unix_ms <= self.core.host.clock.millis()?
            {
                return Err(Error::Identity);
            }
            // A device-only OS binding permits cached local reads while offline.
            // Live Policy access and all submissions retain their independent identity gate.
            if (self.core.host.revoked.load(Ordering::Acquire) && value.user().is_some())
                || self.core.stopping
            {
                return Err(Error::Denied);
            }
            let key = peer_key(command)?;
            if self.core.contexts.len() >= 32 && !self.core.contexts.contains_key(&key) {
                return Err(Error::Capacity);
            }
            let scope = SessionKey::command(command)?;
            let epoch = self.core.fences.current(&scope)?;
            if epoch != command.fence_epoch {
                return Err(Error::Identity);
            }
            let old = self.core.contexts.get(&key);
            let context = match (expected, old) {
                (Some(id), Some(old))
                    if &old.context.id == id
                        && old.epoch == epoch
                        && old.expires_at_unix_ms > self.core.host.clock.millis()?
                        && old.generation == value.generation
                        && old.user() == value.user() =>
                {
                    old.context.clone()
                }
                (None, Some(old))
                    if old.epoch != epoch
                        && old.generation == value.generation
                        && old.user() == value.user() =>
                {
                    old.context.clone()
                }
                (None, Some(old)) if old.epoch != epoch && old.generation != value.generation => {
                    execution_contract::NativeContextRef {
                        id: value.generation.clone(),
                        generation: value.generation.clone(),
                    }
                }
                (None, Some(old))
                    if old.epoch == epoch
                        && old.generation == value.generation
                        && old.user() == value.user() =>
                {
                    old.context.clone()
                }
                (None, None) => execution_contract::NativeContextRef {
                    id: value.generation.clone(),
                    generation: value.generation.clone(),
                },
                _ => return Err(Error::Identity),
            };
            let lease = value.credential().map(|secret| Arc::new(secret.clone()));
            if let Some(secret) = &lease {
                self.bridge.protect_user_session(secret)?;
                self.core.data.protect(secret)?;
            }
            // The control lane may retire the peer while credential preparation is in progress.
            let _fence = self.core.fences.admit(&scope, epoch)?;
            // Lease refreshes from older peers must not replace a later explicit login.
            if expected.is_none() && old.is_none_or(|old| old.generation != value.generation) {
                if value.user().is_some() {
                    self.core.current_user_context = Some(agent_client::current_user::Owner {
                        peer: key.clone(),
                        context: context.clone(),
                    });
                } else if self
                    .core
                    .current_user_context
                    .as_ref()
                    .is_some_and(|o| o.peer == key)
                {
                    self.core.current_user_context = None;
                }
            }
            self.client.clear_management_cache()?;
            self.core.management = None;
            self.core.contexts.insert(
                key,
                NativeContext {
                    session: value.clone(),
                    scope,
                    epoch,
                    lease,
                    context: context.clone(),
                },
            );
            return Ok(Reply::ContextBound { value: context });
        }
        if let Request::ObserveSoftware { choice, origin } = request {
            return self.observe_software(command, choice, origin);
        }
        let bound = self.core.context(command)?.clone();
        let expected = match request {
            Request::PolicyDirectory { context, .. }
            | Request::ResolvePolicy { context, .. }
            | Request::InvokePolicy { context, .. }
            | Request::PreparePolicy { context, .. }
            | Request::SubmitPolicy { context, .. }
            | Request::ConfirmPolicy { context, .. }
            | Request::PolicyStatus { context, .. } => context,
            _ => return Err(Error::Protocol),
        };
        if expected != &bound.context {
            return Err(Error::Identity);
        }
        let result = match request {
            Request::ResolvePolicy {
                selector, origin, ..
            } => {
                self.core.require_origin_context(
                    &command.subject,
                    command.binding.as_ref().ok_or(Error::Denied)?,
                    origin,
                )?;
                Reply::PolicyResolved {
                    value: Box::new(
                        self.resolve_policy(&bound, origin, selector.clone(), commands)
                            .await?,
                    ),
                }
            }
            Request::PolicyDirectory { after, origin, .. } => {
                self.core.require_origin_context(
                    &command.subject,
                    command.binding.as_ref().ok_or(Error::Denied)?,
                    origin,
                )?;
                Reply::PolicyDirectory {
                    value: self
                        .policy_directory(&bound, origin, after.clone(), commands)
                        .await?,
                }
            }
            Request::InvokePolicy { choice, origin, .. }
            | Request::PreparePolicy { choice, origin, .. } => {
                self.invoke_task_policy(
                    command,
                    &bound,
                    origin,
                    choice,
                    matches!(request, Request::PreparePolicy { .. }),
                    None,
                    commands,
                )
                .await?
            }
            Request::SubmitPolicy { input, .. } => {
                let previous = self
                    .core
                    .app
                    .backend_request(&self.core.caller(), &input.request)
                    .map_err(map_app_error)?
                    .ok_or(Error::Denied)?;
                if trigger_subject(&previous.trigger) != Some(command.subject.as_str())
                    || previous.trigger_os_session() != command.binding.as_ref()
                {
                    return Err(Error::Denied);
                }
                self.core.current_policy_access(&previous)?;
                let PreparedAction::Policy { choice, .. } = &previous.action else {
                    return Err(Error::Denied);
                };
                if previous.state != BackendRequestState::Draft {
                    if choice.parameters != input.parameters {
                        return Ok(Reply::OperationRejected {
                            reason: execution_contract::OperationRejection::Conflict,
                        });
                    }
                    Reply::PolicyRequest {
                        value: Box::new(previous),
                    }
                } else {
                    if previous.revision != input.revision {
                        return Err(Error::Conflict);
                    }
                    let origin = match &previous.trigger {
                        BackendTrigger::Ai {
                            config,
                            conversation,
                            tool_call,
                            context,
                            ..
                        } => ClientOrigin::Ai {
                            config: config.clone(),
                            conversation: conversation.clone(),
                            tool_call: tool_call.clone(),
                            context: context.clone(),
                        },
                        _ => return Err(Error::Denied),
                    };
                    let ClientOrigin::Ai {
                        context: original_context,
                        ..
                    } = &origin
                    else {
                        return Err(Error::Denied);
                    };
                    if original_context != &bound.context {
                        return Err(Error::Identity);
                    }
                    self.core.require_origin_context(
                        &command.subject,
                        &bound.scope.binding,
                        &origin,
                    )?;
                    let current = self
                        .current_policy(&bound, &origin, choice, commands)
                        .await?;
                    operation_parameters_supported(&current)?;
                    parameters(&current, &input.parameters)?;
                    // Network waits can revoke the context; recheck before the atomic freeze.
                    self.core.context(command)?;
                    self.core.require_origin_context(
                        &command.subject,
                        &bound.scope.binding,
                        &origin,
                    )?;
                    self.core.current_policy_access(&previous)?;
                    let mut next = previous.clone();
                    next.revision += 1;
                    next.state = BackendRequestState::Ready;
                    if let PreparedAction::Policy {
                        choice,
                        task,
                        presentation,
                        ..
                    } = &mut next.action
                    {
                        choice.parameters = input.parameters.clone();
                        **task = current.task.clone();
                        *presentation = current.self_service.presentation.clone();
                    }
                    if let Some(risk) = &mut next.risk {
                        risk.level = match current.self_service.risk_level {
                            1 => RiskLevel::One,
                            2 => RiskLevel::Two,
                            _ => return Err(Error::Denied),
                        };
                        risk.source = RiskSource::Policy {
                            policy: plan::reference(
                                current.policy_id.to_string(),
                                current.policy_revision.to_string(),
                            )?,
                        };
                    }
                    if next
                        .risk
                        .as_ref()
                        .is_some_and(|risk| risk.level == RiskLevel::Two)
                    {
                        let BackendTrigger::Ai { os_session, .. } = &next.trigger else {
                            return Err(Error::Denied);
                        };
                        let now = self.core.host.clock.millis()?;
                        next.confirmation = Some(execution_contract::BackendConfirmation {
                            os_session: os_session.clone(),
                            confirmed_at_unix_ms: now,
                            expires_at_unix_ms: next
                                .action
                                .expires_at_unix_ms()
                                .min(now.saturating_add(60_000)),
                        });
                    }
                    {
                        let _fence = self.core.fences.admit(&bound.scope, bound.epoch)?;
                        self.core
                            .app
                            .record_backend_request(&self.core.caller(), Some(&previous), &next)
                            .map_err(map_app_error)?;
                    }
                    Reply::PolicyRequest {
                        value: Box::new(self.send_policy(next, &bound, commands).await?),
                    }
                }
            }
            Request::ConfirmPolicy { request, .. } => {
                let previous = self
                    .core
                    .app
                    .backend_request(&self.core.caller(), request)
                    .map_err(map_app_error)?
                    .ok_or(Error::Denied)?;
                if trigger_subject(&previous.trigger) != Some(command.subject.as_str())
                    || previous.trigger_os_session() != command.binding.as_ref()
                {
                    return Err(Error::Denied);
                }
                if let Err(error) = self.core.current_policy_access(&previous) {
                    if error != Error::Expired {
                        return Err(error);
                    }
                    return Ok(Reply::PolicyRequest {
                        value: Box::new(self.expire_policy(&previous)?),
                    });
                }
                if previous.state != BackendRequestState::AwaitingConfirmation {
                    return Ok(Reply::PolicyRequest {
                        value: Box::new(self.send_policy(previous, &bound, commands).await?),
                    });
                }
                let origin = policy_origin(&previous)?;
                let PreparedAction::Policy { choice, .. } = &previous.action else {
                    return Err(Error::Denied);
                };
                let current = self
                    .current_policy(&bound, &origin, choice, commands)
                    .await?;
                if let PreparedAction::Policy {
                    definition,
                    policy_revision,
                    ..
                } = &previous.action
                {
                    if current.policy_id.to_string() != definition.id.as_str()
                        || current.policy_version.to_string() != definition.revision.as_str()
                        || current.policy_revision != *policy_revision
                    {
                        return Ok(Reply::OperationRejected {
                            reason: execution_contract::OperationRejection::Conflict,
                        });
                    }
                }
                parameters(&current, &choice.parameters)?;
                self.core.context(command)?;
                self.core.current_policy_access(&previous)?;
                let BackendTrigger::Ai { os_session, .. } = &previous.trigger else {
                    return Err(Error::Denied);
                };
                if previous
                    .risk
                    .as_ref()
                    .is_none_or(|r| r.level != RiskLevel::Two)
                {
                    return Err(Error::Denied);
                }
                let now = self.core.host.clock.millis()?;
                let mut next = previous.clone();
                next.revision += 1;
                next.state = BackendRequestState::Ready;
                if let PreparedAction::Policy {
                    task, presentation, ..
                } = &mut next.action
                {
                    **task = current.task;
                    *presentation = current.self_service.presentation;
                }
                if let Some(risk) = &mut next.risk {
                    risk.level = match current.self_service.risk_level {
                        1 => RiskLevel::One,
                        2 => RiskLevel::Two,
                        _ => return Err(Error::Denied),
                    };
                    risk.source = RiskSource::Policy {
                        policy: plan::reference(
                            current.policy_id.to_string(),
                            current.policy_revision.to_string(),
                        )?,
                    };
                }
                next.confirmation = Some(execution_contract::BackendConfirmation {
                    os_session: os_session.clone(),
                    confirmed_at_unix_ms: now,
                    expires_at_unix_ms: next
                        .action
                        .expires_at_unix_ms()
                        .min(now.saturating_add(60_000)),
                });
                let _fence = self.core.fences.admit(&bound.scope, bound.epoch)?;
                self.core
                    .app
                    .record_backend_request(&self.core.caller(), Some(&previous), &next)
                    .map_err(map_app_error)?;
                drop(_fence);
                Reply::PolicyRequest {
                    value: Box::new(self.send_policy(next, &bound, commands).await?),
                }
            }
            Request::PolicyStatus {
                request, origin, ..
            } => {
                let Some(previous) = self
                    .core
                    .app
                    .backend_request(&self.core.caller(), request)
                    .map_err(map_app_error)?
                else {
                    return Ok(Reply::PolicyAbsent);
                };
                self.core.require_origin_context(
                    &command.subject,
                    command.binding.as_ref().ok_or(Error::Denied)?,
                    origin,
                )?;
                if matches!(origin, ClientOrigin::Ai { .. })
                    && !matches!(previous.trigger, BackendTrigger::Ai { .. })
                {
                    return Err(Error::Denied);
                }
                let PreparedAction::Policy { access, .. } = &previous.action else {
                    return Err(Error::Denied);
                };
                if trigger_subject(&previous.trigger) != Some(command.subject.as_str())
                    || previous.trigger_os_session() != command.binding.as_ref()
                {
                    return Err(Error::Denied);
                }
                self.core.policy_context(&previous)?;
                let secret = match access {
                    PolicyAccess::Device {} => None,
                    PolicyAccess::User { .. } => bound.credential().map(|secret| secret.as_str()),
                };
                if matches!(
                    previous.state,
                    BackendRequestState::AwaitingConfirmation | BackendRequestState::Ready
                ) {
                    Reply::PolicyRequest {
                        value: Box::new(self.send_policy(previous, &bound, commands).await?),
                    }
                } else if previous.state == BackendRequestState::Draft
                    || previous.state == BackendRequestState::Cancelled
                    || (previous.state == BackendRequestState::Failed
                        && previous.failure != Some(BackendRequestFailure::Expired))
                {
                    Reply::PolicyRequest {
                        value: Box::new(previous),
                    }
                } else {
                    match network(
                        self.client
                            .self_service_status(secret, uuid(request.as_str())?),
                        &mut self.core,
                        commands,
                    )
                    .await
                    {
                        Ok(value) => Reply::PolicyStatus {
                            value: self.bridge.protect_status(value)?,
                        },
                        Err(Error::NotFound) => Reply::PolicyRequest {
                            value: Box::new(self.send_policy(previous, &bound, commands).await?),
                        },
                        Err(error) => return Err(error),
                    }
                }
            }
            _ => return Err(Error::Protocol),
        };
        let current = self.core.context(command)?;
        if current.context != bound.context {
            return Err(Error::Identity);
        }
        Ok(result)
    }
    fn expire_policy(&mut self, record: &BackendRequest) -> Result<BackendRequest, Error> {
        self.core
            .transition(
                record.action.request(),
                BackendRequestState::Failed,
                Some(BackendRequestFailure::Expired),
            )?
            .ok_or(Error::Conflict)
    }
    async fn send_policy(
        &mut self,
        record: BackendRequest,
        bound: &NativeContext,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<BackendRequest, Error> {
        if !matches!(
            record.state,
            BackendRequestState::AwaitingConfirmation
                | BackendRequestState::Ready
                | BackendRequestState::Submitting
        ) {
            return Ok(record);
        }
        match self.core.current_policy_access(&record) {
            Err(Error::Expired) => return self.expire_policy(&record),
            result => result?,
        }
        if record.state == BackendRequestState::AwaitingConfirmation {
            return Ok(record);
        }
        let PreparedAction::Policy {
            choice,
            task,
            access,
            run_id,
            ..
        } = &record.action
        else {
            return Err(Error::Protocol);
        };
        if run_id.is_some() {
            return Ok(record);
        }
        if matches!(task.as_ref(), wire::SelfServiceTask::Enrollment { .. })
            && self.core.mdm.state != wire::MdmEnrollmentState::Unenrolled
        {
            return Err(Error::Denied);
        }
        let secret = match access {
            PolicyAccess::Device {} => None,
            PolicyAccess::User { .. } => Some(bound.credential().ok_or(Error::Identity)?.as_str()),
        };
        if record.state == BackendRequestState::Submitting {
            match network(
                self.client
                    .self_service_status(secret, uuid(choice.request.as_str())?),
                &mut self.core,
                commands,
            )
            .await
            {
                Ok(status) => return self.accept_policy(&record, status.run_id),
                Err(Error::NotFound) => (),
                Err(Error::Unavailable) => return Ok(record),
                Err(error) => return Err(error),
            }
            // Only an authoritative absence allows resending this exact frozen invocation.
            self.core.current_policy_access(&record)?;
        }
        let input = wire::SelfServiceInvoke {
            operation_id: uuid(choice.request.as_str())?,
            policy_id: uuid(choice.policy_id.as_str())?,
            source: match record.trigger {
                BackendTrigger::Ai { .. } => wire::SelfServiceCaller::Ai,
                _ => wire::SelfServiceCaller::Human,
            },
            confirmed: record.confirmation.is_some(),
            parameters: choice.parameters.clone(),
        };
        let record = if record.state == BackendRequestState::Ready {
            self.core
                .transition(
                    record.action.request(),
                    BackendRequestState::Submitting,
                    None,
                )?
                .ok_or(Error::Conflict)?
        } else {
            record
        };
        let response = network(
            self.client.invoke_self_service(secret, &input),
            &mut self.core,
            commands,
        )
        .await;
        match response {
            Ok(accepted) => self.accept_policy(&record, accepted.run_id),
            Err(Error::Unavailable) => Ok(record),
            Err(error) => {
                self.core.transition(
                    record.action.request(),
                    BackendRequestState::Failed,
                    Some(BackendRequestFailure::Revoked),
                )?;
                Err(error)
            }
        }
    }
    fn accept_policy(
        &mut self,
        record: &BackendRequest,
        accepted_run: Uuid,
    ) -> Result<BackendRequest, Error> {
        let current = self
            .core
            .app
            .backend_request(&self.core.caller(), record.action.request())
            .map_err(map_app_error)?
            .ok_or(Error::Conflict)?;
        if &current != record {
            return Err(Error::Conflict);
        }
        let mut next = current.clone();
        next.revision += 1;
        let PreparedAction::Policy { run_id, .. } = &mut next.action else {
            return Err(Error::Protocol);
        };
        *run_id = Some(plan::id(accepted_run.to_string())?);
        self.core
            .app
            .record_backend_request(&self.core.caller(), Some(&current), &next)
            .map_err(map_app_error)?;
        Ok(next)
    }
    pub(super) fn bind_policy_offer(&mut self, offer: &Offer) -> Result<Selection, Error> {
        let authorization = match offer.payload() {
            wire::TaskPayload::Script(spec) => &spec.authorization,
            wire::TaskPayload::Software(spec) => &spec.authorization,
            wire::TaskPayload::Enrollment(spec) => &spec.authorization,
            _ => return Err(Error::Denied),
        };
        let wire::TaskAuthorization::SelfService {
            request_id,
            source,
            policy_id,
            ..
        } = authorization
        else {
            return Err(Error::Denied);
        };
        let request = backend::request_id(offer)?;
        let previous = self
            .core
            .app
            .backend_request(&self.core.caller(), &request)
            .map_err(map_app_error)?
            .ok_or(Error::Denied)?;
        let PreparedAction::Policy { choice, run_id, .. } = &previous.action else {
            return Err(Error::Denied);
        };
        let expected_source = if matches!(previous.trigger, BackendTrigger::Ai { .. }) {
            wire::SelfServiceCaller::Ai
        } else {
            wire::SelfServiceCaller::Human
        };
        if previous.state != BackendRequestState::Submitting
            || request_id.to_string() != choice.request.as_str()
            || *source != expected_source
            || policy_id.to_string() != choice.policy_id.as_str()
        {
            return Err(Error::Untrusted);
        }
        if let wire::TaskPayload::Script(spec) = offer.payload() {
            if choice
                .parameters
                .iter()
                .any(|(key, value)| spec.parameters.get(key) != Some(value))
            {
                return Err(Error::Untrusted);
            }
        } else if !choice.parameters.is_empty() {
            return Err(Error::Untrusted);
        }
        let task_id = offer.task_id();
        if run_id
            .as_ref()
            .is_some_and(|id| id.as_str() != task_id.to_string())
        {
            return Err(Error::Untrusted);
        }
        let current = offered(offer)?;
        if previous.action.offer() == Some(&current) {
            return Selection::from_record(&previous);
        }
        let mut next = previous.clone();
        next.revision += 1;
        if let PreparedAction::Policy {
            offer: bound,
            run_id,
            ..
        } = &mut next.action
        {
            if bound.as_deref().is_some_and(|old| old != &current) {
                return Err(Error::Conflict);
            }
            *bound = Some(Box::new(current));
            *run_id = Some(plan::id(task_id.to_string())?);
        }
        self.core
            .app
            .record_backend_request(&self.core.caller(), Some(&previous), &next)
            .map_err(map_app_error)?;
        Selection::from_record(&next)
    }
}

impl<S: IdentityProvider> DeviceService<S> {
    fn observe_software(
        &mut self,
        command: &Command,
        choice: &execution_contract::SoftwareReadChoice,
        origin: &ClientOrigin,
    ) -> Result<Reply, Error> {
        use execution_contract::*;
        uuid(choice.request.as_str())?;
        if self.core.stopping || self.core.host.revoked.load(Ordering::Acquire) {
            return Err(Error::Denied);
        }
        let original_trigger = trigger(&self.core, command, origin)?;
        if let Some(previous) = self
            .core
            .app
            .backend_request(&self.core.caller(), &choice.request)
            .map_err(map_app_error)?
        {
            let PreparedAction::SoftwareRead {
                choice: original, ..
            } = &previous.action
            else {
                return Err(Error::Conflict);
            };
            if original != choice || previous.trigger != original_trigger {
                return Err(Error::Conflict);
            }
            return Ok(Reply::PolicyRequest {
                value: Box::new(previous),
            });
        }
        let (offer, _) = self.waiting.as_ref().ok_or(Error::Denied)?;
        let view = offered(offer)?;
        let selected = &choice.input.selection;
        if view.request != selected.request
            || view.task != selected.task
            || view.attempt != selected.attempt
            || view.revision != selected.revision
        {
            return Err(Error::Denied);
        }
        let now = self.core.host.clock.millis()?;
        let until = u64::try_from(view.expires_at)
            .map_err(|_| Error::Expired)?
            .saturating_mul(1000);
        if now >= until {
            return Err(Error::Expired);
        }
        let wire::TaskPayload::Software(spec) = offer.payload() else {
            return Err(Error::Denied);
        };
        let step = spec
            .steps
            .get(choice.input.step as usize)
            .ok_or(Error::Denied)?;
        let commands = execution_contract::software_commands(&step.action);
        let run_as = match commands.install.run_as {
            wire::ExecutionIdentity::System => RunAs::System {
                platform: plan::platform()?,
            },
            wire::ExecutionIdentity::LoggedInUser => RunAs::User {
                account: OsAccountRef {
                    platform: plan::platform()?,
                    subject: plan::id(&command.subject)?,
                },
            },
        };
        let session_requirement = match &run_as {
            RunAs::System { .. } => SessionRequirement::NotRequired {},
            RunAs::User { account } => SessionRequirement::ActiveUser {
                account: account.clone(),
                session: command.binding.clone().ok_or(Error::Identity)?,
            },
        };
        let detector = match commands.detection {
            Some(wire::SoftwareTaskDetection::PkgReceipt { receipt, version }) => {
                SoftwareDetector::PkgReceipt {
                    receipt: receipt.clone(),
                    run_as: run_as.clone(),
                    session_requirement: session_requirement.clone(),
                    version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
                }
            }
            Some(wire::SoftwareTaskDetection::MsiProduct {
                product_code,
                version,
            }) => SoftwareDetector::MsiProduct {
                product_code: product_code.clone(),
                run_as: run_as.clone(),
                session_requirement: session_requirement.clone(),
                version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
            },
            _ => return Err(Error::Unsupported),
        };
        let native_selection = Selection {
            request: selected.request.clone(),
            task: selected.task.clone(),
            attempt: selected.attempt.clone(),
            revision: selected.revision.clone(),
            origin: origin.clone(),
            subject: command.subject.clone(),
            session: command.session,
            binding: command.binding.clone().ok_or(Error::Denied)?,
        };
        let login = self.core.helpers.connect(Some(&native_selection))?;
        if let wire::SoftwareExecutionTarget::User {
            identity,
            session_id,
        } = &step.target
        {
            if identity != &command.subject || *session_id != login_id(login.context()) {
                return Err(Error::Denied);
            }
        }
        let definition = execution_contract::Digest::new(plan::hex(&spec.definition_digest))
            .map_err(|_| Error::Protocol)?;
        let risk = matches!(origin, ClientOrigin::Ai { .. }).then(|| ExecutionRisk {
            level: RiskLevel::One,
            source: RiskSource::Software {
                definition: definition.clone(),
            },
            expires_at_unix_ms: until,
        });
        let record = BackendRequest {
            retry_of: None,
            retry_expected_revision: None,
            recorded_at_unix_ms: self.core.host.clock.millis()?,
            cancellation_control: None,
            cancellation_expected_revision: None,
            action: PreparedAction::SoftwareRead {
                choice: choice.clone(),
                definition,
                expires_at_unix_ms: until,
                observation: None,
            },
            trigger: original_trigger,
            risk,
            confirmation: None,
            revision: 1,
            state: BackendRequestState::Ready,
            failure: None,
        };
        let observed = self.core.record_software_observation(
            record,
            || {
                execution_runner::software::observe_native(&detector, &run_as)
                    .map_err(crate::error::app_error)
            },
            step.action.package.clone(),
            step.action.version.clone(),
        )?;
        Ok(Reply::PolicyRequest {
            value: Box::new(observed),
        })
    }
}

impl<S: IdentityProvider> DeviceService<S> {
    pub(super) async fn synchronize_current_user(
        &mut self,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Error> {
        let now = self.core.host.clock.millis()?;
        let live = |bound: &NativeContext| {
            now < bound.expires_at_unix_ms
                && self
                    .core
                    .fences
                    .current(&bound.scope)
                    .is_ok_and(|epoch| epoch == bound.epoch)
        };
        let owner = self.client.current_user_owner()?;
        let phase = self.client.current_user_status()?.phase;
        // Finish the original uncertain PUT with its exact proof; a later login cannot
        // rewrite that operation. Confirmed presence follows the latest explicit login.
        let selected = if phase == agent_client::current_user::Phase::BindingUnknown {
            owner.as_ref()
        } else {
            self.core.current_user_context.as_ref()
        };
        let desired = selected.and_then(|selected| {
            self.core
                .contexts
                .get(&selected.peer)
                .filter(|bound| {
                    live(bound) && bound.context == selected.context && bound.user().is_some()
                })
                .map(|bound| (selected.clone(), bound.clone()))
        });
        let result = if phase == agent_client::current_user::Phase::CleanupPending {
            let Some(owner) = owner else {
                return Ok(());
            };
            background(
                self.client.retire_current_user(&owner, None, false),
                &mut self.core,
                commands,
            )
            .await
            .map(|_| ())
        } else if let Some((selected, bound)) = desired {
            if matches!(
                phase,
                agent_client::current_user::Phase::Conflict
                    | agent_client::current_user::Phase::Invalidated
            ) || (phase == agent_client::current_user::Phase::Bound
                && owner.as_ref() == Some(&selected))
            {
                return Ok(());
            }
            let (Some(user), Some(proof)) = (bound.user(), bound.credential()) else {
                return Ok(());
            };
            background(
                self.client.bind_current_user(&selected, user, proof),
                &mut self.core,
                commands,
            )
            .await
            .map(|_| ())
        } else if let Some(owner) = owner {
            if !matches!(
                phase,
                agent_client::current_user::Phase::Bound
                    | agent_client::current_user::Phase::CleanupPending
            ) {
                return Ok(());
            }
            background(
                self.client.retire_current_user(&owner, None, false),
                &mut self.core,
                commands,
            )
            .await
            .map(|_| ())
        } else {
            return Ok(());
        };
        if let Err(error) = result {
            // A presence projection grants no execution authority and never closes local login.
            execution_ipc::record_background_failure(crate::deployment::startup_failure(error));
        }
        Ok(())
    }
    pub(super) fn prune_credentials(&mut self) -> Result<(), Error> {
        let now = self.core.host.clock.millis()?;
        let fences = &self.core.fences;
        for bound in self.core.contexts.values_mut() {
            if now >= bound.expires_at_unix_ms
                || !fences
                    .current(&bound.scope)
                    .is_ok_and(|epoch| epoch == bound.epoch)
            {
                bound.lease = None;
                bound.session.clear_credential();
            }
        }
        // Communication retains every unsettled task, including ambiguous native outcomes.
        // Drop a retired lease only after that owner has drained its result and task state.
        let pending = self
            .client
            .pending_tasks(self.client.configuration().limits.pending_tasks)?;
        let caller = self.core.caller();
        let app = &self.core.app;
        self.core.credential_leases.retain(|request, _| {
            app.backend_request(&caller, request)
                .ok()
                .flatten()
                .and_then(|record| record.action.offer().map(|offer| offer.task.clone()))
                .is_none_or(|task| pending.iter().any(|id| id.to_string() == task.as_str()))
        });
        Ok(())
    }
}

impl Core {
    pub(super) fn record_software_observation(
        &mut self,
        record: BackendRequest,
        collect: impl FnOnce() -> Result<execution_contract::SoftwareState, Error>,
        package: String,
        version: String,
    ) -> Result<BackendRequest, Error> {
        use execution_contract::*;
        let caller = self.caller();
        self.app
            .record_backend_request(&caller, None, &record)
            .map_err(map_app_error)?;
        let mut submitting = record.clone();
        submitting.revision += 1;
        submitting.state = BackendRequestState::Submitting;
        self.app
            .record_backend_request(&caller, Some(&record), &submitting)
            .map_err(map_app_error)?;
        let state = collect()?;
        let mut observed = submitting.clone();
        observed.revision += 1;
        observed.state = BackendRequestState::Observed;
        if let PreparedAction::SoftwareRead { observation, .. } = &mut observed.action {
            *observation = Some(SoftwareObservation {
                quality: if matches!(state, SoftwareState::Unknown { .. }) {
                    OutputQuality::Failed
                } else {
                    OutputQuality::Complete
                },
                collected_at_unix_ms: self.host.clock.millis()?,
                package,
                expected_version: version,
                state,
            });
        } else {
            return Err(Error::Protocol);
        }
        self.app
            .record_backend_request(&caller, Some(&submitting), &observed)
            .map_err(map_app_error)?;
        Ok(observed)
    }
}

fn policy_origin(record: &BackendRequest) -> Result<ClientOrigin, Error> {
    match &record.trigger {
        BackendTrigger::Human { .. } => Ok(ClientOrigin::Desktop {}),
        BackendTrigger::Ai {
            config,
            conversation,
            tool_call,
            context,
            ..
        } => Ok(ClientOrigin::Ai {
            config: config.clone(),
            conversation: conversation.clone(),
            tool_call: tool_call.clone(),
            context: context.clone(),
        }),
        BackendTrigger::Automatic {} => Err(Error::Denied),
    }
}
