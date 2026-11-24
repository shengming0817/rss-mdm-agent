//! Independent device authentication for the existing macOS extension and registration engine.
//! ref: Apple AuthenticationServices/ASAuthorizationProviderExtensionRegistrationHandler.h
use super::*;
use crate::da_service::{
    mac_accounts::{Employee, Presence, Session, Setup},
    policy::Machine,
};
use crate::deployment::Deployment;
use agent_client::da::{self, PolicyConsumer};
use execution_ipc::device_access::{Operation as AccountOp, Response as AccountReply};
use execution_ipc::host::{Peer, PeerPolicy, SessionFence};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

pub(super) fn mdm_assigned(d: &Deployment) -> Result<bool, Error> {
    if !d.modules.da.enabled() {
        return Ok(true);
    }
    if !d.modules.mdm.enabled() {
        return Ok(false);
    }
    match da::mac_assigned(
        &d.da_root()?,
        d.modules.da.backend()?,
        d.installation_id,
        SystemClock::new()?.now()?,
    ) {
        Ok(None | Some(false)) => Ok(true),
        Ok(Some(true)) => Ok(false),
        Err(e) => Err(e),
    }
}

type Client = da::Client<DeviceSecrets, SystemClock>;

/// Query both issuers for a DA employee accepted by either verified session owner.
/// Pending or rejected requests have no route of their own.
#[derive(Default)]
struct AccountRoutes {
    da: Option<execution_contract::Id>,
    mdm: Option<(u64, execution_contract::Id)>,
    issued: u64,
}
impl AccountRoutes {
    fn targets(&self, generation: &execution_contract::Id) -> bool {
        self.da.as_ref() == Some(generation)
            || self.mdm.as_ref().is_some_and(|(_, g)| g == generation)
    }
    fn employee_route(&self, generation: &execution_contract::Id) -> bool {
        self.da.as_ref() == Some(generation)
    }
    fn reserve(&mut self) -> Result<u64, Error> {
        self.issued = self.issued.checked_add(1).ok_or(Error::Capacity)?;
        Ok(self.issued)
    }
    fn mdm_reply(
        &mut self,
        order: u64,
        generation: &execution_contract::Id,
        reply: &Reply,
    ) -> Result<(), Error> {
        if let Reply::ContextBound { value } = reply {
            if &value.generation != generation {
                return Err(Error::Identity);
            }
            if self
                .mdm
                .as_ref()
                .is_none_or(|(accepted, _)| order > *accepted)
            {
                self.mdm = Some((order, generation.clone()));
            }
        }
        Ok(())
    }
}

/// Logout fences a directory lookup even while its HTTPS response is outstanding.
pub(crate) struct Fence {
    policy: PeerPolicy,
    epoch: Arc<Mutex<u64>>,
    pub accounts: Arc<Setup>,
    active: Mutex<std::sync::Weak<Session>>,
    routes: Mutex<AccountRoutes>,
    other: Option<Arc<dyn SessionFence>>,
}
impl SessionFence for Fence {
    fn revoke(&self, peer: &Peer) -> Reply {
        let other = self.other.as_ref().map(|f| f.revoke(peer));
        if peer.authenticate(&self.policy).is_err() {
            return other.unwrap_or(Reply::Rejected);
        }
        let Ok(mut epoch) = self.epoch.lock() else {
            return Reply::Unavailable;
        };
        let Some(next) = epoch.checked_add(1) else {
            return Reply::Unavailable;
        };
        *epoch = next;
        if let Some(session) = self.active.lock().ok().and_then(|s| s.upgrade()) {
            session.revoke();
        }
        drop(epoch);
        if self.accounts.clear_input().is_err() {
            return Reply::Unavailable;
        }
        Reply::ContextRevoked
    }
}

