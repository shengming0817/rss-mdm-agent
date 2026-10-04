//! Policy self-service is a projection of backend authority, driven by the sole device owner.
use super::*;
use execution_contract::{ExecutionRisk, Id, PreparedAction, RiskLevel, RiskSource};
use execution_ipc::host::{ClientOrigin, EmployeeSession, Reply, Request};
use std::collections::{BTreeMap, VecDeque};
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct Fences(std::sync::Mutex<BTreeMap<String, u64>>);
impl Fences {
    pub(super) fn current(&self, subject: &str) -> Result<u64, Error> {
        Ok(*self
            .0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(subject)
            .unwrap_or(&0))
    }
    pub(crate) fn admit(
        &self,
        subject: &str,
        expected: u64,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, u64>>, Error> {
        let epochs = self.0.lock().map_err(|_| Error::Unavailable)?;
        if *epochs.get(subject).unwrap_or(&0) != expected {
            return Err(Error::Identity);
        }
        Ok(epochs)
    }
    pub(super) fn revoke(&self, subject: &str) -> Result<(), Error> {
        let mut epochs = self.0.lock().map_err(|_| Error::Unavailable)?;
        let epoch = epochs.entry(subject.to_owned()).or_default();
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
            peer.session_binding()?;
            self.fences
                .revoke(&subject)
                .map_err(|_| execution_app::Error::Unavailable)
        }) {
            Ok(()) => Reply::EmployeeBound,
            Err(_) => Reply::Rejected,
        }
    }
}
#[derive(Clone)]
pub(super) struct Employee {
    session: EmployeeSession,
    subject: String,
    epoch: u64,
    lease: Arc<zeroize::Zeroizing<String>>,
}
impl std::ops::Deref for Employee {
    type Target = EmployeeSession;
    fn deref(&self) -> &Self::Target {
        &self.session
    }
}
pub(super) type Employees = BTreeMap<String, Employee>;
pub(super) type Commands = VecDeque<Command>;
fn peer_key(command: &Command) -> Result<String, Error> {
    Ok(format!(
        "{}:{}:{}",
        command.subject,
        command.session,
        command.binding.as_ref().ok_or(Error::Denied)?.as_str()
    ))
}
fn source(origin: &ClientOrigin) -> wire::ScriptCaller {
    match origin {
        ClientOrigin::Desktop {} => wire::ScriptCaller::Human,
        ClientOrigin::Ai { .. } => wire::ScriptCaller::Ai,
    }
}
fn trigger(core: &Core, command: &Command, origin: &ClientOrigin) -> Result<BackendTrigger, Error> {
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
        } => BackendTrigger::Ai {
            os_session,
            config: config.clone(),
            conversation: conversation.clone(),
            tool_call: tool_call.clone(),
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
fn parameters(
    directory: &wire::SelfServiceDirectoryItem,
    inputs: &BTreeMap<String, serde_json::Value>,
) -> Result<BTreeMap<String, serde_json::Value>, Error> {
    let mut resolved = BTreeMap::new();
    let mut expected = 0;
    for (key, source) in &directory.parameter_sources {
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
    let validator =
        jsonschema::validator_for(&directory.parameter_schema).map_err(|_| Error::Protocol)?;
    if !validator.is_valid(&value) {
        return Err(Error::Protocol);
    }
    Ok(resolved)
}
impl Core {
    pub(super) fn employee(&self, command: &Command) -> Result<&Employee, Error> {
        if self.host.revoked.load(Ordering::Acquire) || self.stopping {
            return Err(Error::Denied);
        }
        let employee = self
            .employees
            .get(&peer_key(command)?)
            .ok_or(Error::Identity)?;
        if self.fences.current(&employee.subject)? != employee.epoch {
            return Err(Error::Identity);
        }
        if self.host.clock.millis()? >= employee.expires_at_unix_ms {
            return Err(Error::Expired);
        }
        Ok(employee)
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
            return Ok(true);
        };
        let PreparedAction::Policy { actor, .. } = &record.action else {
            return Ok(true);
        };
        if trigger_subject(&record.trigger) != Some(subject)
            || record.trigger_os_session() != binding
        {
            return Ok(false);
        }
        let current = self
            .employees
            .iter()
            .find(|(key, _)| {
                key.starts_with(&format!("{subject}:"))
                    && binding.is_some_and(|id| key.ends_with(&format!(":{}", id.as_str())))
            })
            .map(|(_, value)| value);
        Ok(current.is_some_and(|current| {
            self.fences
                .current(&current.subject)
                .is_ok_and(|epoch| epoch == current.epoch)
                && &current.actor == actor
                && self
                    .host
                    .clock
                    .millis()
                    .is_ok_and(|now| now < current.expires_at_unix_ms)
        }))
    }
    pub(crate) fn policy_fence(
        &self,
        record: &BackendRequest,
    ) -> Result<Option<(Arc<Fences>, String, u64)>, Error> {
        if let PreparedAction::Policy {
            actor, generation, ..
        } = &record.action
        {
            self.current_policy_login(record)?;
            let subject = trigger_subject(&record.trigger).ok_or(Error::Identity)?;
            let employee = self
                .employees
                .values()
                .find(|employee| {
                    employee.subject == subject
                        && &employee.actor == actor
                        && &employee.generation == generation
                })
                .ok_or(Error::Identity)?;
            Ok(Some((
                self.fences.clone(),
                subject.to_owned(),
                employee.epoch,
            )))
        } else {
            Ok(None)
        }
    }
    pub(crate) fn current_policy_login(&self, record: &BackendRequest) -> Result<(), Error> {
        if self.host.revoked.load(Ordering::Acquire) || self.stopping {
            return Err(Error::Denied);
        }
        if let PreparedAction::Policy {
            actor, generation, ..
        } = &record.action
        {
            let login = match &record.trigger {
                BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. } => {
                    os_session
                }
                _ => return Err(Error::Denied),
            };
            let current = self
                .employees
                .iter()
                .find(|(key, _)| {
                    key.starts_with(&format!("{}:", login.account.subject.as_str()))
                        && key.ends_with(&format!(":{}", login.session.as_str()))
                })
                .map(|(_, v)| v)
                .ok_or(Error::Identity)?;
            if self.fences.current(&current.subject)? != current.epoch
                || &current.actor != actor
                || &current.generation != generation
            {
                return Err(Error::Identity);
            }
            if self.host.clock.millis()? >= current.expires_at_unix_ms
                || self.host.clock.millis()? >= record.action.expires_at_unix_ms()
            {
                return Err(Error::Expired);
            }
        }
        Ok(())
    }
    pub(super) fn enqueue_policy(&mut self, command: Command) {
        if let Some(Request::EmployeeSession { value: None }) = command.request.as_operation() {
            let _ = self.fences.revoke(&command.subject);
            if let Ok(key) = peer_key(&command) {
                self.employees.remove(&key);
            }
            let _ = command.reply.send(Reply::EmployeeBound);
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
        while let Some(command) = self.core.policy_commands.pop_front() {
            if command.reply.is_closed() {
                continue;
            }
            let result = self.policy_command(&command, commands).await;
            let reply = match result {
                Ok(reply) => reply,
                Err(Error::Unavailable | Error::Capacity | Error::Expired) => Reply::Unavailable,
                Err(_) => Reply::Rejected,
            };
            let _ = command.reply.send(reply);
        }
        Ok(())
    }
    async fn policy_directory(
        &mut self,
        employee: &EmployeeSession,
        origin: &ClientOrigin,
        after: Option<Id>,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<wire::SelfServiceDirectoryResponse, Error> {
        let query = wire::SelfServiceDirectory {
            source: source(origin),
            after: after.map(|id| uuid(id.as_str())).transpose()?,
        };
        network(
            self.client
                .self_service_directory(&employee.credential, &query),
            &mut self.core,
            commands,
        )
        .await
    }
    async fn policy_command(
        &mut self,
        command: &Command,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<Reply, Error> {
        let Some(request) = command.request.as_operation() else {
            return Err(Error::Protocol);
        };
        if let Request::EmployeeSession { value: Some(value) } = request {
            if value.actor.tenant_id != self.client.configuration().tenant
                || value.actor.instance_id.is_nil()
                || value.actor.principal_id.is_nil()
                || value.expires_at_unix_ms <= self.core.host.clock.millis()?
            {
                return Err(Error::Identity);
            }
            if self.core.host.revoked.load(Ordering::Acquire) || self.core.stopping {
                return Err(Error::Denied);
            }
            if self.core.employees.len() >= 32
                && !self.core.employees.contains_key(&peer_key(command)?)
            {
                return Err(Error::Capacity);
            }
            if self.core.fences.current(&command.subject)? != command.fence_epoch {
                return Err(Error::Identity);
            }
            self.prune_credentials()?;
            let key = peer_key(command)?;
            let lease = self
                .core
                .employees
                .get(&key)
                .filter(|old| {
                    old.generation == value.generation && old.credential == value.credential
                })
                .map(|old| old.lease.clone())
                .unwrap_or_else(|| Arc::new(value.credential.clone()));
            self.bridge.protect_employee(&lease)?;
            self.core.employees.insert(
                key,
                Employee {
                    session: value.clone(),
                    subject: command.subject.clone(),
                    epoch: command.fence_epoch,
                    lease,
                },
            );
            return Ok(Reply::EmployeeBound);
        }
        if let Request::ObserveSoftware { choice, origin } = request {
            return self.observe_software(command, choice, origin);
        }
        let employee = self.core.employee(command)?.clone();
        let result = match request {
            Request::PolicyDirectory { after, origin } => Reply::PolicyDirectory {
                value: self
                    .policy_directory(&employee, origin, after.clone(), commands)
                    .await?,
            },
            Request::InvokePolicy { choice, origin } => {
                uuid(choice.request.as_str())?;
                let caller = self.core.caller();
                let bound_trigger = trigger(&self.core, command, origin)?;
                if let Some(previous) = self
                    .core
                    .app
                    .backend_request(&caller, &choice.request)
                    .map_err(map_app_error)?
                {
                    let PreparedAction::Policy {
                        choice: original,
                        actor,
                        generation,
                        ..
                    } = &previous.action
                    else {
                        return Err(Error::Conflict);
                    };
                    if original != choice
                        || actor != &employee.actor
                        || generation != &employee.generation
                        || previous.trigger != bound_trigger
                    {
                        return Err(Error::Conflict);
                    }
                    Reply::PolicyRequest {
                        value: Box::new(previous),
                    }
                } else {
                    let mut after = None;
                    let mut found = None;
                    for _ in 0..16 {
                        let page = self
                            .policy_directory(&employee, origin, after.take(), commands)
                            .await?;
                        found = page
                            .items
                            .into_iter()
                            .find(|d| d.policy_id.to_string() == choice.policy_id.as_str());
                        if found.is_some() || page.next_cursor.is_none() {
                            break;
                        }
                        after = page
                            .next_cursor
                            .map(|id| plan::id(id.to_string()))
                            .transpose()?;
                    }
                    let directory = found.ok_or(Error::Denied)?;
                    if directory.policy_revision != choice.policy_revision
                        || directory.policy_version.to_string() != choice.policy_version.as_str()
                        || !directory.self_service.published
                        || (source(origin) == wire::ScriptCaller::Ai
                            && !directory.self_service.allow_ai)
                    {
                        return Err(Error::Denied);
                    }
                    parameters(&directory, &choice.parameters)?;
                    let now = self.core.host.clock.millis()?;
                    let until = now.saturating_add(60_000).min(employee.expires_at_unix_ms);
                    let risk = if source(origin) == wire::ScriptCaller::Ai {
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
                        action: PreparedAction::Policy {
                            choice: choice.clone(),
                            directory: Box::new(directory),
                            run_id: None,
                            offer: None,
                            actor: employee.actor.clone(),
                            generation: employee.generation.clone(),
                            expires_at_unix_ms: until,
                        },
                        trigger: bound_trigger,
                        risk: risk.clone(),
                        confirmation: None,
                        revision: 1,
                        state: if risk.is_some_and(|r| r.level == RiskLevel::Two) {
                            BackendRequestState::AwaitingConfirmation
                        } else {
                            BackendRequestState::Ready
                        },
                        failure: None,
                    };
                    let current = self.core.employee(command)?;
                    if current.actor != employee.actor || current.generation != employee.generation
                    {
                        return Err(Error::Identity);
                    }
                    self.core
                        .app
                        .record_backend_request(&caller, None, &record)
                        .map_err(map_app_error)?;
                    Reply::PolicyRequest {
                        value: Box::new(self.send_policy(record, &employee, commands).await?),
                    }
                }
            }
            Request::ConfirmPolicy { request } => {
                let previous = self
                    .core
                    .app
                    .backend_request(&self.core.caller(), request)
                    .map_err(map_app_error)?
                    .ok_or(Error::Denied)?;
                self.core.current_policy_login(&previous)?;
                if trigger_subject(&previous.trigger) != Some(command.subject.as_str())
                    || previous.trigger_os_session() != command.binding.as_ref()
                {
                    return Err(Error::Denied);
                }
                if previous.state != BackendRequestState::AwaitingConfirmation {
                    return Ok(Reply::PolicyRequest {
                        value: Box::new(previous),
                    });
                }
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
                next.confirmation = Some(execution_contract::BackendConfirmation {
                    os_session: os_session.clone(),
                    confirmed_at_unix_ms: now,
                    expires_at_unix_ms: next
                        .action
                        .expires_at_unix_ms()
                        .min(now.saturating_add(60_000)),
                });
                self.core
                    .app
                    .record_backend_request(&self.core.caller(), Some(&previous), &next)
                    .map_err(map_app_error)?;
                Reply::PolicyRequest {
                    value: Box::new(self.send_policy(next, &employee, commands).await?),
                }
            }
            Request::PolicyStatus { request } => {
                let Some(previous) = self
                    .core
                    .app
                    .backend_request(&self.core.caller(), request)
                    .map_err(map_app_error)?
                else {
                    return Ok(Reply::PolicyAbsent);
                };
                let PreparedAction::Policy {
                    actor, generation, ..
                } = &previous.action
                else {
                    return Err(Error::Denied);
                };
                if actor != &employee.actor
                    || previous.trigger_os_session() != command.binding.as_ref()
                {
                    return Err(Error::Denied);
                }
                if matches!(
                    previous.state,
                    BackendRequestState::AwaitingConfirmation
                        | BackendRequestState::Ready
                        | BackendRequestState::Failed
                        | BackendRequestState::Cancelled
                ) {
                    Reply::PolicyRequest {
                        value: Box::new(previous),
                    }
                } else {
                    match network(
                        self.client
                            .self_service_status(&employee.credential, uuid(request.as_str())?),
                        &mut self.core,
                        commands,
                    )
                    .await
                    {
                        Ok(value) => Reply::PolicyStatus {
                            value: self.bridge.protect_status(value)?,
                        },
                        Err(Error::NotFound) if generation != &employee.generation => {
                            Reply::PolicyRequest {
                                value: Box::new(previous),
                            }
                        }
                        Err(Error::NotFound) => Reply::PolicyRequest {
                            value: Box::new(self.send_policy(previous, &employee, commands).await?),
                        },
                        Err(error) => return Err(error),
                    }
                }
            }
            _ => return Err(Error::Protocol),
        };
        let current = self.core.employee(command)?;
        if current.actor != employee.actor || current.generation != employee.generation {
            return Err(Error::Identity);
        }
        Ok(result)
    }
    async fn send_policy(
        &mut self,
        record: BackendRequest,
        employee: &EmployeeSession,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<BackendRequest, Error> {
        if record.state == BackendRequestState::AwaitingConfirmation {
            return Ok(record);
        }
        if !matches!(
            record.state,
            BackendRequestState::Ready | BackendRequestState::Submitting
        ) {
            return Ok(record);
        }
        self.core.current_policy_login(&record)?;
        let PreparedAction::Policy { choice, run_id, .. } = &record.action else {
            return Err(Error::Protocol);
        };
        if run_id.is_some() {
            return Ok(record);
        }
        let input = wire::SelfServiceInvoke {
            operation_id: uuid(choice.request.as_str())?,
            policy_id: uuid(choice.policy_id.as_str())?,
            policy_revision: choice.policy_revision,
            policy_version: uuid(choice.policy_version.as_str())?,
            source: match record.trigger {
                BackendTrigger::Ai { .. } => wire::ScriptCaller::Ai,
                _ => wire::ScriptCaller::Human,
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
            self.client
                .invoke_self_service(&employee.credential, &input),
            &mut self.core,
            commands,
        )
        .await;
        match response {
            Ok(accepted) => {
                let current = self
                    .core
                    .app
                    .backend_request(&self.core.caller(), record.action.request())
                    .map_err(map_app_error)?
                    .ok_or(Error::Conflict)?;
                if current != record {
                    return Err(Error::Conflict);
                }
                let mut next = current.clone();
                next.revision += 1;
                if let PreparedAction::Policy { run_id, .. } = &mut next.action {
                    *run_id = Some(plan::id(accepted.run_id.to_string())?);
                }
                self.core
                    .app
                    .record_backend_request(&self.core.caller(), Some(&current), &next)
                    .map_err(map_app_error)?;
                Ok(next)
            }
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
    pub(super) fn bind_policy_offer(&mut self, offer: &Offer) -> Result<Selection, Error> {
        let wire::TaskPayload::Script(spec) = offer.payload() else {
            return Err(Error::Denied);
        };
        let wire::ScriptAuthorization::SelfService {
            request_id,
            actor,
            source,
            policy_id,
            policy_revision,
            policy_version,
            allow_ai,
            risk_level,
            confirmed,
        } = &spec.authorization
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
        self.core.current_policy_login(&previous)?;
        let PreparedAction::Policy {
            choice,
            directory,
            actor: expected,
            run_id,
            ..
        } = &previous.action
        else {
            return Err(Error::Denied);
        };
        let expected_source = match previous.trigger {
            BackendTrigger::Ai { .. } => wire::ScriptCaller::Ai,
            _ => wire::ScriptCaller::Human,
        };
        if previous.state != BackendRequestState::Submitting
            || request_id.to_string() != choice.request.as_str()
            || actor != expected
            || *source != expected_source
            || policy_id.to_string() != choice.policy_id.as_str()
            || *policy_revision != choice.policy_revision
            || policy_version.to_string() != choice.policy_version.as_str()
            || *allow_ai != directory.self_service.allow_ai
            || *risk_level != directory.self_service.risk_level
            || *confirmed != previous.confirmation.is_some()
            || spec.resource.id != directory.resource.id
            || spec.resource.version != directory.resource.version
            || spec.resource.variant != directory.resource.variant
            || spec.parameters != parameters(directory, &choice.parameters)?
            || run_id
                .as_ref()
                .is_some_and(|id| id.as_str() != spec.task_id.to_string())
        {
            return Err(Error::Untrusted);
        }
        let lease = self.core.employees.values().find(|employee| {
            employee.actor == *expected && matches!(&previous.action, PreparedAction::Policy { generation, .. } if generation == &employee.generation)
        }).ok_or(Error::Identity)?.lease.clone();
        self.core.credential_leases.insert(request.clone(), lease);
        let mut next = previous.clone();
        next.revision += 1;
        if let PreparedAction::Policy {
            offer: bound,
            run_id,
            ..
        } = &mut next.action
        {
            let current = offered(offer)?;
            if bound.as_ref().is_some_and(|old| old != &current) {
                return Err(Error::Conflict);
            }
            *bound = Some(current);
            *run_id = Some(plan::id(spec.task_id.to_string())?);
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
        let commands = backend::commands::software_commands(&step.action)?;
        let detector = match commands.detection {
            Some(wire::SoftwareTaskDetection::PkgReceipt { receipt, version }) => {
                SoftwareDetector::PkgReceipt {
                    receipt: receipt.clone(),
                    version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
                }
            }
            Some(wire::SoftwareTaskDetection::MsiProduct {
                product_code,
                version,
            }) => SoftwareDetector::MsiProduct {
                product_code: product_code.clone(),
                version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
            },
            _ => return Err(Error::Unsupported),
        };
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
    pub(super) fn prune_credentials(&mut self) -> Result<(), Error> {
        let now = self.core.host.clock.millis()?;
        let fences = &self.core.fences;
        self.core.employees.retain(|_, employee| {
            now < employee.expires_at_unix_ms
                && fences
                    .current(&employee.subject)
                    .is_ok_and(|epoch| epoch == employee.epoch)
        });
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
