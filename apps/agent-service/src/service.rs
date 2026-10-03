use crate::error::app_error as map_app_error;
use crate::{
    backend::{self, host::EnterpriseHost, offered, plan, ExecutionBridge},
    DeviceSecrets, Interpreter, SystemClock,
};
use agent_client::{wire, Client, CredentialRedactor, Error, Materials, Offer, SecretProvider};
use execution_app::{AppConfig, ExecutionApp, RequestContext};
use execution_contract::{
    BackendRequest, BackendRequestFailure, BackendRequestState, BackendTrigger, RequestId,
};
use execution_runner::{MaterialRegistry, NativeRunner};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// Explicit local execution resources from the protected deployment configuration.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    pub work_root: PathBuf,
    pub material_root: PathBuf,
    pub interpreters: Vec<Interpreter>,
    pub managers: Vec<crate::backend::plan::SoftwareManager>,
    pub processes: usize,
}
// Helper availability is not device registration identity. A closed login/endpoint
// rejects helper work without entering the backend credential revocation path.
pub(crate) fn helper_error(error: execution_app::Error) -> Error {
    match error {
        execution_app::Error::Unbound => Error::Unavailable,
        other => map_app_error(other),
    }
}
#[cfg(test)]
#[test]
fn unavailable_helper_does_not_revoke_device_identity() {
    assert_eq!(
        helper_error(execution_app::Error::Unbound),
        Error::Unavailable
    );
    assert_eq!(helper_error(execution_app::Error::Denied), Error::Denied);
    assert_eq!(
        map_app_error(execution_app::Error::Unbound),
        Error::Identity
    );
}

