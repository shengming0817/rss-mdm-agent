//! Independent device authentication for the existing macOS extension and registration engine.
//! ref: Apple AuthenticationServices/ASAuthorizationProviderExtensionRegistrationHandler.h
use super::*;
use crate::da_service::policy::Machine;
use crate::deployment::Deployment;
use agent_client::da::{self, PolicyConsumer};
use execution_ipc::host::{Peer, PeerPolicy, SessionFence};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

type Client = da::Client<DeviceSecrets, SystemClock>;

/// Logout fences a directory lookup even while its HTTPS response is outstanding.
pub(crate) struct Fence {
    policy: PeerPolicy,
    epoch: Mutex<u64>,
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
        Reply::ContextRevoked
    }
}

pub(crate) struct Endpoint {
    pub fence: Arc<Fence>,
    sender: tokio::sync::mpsc::Sender<Command>,
    deployment: Deployment,
}
impl Endpoint {
    pub fn new(d: &Deployment, other: Option<Arc<dyn SessionFence>>) -> (Self, Worker) {
        let (sender, rx) = tokio::sync::mpsc::channel(16);
        let fence = Arc::new(Fence {
            policy: d.registration_clients.clone(),
            epoch: Mutex::new(0),
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
                employee: None,
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
    fn mdm_owns(&self, request: &Request) -> Result<bool, Error> {
        let d = &self.deployment;
        if !d.modules.mdm.enabled() {
            return Ok(false);
        }
        let root = crate::access_worker::root(&d.execution);
        // Cleanup belongs to the issuer of the retained version, even after policy withdrawal.
        if let Request::PlatformSso {
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
        } = request
        {
            if let Some(c) = find_source(&root, *source, *source_version)? {
                return Ok(c.context.authority == w::RegistrationTrust::MdmBootstrapPop);
            }
        }
        let backend = d.modules.da.backend()?;
        let channel = d.da_root()?;
        let Some(status) = agent_client::da::inspect(
            &channel,
            backend,
            d.installation_id,
            SystemClock::new()?.now()?,
        )?
        else {
            // A combined package can have only its MDM registration provisioned.
            return Ok(!has_authority(&root, w::RegistrationTrust::DaDevicePop)?);
        };
        if status.valid_until.is_none() || status.effective_digest.is_none() {
            return Ok(false);
        }
        let selection =
            state::read_optional::<Option<da::Selection>>(&channel, "selected-configuration.json")?
                .flatten();
        Ok(selection.is_some_and(|s| {
            Some(s.identity.device_id) == status.registration.as_ref().map(|r| r.device)
                && s.identity.tenant == backend.tenant
                && s.identity.service_id == backend.instance
                && s.settings.mac.is_none()
        }) && !has_authority(&root, w::RegistrationTrust::DaDevicePop)?)
    }
    pub fn handle(&self, peer: &Peer, request: &Request) -> Option<Reply> {
        let d = &self.deployment;
        let config = d.platform_sso.as_ref()?;
        if matches!(
            request,
            Request::PlatformSso { .. }
                | Request::BindContext { .. }
                | Request::DeviceAccess {
                    input: execution_ipc::device_access::Operation::MacStatus {},
                    ..
                }
        ) {
            match self.mdm_owns(request) {
                Ok(true) => return None,
                Err(_) => return Some(Reply::Rejected),
                Ok(false) => (),
            }
        }
        let subject = match request {
            Request::PlatformSso { .. } => config
                .verify()
                .ok()
                .and_then(|()| peer.authenticate_extension(&config.extension).ok()),
            Request::DeviceAccess {
                origin: execution_ipc::host::ClientOrigin::Desktop {},
                input: execution_ipc::device_access::Operation::MacStatus {},
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
            return Some(Reply::Rejected);
        };
        let Ok(epoch) = self.fence.epoch.lock().map(|g| *g) else {
            return Some(Reply::Unavailable);
        };
        let (reply, rx) = tokio::sync::oneshot::channel();
        if self
            .sender
            .try_send(Command {
                request: LocalRequest::operation(request.clone()),
                subject,
                session: peer.session(),
                binding: Some(binding),
                fence_epoch: epoch,
                reply,
            })
            .is_err()
        {
            return Some(Reply::Unavailable);
        }
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        else {
            return Some(Reply::Unavailable);
        };
        let result = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(3), rx)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(Reply::Unavailable)
        });
        Some(result)
    }
}

struct BoundEmployee {
    subject: String,
    session: u32,
    binding: execution_contract::Id,
    generation: execution_contract::Id,
    epoch: u64,
    expires: u64,
}
impl BoundEmployee {
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
    pub rx: tokio::sync::mpsc::Receiver<Command>,
    fence: Arc<Fence>,
    state: State,
    employee: Option<BoundEmployee>,
}
impl Worker {
    pub async fn handle(&mut self, d: &Deployment, client: &Client, command: Command) {
        if command.reply.is_closed() {
            return;
        }
        let mut owner = Da {
            d,
            client,
            root: crate::access_worker::root(&d.execution),
            state: &mut self.state,
        };
        let result = async {
            platform_private_storage::directory(&d.execution.material_root)?;
            platform_private_storage::directory(&owner.root)?;
            match command.request.as_operation() {
                Some(Request::PlatformSso { .. }) => owner
                    .platform_sso(&command)
                    .await
                    .map(|value| Reply::PlatformSso { value }),
                Some(Request::BindContext { value, expected }) => {
                    if expected.as_ref().is_some_and(|id| {
                        !self.employee.as_ref().is_some_and(|e| {
                            e.subject == command.subject
                                && Some(&e.binding) == command.binding.as_ref()
                                && &e.generation == id
                                && &value.generation == id
                                && e.epoch == command.fence_epoch
                        })
                    }) {
                        return Err(Error::Identity);
                    }
                    self.employee = None;
                    let expires = owner.bind(&command, value, &self.fence).await?;
                    if value.user().is_some() {
                        self.employee = Some(BoundEmployee {
                            subject: command.subject.clone(),
                            session: command.session,
                            binding: command.binding.clone().ok_or(Error::Identity)?,
                            generation: value.generation.clone(),
                            epoch: command.fence_epoch,
                            expires,
                        });
                    }
                    Ok(Reply::ContextBound {
                        value: execution_contract::NativeContextRef {
                            id: value.generation.clone(),
                            generation: value.generation.clone(),
                        },
                    })
                }
                Some(Request::DeviceAccess {
                    context,
                    input: execution_ipc::device_access::Operation::MacStatus {},
                    ..
                }) => {
                    let e = self.employee.as_ref().ok_or(Error::Identity)?;
                    if !e.permits(&command, context, &self.fence) {
                        return Err(Error::Identity);
                    }
                    let mut status = owner
                        .mac_status(
                            &command,
                            d.platform_sso.as_ref().is_some_and(|c| c.verify().is_ok()),
                        )
                        .await?;
                    if let Ok(peer) = d.helpers().connect_peer(&e.subject, e.session, &e.binding) {
                        if let Ok(execution_ipc::helper::Reply::MacKeychain(v)) =
                            peer.exchange(execution_ipc::helper::Command::MacKeychain)
                        {
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
        }
        .await;
        let _ = command.reply.send(match result {
            Ok(reply) => reply,
            Err(Error::Unavailable | Error::Network | Error::Storage) => Reply::Unavailable,
            Err(_) => Reply::Rejected,
        });
    }
    pub async fn maintenance(&mut self, d: &Deployment, client: &Client) -> Result<(), Error> {
        let root = crate::access_worker::root(&d.execution);
        if let Some(r) = client.revocation().await? {
            if has_authority(&root, w::RegistrationTrust::DaDevicePop)? {
                // Persist the exact withdrawal separately from source revocation before releasing the device key.
                let mut sources = Vec::new();
                visit_live_sources(&root, |c| {
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
                    if resolution(&root, &c)?.is_none() {
                        let c = queue_retirement(&root, c)?;
                        finish_retirement(
                            &root,
                            &c,
                            RemoteRetirement::RegistrationWithdrawn {
                                operation: r.operation_id,
                                credential: r.credential,
                            },
                        )?;
                    }
                }
                if !sweep_retirement(&root, r.service_id)? {
                    return Err(Error::Unavailable);
                }
            }
            return client.retire(&r);
        }
        Da {
            d,
            client,
            root,
            state: &mut self.state,
        }
        .retry_retirement()
        .await;
        Ok(())
    }
}
struct Da<'a> {
    d: &'a Deployment,
    client: &'a Client,
    root: PathBuf,
    state: &'a mut State,
}
impl Da<'_> {
    async fn selected(&self) -> Result<da::Selection, Error> {
        let machine = Machine::observe(self.d.da_root()?).await;
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
        let Some(employee) = value.user() else {
            return Ok(value.expires_at_unix_ms);
        };
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
        let issuer = self
            .d
            .modules
            .employee_portal()?
            .ok_or(Error::Configuration)?;
        if !issuer.accepts(employee) || value.expires_at_unix_ms <= crate::wall()? {
            return Err(Error::Identity);
        }
        let proof = value.credential().ok_or(Error::Identity)?;
        let expires = da::verify_employee(
            &issuer.origin,
            issuer.ca_pem.as_deref(),
            issuer.tenant,
            employee.principal_id,
            proof,
        )
        .await?;
        let account =
            crate::account_identity::generated_uid(&command.subject)?.ok_or(Error::Identity)?;
        let uid = Uuid::parse_str(&account).map_err(|_| Error::Identity)?;
        let operations = self.client.operations()?;
        let directory = operations.mac_bindings(proof).await?;
        let selected = self.selected().await?;
        let _lock = state::lock(&self.root)?;
        let old = state::read_optional::<::platform_sso::LocalBinding>(
            &self.root,
            &binding_name(&account)?,
        )?;
        if let Some(mut old) = old.filter(|b| {
            b.authority == w::RegistrationTrust::DaDevicePop
                && b.installation == selected.identity.service_id
                && b.tenant == selected.identity.tenant
                && b.device_id == selected.identity.device_id
        }) {
            if let Some(revoked) = directory.bindings.iter().find(|b| {
                b.id == old.binding
                    && b.revoked
                    && b.revision >= old.revision
                    && b.binding_generation >= old.binding_generation
            }) {
                old.enabled = false;
                old.revision = revoked.revision;
                old.binding_generation = revoked.binding_generation;
                save_binding(&self.root, &old)?;
            }
        }
        let mut bindings = directory.bindings.iter().filter(|b| {
            !b.revoked && b.account == Some(wire::AccessAccount::Macos { generated_uid: uid })
        });
        let found = bindings.next().ok_or(Error::NotFound)?;
        if bindings.next().is_some()
            || found.configuration.mac != selected.settings.mac
            || !found.configuration.enabled
            || found.binding_generation < 1
            || found.revision < 1
            || found.id.is_nil()
        {
            return Err(Error::Conflict);
        }
        verify()?;
        let epoch = fence.epoch.lock().map_err(|_| Error::Unavailable)?;
        if *epoch != command.fence_epoch
            || command.reply.is_closed()
            || value.expires_at_unix_ms <= crate::wall()?
            || expires <= (crate::wall()? / 1000) as i64
        {
            return Err(Error::Identity);
        }
        save_binding(
            &self.root,
            &::platform_sso::LocalBinding {
                authority: w::RegistrationTrust::DaDevicePop,
                installation: selected.identity.service_id,
                tenant: selected.identity.tenant,
                device_id: selected.identity.device_id,
                binding: found.id,
                binding_generation: found.binding_generation,
                revision: found.revision,
                generated_uid: account,
                enabled: true,
                configuration: found.configuration.mac.clone().ok_or(Error::Unsupported)?,
            },
        )?;
        Ok(value
            .expires_at_unix_ms
            .min((expires as u64).saturating_mul(1000)))
    }
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
    async fn context(
        &self,
        cmd: &Command,
        kind: w::Kind,
        require_enabled: bool,
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
            enabled(&self.root, installation)?;
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
        if let (
            Some(target),
            Some(Request::PlatformSso {
                input: Operation::Bootstrap { authentication, .. },
            }),
        ) = (&selected, cmd.request.as_operation())
        {
            if !target
                .settings
                .mac
                .as_ref()
                .is_some_and(|settings| settings.authentication == *authentication)
            {
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
