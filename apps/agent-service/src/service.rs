use crate::{
    host::{BackendPermit, EnterpriseHost},
    plan, DeviceSecrets, Interpreter, SystemClock,
};
use agent_client::{
    wire, Client, CredentialRedactor, Error, ExecutionBridge, Materials, Offer, SecretProvider,
};
use execution_app::{AppConfig, ExecutionApp, ExecutionStatus, ProductionStartup, RequestContext};
use execution_contract::RequestId;
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
    pub managers: Vec<crate::plan::SoftwareManager>,
    pub processes: usize,
}
/// Administrator-pinned helpers; these select OS mechanisms, not enterprise authorization.
#[derive(Clone)]
pub struct UserResources {
    pub image: local_service::Artifact,
    pub work_roots: std::collections::BTreeMap<String, PathBuf>,
}
impl UserResources {
    fn connect(
        &self,
        selection: Option<&Selection>,
    ) -> Result<Arc<execution_runner::helper::Connection>, Error> {
        let (subject, session) = match selection {
            Some(selection) => (selection.subject.clone(), selection.session),
            None => execution_runner::host::active_user_session()?,
        };
        let root = self.work_roots.get(&subject).ok_or(Error::Denied)?;
        let connection = execution_runner::helper::Connection::connect(
            execution_runner::host::PeerPolicy {
                images: vec![self.image.clone()],
                subjects: vec![subject.clone()],
                interactive: true,
            },
            subject,
            session,
        )?;
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
    client: Client<S, SystemClock>,
    core: Core,
    bridge: ExecutionBridge<CredentialRedactor>,
    waiting: Option<(Offer, Materials)>,
    next_network: std::time::Instant,
    recovering: bool,
}
impl<S: SecretProvider> DeviceService<S> {
    /// Open an already registered device. Create/open selection never follows an error fallback.
    pub fn open(
        client: Client<S, SystemClock>,
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
        native_process::private_storage::validate(&config.work_root)?;
        let (binding, actor) = plan::context(
            client.configuration().origin.as_str(),
            client.configuration().tenant,
            &client.registration()?,
        )?;
        let materials = MaterialRegistry::new(client.configuration().limits.pending_tasks)?;
        let runner = NativeRunner::with_materials(
            plan::id("native-device-runner")?,
            materials.clone(),
            config.processes,
        )?;
        let host = EnterpriseHost {
            binding,
            actor,
            clock,
            materials,
            subject: execution_runner::host::current_subject()?,
            current: Arc::new(Mutex::new(None)),
            revoked: Arc::new(AtomicBool::new(false)),
        };
        let app = ExecutionApp::start_production(
            journal,
            startup,
            host.clone(),
            runner,
            AppConfig {
                revision: 1,
                max_rules: 1,
                max_profiles: 1,
                max_capability_entries: 32,
                max_timeout_ms: 86_400_000,
                max_output_bytes: 1_048_576,
            },
            plan::storage_limits(),
        )?;
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
                proposal: None,
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
            self.core.reconcile()?;
            return Ok(());
        }
        if self.core.selected.is_some() {
            if let Some((offer, materials)) = self.waiting.take() {
                let selection = self.core.selected.take();
                self.submit(offer, materials, selection, commands).await?;
                self.core.available = None;
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
            self.waiting = None;
            self.core.available = None;
            self.core.proposal = None;
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
                    self.bridge
                        .abandon(&mut self.client, task, &self.core.app)
                        .await?;
                    continue;
                }
                self.bridge
                    .flush(&mut self.client, task, &mut self.core.app, 64)
                    .await?;
                let request = self.client.bound_request(task)?.ok_or(Error::Conflict)?;
                let input = self.core.app.frozen_input(&caller, &request)?;
                if !self.core.host.materials.registered(&input)? {
                    if let Ok(materials) =
                        crate::recovery::materials(&input, &self.core.config, &self.core.helpers)
                    {
                        self.core.host.materials.register(&input, materials)?;
                    }
                }
                match self
                    .bridge
                    .finish(&mut self.client, task, &self.core.app, &caller)
                {
                    Ok(()) => match self.core.host.materials.retire(&input) {
                        Ok(()) | Err(execution_app::Error::Conflict) => (),
                        Err(error) => return Err(error.into()),
                    },
                    Err(Error::Conflict) => (), // Original intent remains unresolved.
                    Err(error) => return Err(error),
                }
            } else if self.recovering
                || !self
                    .waiting
                    .as_ref()
                    .is_some_and(|(offer, _)| offer.task_id() == task)
            {
                // No local intent means no execution may have started. Retire the original
                // remote attempt only after checking this same authoritative journal; a prior
                // local user selection is never reconstructed from transport or request data.
                self.bridge
                    .abandon(&mut self.client, task, &self.core.app)
                    .await?;
            }
        }
        self.recovering = false;
        self.client
            .flush_reports(16.min(self.client.configuration().limits.pending_reports))
            .await?;
        let claim = match network(self.client.claim(), &mut self.core, commands).await {
            Ok(v) => v,
            Err(e) => {
                if e == Error::Identity {
                    self.core.host.revoked.store(true, Ordering::Release);
                    self.core.app.stop_active(128)?;
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
                self.core.proposal = None;
                self.bridge
                    .abandon(&mut self.client, cancel.task_id(), &self.core.app)
                    .await?;
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
            self.submit(offer, materials, None, commands).await?;
            self.core.available = None;
        }
        Ok(())
    }
    async fn submit(
        &mut self,
        offer: Offer,
        materials: Materials,
        selection: Option<Selection>,
        commands: &mut tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<ExecutionStatus, Error> {
        if let Some(selection) = &selection {
            let current = offered(&offer)?;
            if selection.task != current.task
                || selection.attempt != current.attempt
                || selection.revision != current.revision
            {
                return Err(Error::Conflict);
            }
        }
        self.core.available = None;
        let needs_user = match offer.payload() {
            wire::TaskPayload::Enrollment(_) => return Err(Error::Unsupported),
            wire::TaskPayload::Script(p) => p.run_as == wire::ExecutionIdentity::LoggedInUser,
            wire::TaskPayload::Software(p) => p.steps.iter().any(|step| {
                step.action.install.run_as == wire::ExecutionIdentity::LoggedInUser
                    || step.action.uninstall.as_ref().is_some_and(|c| c.run_as == wire::ExecutionIdentity::LoggedInUser)
                    || matches!(&step.action.detect, wire::SoftwareTaskDetection::Script { command } if command.run_as == wire::ExecutionIdentity::LoggedInUser)
            }),
        };
        let delegate = if needs_user {
            Some(self.core.helpers.connect(selection.as_ref())?)
        } else {
            None
        };
        let (preview, artifacts) =
            self.compile(&offer, &materials, offer.payload(), delegate.clone())?;
        artifacts.inspect(&preview)?;
        let start = if selection.is_some() {
            network(
                self.client.start_user_initiated(&offer, &materials),
                &mut self.core,
                commands,
            )
            .await?
        } else {
            network(
                self.client.request_start(&offer, &materials),
                &mut self.core,
                commands,
            )
            .await?
        };
        self.client.validate_start(&start)?;
        let (mut plan, artifacts) = self.compile(&offer, &materials, start.payload(), delegate)?;
        if let Some(selection) = selection {
            let mut input = plan.spec().clone();
            let execution_contract::Initiator::Backend { trigger, .. } =
                &mut input.request.initiator
            else {
                return Err(Error::Untrusted);
            };
            let os_session = execution_contract::OsSessionRef {
                device: self.core.host.binding.device.clone(),
                account: execution_contract::OsAccountRef {
                    platform: plan::platform()?,
                    subject: plan::id(selection.subject)?,
                },
                session: selection.binding,
            };
            *trigger = match selection.origin {
                execution_runner::host::ClientOrigin::Desktop {} => {
                    execution_contract::BackendTrigger::Human { os_session }
                }
                execution_runner::host::ClientOrigin::Ai {
                    config,
                    conversation,
                    tool_call,
                } => execution_contract::BackendTrigger::Ai {
                    os_session,
                    config,
                    conversation,
                    tool_call,
                },
            };
            plan =
                execution_contract::FrozenExecution::freeze(input, &plan::storage_limits().input)
                    .map_err(|_| Error::Untrusted)?;
        }
        self.core.host.materials.register(&plan, artifacts)?;
        *self
            .core
            .host
            .current
            .lock()
            .map_err(|_| Error::Unavailable)? = Some(Arc::new(BackendPermit {
            plan: plan.clone(),
            start: start.clone(),
        }));
        let caller = self.core.caller();
        let result = (|| {
            let prepared =
                self.bridge
                    .prepare(&offer, &materials, &self.core.app, &caller, &plan)?;
            self.bridge.dispatch(
                &mut self.client,
                start,
                &materials,
                &mut self.core.app,
                prepared,
            )
        })();
        *self
            .core
            .host
            .current
            .lock()
            .map_err(|_| Error::Unavailable)? = None;
        result
    }
    fn compile(
        &self,
        offer: &Offer,
        materials: &Materials,
        payload: &wire::TaskPayload,
        delegate: Option<Arc<execution_runner::helper::Connection>>,
    ) -> Result<
        (
            execution_contract::FrozenExecution,
            execution_runner::Artifacts,
        ),
        Error,
    > {
        match payload {
            wire::TaskPayload::Enrollment(_) => Err(Error::Unsupported),
            wire::TaskPayload::Software(payload) => crate::software::compile(
                offer,
                materials,
                payload,
                &self.core.host.binding,
                &self.core.host.actor,
                &self.core.config,
                delegate,
            ),
            wire::TaskPayload::Script(payload) => {
                let work_root = delegate
                    .as_ref()
                    .map_or(&self.core.config.work_root, |h| &h.context().work_root);
                let file = materials.files().first().ok_or(Error::Untrusted)?;
                let content = execution_runner::staging::publish(
                    &self.core.config.material_root,
                    delegate.as_ref().map(|h| h.context().subject.as_str()),
                    file.reader()?,
                    &execution_contract::Digest::new(plan::hex(&payload.content.sha256))
                        .map_err(|_| Error::Protocol)?,
                    payload.content.length,
                )?;
                plan::script(
                    offer,
                    materials,
                    payload,
                    &self.core.host.binding,
                    &self.core.host.actor,
                    &self.core.config.interpreters,
                    work_root,
                    &content,
                    delegate.clone(),
                )
            }
        }
    }
    /// Start the single service owner; native callbacks never access SQLite or network directly.
    pub fn spawn(
        mut self,
        policy: execution_runner::host::PeerPolicy,
        helper_policy: Option<execution_runner::host::PeerPolicy>,
    ) -> Result<ServiceHandle, Error>
    where
        S: Send + 'static,
    {
        policy.validate()?;
        if let Some(policy) = &helper_policy {
            policy.validate()?;
        }
        let (sender, mut commands) = tokio::sync::mpsc::channel(32);
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let (completed, completion) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new().name("rss-device-owner".into()).spawn(move||{
            struct Finished(Arc<AtomicBool>);impl Drop for Finished{fn drop(&mut self){self.0.store(true,Ordering::Release);}}
            let _finished=Finished(done);
            let result=(||->Result<(),Error>{
                let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_|Error::Unavailable)?;
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
            self.core.available = None;
            self.core.selected = None;
            if let Err(error) = self.core.app.stop_active(128) {
                eprintln!("agent_revoke_stop: {error}");
            }
        }
    }
    /// Stop accepted work. Stopping never claims process termination or effect rollback.
    pub fn stop(&mut self) -> Result<(), Error> {
        Ok(self.core.app.stop_active(128)?)
    }
}

struct Selection {
    task: execution_contract::Id,
    attempt: execution_contract::Id,
    revision: execution_contract::Digest,
    subject: String,
    session: u32,
    binding: execution_contract::Id,
    origin: execution_runner::host::ClientOrigin,
}
struct Core {
    app: ExecutionApp<EnterpriseHost, NativeRunner>,
    host: EnterpriseHost,
    config: ExecutionConfig,
    helpers: UserResources,
    available: Option<execution_contract::BackendTask>,
    proposal: Option<Selection>,
    selected: Option<Selection>,
    stopping: bool,
    recovery_cursor: Option<RequestId>,
}
impl Core {
    fn reconcile(&mut self) -> Result<(), Error> {
        let page = self.app.service_tasks(self.recovery_cursor.as_ref(), 64)?;
        for item in &page.items {
            let request = &item.status.operation_request_id;
            let status = self.app.reconcile(request)?;
            if matches!(
                status.phase,
                execution_app::TaskPhase::Verified
                    | execution_app::TaskPhase::Cancelled
                    | execution_app::TaskPhase::FailedBeforeDispatch
            ) {
                let input = self.app.frozen_input(&self.caller(), request)?;
                match self.host.materials.retire(&input) {
                    Ok(()) | Err(execution_app::Error::Conflict) => (),
                    Err(error) => return Err(error.into()),
                }
            }
        }
        self.recovery_cursor = page.next;
        Ok(())
    }
    fn caller(&self) -> RequestContext {
        RequestContext {
            actor: self.host.actor.clone(),
        }
    }
    fn can_read(&self, request: &RequestId, subject: &str) -> Result<bool, Error> {
        use execution_contract::{BackendTrigger, Initiator, RunAs};
        let input = self.app.frozen_input(&self.caller(), request)?;
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
        use execution_runner::host::{Reply, Request};
        if command.reply.is_closed() {
            return;
        }
        let reply = (|| -> Result<Reply, Error> {
            match command.request {
                LocalRequest::Stop => {
                    self.stopping = true;
                    Ok(Reply::Unavailable)
                }
                LocalRequest::Operation(Request::Tasks { after }) => {
                    let mut value = self.app.tasks(&self.caller(), after.as_ref(), 64)?;
                    value.items.retain(|item| {
                        self.can_read(&item.action.request_id, &command.subject)
                            .unwrap_or(false)
                    });
                    Ok(Reply::Tasks {
                        value,
                        available: self.available.iter().cloned().collect(),
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
                        matches!(origin, execution_runner::host::ClientOrigin::Ai { .. });
                    let selection = Selection {
                        task,
                        attempt,
                        revision,
                        subject: command.subject,
                        session: command.session,
                        binding: command.binding.ok_or(Error::Denied)?,
                        origin,
                    };
                    if confirmation_required {
                        self.proposal = Some(selection);
                    } else {
                        self.selected = Some(match self.proposal.take() {
                            Some(proposal)
                                if proposal.task == selection.task
                                    && proposal.attempt == selection.attempt
                                    && proposal.revision == selection.revision
                                    && proposal.subject == selection.subject
                                    && proposal.session == selection.session
                                    && proposal.binding == selection.binding =>
                            {
                                proposal
                            }
                            _ => selection,
                        });
                    }
                    Ok(Reply::Queued {
                        task: offer.task.clone(),
                        attempt: offer.attempt.clone(),
                        request: offer.request.clone(),
                        confirmation_required,
                    })
                }
                LocalRequest::Operation(Request::Status { request }) => {
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    Ok(Reply::Status {
                        value: self.app.status(&self.caller(), &request)?,
                    })
                }
                LocalRequest::Operation(Request::Details { request }) => {
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    Ok(Reply::Details {
                        value: self.app.task_details(&self.caller(), &request)?,
                    })
                }
                LocalRequest::Operation(Request::Cancel { request }) => {
                    if !self.can_read(&request, &command.subject)? {
                        return Err(Error::Denied);
                    }
                    let input = self.app.frozen_input(&self.caller(), &request)?;
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
                        value: self.app.cancel(&self.caller(), &request)?,
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
    Operation(execution_runner::host::Request),
    Stop,
}
struct Command {
    request: LocalRequest,
    subject: String,
    session: u32,
    binding: Option<execution_contract::Id>,
    reply: tokio::sync::oneshot::Sender<execution_runner::host::Reply>,
}
async fn network<T>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    core: &mut Core,
    commands: &mut tokio::sync::mpsc::Receiver<Command>,
) -> Result<T, Error> {
    tokio::pin!(future);
    let mut progress = tokio::time::interval(std::time::Duration::from_millis(50));
    progress.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            result=&mut future=>return result,
            _=progress.tick()=>core.reconcile()?,
            command=commands.recv()=>{
                if let Some(command)=command {core.handle(command);} else {core.stopping=true;}
                if core.stopping{return Err(Error::Unavailable);}
            }
        }
    }
}
/// Native callbacks forward bounded commands to the only journal/network owner.
pub struct ServiceHandle {
    sender: tokio::sync::mpsc::Sender<Command>,
    policy: execution_runner::host::PeerPolicy,
    helper_policy: Option<execution_runner::host::PeerPolicy>,
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
    ) -> execution_runner::host::Reply {
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
            return execution_runner::host::Reply::Unavailable;
        }
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        {
            Ok(v) => v,
            Err(_) => return execution_runner::host::Reply::Unavailable,
        };
        runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(3), receiver)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(execution_runner::host::Reply::Unavailable)
        })
    }
}
impl execution_runner::host::Handler for ServiceHandle {
    fn peer_policy(&self) -> Option<execution_runner::host::PeerPolicy> {
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
        peer: &execution_runner::host::Peer,
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
        peer: &execution_runner::host::Peer,
        request: execution_runner::host::Request,
    ) -> execution_runner::host::Reply {
        match peer.authenticate(&self.policy) {
            Ok(subject) => match peer.session_binding() {
                Ok(binding) => self.call(
                    LocalRequest::Operation(request),
                    subject,
                    peer.session(),
                    Some(binding),
                ),
                Err(_) => execution_runner::host::Reply::Rejected,
            },
            Err(_) => execution_runner::host::Reply::Rejected,
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

fn offered(offer: &Offer) -> Result<execution_contract::BackendTask, Error> {
    use sha2::{Digest as _, Sha256};
    let bytes = serde_json::to_vec(offer.payload()).map_err(|_| Error::Protocol)?;
    Ok(execution_contract::BackendTask {
        request: offer.request_id()?,
        task: plan::id(offer.task_id().to_string())?,
        attempt: plan::id(offer.attempt_id().to_string())?,
        revision: execution_contract::Digest::new(format!("{:x}", Sha256::digest(bytes)))
            .map_err(|_| Error::Protocol)?,
        title: match offer.payload() {
            wire::TaskPayload::Software(p) => p
                .steps
                .last()
                .map(|s| format!("{} {}", s.action.package, s.action.version))
                .ok_or(Error::Protocol)?,
            _ => format!("Task {}", offer.task_id()),
        },
        expires_at: offer.payload().expires_at(),
        user_initiated: matches!(offer.payload(),wire::TaskPayload::Software(p) if p.start_mode==wire::SoftwareStartMode::UserInitiated),
    })
}