/// Administrator-pinned helpers; these select OS mechanisms, not enterprise authorization.
#[derive(Clone)]
pub struct UserResources {
    pub image: installation_security::Artifact,
    pub work_roots: std::collections::BTreeMap<String, PathBuf>,
}
impl UserResources {
    pub(crate) fn connect(
        &self,
        selection: Option<&Selection>,
    ) -> Result<Arc<execution_ipc::helper::Connection>, Error> {
        let (subject, session) = match selection {
            Some(selection) => (selection.subject.clone(), selection.session),
            None => execution_ipc::host::active_user_session().map_err(helper_error)?,
        };
        let root = self.work_roots.get(&subject).ok_or(Error::Denied)?;
        let connection = execution_ipc::helper::Connection::connect(
            execution_ipc::host::PeerPolicy {
                images: vec![self.image.clone()],
                subjects: vec![subject.clone()],
                interactive: true,
            },
            subject,
            session,
        )
        .map_err(helper_error)?;
        if selection.is_some_and(|s| s.binding != connection.context().binding) {
            return Err(Error::Untrusted);
        }
        if root.canonicalize()? != connection.context().work_root {
            return Err(Error::Denied);
        }
        Ok(Arc::new(connection))
    }
}
/// One device communication owner and one authoritative execution journal.
/// The OS service supplies this assembly; no fixture runner or authority is selectable.
pub struct DeviceService<S: SecretProvider = DeviceSecrets> {
    pub(crate) client: Client<S, SystemClock>,
    pub(crate) core: Core,
    pub(crate) bridge: ExecutionBridge<CredentialRedactor>,
    waiting: Option<(Offer, Materials)>,
    next_network: std::time::Instant,
    recovering: bool,
}
impl<S: SecretProvider> DeviceService<S> {
    /// Open an already registered device. Create/open selection never follows an error fallback.
    pub fn open(
        mut client: Client<S, SystemClock>,
        journal: &Path,
        startup: ProductionStartup,
        config: ExecutionConfig,
        clock: SystemClock,
        helpers: UserResources,
    ) -> Result<Self, Error> {
        if config.interpreters.is_empty()
            || config.interpreters.len() > 8
            || config.processes == 0
            || config.processes > 128
        {
            return Err(Error::Configuration);
        }
        let mut profiles = Vec::new();
        for interpreter in &config.interpreters {
            interpreter
                .image
                .verify(&interpreter.image.path)
                .map_err(|_| Error::Untrusted)?;
            profiles.push(interpreter.profile);
        }
        client.set_profiles(profiles)?;
        platform_private_storage::validate(&config.work_root)?;
        let (binding, actor) = plan::context(
            client.configuration().origin.as_str(),
            client.configuration().tenant,
            &client.registration()?,
        )?;
        let materials = MaterialRegistry::new(client.configuration().limits.pending_tasks)
            .map_err(crate::error::app_error)?;
        let runner = NativeRunner::with_materials(
            plan::id("native-device-runner")?,
            materials.clone(),
            config.processes,
        )
        .map_err(crate::error::app_error)?;
        let host = EnterpriseHost {
            binding,
            actor,
            clock,
            materials,
            subject: execution_ipc::host::current_subject().map_err(crate::error::app_error)?,
            current: Arc::new(Mutex::new(None)),
            revoked: Arc::new(AtomicBool::new(false)),
        };
        let app = assemble_journal(
            journal,
            startup,
            host.clone(),
            runner,
            app_config(),
            plan::storage_limits(),
        )
        .map_err(crate::error::app_error)?;
        let output = client.output_policy()?;
        Ok(Self {
            client,
            core: Core {
                app,
                host,
                config,
                helpers,
                available: None,
                selected: None,
                stopping: false,
                recovery_cursor: None,
            },
            bridge: ExecutionBridge::new(plan::id("backend-results")?, output),
            waiting: None,
            next_network: std::time::Instant::now(),
            recovering: true,
        })
    }
    /// Reconcile persisted intents and deliver existing evidence before claiming new work.
    async fn drive(
        &mut self,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Error> {
        while let Ok(command) = commands.try_recv() {
            self.core.handle(command);
        }
        if self.core.stopping {
            return Ok(());
        }
        if self.core.host.revoked.load(Ordering::Acquire) {
            self.core.revoke_preparations()?;
            self.core.reconcile()?;
            return Ok(());
        }
        if let Some((offer, _)) = &self.waiting {
            let request = backend::request_id(&offer)?;
            if self
                .core
                .app
                .backend_request(&self.core.caller(), &request)
                .map_err(crate::error::app_error)?
                .is_some_and(|p| p.state == BackendRequestState::Cancelled)
            {
                let task = offer.task_id();
                self.waiting = None;
                self.core.available = None;
                self.core.selected = None;
                self.abandon(task, commands).await?;
            }
        }
        if self.core.selected.is_some() {
            if let Some((offer, materials)) = self.waiting.take() {
                let request = backend::request_id(&offer)?;
                let selection = self.core.selected.take();
                self.core
                    .transition(&request, BackendRequestState::Submitting, None)?;
                let result = backend::submit(self, offer, materials, selection, commands).await;
                self.core.available = None;
                if let Err(error) = result {
                    self.core.transition(
                        &request,
                        BackendRequestState::Failed,
                        Some(BackendRequestFailure::PreparationFailed),
                    )?;
                    return Err(error);
                }
            }
        }
        self.core.reconcile()?;
        if std::time::Instant::now() < self.next_network {
            return Ok(());
        }
        self.next_network = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let caller = self.core.caller();
        if self.waiting.as_ref().is_some_and(|(offer, _)| {
            self.core
                .host
                .clock
                .millis()
                .is_ok_and(|now| now / 1000 >= offer.payload().expires_at() as u64)
        }) {
            if let Some((offer, _)) = &self.waiting {
                self.core.transition(
                    &backend::request_id(&offer)?,
                    BackendRequestState::Failed,
                    Some(BackendRequestFailure::Expired),
                )?;
            }
            self.waiting = None;
            self.core.available = None;
            self.core.selected = None;
        }
        let pending = self
            .client
            .pending_tasks(self.client.configuration().limits.pending_tasks)?;
        for task in pending {
            while let Ok(command) = commands.try_recv() {
                self.core.handle(command);
            }
            if self.core.stopping {
                return Ok(());
            }
            if self.client.bound_request(task)?.is_some() {
                if !self
                    .bridge
                    .recover_binding(&mut self.client, task, &self.core.app)?
                {
                    self.abandon(task, commands).await?;
                    continue;
                }
                if let Some(mut pending) =
                    self.bridge
                        .prepare_delivery(&mut self.client, task, &mut self.core.app, 64)?
                {
                    network(
                        self.bridge.send_delivery(&mut self.client, &mut pending),
                        &mut self.core,
                        commands,
                    )
                    .await?;
                    self.bridge
                        .finish_delivery(&mut self.client, pending, &mut self.core.app)?;
                }
                let request = self.client.bound_request(task)?.ok_or(Error::Conflict)?;
                let input = self
                    .core
                    .app
                    .frozen_input(&caller, &request)
                    .map_err(crate::error::app_error)?;
                match self
                    .bridge
                    .finish(&mut self.client, task, &self.core.app, &caller)
                {
                    Ok(()) => match self.core.host.materials.retire(&input) {
                        Ok(()) | Err(execution_app::Error::Conflict) => (),
                        Err(error) => return Err(map_app_error(error)),
                    },
                    Err(Error::Conflict) => {
                        // Only unresolved original work needs recovery material. Settled tasks
                        // must release first, without rebuilding every native phase after ACK.
                        if !self
                            .core
                            .host
                            .materials
                            .registered(&input)
                            .map_err(crate::error::app_error)?
                        {
                            if let Ok(materials) = crate::recovery::materials(
                                &input,
                                &self.core.config,
                                &self.core.helpers,
                            ) {
                                self.core
                                    .host
                                    .materials
                                    .register(&input, materials)
                                    .map_err(crate::error::app_error)?;
                            }
                        }
                    }
                    Err(error) => return Err(error),
                }
            } else if self.recovering
                || self
                    .waiting
                    .as_ref()
                    .is_none_or(|(offer, _)| offer.task_id() != task)
            {
                // No local intent means no execution may have started. Retire the original
                // remote attempt only after checking this same authoritative journal; a prior
                // local user selection is never reconstructed from transport or request data.
                self.abandon(task, commands).await?;
            }
        }
        self.recovering = false;
        let reports = 16.min(self.client.configuration().limits.pending_reports);
        network(self.client.flush_reports(reports), &mut self.core, commands).await?;
        let mut context = self.client.execution_context()?;
        context.os_version = crate::os_version::current()?;
        context.interactive_user =
            self.core
                .helpers
                .connect(None)
                .ok()
                .map(|helper| wire::SoftwareInteractiveUser {
                    identity: helper.context().subject.clone(),
                    session_id: login_id(helper.context()),
                    administrator: false,
                });
        self.client.set_execution_context(context)?;
        let claim = match network(self.client.claim(), &mut self.core, commands).await {
            Ok(v) => v,
            Err(e) => {
                if e == Error::Identity {
                    self.core.host.revoked.store(true, Ordering::Release);
                    self.core
                        .app
                        .stop_active(128)
                        .map_err(crate::error::app_error)?;
                }
                return Err(e);
            }
        };
        for cancel in claim.cancellations {
            if self.client.bound_request(cancel.task_id())?.is_some() {
                self.bridge
                    .cancel(&self.client, &cancel, &mut self.core.app, &caller)?;
            } else if self.waiting.as_ref().is_some_and(|(offer, _)| {
                offer.task_id() == cancel.task_id() && offer.attempt_id() == cancel.attempt_id()
            }) {
                self.waiting = None;
                self.core.available = None;
                self.core.selected = None;
                // Withdrawal has no execution effect; the original request remains queryable.
                let pending = self.bridge.prepare_abandonment(
                    &mut self.client,
                    cancel.task_id(),
                    &self.core.app,
                )?;
                self.core.transition(
                    pending.request_id(),
                    BackendRequestState::Cancelled,
                    Some(BackendRequestFailure::Revoked),
                )?;
                self.abandon(cancel.task_id(), commands).await?;
            }
        }
        if let Some(offer) = claim.offer {
            if self.waiting.as_ref().is_some_and(|(old, _)| {
                old.task_id() == offer.task_id() && old.attempt_id() == offer.attempt_id()
            }) {
                return Ok(());
            }
            if self.waiting.is_some() {
                return Err(Error::Capacity);
            }
            if self.client.bound_request(offer.task_id())?.is_some() {
                return Ok(());
            }
            self.core.available = Some(offered(&offer)?);
            network(self.client.received(&offer), &mut self.core, commands).await?;
            let materials = network(self.client.prepare(&offer), &mut self.core, commands).await?;
            if matches!(offer.payload(),wire::TaskPayload::Software(p) if p.start_mode==wire::SoftwareStartMode::UserInitiated)
            {
                self.waiting = Some((offer, materials));
                return Ok(());
            }
            backend::submit(self, offer, materials, None, commands).await?;
            self.core.available = None;
        }
        Ok(())
    }
    async fn abandon(
        &mut self,
        task: uuid::Uuid,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Error> {
        let mut pending =
            self.bridge
                .prepare_abandonment(&mut self.client, task, &self.core.app)?;
        self.core.transition(
            pending.request_id(),
            BackendRequestState::Failed,
            Some(BackendRequestFailure::Interrupted),
        )?;
        network(
            self.bridge.send_abandonment(&mut self.client, &mut pending),
            &mut self.core,
            commands,
        )
        .await?;
        self.bridge
            .finish_abandonment(&mut self.client, pending, &self.core.app)
    }
    /// Start the single service owner; native callbacks never access SQLite or network directly.
    pub fn spawn(
        mut self,
        policy: execution_ipc::host::PeerPolicy,
        helper_policy: Option<execution_ipc::host::PeerPolicy>,
    ) -> Result<ServiceHandle, Error>
    where
        S: Send + 'static,
    {
        policy.validate().map_err(crate::error::app_error)?;
        if let Some(policy) = &helper_policy {
            policy.validate().map_err(crate::error::app_error)?;
        }
        let (sender, mut commands) = tokio::sync::mpsc::channel(32);
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let (completed, completion) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new().name("rss-device-owner".into()).spawn(move||{
            struct Finished(Arc<AtomicBool>);impl Drop for Finished{fn drop(&mut self){self.0.store(true,Ordering::Release);}}
            let _finished=Finished(done);
            let result=(||->Result<(),Error>{
                let runtime=owner_runtime()?;
                runtime.block_on(async{
                    let mut timer=tokio::time::interval(std::time::Duration::from_millis(50));
                    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    while !self.core.stopping {
                        tokio::select! {
                            command=commands.recv()=>match command{Some(command)=>self.core.handle(command),None=>break},
                            _=timer.tick()=>{if let Err(error)=self.drive(&mut commands).await{self.drive_error(error);}}
                        }
                        if self.core.selected.is_some(){if let Err(error)=self.drive(&mut commands).await{self.drive_error(error);}}
                    }
                    self.stop()
                })
            })();
            if let Err(error)=&result{eprintln!("agent_owner: {error}");}
            let _ = completed.send(result);
        })?;
        Ok(ServiceHandle {
            sender,
            policy,
            helper_policy,
            finished,
            completion,
        })
    }
    fn drive_error(&mut self, error: Error) {
        eprintln!("agent_drive: {error}");
        if error == Error::Identity {
            self.core.host.revoked.store(true, Ordering::Release);
            if let Err(error) = self.core.revoke_preparations() {
                eprintln!("agent_revoke_preparation: {error}");
            }
            self.core.available = None;
            self.core.selected = None;
            if let Err(error) = self.core.app.stop_active(128) {
                eprintln!("agent_revoke_stop: {error}");
            }
        }
    }
    /// Stop accepted work. Stopping never claims process termination or effect rollback.
    pub fn stop(&mut self) -> Result<(), Error> {
        Ok(self
            .core
            .app
            .stop_active(128)
            .map_err(crate::error::app_error)?)
    }
}

pub(crate) struct Selection {
    pub(crate) request: RequestId,
    pub(crate) task: execution_contract::Id,
    pub(crate) attempt: execution_contract::Id,
    pub(crate) revision: execution_contract::Digest,
    pub(crate) subject: String,
    pub(crate) session: u32,
    pub(crate) binding: execution_contract::Id,
    pub(crate) origin: execution_ipc::host::ClientOrigin,
}
pub(crate) struct Core {
    pub(crate) app: ExecutionApp<EnterpriseHost, NativeRunner, execution_sqlite::Store>,
    pub(crate) host: EnterpriseHost,
    pub(crate) config: ExecutionConfig,
    pub(crate) helpers: UserResources,
    pub(crate) available: Option<execution_contract::BackendTask>,
    pub(crate) selected: Option<Selection>,
    pub(crate) stopping: bool,
    pub(crate) recovery_cursor: Option<RequestId>,
}
impl Core {
    fn reconcile(&mut self) -> Result<(), Error> {
        let page = self
            .app
            .service_tasks(self.recovery_cursor.as_ref(), 64)
            .map_err(crate::error::app_error)?;
        for item in &page.items {
            let request = &item.status.operation_request_id;
            let status = self
                .app
                .reconcile(request)
                .map_err(crate::error::app_error)?;
            if matches!(
                status.phase,
                execution_app::TaskPhase::Verified
                    | execution_app::TaskPhase::Cancelled
                    | execution_app::TaskPhase::FailedBeforeDispatch
            ) {
                let input = self
                    .app
                    .frozen_input(&self.caller(), request)
                    .map_err(crate::error::app_error)?;
                match self.host.materials.retire(&input) {
                    Ok(()) | Err(execution_app::Error::Conflict) => (),
                    Err(error) => return Err(map_app_error(error)),
                }
            }
        }
        self.recovery_cursor = page.next;
        Ok(())
    }
    fn revoke_preparations(&mut self) -> Result<(), Error> {
        let active = self
            .available
            .as_ref()
            .map(|v| v.request.clone())
            .or_else(|| self.selected.as_ref().map(|v| v.request.clone()));
        if let Some(request) = active {
            self.transition(
                &request,
                BackendRequestState::Failed,
                Some(BackendRequestFailure::Revoked),
            )?;
        }
        for record in self
            .app
            .backend_requests(&self.caller())
            .map_err(crate::error::app_error)?
        {
            if !self
                .app
                .has_service_execution(&record.offer.request, &self.host.binding.device)
                .map_err(crate::error::app_error)?
            {
                self.transition(
                    &record.offer.request,
                    BackendRequestState::Failed,
                    Some(BackendRequestFailure::Revoked),
                )?;
            }
        }
        Ok(())
    }
    fn pending(&self, request: &RequestId, subject: &str) -> Result<Option<BackendRequest>, Error> {
        if self
            .app
            .has_service_execution(request, &self.host.binding.device)
            .map_err(crate::error::app_error)?
        {
            return Ok(None);
        }
        let value = self
            .app
            .backend_request(&self.caller(), request)
            .map_err(crate::error::app_error)?;
        if value
            .as_ref()
            .is_some_and(|p| trigger_subject(&p.trigger) != Some(subject))
        {
            return Err(Error::Denied);
        }
        Ok(value)
    }
    fn transition(
        &mut self,
        request: &RequestId,
        state: BackendRequestState,
        failure: Option<BackendRequestFailure>,
    ) -> Result<Option<BackendRequest>, Error> {
        let Some(previous) = self
            .app
            .backend_request(&self.caller(), request)
            .map_err(crate::error::app_error)?
        else {
            return Ok(None);
        };
        if matches!(
            previous.state,
            BackendRequestState::Cancelled | BackendRequestState::Failed
        ) {
            return Ok(Some(previous));
        }
        let mut next = previous.clone();
        next.revision += 1;
        next.state = state;
        next.failure = failure;
        self.app
            .record_backend_request(&self.caller(), Some(&previous), &next)
            .map_err(crate::error::app_error)?;
        Ok(Some(next))
    }
    pub(crate) fn caller(&self) -> RequestContext {
        RequestContext {
            actor: self.host.actor.clone(),
        }
    }
    fn can_read(&self, request: &RequestId, subject: &str) -> Result<bool, Error> {
        use execution_contract::{BackendTrigger, Initiator, RunAs};
        let input = self
            .app
            .frozen_input(&self.caller(), request)
            .map_err(crate::error::app_error)?;
        Ok(match &input.spec().request.initiator {
            Initiator::Backend {
                trigger:
                    BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. },
                ..
            } => os_session.account.subject.as_str() == subject,
            Initiator::Backend {
                trigger: BackendTrigger::Automatic {},
                ..
            } => match &input.spec().run_as {
                RunAs::User { account } => account.subject.as_str() == subject,
                _ => true,
            },
            _ => false,
        })
    }
    fn handle(&mut self, command: Command) {
        use execution_ipc::host::{Reply, Request};
        if command.reply.is_closed() {
            return;
        }
        let reply = (|| -> Result<Reply, Error> {
            match command.request {
                LocalRequest::Operation(Request::ServiceStatus {}) => {
                    let readiness = if self.host.revoked.load(Ordering::Acquire) {
                        execution_ipc::host::Readiness::NotReady
                    } else {
                        execution_ipc::host::Readiness::Ready
                    };
                    Ok(Reply::ServiceStatus {
                        value: execution_ipc::host::ServiceStatus::new(readiness),
                    })
                }
                LocalRequest::Stop => {
                    self.stopping = true;
                    Ok(Reply::Unavailable)
                }
                LocalRequest::Operation(Request::Tasks { after }) => {
                    let mut value = self
                        .app
                        .tasks(&self.caller(), after.as_ref(), 64)
                        .map_err(crate::error::app_error)?;
                    value.items.retain(|item| {
                        self.can_read(&item.action.request_id, &command.subject)
                            .unwrap_or(false)
                    });
                    Ok(Reply::Tasks {
                        value,
                        available: self.available.iter().cloned().collect(),
                        preparations: self
                            .app
                            .backend_requests(&self.caller())
                            .map_err(crate::error::app_error)?
                            .into_iter()
                            .filter(|p| {
                                trigger_subject(&p.trigger) == Some(command.subject.as_str())
                                    && !self
                                        .app
                                        .has_service_execution(
                                            &p.offer.request,
                                            &self.host.binding.device,
                                        )
                                        .unwrap_or(true)
                            })
                            .collect(),
                    })
                }
                LocalRequest::Operation(Request::StartTask {
                    request,
                    task,
                    attempt,
                    revision,
                    origin,
                }) => {
                    if self.host.revoked.load(Ordering::Acquire) {
                        return Err(Error::Denied);
                    }
                    let replay = Selection {
                        request: request.clone(),
                        task: task.clone(),
                        attempt: attempt.clone(),
                        revision: revision.clone(),
                        subject: command.subject.clone(),
                        session: command.session,
                        binding: command.binding.clone().ok_or(Error::Denied)?,
                        origin: origin.clone(),
                    };
                    if let Some(previous) = self
                        .app
                        .backend_request(&self.caller(), &request)
                        .map_err(crate::error::app_error)?
                    {
                        // A proposed AI request still needs its explicit desktop confirmation.
                        // Once selected, replay returns facts and never repeats that transition.
                        if previous.state != BackendRequestState::Proposed {
                            let original_origin =
                                previous.trigger == replay.trigger(&self.host.binding.device)?;
                            let desktop_confirmation =
                                matches!(origin, execution_ipc::host::ClientOrigin::Desktop {})
                                    && matches!(previous.trigger, BackendTrigger::Ai { .. })
                                    && same_login(&previous.trigger, &replay);
                            if previous.offer.task != task
                                || previous.offer.attempt != attempt
                                || previous.offer.revision != revision
                                || !(original_origin || desktop_confirmation)
                            {
                                return Err(Error::Conflict);
                            }
                            return Ok(Reply::Pending {
                                value: Box::new(previous),
                            });
                        }
                    }
                    let offer = self.available.as_ref().ok_or(Error::Unavailable)?;
                    if offer.request != request
                        || offer.task != task
                        || offer.attempt != attempt
                        || offer.revision != revision
                        || !offer.user_initiated
                        || self.host.clock.millis()? / 1000 >= offer.expires_at as u64
                    {
                        return Err(Error::Denied);
                    }
                    if self.selected.as_ref().is_some_and(|s| {
                        s.subject != command.subject || s.session != command.session
                    }) {
                        return Err(Error::Conflict);
                    }
                    let confirmation_required =
                        matches!(origin, execution_ipc::host::ClientOrigin::Ai { .. });
                    let selection = Selection {
                        request: request.clone(),
                        task,
                        attempt,
                        revision,
                        subject: command.subject,
                        session: command.session,
                        binding: command.binding.ok_or(Error::Denied)?,
                        origin,
                    };
                    let offer = offer.clone();
                    let previous = self
                        .app
                        .backend_request(&self.caller(), &request)
                        .map_err(crate::error::app_error)?;
                    if let Some(previous) = &previous {
                        if previous.offer != offer || !same_login(&previous.trigger, &selection) {
                            return Err(Error::Conflict);
                        }
                        if matches!(
                            previous.state,
                            BackendRequestState::Failed | BackendRequestState::Cancelled
                        ) {
                            return Ok(Reply::Pending {
                                value: Box::new(previous.clone()),
                            });
                        }
                    }
                    let trigger = previous
                        .as_ref()
                        .map(|p| p.trigger.clone())
                        .unwrap_or(selection.trigger(&self.host.binding.device)?);
                    let state = if confirmation_required {
                        BackendRequestState::Proposed
                    } else {
                        BackendRequestState::Selected
                    };
                    if previous.as_ref().is_none_or(|p| p.state != state) {
                        if previous
                            .as_ref()
                            .is_some_and(|p| p.state != BackendRequestState::Proposed)
                        {
                            return Err(Error::Conflict);
                        }
                        let next = BackendRequest {
                            offer: offer.clone(),
                            trigger,
                            revision: previous.as_ref().map_or(1, |p| p.revision + 1),
                            state,
                            failure: None,
                        };
                        self.app
                            .record_backend_request(&self.caller(), previous.as_ref(), &next)
                            .map_err(crate::error::app_error)?;
                    }
                    if !confirmation_required {
                        let saved = self
                            .app
                            .backend_request(&self.caller(), &request)
                            .map_err(crate::error::app_error)?
                            .ok_or(Error::Conflict)?;
                        self.selected = Some(Selection::from_record(&saved)?);
                    }
                    Ok(Reply::Queued {
                        task: offer.task.clone(),
                        attempt: offer.attempt.clone(),
                        request: offer.request.clone(),
                        confirmation_required,
                    })
                }
                LocalRequest::Operation(Request::Status { request }) => {
                    if let Some(value) = self.pending(&request, &command.subject)? {
                        return Ok(Reply::Pending {
                            value: Box::new(value),
                        });
                    }
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    Ok(Reply::Status {
                        value: self
                            .app
                            .status(&self.caller(), &request)
                            .map_err(crate::error::app_error)?,
                    })
                }
                LocalRequest::Operation(Request::Details { request }) => {
                    if let Some(value) = self.pending(&request, &command.subject)? {
                        return Ok(Reply::Pending {
                            value: Box::new(value),
                        });
                    }
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    Ok(Reply::Details {
                        value: Box::new(
                            self.app
                                .task_details(&self.caller(), &request)
                                .map_err(crate::error::app_error)?,
                        ),
                    })
                }
                LocalRequest::Operation(Request::Cancel { request }) => {
                    if let Some(value) = self.pending(&request, &command.subject)? {
                        let value = self
                            .transition(&value.offer.request, BackendRequestState::Cancelled, None)?
                            .ok_or(Error::Conflict)?;
                        if self.selected.as_ref().is_some_and(|s| s.request == request) {
                            self.selected = None;
                        }
                        return Ok(Reply::Pending {
                            value: Box::new(value),
                        });
                    }
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    let input = self
                        .app
                        .frozen_input(&self.caller(), &request)
                        .map_err(crate::error::app_error)?;
                    if !matches!(
                        &input.spec().request.initiator,
                        execution_contract::Initiator::Backend {
                            trigger: execution_contract::BackendTrigger::Human { .. }
                                | execution_contract::BackendTrigger::Ai { .. },
                            ..
                        }
                    ) {
                        return Err(Error::Denied);
                    }
                    Ok(Reply::Status {
                        value: self
                            .app
                            .cancel(&self.caller(), &request)
                            .map_err(crate::error::app_error)?,
                    })
                }
            }
        })();
        let _ = command.reply.send(match reply {
            Ok(reply) => reply,
            Err(
                Error::Denied
                | Error::Conflict
                | Error::Untrusted
                | Error::Identity
                | Error::Protocol,
            ) => Reply::Rejected,
            Err(_) => Reply::Unavailable,
        });
    }
}
enum LocalRequest {
    Operation(execution_ipc::host::Request),
    Stop,
}
pub(crate) struct Command {
    request: LocalRequest,
    subject: String,
    session: u32,
    binding: Option<execution_contract::Id>,
    reply: tokio::sync::oneshot::Sender<execution_ipc::host::Reply>,
}
/// Synchronous owner work selected while transport remains pending.
pub(crate) enum NetworkEvent {
    Reconcile,
    Command(Command),
    Closed,
}
pub(crate) async fn network<T>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    core: &mut Core,
    commands: &mut tokio::sync::mpsc::Receiver<Command>,
) -> Result<T, Error> {
    network_loop(future, commands, |event| {
        match event {
            NetworkEvent::Reconcile => core.reconcile()?,
            NetworkEvent::Command(command) => core.handle(command),
            NetworkEvent::Closed => core.stopping = true,
        }
        Ok(core.stopping)
    })
    .await
}
// One synchronous callback borrows the owner exclusively; neither a task nor a lock owns journal IO.
// ref: tokio tokio-1.47.1 tokio/src/sync/mpsc/bounded.rs (single receiver, cancel-safe recv).
pub(crate) async fn network_loop<T>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    commands: &mut tokio::sync::mpsc::Receiver<Command>,
    mut owner: impl FnMut(NetworkEvent) -> Result<bool, Error>,
) -> Result<T, Error> {
    tokio::pin!(future);
    let period = std::time::Duration::from_millis(50);
    let mut timer = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let event = tokio::select! {
            result = &mut future => return result,
            command = commands.recv() => match command {
                Some(command) => NetworkEvent::Command(command),
                None => NetworkEvent::Closed,
            },
            _ = timer.tick() => NetworkEvent::Reconcile,
        };
        if owner(event)? {
            return Err(Error::Unavailable);
        }
    }
}
/// Native callbacks forward bounded commands to the only journal/network owner.
pub struct ServiceHandle {
    sender: tokio::sync::mpsc::Sender<Command>,
    policy: execution_ipc::host::PeerPolicy,
    helper_policy: Option<execution_ipc::host::PeerPolicy>,
    finished: Arc<AtomicBool>,
    completion: std::sync::mpsc::Receiver<Result<(), Error>>,
}
impl ServiceHandle {
    fn call(
        &self,
        request: LocalRequest,
        subject: String,
        session: u32,
        binding: Option<execution_contract::Id>,
    ) -> execution_ipc::host::Reply {
        let (reply, receiver) = tokio::sync::oneshot::channel();
        if self
            .sender
            .try_send(Command {
                request,
                subject,
                session,
                binding,
                reply,
            })
            .is_err()
        {
            return execution_ipc::host::Reply::Unavailable;
        }
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        {
            Ok(v) => v,
            Err(_) => return execution_ipc::host::Reply::Unavailable,
        };
        let started = std::time::Instant::now();
        let reply = runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(3), receiver)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(execution_ipc::host::Reply::Unavailable)
        });
        let elapsed = started.elapsed().as_millis();
        if elapsed > 1500 {
            eprintln!("RSS_IPC_OWNER_TIMING elapsedMs={elapsed}");
        }
        reply
    }
}
impl execution_ipc::host::Handler for ServiceHandle {
    fn peer_policy(&self) -> Option<execution_ipc::host::PeerPolicy> {
        let mut policy = self.policy.clone();
        if let Some(helper) = &self.helper_policy {
            for image in &helper.images {
                if !policy.images.iter().any(|i| {
                    i.path == image.path && i.sha256 == image.sha256 && i.cdhash == image.cdhash
                }) {
                    policy.images.push(image.clone());
                }
            }
            for subject in &helper.subjects {
                if !policy.subjects.contains(subject) {
                    policy.subjects.push(subject.clone());
                }
            }
        }
        Some(policy)
    }
    fn register_helper(
        &mut self,
        peer: &execution_ipc::host::Peer,
    ) -> Result<(), execution_app::Error> {
        peer.authenticate(
            self.helper_policy
                .as_ref()
                .ok_or(execution_app::Error::Denied)?,
        )?;
        Ok(())
    }
    fn handle(
        &mut self,
        peer: &execution_ipc::host::Peer,
        request: execution_ipc::host::Request,
    ) -> execution_ipc::host::Reply {
        match peer.authenticate(&self.policy) {
            Ok(subject) => match peer.session_binding() {
                Ok(binding) => self.call(
                    LocalRequest::Operation(request),
                    subject,
                    peer.session(),
                    Some(binding),
                ),
                Err(_) => execution_ipc::host::Reply::Rejected,
            },
            Err(_) => execution_ipc::host::Reply::Rejected,
        }
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        if self.finished.load(Ordering::Acquire) {
            Err(execution_app::Error::Unavailable)
        } else {
            Ok(())
        }
    }
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        let _ = self.call(LocalRequest::Stop, String::new(), 0, None);
        match self
            .completion
            .recv_timeout(std::time::Duration::from_secs(5))
        {
            Ok(Ok(())) => Ok(()),
            _ => Err(execution_app::Error::OutcomeUnknown),
        }
    }
}