pub(crate) struct Work {
    command: Command,
    deadline: tokio::time::Instant,
}
pub(crate) struct Pending {
    reply: tokio::sync::oneshot::Receiver<Reply>,
    deadline: tokio::time::Instant,
}
impl Pending {
    pub fn wait(self) -> Reply {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        else {
            return Reply::Unavailable;
        };
        runtime.block_on(async {
            tokio::time::timeout_at(self.deadline, self.reply)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(Reply::Unavailable)
        })
    }
}
pub(crate) struct Endpoint {
    pub fence: Arc<Fence>,
    sender: tokio::sync::mpsc::Sender<Work>,
    deployment: Deployment,
}
impl Endpoint {
    pub fn new(d: &Deployment, other: Option<Arc<dyn SessionFence>>) -> (Self, Worker) {
        let (sender, rx) = tokio::sync::mpsc::channel(16);
        let fence = Arc::new(Fence {
            policy: d.registration_clients.clone(),
            epoch: Arc::new(Mutex::new(0)),
            accounts: Arc::default(),
            active: Mutex::default(),
            routes: Mutex::default(),
            other,
        });
        (
            Self {
                deployment: d.clone(),
                fence: fence.clone(),
                sender,
            },
            Worker {
                rx,
                fence,
                state: State::default(),
                bound: None,
            },
        )
    }
    pub fn policy(&self, policy: &mut PeerPolicy) {
        if let Some(config) = &self.deployment.platform_sso {
            if config.verify().is_ok()
                && !policy.images.iter().any(|i| {
                    i.path == config.extension.path
                        && i.cdhash == config.extension.cdhash
                        && i.sha256 == config.extension.sha256
                })
            {
                policy.images.push(config.extension.clone());
            }
        }
        if let Some(helper) = self.deployment.helper_policy() {
            if !policy.images.iter().any(|i| {
                i.path == self.deployment.service.path
                    && i.cdhash == self.deployment.service.cdhash
                    && i.sha256 == self.deployment.service.sha256
            }) {
                policy.images.push(self.deployment.service.clone());
            }
            for uid in helper.subjects {
                if !policy.subjects.contains(&uid) {
                    policy.subjects.push(uid);
                }
            }
        }
    }
    pub fn helper(&self, peer: &Peer) -> Result<(), execution_app::Error> {
        peer.authenticate(
            &self
                .deployment
                .helper_policy()
                .ok_or(execution_app::Error::Denied)?,
        )?;
        Ok(())
    }
    fn mdm_owns(&self, peer: &Peer, request: &Request) -> Result<bool, Error> {
        let d = &self.deployment;
        if !d.modules.mdm.enabled() {
            return Ok(false);
        }
        let root = crate::access_worker::root(&d.execution);
        // Cleanup belongs to the issuer of the retained version, even after policy withdrawal.
        if let Request::PlatformSso {
            input:
                Operation::Unregister {
                    scope,
                    source,
                    source_version,
                    operation,
                }
                | Operation::Observe {
                    scope,
                    source,
                    source_version,
                    operation,
                    ..
                },
        } = request
        {
            let account = match scope {
                w::Kind::Device => "device".to_owned(),
                w::Kind::User => crate::account_identity::generated_uid(
                    &peer.uid().ok_or(Error::Identity)?.to_string(),
                )?
                .ok_or(Error::Identity)?,
            };
            if let Some(attempt) = attempt_record(&root, *operation)? {
                let c = &attempt.coordinate;
                if c.context.kind != *scope
                    || c.context.generated_uid.as_deref().unwrap_or("device") != account
                    || c.source != *source
                    || c.source_version != *source_version
                    || !supports_owner(
                        d,
                        (
                            c.context.authority,
                            c.context.installation,
                            c.context.tenant,
                        ),
                    )
                {
                    return Err(Error::Identity);
                }
                return Ok(c.context.authority == w::RegistrationTrust::MdmBootstrapPop);
            }
            if let Some(r) = retired_by_source(&root, &account, *source, *source_version)? {
                return Ok(r.coordinate.context.authority == w::RegistrationTrust::MdmBootstrapPop);
            }
            if let Some(c) = find_source(&root, *source, *source_version)? {
                return Ok(c.context.authority == w::RegistrationTrust::MdmBootstrapPop);
            }
        }
        Ok(configured_owner(d)?.0 == w::RegistrationTrust::MdmBootstrapPop)
    }
    pub fn track_mdm(
        &self,
        generation: Option<execution_contract::Id>,
        wait: execution_ipc::host::DeferredReply,
    ) -> execution_ipc::host::DeferredReply {
        let Some(generation) = generation else {
            return wait;
        };
        let fence = self.fence.clone();
        let Ok(order) = fence
            .routes
            .lock()
            .map_err(|_| Error::Unavailable)
            .and_then(|mut routes| routes.reserve())
        else {
            return Box::new(|| Reply::Unavailable);
        };
        Box::new(move || {
            let reply = wait();
            let published = fence
                .routes
                .lock()
                .map_err(|_| Error::Unavailable)
                .and_then(|mut routes| routes.mdm_reply(order, &generation, &reply));
            if published.is_err() {
                Reply::Unavailable
            } else {
                reply
            }
        })
    }
    pub fn start(&self, peer: &Peer, request: &Request) -> Option<Result<Pending, Reply>> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
        let d = &self.deployment;
        if matches!(
            request,
            Request::PlatformSso { .. }
                | Request::DeviceAccess {
                    input: AccountOp::MacStatus {} | AccountOp::OpenApplication { .. },
                    ..
                }
        ) {
            d.platform_sso.as_ref()?;
            match self.mdm_owns(peer, request) {
                Ok(true) => return None,
                Err(_) => return Some(Err(Reply::Rejected)),
                Ok(false) => (),
            }
        }
        let subject = match request {
            Request::PlatformSso { .. } => {
                let config = d.platform_sso.as_ref()?;
                config
                    .verify()
                    .ok()
                    .and_then(|()| peer.authenticate_extension(&config.extension).ok())
            }
            Request::DeviceAccess {
                origin: execution_ipc::host::ClientOrigin::Desktop {},
                input: AccountOp::PrepareMac { task, attempt },
                ..
            } => match self.fence.accounts.owns((*task, *attempt)) {
                Ok(true) => peer.authenticate(&d.registration_clients).ok(),
                Ok(false) => return None,
                Err(_) => return Some(Err(Reply::Unavailable)),
            },
            Request::DeviceAccess {
                origin: execution_ipc::host::ClientOrigin::Desktop {},
                input:
                    AccountOp::MacStatus {}
                    | AccountOp::Tasks {}
                    | AccountOp::OwnBindings {}
                    | AccountOp::OpenApplication { .. },
                ..
            } => peer.authenticate(&d.registration_clients).ok(),
            Request::BindContext { value, .. } => {
                if d.modules.mdm.enabled()
                    && !value.user().is_some_and(|u| {
                        d.modules
                            .employee_portal()
                            .ok()
                            .flatten()
                            .is_some_and(|p| p.accepts(u))
                    })
                {
                    return None;
                }
                peer.authenticate(&d.registration_clients).ok()
            }
            _ => return None,
        };
        let (Some(subject), Ok(binding)) = (subject, peer.session_binding()) else {
            return Some(Err(Reply::Rejected));
        };
        if let Request::DeviceAccess { context, input, .. } = request {
            if d.modules.mdm.enabled()
                && matches!(
                    input,
                    AccountOp::Tasks {}
                        | AccountOp::OwnBindings {}
                        | AccountOp::OpenApplication { .. }
                )
            {
                let tasks = matches!(input, AccountOp::Tasks {});
                let active = match self.fence.routes.lock() {
                    Ok(routes) if tasks => routes.targets(&context.generation),
                    Ok(routes) => routes.employee_route(&context.generation),
                    Err(_) => return Some(Err(Reply::Unavailable)),
                };
                if !active
                    || (!tasks
                        && !crate::da_service::status(d)
                            .ok()
                            .flatten()
                            .is_some_and(|s| s.credential == da::RegistrationState::Registered))
                {
                    // The PSSO owner was already resolved for a launch. No other issuer can sign for it.
                    return if matches!(input, AccountOp::OpenApplication { .. }) {
                        Some(Err(Reply::Unavailable))
                    } else {
                        None
                    };
                }
            }
        }
        let Ok(epoch) = self.fence.epoch.lock().map(|g| *g) else {
            return Some(Err(Reply::Unavailable));
        };
        let (reply, rx) = tokio::sync::oneshot::channel();
        if self
            .sender
            .try_send(Work {
                deadline,
                command: Command {
                    request: LocalRequest::operation(request.clone()),
                    subject,
                    session: peer.session(),
                    binding: Some(binding),
                    fence_epoch: epoch,
                    reply,
                },
            })
            .is_err()
        {
            return Some(Err(Reply::Unavailable));
        }
        Some(Ok(Pending {
            reply: rx,
            deadline,
        }))
    }
}

