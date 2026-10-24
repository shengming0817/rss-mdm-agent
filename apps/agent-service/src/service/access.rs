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
impl State {
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
fn local_account(subject: &str) -> Result<w::AccessAccount, Error> {
    #[cfg(windows)]
    {
        Ok(w::AccessAccount::Windows {
            sid: subject.into(),
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
            if *account != local_account(subject)? {
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
            Op::Directory { after } => Out::Directory(
                super::network(
                    self.client.access_directory(&proof, *after),
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
                        account: local_account(&owner.subject)?,
                        session_id: super::login_binding_id(&owner.binding),
                    })
                {
                    return Err(Error::Untrusted);
                }
                let expires_at = challenge.expires_at;
                self.core
                    .access
                    .challenges
                    .insert(begin.operation_id, Challenge { owner, challenge });
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
                Out::Binding(status)
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
                            owner,
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