fn trigger_subject(trigger: &BackendTrigger) -> Option<&str> {
    match trigger {
        BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. } => {
            Some(os_session.account.subject.as_str())
        }
        _ => None,
    }
}
fn same_login(trigger: &BackendTrigger, selection: &Selection) -> bool {
    match trigger {
        BackendTrigger::Human { os_session } | BackendTrigger::Ai { os_session, .. } => {
            os_session.account.subject.as_str() == selection.subject
                && os_session.session == selection.binding
        }
        _ => false,
    }
}
impl Selection {
    fn trigger(&self, device: &execution_contract::DeviceId) -> Result<BackendTrigger, Error> {
        let os_session = execution_contract::OsSessionRef {
            device: device.clone(),
            account: execution_contract::OsAccountRef {
                platform: plan::platform()?,
                subject: plan::id(&self.subject)?,
            },
            session: self.binding.clone(),
        };
        Ok(match &self.origin {
            execution_ipc::host::ClientOrigin::Desktop {} => BackendTrigger::Human { os_session },
            execution_ipc::host::ClientOrigin::Ai {
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
    fn from_record(record: &BackendRequest) -> Result<Self, Error> {
        let (os_session, origin) = match &record.trigger {
            BackendTrigger::Human { os_session } => {
                (os_session, execution_ipc::host::ClientOrigin::Desktop {})
            }
            BackendTrigger::Ai {
                os_session,
                config,
                conversation,
                tool_call,
            } => (
                os_session,
                execution_ipc::host::ClientOrigin::Ai {
                    config: config.clone(),
                    conversation: conversation.clone(),
                    tool_call: tool_call.clone(),
                },
            ),
            _ => return Err(Error::Denied),
        };
        Ok(Self {
            request: record.offer.request.clone(),
            task: record.offer.task.clone(),
            attempt: record.offer.attempt.clone(),
            revision: record.offer.revision.clone(),
            subject: os_session.account.subject.as_str().into(),
            session: os_session
                .session
                .as_str()
                .split('/')
                .nth(1)
                .ok_or(Error::Identity)?
                .parse()
                .map_err(|_| Error::Identity)?,
            binding: os_session.session.clone(),
            origin,
        })
    }
}

pub(crate) fn login_id(context: &execution_ipc::helper::UserContext) -> uuid::Uuid {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(context.binding.as_str().as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

pub(crate) fn app_config() -> AppConfig {
    let bounds = plan::storage_limits().input;
    AppConfig {
        revision: 1,
        max_rules: 1,
        max_profiles: 1,
        max_capability_entries: 32,
        max_timeout_ms: bounds.max_timeout_ms,
        max_output_bytes: bounds.max_output_bytes,
    }
}

pub(crate) fn owner_runtime() -> Result<tokio::runtime::Runtime, Error> {
    // The block_on caller remains the only SQLite/journal owner. Two fixed reactor workers
    // keep bounded transport alive while that caller performs synchronous OS fact checks.
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| Error::Unavailable)
}

/// Explicit production journal lifecycle, owned by the service composition root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionStartup {
    /// Create a new journal without replacing an existing file.
    Create,
    /// Open the existing authority and exact supported schema.
    Open,
}

pub(crate) fn assemble_journal<H: execution_app::AppHost, R: execution_app::RunnerPort>(
    path: &Path,
    startup: ProductionStartup,
    host: H,
    runner: R,
    config: AppConfig,
    limits: execution_sqlite::Limits,
) -> Result<ExecutionApp<H, R, execution_sqlite::Store>, execution_app::Error> {
    // Validate production inputs before any database access, including read-only inspection.
    execution_app::Configuration::new(config, limits.input)?;
    let binding = host.service_binding()?;
    if matches!(
        binding.authority,
        execution_contract::Authority::Test { .. }
    ) || runner.mode() != execution_lifecycle::ExecutionMode::Real
    {
        return Err(execution_app::Error::Unbound);
    }
    host.reliable_now()?;
    let journal = match startup {
        ProductionStartup::Create => {
            execution_sqlite::Store::initialize_production(path, binding.authority, limits)?
        }
        ProductionStartup::Open => {
            match execution_sqlite::Store::open(path, &binding.authority, limits)? {
                execution_sqlite::OpenOutcome::Ready(journal) => *journal,
                execution_sqlite::OpenOutcome::UnsupportedSchema { found, supported } => {
                    return Err(execution_app::Error::UnsupportedSchema { found, supported });
                }
            }
        }
    };
    ExecutionApp::new(journal, host, runner, config)
}