struct BoundContext {
    current: Arc<Session>,
    principal: Option<Uuid>,
    subject: String,
    session: u32,
    binding: execution_contract::Id,
    generation: execution_contract::Id,
    epoch: u64,
    expires: u64,
}
impl Drop for BoundContext {
    fn drop(&mut self) {
        self.current.revoke();
    }
}
impl BoundContext {
    fn refreshes(
        &self,
        command: &Command,
        value: &execution_ipc::host::NativeContextSession,
        expected: &execution_contract::Id,
        now: u64,
    ) -> bool {
        self.subject == command.subject
            && Some(&self.binding) == command.binding.as_ref()
            && &self.generation == expected
            && &value.generation == expected
            && self.principal == value.user().map(|u| u.principal_id)
            && self.expires > now
            && self.session == command.session
            && self.epoch == command.fence_epoch
    }
    fn permits(
        &self,
        cmd: &Command,
        reference: &execution_contract::NativeContextRef,
        fence: &Fence,
    ) -> bool {
        self.subject == cmd.subject
            && self.session == cmd.session
            && Some(&self.binding) == cmd.binding.as_ref()
            && reference.id == self.generation
            && reference.generation == self.generation
            && crate::wall().is_ok_and(|now| now < self.expires)
            && fence.epoch.lock().is_ok_and(|e| *e == self.epoch)
            && execution_ipc::macos_identity::verify_console(
                &self.subject,
                self.session,
                &self.binding,
            )
            .is_ok()
    }
}
pub(crate) struct Worker {
    pub rx: tokio::sync::mpsc::Receiver<Work>,
    fence: Arc<Fence>,
    state: State,
    bound: Option<BoundContext>,
}
impl Worker {
    pub fn accounts(&self) -> Arc<Setup> {
        self.fence.accounts.clone()
    }
    pub async fn handle(&mut self, d: &Deployment, client: &Client, work: Work) {
        let Work { command, deadline } = work;
        if command.reply.is_closed() {
            return;
        }
        let mut owner = Da {
            d,
            client,
            root: crate::access_worker::root(&d.execution),
            state: &mut self.state,
        };
        let result = tokio::time::timeout_at(deadline, async {
            platform_private_storage::directory(&d.execution.material_root)?;
            platform_private_storage::directory(&owner.root)?;
            match command.request.as_operation() {
                Some(Request::PlatformSso { .. }) => owner
                    .platform_sso(&command)
                    .await
                    .map(|value| Reply::PlatformSso { value }),
                Some(Request::BindContext { value, expected }) => {
                    let now = crate::wall()?;
                    if let (Some(generation), Some(bound)) = (expected, self.bound.as_ref()) {
                        if !bound.refreshes(&command, value, generation, now) {
                            return Err(Error::Identity);
                        }
                    }
                    if expected.as_ref().is_some_and(|g| g != &value.generation) {
                        return Err(Error::Identity);
                    }
                    let same = self.bound.as_ref().is_some_and(|b| {
                        b.refreshes(&command, value, &value.generation, now)
                            && b.current.same(value.credential())
                    });
                    let expires = match owner.bind(&command, value, &self.fence).await {
                        Ok(v) => v,
                        Err(e) => {
                            if same
                                && matches!(
                                    e,
                                    Error::Denied | Error::Expired | Error::CredentialRevoked
                                )
                            {
                                self.bound = None;
                                self.fence.accounts.clear_input()?;
                            }
                            return Err(e);
                        }
                    };
                    let epoch = self.fence.epoch.lock().map_err(|_| Error::Unavailable)?;
                    if *epoch != command.fence_epoch
                        || command.reply.is_closed()
                        || expires <= crate::wall()?
                    {
                        return Err(Error::Identity);
                    }
                    if same {
                        let bound = self.bound.as_mut().ok_or(Error::Identity)?;
                        bound.current.extend(expires)?;
                        bound.expires = expires;
                    } else {
                        let current = Session::new(expires, value.credential().cloned());
                        *self.fence.active.lock().map_err(|_| Error::Unavailable)? =
                            Arc::downgrade(&current);
                        current.observe_deadline();
                        self.bound = Some(BoundContext {
                            current,
                            principal: value.user().map(|u| u.principal_id),
                            subject: command.subject.clone(),
                            session: command.session,
                            binding: command.binding.clone().ok_or(Error::Identity)?,
                            generation: value.generation.clone(),
                            epoch: command.fence_epoch,
                            expires,
                        });
                    }
                    self.fence.routes.lock().map_err(|_| Error::Unavailable)?.da =
                        Some(value.generation.clone());
                    Ok(Reply::ContextBound {
                        value: execution_contract::NativeContextRef {
                            id: value.generation.clone(),
                            generation: value.generation.clone(),
                        },
                    })
                }
                Some(Request::DeviceAccess {
                    context,
                    input: input @ (AccountOp::Tasks {} | AccountOp::PrepareMac { .. }),
                    mac_password,
                    mac_confirmation,
                    ..
                }) => {
                    let bound = self.bound.as_ref().ok_or(Error::Identity)?;
                    if !bound.permits(&command, context, &self.fence) {
                        return Err(Error::Identity);
                    }
                    let mut employee = Employee {
                        portal: d.modules.employee_portal()?.ok_or(Error::Identity)?,
                        principal: bound.principal.ok_or(Error::Identity)?,
                        subject: bound.subject.clone(),
                        session: bound.session,
                        binding: bound.binding.clone(),
                        epoch: self.fence.epoch.clone(),
                        generation: bound.epoch,
                        current: bound.current.clone(),
                        presence: None,
                        console: execution_ipc::macos_identity::verify_console,
                        expires: bound.expires,
                    };
                    let capture = || {
                        let mut pins = d
                            .helper_policy()
                            .filter(|p| p.subjects.contains(&bound.subject))
                            .ok_or(Error::Identity)?;
                        let subject = bound.subject.clone();
                        let session = bound.session;
                        let binding = bound.binding.clone();
                        pins.subjects = vec![subject.clone()];
                        Presence::capture(move || {
                            execution_ipc::helper::presence(&pins, &subject, session, &binding)
                                .map_err(|_| Error::Identity)
                        })
                    };
                    let value = match input {
                        AccountOp::Tasks {} => AccountReply::Tasks {
                            tasks: self.fence.accounts.tasks(&employee, capture)?,
                        },
                        AccountOp::PrepareMac { task, attempt } => {
                            employee.presence = Some(capture()?);
                            self.fence.accounts.prepare(
                                (*task, *attempt),
                                employee,
                                mac_password.as_ref().ok_or(Error::Denied)?,
                                mac_confirmation.as_ref().ok_or(Error::Denied)?,
                            )?;
                            AccountReply::Prepared {}
                        }
                        _ => return Err(Error::Denied),
                    };
                    Ok(Reply::DeviceAccess { value })
                }
                Some(Request::DeviceAccess {
                    context,
                    input: input @ (AccountOp::OwnBindings {} | AccountOp::OpenApplication { .. }),
                    ..
                }) => {
                    let bound = self.bound.as_ref().ok_or(Error::Identity)?;
                    if bound.principal.is_none() || !bound.permits(&command, context, &self.fence) {
                        return Err(Error::Identity);
                    }
                    let proof = bound.current.credential()?;
                    let directory = client.operations()?.employee_bindings(&proof).await?;
                    if !bound.permits(&command, context, &self.fence) {
                        return Err(Error::Identity);
                    }
                    let backend = d.modules.da.backend()?;
                    let employee = rss_mdm_agent_wire::runtime::SelfServiceUser {
                        principal_id: bound.principal.ok_or(Error::Identity)?,
                        tenant_id: backend.tenant,
                        instance_id: backend.instance,
                    };
                    let rows = crate::service::access::own_bindings(
                        directory,
                        &command.subject,
                        &employee,
                    )?;
                    let value = if let AccountOp::OpenApplication { binding } = input {
                        let current = bound.current.clone();
                        let epoch = self.fence.epoch.clone();
                        let expected = bound.epoch;
                        super::broker::select(
                            rows,
                            *binding,
                            crate::service::login_binding_id(&bound.binding),
                            &employee,
                            move || {
                                current.credential().is_ok()
                                    && epoch.lock().is_ok_and(|v| *v == expected)
                            },
                        )?
                    } else {
                        rows
                    };
                    Ok(Reply::DeviceAccess { value })
                }
                Some(Request::DeviceAccess {
                    context,
                    input: execution_ipc::device_access::Operation::MacStatus {},
                    ..
                }) => {
                    let e = self
                        .bound
                        .as_ref()
                        .filter(|c| c.principal.is_some())
                        .ok_or(Error::Identity)?;
                    if !e.permits(&command, context, &self.fence) {
                        return Err(Error::Identity);
                    }
                    let mut status = owner
                        .mac_status(
                            &command,
                            d.platform_sso.as_ref().is_some_and(|c| c.verify().is_ok()),
                        )
                        .await?;
                    if let Some(mut pins) = d
                        .helper_policy()
                        .filter(|p| p.subjects.contains(&e.subject))
                    {
                        let (subject, session, binding) =
                            (e.subject.clone(), e.session, e.binding.clone());
                        pins.subjects = vec![subject.clone()];
                        let sample = tokio::task::spawn_blocking(move || {
                            execution_ipc::helper::keychain_until(
                                &pins,
                                &subject,
                                session,
                                &binding,
                                deadline.into_std(),
                            )
                        });
                        if let Ok(Ok(v)) = sample.await {
                            status.login_keychain = v;
                        }
                    }
                    if !e.permits(&command, context, &self.fence) {
                        return Err(Error::Identity);
                    }
                    Ok(Reply::DeviceAccess {
                        value: execution_ipc::device_access::Response::MacStatus(status),
                    })
                }
                _ => Err(Error::Denied),
            }
        })
        .await
        .unwrap_or(Err(Error::Unavailable));
        let _ = command.reply.send(match result {
            Ok(reply) => reply,
            Err(Error::Unavailable | Error::Network | Error::Storage) => Reply::Unavailable,
            Err(_) => Reply::Rejected,
        });
    }
    pub async fn during<T>(
        &mut self,
        d: &Deployment,
        client: &Client,
        future: impl std::future::Future<Output = T>,
    ) -> T {
        tokio::pin!(future);
        loop {
            tokio::select! {biased;
                result=&mut future=>return result,
                Some(cmd)=self.rx.recv()=>self.handle(d,client,cmd).await,
            }
        }
    }
    pub async fn maintenance(
        &mut self,
        d: &Deployment,
        client: &Client,
        retirement: &mut State,
    ) -> Result<(), Error> {
        let root = crate::access_worker::root(&d.execution);
        if let Some(r) = self.during(d, client, client.revocation()).await? {
            self.bound = None;
            self.fence.accounts.reset()?;
            withdraw_sources(&root, &r)?;
            return client.retire(&r);
        }
        let mut owner = Da {
            d,
            client,
            root,
            state: retirement,
        };
        self.during(d, client, owner.retry_retirement()).await;
        Ok(())
    }
}
pub(super) fn withdraw_sources(
    root: &Path,
    r: &rss_mdm_agent_wire::da::RevocationReceipt,
) -> Result<(), Error> {
    if !root.try_exists()? {
        return Ok(());
    }
    {
        let _lock = state::lock(root)?;
        match state::read_optional::<rss_mdm_agent_wire::da::RevocationReceipt>(
            root,
            DA_WITHDRAWAL,
        )? {
            Some(held) if held != *r => return Err(Error::Conflict),
            Some(_) => (),
            None => state::write(root, DA_WITHDRAWAL, r)?,
        }
    }
    if has_authority(root, w::RegistrationTrust::DaDevicePop)? {
        // Persist the exact withdrawal separately from source revocation before releasing the device key.
        let mut sources = Vec::new();
        visit_live_sources(root, |c| {
            if (
                c.context.authority,
                c.context.installation,
                c.context.tenant,
                c.context.device_id,
            ) != (
                w::RegistrationTrust::DaDevicePop,
                r.service_id,
                r.tenant,
                r.device,
            ) {
                return Err(Error::Conflict);
            }
            if !sources.contains(&c) {
                sources.push(c);
            }
            Ok(())
        })?;
        for c in sources {
            if resolution(root, &c)?.is_none() {
                let c = queue_retirement(root, c)?;
                finish_retirement(
                    root,
                    &c,
                    RemoteRetirement::RegistrationWithdrawn {
                        operation: r.operation_id,
                        credential: r.credential,
                    },
                )?;
            }
        }
        if !sweep_retirement(root, r.service_id, Some(r))? {
            return Err(Error::Unavailable);
        }
    }
    let _lock = state::lock(root)?;
    let held =
        state::read_optional::<rss_mdm_agent_wire::da::RevocationReceipt>(root, DA_WITHDRAWAL)?;
    if held.as_ref().is_some_and(|other| other != r) {
        return Err(Error::Conflict);
    }
    settle_prepared(root, r.service_id, Some(r))?;
    release_bindings(root, r.service_id, Some(r))?;
    if held.is_some() {
        state::remove(root, DA_WITHDRAWAL)?;
    }
    Ok(())
}
struct Da<'a> {
    d: &'a Deployment,
    client: &'a Client,
    root: PathBuf,
    state: &'a mut State,
}
impl Da<'_> {
    async fn selected(&self) -> Result<da::Selection, Error> {
        let root = self.d.da_root()?;
        let machine = Machine::observe(root).await;
        let selected = self
            .client
            .platform_configuration(&machine.managed_snapshot())?;
        let r = self.client.operations()?;
        let r = r.registration();
        let config = self.d.platform_sso.as_ref().ok_or(Error::Unsupported)?;
        let mac = selected.native.mac.as_ref().ok_or(Error::Unsupported)?;
        if !machine.matches_platform(&selected)?
            || (
                selected.identity.service_id,
                selected.identity.tenant,
                selected.identity.device_id,
            ) != (r.service_id, r.tenant, r.device)
            || mac.origin != config.origin
            || mac.deployment.extension_id != "com.rss.mdmagent.psso"
            || mac.deployment.application_id != "com.rss.mdmagent"
        {
            return Err(Error::Identity);
        }
        Ok(selected)
    }
    async fn bind(
        &mut self,
        command: &Command,
        value: &execution_ipc::host::NativeContextSession,
        fence: &Fence,
    ) -> Result<u64, Error> {
        let binding = command.binding.as_ref().ok_or(Error::Denied)?;
        let verify = || {
            execution_ipc::macos_identity::verify_console(
                &command.subject,
                command.session,
                binding,
            )
            .map_err(|_| Error::Identity)
        };
        verify()?;
        let Some(employee) = value.user() else {
            return Ok(value.expires_at_unix_ms);
        };
        let issuer = self
            .d
            .modules
            .employee_portal()?
            .ok_or(Error::Configuration)?;
        if !issuer.accepts(employee) || value.expires_at_unix_ms <= crate::wall()? {
            return Err(Error::Identity);
        }
        let proof = value.credential().ok_or(Error::Identity)?;
        let (expires, psso) = tokio::join!(
            da::verify_employee(
                &issuer.origin,
                issuer.ca_pem.as_deref(),
                issuer.tenant,
                employee.principal_id,
                proof
            ),
            tokio::time::timeout(Duration::from_secs(2), async {
                if self.d.platform_sso.is_none() || mdm_assigned(self.d)? {
                    return Ok(None);
                }
                let operations = self.client.operations()?;
                let (directory, selected) =
                    tokio::try_join!(operations.employee_bindings(proof), self.selected())?;
                Ok::<_, Error>(Some((directory, selected)))
            })
        );
        let expires = expires?;
        let epoch = fence.epoch.lock().map_err(|_| Error::Unavailable)?;
        let recheck = || {
            verify()?;
            if *epoch != command.fence_epoch
                || command.reply.is_closed()
                || value.expires_at_unix_ms <= crate::wall()?
                || expires <= (crate::wall()? / 1000) as i64
            {
                return Err(Error::Identity);
            }
            Ok(())
        };
        recheck()?;
        // Existing-account PSSO materializes only from its own verified directory and policy.
        // A valid employee session is independently useful to an administrator-assigned Create.
        if let Ok(Ok(Some((directory, selected)))) = psso {
            let account =
                crate::account_identity::generated_uid(&command.subject)?.ok_or(Error::Identity)?;
            let uid = Uuid::parse_str(&account).map_err(|_| Error::Identity)?;
            match admit_existing(
                &self.root,
                account,
                uid,
                &selected.identity,
                &selected.settings,
                &directory,
                recheck,
            ) {
                Ok(()) | Err(Error::NotFound) => (),
                Err(e) => eprintln!("device_access: existing Mac binding: {e}"),
            }
        }
        Ok(value
            .expires_at_unix_ms
            .min((expires as u64).saturating_mul(1000)))
    }
}
fn admit_existing(
    root: &Path,
    account: String,
    uid: Uuid,
    id: &rss_mdm_agent_wire::da::ReportIdentity,
    settings: &wire::AccessConfiguration,
    directory: &wire::AccessDirectory,
    recheck: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    if directory.device_id != id.device_id {
        return Err(Error::Identity);
    }
    let _lock = state::lock(root)?;
    let old = state::read_optional::<::platform_sso::LocalBinding>(root, &binding_name(&account)?)?;
    if let Some(mut old) = old.filter(|b| {
        b.authority == w::RegistrationTrust::DaDevicePop
            && b.installation == id.service_id
            && b.tenant == id.tenant
            && b.device_id == id.device_id
    }) {
        if let Some(revoked) = directory.bindings.iter().find(|b| {
            b.id == old.binding
                && b.revoked
                && b.revision >= old.revision
                && b.binding_generation >= old.binding_generation
        }) {
            old.enabled = false;
            old.revision = revoked.revision;
            old.definition_digest = revoked.definition_digest;
            old.binding_generation = revoked.binding_generation;
            save_binding(root, &old)?;
        }
    }
    let mut bindings = directory.bindings.iter().filter(|b| {
        !b.revoked && b.account == Some(wire::AccessAccount::Macos { generated_uid: uid })
    });
    let found = bindings.next().ok_or(Error::NotFound)?;
    if bindings.next().is_some()
        || found.configuration.mac != settings.mac
        || !found.configuration.enabled
        || found.binding_generation < 1
        || found.revision < 1
        || found.id.is_nil()
    {
        return Err(Error::Conflict);
    }
    recheck()?;
    save_binding(
        root,
        &::platform_sso::LocalBinding {
            definition_digest: found.definition_digest,
            required_role: found.configuration.role,
            authority: w::RegistrationTrust::DaDevicePop,
            installation: id.service_id,
            tenant: id.tenant,
            device_id: id.device_id,
            binding: found.id,
            binding_generation: found.binding_generation,
            revision: found.revision,
            generated_uid: account,
            enabled: true,
            configuration: found.configuration.mac.clone().ok_or(Error::Unsupported)?,
        },
    )
}
impl Owner for Da<'_> {
    fn root(&self) -> &Path {
        &self.root
    }
    fn state(&mut self) -> &mut State {
        self.state
    }
    fn identity(&self) -> Result<Identity, Error> {
        let ops = self.client.operations()?;
        let r = ops.registration();
        Ok((
            w::RegistrationTrust::DaDevicePop,
            r.service_id,
            r.tenant,
            r.device,
        ))
    }
    fn identity_hint(&self) -> Result<(w::RegistrationTrust, Uuid, Uuid), Error> {
        let b = self.d.modules.da.backend()?;
        Ok((w::RegistrationTrust::DaDevicePop, b.instance, b.tenant))
    }
    async fn context(
        &self,
        cmd: &Command,
        kind: w::Kind,
        require_enabled: bool,
        mode: Option<w::AuthenticationMode>,
    ) -> Result<::platform_sso::Context, Error> {
        if !require_enabled {
            if let Some(Request::PlatformSso {
                input:
                    Operation::Unregister {
                        source,
                        source_version,
                        ..
                    }
                    | Operation::Observe {
                        source,
                        source_version,
                        ..
                    },
            }) = cmd.request.as_operation()
            {
                let c =
                    find_source(&self.root, *source, *source_version)?.ok_or(Error::NotFound)?;
                let backend = self.d.modules.da.backend()?;
                if c.context.authority != w::RegistrationTrust::DaDevicePop
                    || c.context.tenant != backend.tenant
                    || c.context.installation != backend.instance
                    || c.context.kind != kind
                {
                    return Err(Error::Identity);
                }
                if kind == w::Kind::User
                    && (crate::account_identity::generated_uid(&cmd.subject)?
                        != c.context.generated_uid
                        || cmd.session == 0)
                {
                    return Err(Error::Identity);
                }
                return Ok(c.context);
            }
        }
        let (authority, installation, tenant, device_id) = self.identity()?;
        let selected = if require_enabled {
            enabled(&self.root, authority, installation)?;
            Some(self.selected().await?)
        } else {
            None
        };
        let c = complete_context(
            &self.root,
            cmd,
            ::platform_sso::Context {
                authority,
                installation,
                tenant,
                device_id,
                origin: self
                    .d
                    .platform_sso
                    .as_ref()
                    .ok_or(Error::Unsupported)?
                    .origin
                    .clone(),
                kind,
                binding: None,
                binding_generation: None,
                binding_revision: 0,
                generated_uid: None,
                configuration: None,
            },
            require_enabled,
        )?;
        if let (Some(target), Some(expected)) = (&selected, mode) {
            if target.settings.mac.as_ref().map(|v| v.authentication) != Some(expected) {
                return Err(Error::Untrusted);
            }
        }
        if kind == w::Kind::User && selected.is_some_and(|s| s.settings.mac != c.configuration) {
            return Err(Error::Untrusted);
        }
        Ok(c)
    }
    async fn bootstrap(&mut self, input: &w::Bootstrap) -> Result<w::BootstrapResponse, Error> {
        self.client.operations()?.psso_bootstrap(input).await
    }
    async fn register(
        &mut self,
        input: &w::Registration,
    ) -> Result<Option<w::RegistrationReceipt>, Error> {
        self.client.operations()?.psso_register(input).await
    }
    async fn status(&mut self, id: Option<Uuid>) -> Result<w::Status, Error> {
        self.client.operations()?.psso_status(id).await
    }
    async fn revoke(&mut self, id: Uuid, version: i64) -> Result<w::RevocationReceipt, Error> {
        self.client.operations()?.psso_revoke(id, version).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn component_and_callbacks_follow_retained_authority_on_mixed_installations() {
        use crate::installation::Module;
        let temp = crate::protocol_test_support::Root::new();
        let mut d = crate::deployment::tests::deployment();
        d.state_root = temp.path.join("state");
        d.execution.material_root = temp.path.join("materials");
        let mut c = super::super::tests::coordinate();
        let mdm = d.mdm().unwrap();
        c.context.installation = mdm.instance;
        c.context.tenant = mdm.tenant;
        let da = da::Backend {
            origin: "https://da.example".into(),
            browser: None,
            ca_pem: mdm.ca_pem.clone(),
            tenant: Uuid::new_v4(),
            instance: Uuid::new_v4(),
        };
        d.modules.da = Module::Enabled {
            backend: da.clone(),
        };
        let root = crate::access_worker::root(&d.execution);
        assert_eq!(
            configured_owner(&d).unwrap().0,
            w::RegistrationTrust::MdmBootstrapPop
        );
        component_state(&d, "inspect").unwrap();
        assert!(!root.exists());
        component_state(&d, "disable").unwrap();
        assert!(matches!(
            enabled(&root, c.context.authority, c.context.installation),
            Err(Error::Denied)
        ));
        state::write(&root, &c.file("source").unwrap(), &c).unwrap();
        assert_eq!(
            configured_owner(&d).unwrap().0,
            w::RegistrationTrust::MdmBootstrapPop
        );
        let mut other = c.clone();
        other.context.authority = w::RegistrationTrust::DaDevicePop;
        other.context.tenant = da.tenant;
        other.context.installation = da.instance;
        state::write(&root, &other.file("registering").unwrap(), &other).unwrap();
        assert!(matches!(configured_owner(&d), Err(Error::Conflict)));
        state::remove(&root, &c.file("source").unwrap()).unwrap();
        assert_eq!(
            configured_owner(&d).unwrap().0,
            w::RegistrationTrust::DaDevicePop
        );
        state::remove(&root, &other.file("registering").unwrap()).unwrap();
        let mut local = super::super::tests::binding();
        local.authority = other.context.authority;
        local.installation = da.instance;
        local.tenant = da.tenant;
        local.device_id = other.context.device_id;
        save_binding(&root, &local).unwrap();
        let withdrawal = rss_mdm_agent_wire::da::RevocationReceipt {
            service_id: da.instance,
            tenant: da.tenant,
            device: local.device_id,
            credential: Uuid::new_v4(),
            operation_id: Uuid::new_v4(),
        };
        withdraw_sources(&root, &withdrawal).unwrap();
        assert_eq!(
            configured_owner(&d).unwrap().0,
            w::RegistrationTrust::MdmBootstrapPop
        );
        d.modules.mdm = Module::Disabled;
        assert_eq!(
            configured_owner(&d).unwrap().0,
            w::RegistrationTrust::DaDevicePop
        );
    }
    #[test]
    fn explicit_disable_survives_da_withdrawal_and_replay_for_the_same_instance() {
        use crate::installation::Module;
        let temp = crate::protocol_test_support::Root::new();
        let mut d = crate::deployment::tests::deployment();
        d.state_root = temp.path.join("state");
        d.execution.material_root = temp.path.join("materials");
        let c = super::super::tests::coordinate();
        let r = rss_mdm_agent_wire::da::RevocationReceipt {
            service_id: c.context.installation,
            tenant: c.context.tenant,
            device: c.context.device_id,
            credential: Uuid::new_v4(),
            operation_id: Uuid::new_v4(),
        };
        d.modules.mdm = Module::Disabled;
        d.modules.da = Module::Enabled {
            backend: da::Backend {
                origin: "https://da.example".into(),
                browser: None,
                ca_pem: String::new(),
                tenant: r.tenant,
                instance: r.service_id,
            },
        };
        component_state(&d, "disable").unwrap();
        let root = crate::access_worker::root(&d.execution);
        let owner = configured_owner(&d).unwrap();
        // Administrative disable alone, then a restart with both independent records present.
        for interrupted in [false, true] {
            if interrupted {
                state::write(&root, DA_WITHDRAWAL, &r).unwrap();
            }
            withdraw_sources(&root, &r).unwrap();
            withdraw_sources(&root, &r).unwrap();
            assert!(root.join("platform-sso.disabled").exists());
            assert!(!root.join(DA_WITHDRAWAL).exists());
            assert_eq!(enabled(&root, owner.0, owner.1), Err(Error::Denied));
        }
    }
    #[test]
    fn rejected_and_late_refreshes_preserve_mdm_confirmed_da_recovery_route() {
        let mut routes = AccountRoutes::default();
        let old = execution_contract::Id::new("previous").unwrap();
        let current = execution_contract::Id::new("mdm-active-da-recovering").unwrap();
        let bound = |generation: &execution_contract::Id| Reply::ContextBound {
            value: execution_contract::NativeContextRef {
                id: generation.clone(),
                generation: generation.clone(),
            },
        };
        let delayed = routes.reserve().unwrap();
        let accepted = routes.reserve().unwrap();
        assert!(!routes.targets(&current));
        routes
            .mdm_reply(accepted, &current, &bound(&current))
            .unwrap();
        let rejected = routes.reserve().unwrap();
        routes.mdm_reply(rejected, &old, &Reply::Rejected).unwrap();
        routes.mdm_reply(delayed, &old, &bound(&old)).unwrap();
        assert!(routes.targets(&current));
        assert!(!routes.targets(&old));
        assert!(!routes.employee_route(&current));
        assert!(routes.mdm_reply(rejected, &old, &bound(&current)).is_err());
        assert!(routes.targets(&current));
        routes.da = Some(old.clone());
        assert!(routes.targets(&current) && routes.targets(&old));
        assert!(routes.employee_route(&old) && !routes.employee_route(&current));
    }

    #[test]
    fn device_context_refresh_is_fenced_by_logout_and_session_change() {
        let id = execution_contract::Id::new("generation").unwrap();
        let binding = execution_contract::Id::new("session-binding").unwrap();
        let value = execution_ipc::host::NativeContextSession::device(id.clone());
        let (reply, _rx) = tokio::sync::oneshot::channel();
        let mut cmd = Command {
            request: LocalRequest::operation(Request::BindContext {
                value: value.clone(),
                expected: Some(id.clone()),
            }),
            subject: "501".into(),
            session: 7,
            binding: Some(binding.clone()),
            reply,
            fence_epoch: 1,
        };
        let bound = BoundContext {
            current: Session::new(99, None),
            principal: None,
            subject: cmd.subject.clone(),
            session: cmd.session,
            binding,
            generation: id.clone(),
            epoch: cmd.fence_epoch,
            expires: 99,
        };
        assert!(bound.refreshes(&cmd, &value, &id, 98));
        assert!(!bound.refreshes(&cmd, &value, &id, 99));
        cmd.fence_epoch += 1;
        assert!(!bound.refreshes(&cmd, &value, &id, 98));
        cmd.fence_epoch -= 1;
        cmd.session += 1;
        assert!(!bound.refreshes(&cmd, &value, &id, 98));
    }
    #[tokio::test]
    async fn both_native_modes_require_one_live_binding_and_a_current_console() {
        let id = rss_mdm_agent_wire::da::ReportIdentity {
            service_id: Uuid::new_v4(),
            tenant: Uuid::new_v4(),
            device_id: rss_mdm_agent_wire::DeviceId::new_v4(),
        };
        let uid = Uuid::new_v4();
        for mode in [
            w::AuthenticationMode::SecureEnclave,
            w::AuthenticationMode::Password,
        ] {
            let root = crate::protocol_test_support::Root::new();
            let mut config = w::LoginConfiguration {
                authentication: mode,
                ..Default::default()
            };
            if mode == w::AuthenticationMode::Password {
                config.password_provider = Some(Uuid::new_v4());
            }
            let settings = wire::AccessConfiguration {
                windows: wire::AccessWindowsConfiguration::Disabled,
                mac: Some(config),
                role: None,
                self_enrollment: false,
                enabled: true,
            };
            let binding = wire::AccessBindingStatus {
                operation_id: Uuid::new_v4(),
                definition_digest: [4; 32],
                account_disposition: wire::AccessAccountDisposition::Keep,
                id: Uuid::new_v4(),
                binding_generation: 1,
                factor_generation: 1,
                revision: 1,
                revoked: false,
                account: Some(wire::AccessAccount::Macos { generated_uid: uid }),
                configuration: settings.clone(),
                received_revision: None,
                applied_revision: None,
                effect: None,
            };
            let mut directory = wire::AccessDirectory {
                device_id: id.device_id,
                policies: vec![],
                next_cursor: None,
                bindings: vec![binding.clone()],
            };
            let admit = |d: &wire::AccessDirectory, current: bool| {
                admit_existing(&root.path, uid.to_string(), uid, &id, &settings, d, || {
                    if current {
                        Ok(())
                    } else {
                        Err(Error::Identity)
                    }
                })
            };
            assert_eq!(admit(&directory, false), Err(Error::Identity));
            directory.bindings.push(binding.clone());
            assert_eq!(admit(&directory, true), Err(Error::Conflict));
            directory.bindings.pop();
            admit(&directory, true).unwrap();
            let name = binding_name(&uid.to_string()).unwrap();
            let saved: ::platform_sso::LocalBinding =
                state::read_optional(&root.path, &name).unwrap().unwrap();
            assert_eq!(
                (saved.binding, saved.configuration.authentication),
                (binding.id, mode)
            );
            super::super::tests::authorization::register_saved_account(&root.path, &saved).await;
            directory.bindings[0].revoked = true;
            directory.bindings[0].revision += 1;
            assert_eq!(admit(&directory, true), Err(Error::Unavailable));
            assert!(
                !state::read_optional::<::platform_sso::LocalBinding>(&root.path, &name)
                    .unwrap()
                    .unwrap()
                    .enabled
            );
        }
    }
}
