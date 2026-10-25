//! Local setup secrets and the only controlled-input bridge for account creation.
use agent_client::Error;
use rss_mdm_agent_wire::runtime as w;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use uuid::Uuid;
use zeroize::Zeroizing;
pub(crate) struct State {
    pub installation: Uuid,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub root: PathBuf,
    pub passwords: BTreeMap<Uuid, Arc<Password>>,
    setups: BTreeMap<Uuid, Setup>,
    challenges: BTreeMap<Uuid, Challenge>,
    fences: Arc<super::policy::Fences>,
}
/// Persist explicit remote revocation before admitting any further offline work.
#[cfg(any(windows, test))]
fn sync_registration<S: agent_client::IdentityProvider, C: agent_client::Clock>(
    store: &mut device_access::Store,
    client: &agent_client::Client<S, C>,
) -> Result<(), Error> {
    if client.enrollment_status()?.failure
        == Some(rss_mdm_agent_wire::enroll::ErrorCode::CredentialRevoked)
    {
        return store.retire_registration().map_err(|_| Error::Storage);
    }
    match client.registration() {
        Ok(r) => store
            .bind_registration(device_access::Registration {
                tenant: client.configuration().tenant,
                device: r.device_id,
                id: r.registration_id,
                generation: r.generation,
            })
            .map_err(|_| Error::Identity)?,
        Err(Error::Identity) => store.retire_registration().map_err(|_| Error::Storage)?,
        Err(error) => return Err(error),
    }
    Ok(())
}
impl State {
    #[cfg(windows)]
    pub(super) fn sync_registration<S: agent_client::IdentityProvider>(
        &self,
        client: &agent_client::Client<S, super::SystemClock>,
    ) -> Result<(), Error> {
        let mut store = device_access::Store::open(&self.root, self.installation)
            .map_err(|_| Error::Storage)?;
        sync_registration(&mut store, client)?;
        match device_access::windows::account::current_cloud_join() {
            Ok(join) => {
                store
                    .observe_join(&join, crate::wall()? as i64)
                    .map_err(|_| Error::Storage)?;
            }
            Err(
                rss_mdm_agent_wire::runtime::AccessFailure::Unsupported
                | rss_mdm_agent_wire::runtime::AccessFailure::AccountChanged,
            ) => (),
            Err(_) => return Err(Error::Identity),
        }
        Ok(())
    }
    pub fn new(
        config: &crate::ExecutionConfig,
        installation: Uuid,
        create: bool,
        fences: Arc<super::policy::Fences>,
    ) -> Result<Self, Error> {
        let root = crate::access_worker::root(config);
        #[cfg(windows)]
        {
            device_access::windows::require_system().map_err(|_| Error::Denied)?;
            if create {
                platform_private_storage::directory(&root)?;
                device_access::Store::create(&root, installation).map_err(|_| Error::Storage)?;
            } else {
                device_access::Store::open(&root, installation).map_err(|_| Error::Storage)?;
            }
        }
        #[cfg(target_os = "macos")]
        {
            if create {
                platform_private_storage::directory(&root)?;
            } else {
                platform_private_storage::PrivateDirectory::open(&root)?;
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = create;
        Ok(Self {
            installation,
            root,
            passwords: BTreeMap::new(),
            setups: BTreeMap::new(),
            challenges: BTreeMap::new(),
            fences,
        })
    }
    pub fn prune(&mut self) {
        self.setups
            .retain(|_, setup| setup.owner.validate(&self.fences).is_ok());
        self.passwords.retain(|_, password| {
            password.owner.validate(&self.fences).is_ok()
                && password.bytes.lock().is_ok_and(|v| v.is_some())
        });
        self.challenges
            .retain(|_, challenge| challenge.owner.validate(&self.fences).is_ok());
    }
    pub fn retire(&mut self, task: Uuid) {
        self.setups.remove(&task);
        self.passwords.remove(&task);
    }
    pub fn ready(&self, task: &w::AccessTaskSpec) -> Result<(), Error> {
        if matches!(task.windows, Some(w::AccessWindowsPlan::Initialize { .. })) {
            let setup = self.setups.get(&task.task_id).ok_or(Error::Unavailable)?;
            setup.owner.validate(&self.fences)?;
            if !setup.confirmed {
                return Err(Error::Unavailable);
            }
        }
        #[cfg(windows)]
        if matches!(task.windows, Some(w::AccessWindowsPlan::Initialize { .. })) {
            device_access::Store::open(&self.root, self.installation)
                .map_err(|_| Error::Storage)?
                .candidate(task.task_id, &device_access::SystemProtector)
                .map_err(|_| Error::Unavailable)?;
        }
        if matches!(task.action, w::AccessAction::Create { .. }) {
            self.password(task)?;
        }
        #[cfg(target_os = "macos")]
        if let w::AccessAction::Apply {
            account: w::AccessAccount::Macos { generated_uid },
            configuration,
        } = &task.action
        {
            if configuration.enabled
                && crate::access_worker::macos::inspect(&generated_uid.to_string(), false)?
                    .secure_token
                    != 1
            {
                self.password(task)?;
            }
        }
        Ok(())
    }
    pub fn password(
        &self,
        task: &w::AccessTaskSpec,
    ) -> Result<Arc<dyn execution_runner::InputResolver>, Error> {
        let password = self
            .passwords
            .get(&task.task_id)
            .ok_or(Error::Unavailable)?;
        if password.attempt != task.attempt_id
            || password.bytes.lock().map_err(|_| Error::Storage)?.is_none()
        {
            return Err(Error::Unavailable);
        }
        Ok(password.clone())
    }
}
pub(crate) struct Password {
    owner: Owner,
    fences: Arc<super::policy::Fences>,
    pub task: Uuid,
    pub attempt: Uuid,
    pub bytes: Mutex<Option<Zeroizing<Vec<u8>>>>,
}
impl execution_runner::InputResolver for Password {
    fn resolve(
        &self,
        plan: &execution_contract::FrozenExecution,
        _: &execution_contract::AttemptId,
        reference: &execution_contract::VersionedRef,
        max_bytes: u64,
    ) -> Result<execution_runner::InputBytes, execution_app::Error> {
        use execution_app::Error as E;
        self.owner.validate(&self.fences).map_err(|_| E::Denied)?;
        if plan.spec().launch.interpreter.profile.id.as_str() != crate::access_worker::PROFILE
            || reference.id.as_str() != self.task.to_string()
            || reference.revision.as_str() != self.attempt.to_string()
            || !matches!(&plan.spec().request.initiator, execution_contract::Initiator::Backend { task, attempt, .. } if task.as_str() == self.task.to_string() && attempt.as_str() == self.attempt.to_string())
        {
            return Err(E::Denied);
        }
        self.take_admitted(max_bytes)
    }
}
impl Password {
    fn take_admitted(
        &self,
        max_bytes: u64,
    ) -> Result<execution_runner::InputBytes, execution_app::Error> {
        use execution_app::Error as E;
        let scope =
            super::policy::SessionKey::new(self.owner.subject.clone(), self.owner.binding.clone());
        // Linearize material release with context retirement after native account checks.
        let _admission = self
            .fences
            .admit(&scope, self.owner.epoch)
            .map_err(|_| E::Denied)?;
        if crate::wall().map_err(|_| E::Unavailable)? >= self.owner.expires {
            return Err(E::Denied);
        }
        let bytes = self
            .bytes
            .lock()
            .map_err(|_| E::Unavailable)?
            .take()
            .ok_or(E::Unavailable)?;
        if bytes.is_empty() || bytes.len() > 1024 || bytes.len() as u64 > max_bytes {
            return Err(E::InvalidInput);
        }
        Ok(execution_runner::InputBytes::new(bytes.to_vec()))
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Owner {
    subject: String,
    session: u32,
    binding: execution_contract::Id,
    context: execution_contract::NativeContextRef,
    epoch: u64,
    expires: u64,
}
impl Owner {
    fn validate(&self, fences: &super::policy::Fences) -> Result<(), Error> {
        if crate::wall()? >= self.expires
            || fences.current(&super::policy::SessionKey::new(
                self.subject.clone(),
                self.binding.clone(),
            ))? != self.epoch
        {
            return Err(Error::Identity);
        }
        console(&self.subject, self.session, &self.binding)
    }
    fn same(&self, other: &Self) -> bool {
        self.subject == other.subject
            && self.session == other.session
            && self.binding == other.binding
            && self.context == other.context
            && self.epoch == other.epoch
    }
}
struct Setup {
    owner: Owner,
    candidate: device_access::Candidate,
    confirmed: bool,
}
struct Challenge {
    owner: Owner,
    challenge: w::AccessChallenge,
}
fn console(subject: &str, session: u32, binding: &execution_contract::Id) -> Result<(), Error> {
    #[cfg(windows)]
    {
        execution_ipc::windows_identity::verify_unlocked_console(subject, session, binding)
            .map_err(super::helper_error)?;
        let account =
            device_access::windows::account::inspect(subject).map_err(|_| Error::Denied)?;
        if !account.enabled {
            return Err(Error::Denied);
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        execution_ipc::macos_identity::verify_console(subject, session, binding)
            .map_err(super::helper_error)?;
        crate::account_identity::generated_uid(subject)?.ok_or(Error::Identity)?;
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (subject, session, binding);
        Err(Error::Unsupported)
    }
}
fn native_account(subject: &str) -> Result<w::AccessAccount, Error> {
    #[cfg(windows)]
    {
        let account =
            device_access::windows::account::inspect(subject).map_err(|_| Error::Identity)?;
        Ok(w::AccessAccount::Windows {
            sid: subject.into(),
            authority: account.source,
        })
    }
    #[cfg(target_os = "macos")]
    {
        Ok(w::AccessAccount::Macos {
            generated_uid: Uuid::parse_str(
                &crate::account_identity::generated_uid(subject)?.ok_or(Error::Identity)?,
            )
            .map_err(|_| Error::Identity)?,
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = subject;
        Err(Error::Unsupported)
    }
}
impl<S: agent_client::IdentityProvider> super::DeviceService<S> {
    fn access_task(
        &self,
        task: Uuid,
        principal: &w::SelfServiceUser,
        subject: &str,
    ) -> Result<w::AccessTaskSpec, Error> {
        let offer = self.client.retained_offer(task)?;
        let w::TaskPayload::DeviceAccess(spec) = offer.payload() else {
            return Err(Error::Denied);
        };
        if spec.principal_id != principal.principal_id
            || spec.instance_id != principal.instance_id
            || spec.tenant_id != principal.tenant_id
            || spec.platform
                != if cfg!(target_os = "macos") {
                    w::TaskPlatform::Macos
                } else {
                    w::TaskPlatform::Windows
                }
            || (spec.platform == w::TaskPlatform::Windows
                && !matches!(spec.windows, Some(w::AccessWindowsPlan::Initialize { .. })))
            || spec.expires_at <= (crate::wall()? / 1000) as i64
            || self.client.association(task)?.is_some()
        {
            return Err(Error::Denied);
        }
        if let w::AccessAction::Apply { account, .. } = &spec.action {
            if *account != native_account(subject)? {
                return Err(Error::Denied);
            }
        }
        Ok((**spec).clone())
    }
    pub(super) async fn access_command(
        &mut self,
        command: &super::Command,
        context: &execution_contract::NativeContextRef,
        origin: &execution_ipc::host::ClientOrigin,
        input: &execution_ipc::device_access::Operation,
        commands: &mut tokio::sync::mpsc::Receiver<super::Command>,
    ) -> Result<execution_ipc::host::Reply, Error> {
        use execution_ipc::{
            device_access::{Operation as Op, Response as Out, SetupTask},
            host::{ClientOrigin, Reply},
        };
        if !matches!(origin, ClientOrigin::Desktop {}) {
            return Err(Error::Denied);
        }
        let bound = self.core.context(command)?.clone();
        if bound.context != *context {
            return Err(Error::Identity);
        }
        let user = bound.user().ok_or(Error::Identity)?.clone();
        let proof = bound.credential().ok_or(Error::Identity)?.clone();
        let owner = Owner {
            subject: command.subject.clone(),
            session: command.session,
            binding: command.binding.clone().ok_or(Error::Denied)?,
            context: context.clone(),
            epoch: command.fence_epoch,
            expires: bound
                .expires_at_unix_ms
                .min(crate::wall()?.saturating_add(600_000)),
        };
        owner.validate(&self.core.access.fences)?;
        #[cfg(windows)]
        if !matches!(
            input,
            Op::WindowsStatus {} | Op::Directory { .. } | Op::Tasks {}
        ) {
            let native = device_access::windows::account::inspect(&owner.subject)
                .map_err(|_| Error::Identity)?;
            if let Some(logon_millis) = native.logon_millis {
                let mut store = device_access::Store::open(
                    &self.core.access.root,
                    self.core.access.installation,
                )
                .map_err(|_| Error::Storage)?;
                store
                    .require_join_session(&native.join, logon_millis, crate::wall()? as i64)
                    .map_err(|_| Error::Identity)?;
            }
        }
        self.core.access.prune();
        let output = match input {
            Op::PrepareMac { task, password } => {
                let spec = self.access_task(*task, &user, &owner.subject)?;
                if spec.platform != w::TaskPlatform::Macos
                    || !matches!(
                        spec.action,
                        w::AccessAction::Create { .. } | w::AccessAction::Apply { .. }
                    )
                    || password.is_empty()
                    || password.len() > 1024
                    || password.chars().any(char::is_control)
                {
                    return Err(Error::Denied);
                }
                self.core.access.passwords.insert(
                    *task,
                    Arc::new(Password {
                        owner: Owner {
                            expires: owner.expires.min(crate::wall()?.saturating_add(30_000)),
                            ..owner.clone()
                        },
                        fences: self.core.access.fences.clone(),
                        task: *task,
                        attempt: spec.attempt_id,
                        bytes: Mutex::new(Some(Zeroizing::new(password.as_bytes().to_vec()))),
                    }),
                );
                Out::Prepared {}
            }
            Op::MacStatus {} => Out::MacStatus(self.mac_access_status(command, commands).await?),
            Op::OpenWindowsHello {} => {
                #[cfg(windows)]
                {
                    owner.validate(&self.core.access.fences)?;
                    let peer = self.core.helpers.connect_peer(
                        &owner.subject,
                        owner.session,
                        &owner.binding,
                    )?;
                    match peer
                        .exchange(execution_ipc::helper::Command::OpenWindowsHello)
                        .map_err(super::helper_error)?
                    {
                        execution_ipc::helper::Reply::Submitted => {
                            Out::WindowsHelloSettingsRequested {}
                        }
                        _ => return Err(Error::Unavailable),
                    }
                }
                #[cfg(not(windows))]
                {
                    return Err(Error::Unsupported);
                }
            }
            Op::WindowsStatus {} => {
                #[cfg(windows)]
                {
                    use execution_ipc::device_access::{WindowsComponentState, WindowsStatus};
                    let mut status = WindowsStatus {
                        component: crate::access_component::readiness(
                            &self.core.access.root,
                            self.core.access.installation,
                        )
                        .unwrap_or(WindowsComponentState::Unavailable),
                        factor_active: None,
                        sign_in_required: false,
                        hello: crate::access_worker::windows::hello::status(
                            &self.core.access.root,
                            self.core.access.installation,
                            &owner.subject,
                        ),
                    };
                    if let Ok(mut store) = device_access::Store::open(
                        &self.core.access.root,
                        self.core.access.installation,
                    ) {
                        status.factor_active = store.enabled(&owner.subject).ok();
                        let native = device_access::windows::account::inspect(&owner.subject)
                            .map_err(|_| Error::Identity)?;
                        if let Some(logon) = native.logon_millis {
                            status.sign_in_required = store
                                .require_join_session(&native.join, logon, crate::wall()? as i64)
                                .is_err();
                        }
                    }
                    Out::WindowsStatus(status)
                }
                #[cfg(not(windows))]
                return Err(Error::Unsupported);
            }
            Op::Directory { cursor } => Out::Directory(
                super::network(
                    self.client.access_directory(&proof, cursor.as_deref()),
                    &mut self.core,
                    commands,
                )
                .await?,
            ),
            Op::Begin(begin) => {
                if self.core.access.challenges.len() >= 8 {
                    return Err(Error::Capacity);
                }
                let challenge = super::network(
                    self.client.access_begin(&proof, begin),
                    &mut self.core,
                    commands,
                )
                .await?;
                owner.validate(&self.core.access.fences)?;
                if challenge.local_session
                    != (w::AccessLocalSession {
                        account: native_account(&owner.subject)?,
                        session_id: super::login_binding_id(&owner.binding),
                    })
                {
                    return Err(Error::Untrusted);
                }
                let expires_at = challenge.expires_at;
                self.core.access.challenges.insert(
                    begin.operation_id,
                    Challenge {
                        owner: owner.clone(),
                        challenge,
                    },
                );
                Out::Challenge {
                    operation: begin.operation_id,
                    expires_at,
                }
            }
            Op::Confirm { operation } => {
                let pending = self
                    .core
                    .access
                    .challenges
                    .get(operation)
                    .ok_or(Error::NotFound)?;
                if !pending.owner.same(&owner)
                    || pending.challenge.expires_at <= (crate::wall()? / 1000) as i64
                {
                    return Err(Error::Identity);
                }
                let request = w::AccessConfirm {
                    operation_id: *operation,
                    challenge: pending.challenge.id,
                    nonce: pending.challenge.nonce.clone(),
                    local_session: pending.challenge.local_session.clone(),
                };
                let status = super::network(
                    self.client.access_confirm(&proof, &request),
                    &mut self.core,
                    commands,
                )
                .await?;
                Out::Binding(Box::new(status))
            }
            Op::Tasks {} => {
                let mut tasks = vec![];
                for id in self
                    .client
                    .pending_tasks(self.client.configuration().limits.pending_tasks)?
                {
                    if let Ok(task) = self.access_task(id, &user, &owner.subject) {
                        tasks.push(SetupTask {
                            task: id,
                            binding: task.binding_id,
                            create_name: match task.action {
                                w::AccessAction::Create { name, .. } => Some(name),
                                _ => None,
                            },
                        });
                    }
                }
                Out::Tasks { tasks }
            }
            Op::Prepare { task } => {
                if cfg!(target_os = "macos") {
                    return Err(Error::Unsupported);
                }
                self.access_task(*task, &user, &owner.subject)?;
                if self.core.access.setups.len() >= 8 && !self.core.access.setups.contains_key(task)
                {
                    return Err(Error::Capacity);
                }
                if !self
                    .core
                    .access
                    .setups
                    .get(task)
                    .is_some_and(|s| s.owner.same(&owner))
                {
                    #[cfg(windows)]
                    let candidate = {
                        let store = device_access::Store::open(
                            &self.core.access.root,
                            self.core.access.installation,
                        )
                        .map_err(|_| Error::Storage)?;
                        if store.has_candidate(*task).map_err(|_| Error::Storage)? {
                            store
                                .candidate(*task, &device_access::SystemProtector)
                                .map_err(|_| Error::Storage)?
                        } else {
                            device_access::Candidate::generate().map_err(|_| Error::Unavailable)?
                        }
                    };
                    #[cfg(not(windows))]
                    let candidate =
                        device_access::Candidate::generate().map_err(|_| Error::Unavailable)?;
                    self.core.access.setups.insert(
                        *task,
                        Setup {
                            owner: owner.clone(),
                            candidate,
                            confirmed: false,
                        },
                    );
                }
                let setup = self.core.access.setups.get(task).ok_or(Error::Storage)?;
                Out::Setup {
                    task: *task,
                    secret: Zeroizing::new(
                        data_encoding::BASE32_NOPAD.encode(setup.candidate.totp_secret()),
                    ),
                }
            }
            Op::ConfirmFactor {
                task,
                code,
                password,
            } => {
                if cfg!(target_os = "macos") {
                    return Err(Error::Unsupported);
                }
                let spec = self.access_task(*task, &user, &owner.subject)?;
                let setup = self
                    .core
                    .access
                    .setups
                    .get_mut(task)
                    .ok_or(Error::NotFound)?;
                if !setup.owner.same(&owner) {
                    return Err(Error::Identity);
                }
                #[cfg(windows)]
                let clock = device_access::windows::clock().map_err(|_| Error::Clock)?;
                #[cfg(not(windows))]
                let clock = device_access::Clock {
                    utc: (crate::wall()? / 1000) as i64,
                    monotonic: 1,
                    boot: Uuid::new_v4(),
                };
                if !setup
                    .candidate
                    .confirm(code, clock)
                    .map_err(|_| Error::Protocol)?
                {
                    return Err(Error::Denied);
                }
                if matches!(spec.action, w::AccessAction::Create { .. }) {
                    let password = password.as_ref().ok_or(Error::Denied)?;
                    if password.is_empty() || password.len() > 1024 || password.contains('\0') {
                        return Err(Error::Protocol);
                    }
                    self.core.access.passwords.insert(
                        *task,
                        Arc::new(Password {
                            owner: owner.clone(),
                            fences: self.core.access.fences.clone(),
                            task: *task,
                            attempt: spec.attempt_id,
                            bytes: Mutex::new(Some(Zeroizing::new(password.as_bytes().to_vec()))),
                        }),
                    );
                } else if password.is_some() {
                    return Err(Error::Protocol);
                }
                #[cfg(windows)]
                {
                    let mut store = device_access::Store::open(
                        &self.core.access.root,
                        self.core.access.installation,
                    )
                    .map_err(|_| Error::Storage)?;
                    if !store.has_candidate(*task).map_err(|_| Error::Storage)? {
                        store
                            .stage(*task, &setup.candidate, &device_access::SystemProtector)
                            .map_err(|_| Error::Storage)?;
                    } else if !store
                        .reconfirm(*task, code, clock, &device_access::SystemProtector)
                        .map_err(|_| Error::Storage)?
                    {
                        return Err(Error::Denied);
                    }
                }
                setup.confirmed = true;
                Out::Prepared {}
            }
        };
        owner.validate(&self.core.access.fences)?;
        // HTTP awaits can process revocation; do not release setup material across a stale context.
        if self.core.context(command)?.context != *context {
            return Err(Error::Identity);
        }
        Ok(Reply::DeviceAccess { value: output })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn non_claim_revocation_retires_offline_registration_after_restart() {
        use crate::protocol_test_support::{Root, Server};
        use agent_client::{Clock as _, OpenMode};
        use device_access::{Authority, Candidate, Clock, Decision, Factor, Protector, Store};
        struct Plain;
        impl Protector for Plain {
            fn seal(&self, bytes: &[u8]) -> Result<Vec<u8>, device_access::Error> {
                Ok(bytes.to_vec())
            }
            fn open(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, device_access::Error> {
                Ok(Zeroizing::new(bytes.to_vec()))
            }
        }
        let server = Server::tls().await;
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        let r = server.register(&mut client).await;
        let offline = root.path.join("offline");
        platform_private_storage::directory(&offline).unwrap();
        let offline = offline.canonicalize().unwrap();
        let installation = Uuid::new_v4();
        let mut store = Store::create(&offline, installation).unwrap();
        sync_registration(&mut store, &client).unwrap();
        let authority = Authority {
            tenant: client.configuration().tenant,
            device: r.device_id,
            registration: r.registration_id,
            registration_generation: r.generation,
            instance: Uuid::new_v4(),
            principal: Uuid::new_v4(),
            binding: Uuid::new_v4(),
            binding_generation: 1,
            factor_generation: 1,
            sid: "S-1-5-21-1-2-3-1001".into(),
            source: w::AccessWindowsAuthority::Local,
            login: "LOCAL\\test".into(),
            join: "local-machine".into(),
        };
        let clock = Clock {
            utc: server.time.now().unwrap(),
            monotonic: 1000,
            boot: Uuid::new_v4(),
        };
        let mut candidate = Candidate::generate().unwrap();
        let tag = ring::hmac::sign(
            &ring::hmac::Key::new(
                ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
                candidate.totp_secret(),
            ),
            &((clock.utc / 30) as u64).to_be_bytes(),
        );
        let bytes = tag.as_ref();
        let offset = (bytes[19] & 15) as usize;
        let code = format!(
            "{:06}",
            (u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff)
                % 1_000_000
        );
        assert!(candidate.confirm(&code, clock).unwrap());
        let recovery = candidate.recovery_secret().to_owned();
        let operation = Uuid::new_v4();
        store.stage(operation, &candidate, &Plain).unwrap();
        store
            .activate(
                authority.clone(),
                1,
                true,
                operation,
                w::AccessRssFacts {
                    factor_generation: 1,
                    recovery: w::CredentialReceipt {
                        subject: w::CredentialSubject::RssLogin {
                            binding: authority.binding,
                            binding_generation: 1,
                            sid: authority.sid.clone(),
                        },
                        material: Uuid::new_v4(),
                        version: 1,
                    },
                },
                clock,
                &Plain,
            )
            .unwrap();
        // Credential expiry is not a remote revocation of the bound offline authority.
        let db = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
        let identity: Vec<u8> = db
            .query_row("SELECT body FROM state WHERE key='identity'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let identity: serde_json::Value = serde_json::from_slice(&identity).unwrap();
        server
            .time
            .set(identity["active"]["not_after"].as_i64().unwrap() + 1);
        drop(db);
        assert_eq!(
            client.enrollment_status().unwrap().failure,
            Some(rss_mdm_agent_wire::enroll::ErrorCode::CertificateExpired)
        );
        sync_registration(&mut store, &client).unwrap();
        assert!(store.enabled(&authority.sid).unwrap());
        server.time.set(clock.utc);
        client
            .queue_report(
                "inventory",
                w::ReportBody::Failed {
                    code: w::FailureCode::CollectionFailed,
                },
                1,
            )
            .unwrap();
        server.data.lock().unwrap().report_failure = true;
        assert_eq!(client.flush_reports(1).await, Err(Error::Unavailable));
        sync_registration(&mut store, &client).unwrap();
        assert_eq!(
            store
                .verify(&authority, Factor::Recovery(&recovery), clock, &Plain)
                .unwrap(),
            Decision::Accepted
        );
        server.data.lock().unwrap().denied = true;
        assert_eq!(client.flush_reports(1).await, Err(Error::CredentialRevoked));
        // Crash before copying the already persisted communication state to the offline owner.
        drop(client);
        drop(store);
        let client = server.client(&root, OpenMode::Existing);
        assert!(client.registration().is_ok()); // Runtime receipt alone is not authorization.
        let mut store = Store::open(&offline, installation).unwrap();
        sync_registration(&mut store, &client).unwrap();
        sync_registration(&mut store, &client).unwrap();
        drop(store);
        let mut store = Store::open(&offline, installation).unwrap();
        assert!(!store.enabled(&authority.sid).unwrap());
        assert!(matches!(
            store.verify(&authority, Factor::Recovery(&recovery), clock, &Plain),
            Err(device_access::Error::Changed)
        ));
        assert!(store.bind_registration(authority.registration()).is_err());
    }
    #[test]
    fn revocation_between_validation_and_release_keeps_password_private() {
        use execution_contract::{Id, NativeContextRef};
        let fences = Arc::new(super::super::policy::Fences::default());
        let binding = Id::new("native-login").unwrap();
        let scope = super::super::policy::SessionKey::new("local-sid".into(), binding.clone());
        let password = Arc::new(Password {
            owner: Owner {
                subject: "local-sid".into(),
                session: 1,
                binding,
                context: NativeContextRef {
                    id: Id::new("context").unwrap(),
                    generation: Id::new("1").unwrap(),
                },
                epoch: 0,
                expires: crate::wall().unwrap() + 60_000,
            },
            fences: fences.clone(),
            task: Uuid::new_v4(),
            attempt: Uuid::new_v4(),
            bytes: Mutex::new(Some(Zeroizing::new(b"local-password".to_vec()))),
        });
        let (checked, observed) = std::sync::mpsc::channel();
        let (resume, resumed) = std::sync::mpsc::channel();
        let worker = password.clone();
        let worker_scope = scope.clone();
        let thread = std::thread::spawn(move || {
            assert_eq!(worker.fences.current(&worker_scope).unwrap(), 0);
            checked.send(()).unwrap();
            resumed.recv().unwrap();
            assert!(worker.take_admitted(1024).is_err());
        });
        observed.recv().unwrap();
        fences.revoke(&scope).unwrap();
        resume.send(()).unwrap();
        thread.join().unwrap();
        assert!(password.bytes.lock().unwrap().is_some());
    }
}
