//! Desktop setup holds secrets only for the authenticated console login and exact DA attempt.
use super::*;
use device_access::{Candidate, Store, SystemProtector};
use execution_contract::{Id, NativeContextRef};
use execution_ipc::{
    device_access::{EmployeePortal, Operation, Response, SetupTask},
    host::{ClientOrigin, NativeContextSession},
};
use rss_mdm_agent_wire::runtime as w;
use std::{
    collections::BTreeMap,
    sync::{mpsc, Mutex},
    time::Instant,
};
use zeroize::Zeroizing;

#[derive(Clone)]
struct Employee {
    credential: Arc<Zeroizing<String>>,
    subject: String,
    session: u32,
    binding: Id,
    context: NativeContextRef,
    identity: w::SelfServiceUser,
    expires: u64,
}
impl Employee {
    fn current(&self) -> bool {
        crate::wall().is_ok_and(|now| now < self.expires)
            && execution_ipc::windows_identity::verify_unlocked_console(
                &self.subject,
                self.session,
                &self.binding,
            )
            .is_ok()
    }
    fn permits(&self, t: &w::AccessTaskSpec) -> bool {
        self.current()
            && t.instance_id == self.identity.instance_id
            && t.tenant_id == self.identity.tenant_id
            && t.principal_id == self.identity.principal_id
            && (matches!(t.action, w::AccessAction::Create { .. })
                || matches!(&t.action,w::AccessAction::Apply{account:w::AccessAccount::Windows{sid,authority:w::AccessWindowsAuthority::Local},..} if sid==&self.subject))
    }
}
#[derive(Default)]
struct State {
    revision: u64,
    employee: Option<Employee>,
    offers: BTreeMap<Uuid, Offer>,
    checking: Option<Check>,
    directory: Option<(Instant, mpsc::Receiver<Result<Response, Error>>)>,
    lookup: Option<(Employee, mpsc::SyncSender<Result<Response, Error>>)>,
    mdm_tasks: Option<(u64, Instant, Vec<SetupTask>)>,
}
struct Offer {
    task: w::AccessTaskSpec,
    setup: Option<Candidate>,
    confirmed: bool,
    password: Option<Zeroizing<String>>,
}
struct Check {
    revision: u64,
    started: Instant,
    result: mpsc::Receiver<Result<(), Error>>,
}
struct Verify {
    employee: Employee,
    result: mpsc::SyncSender<Result<(), Error>>,
}
impl State {
    fn clear_setup(&mut self) {
        for o in self.offers.values_mut() {
            o.setup = None;
            o.confirmed = false;
            o.password = None;
        }
        self.checking = None;
        self.directory = None;
        self.lookup = None;
    }
    fn logout(&mut self) {
        self.clear_setup();
        self.employee = None;
    }
}
pub(super) struct Login {
    deployment: Deployment,
    portal: Option<EmployeePortal>,
    pub(super) registered: AtomicBool,
    state: Mutex<State>,
    verifier: mpsc::SyncSender<Verify>,
    pub(super) wake: tokio::sync::Notify,
    other: Option<Arc<dyn SessionFence>>,
}
impl Login {
    pub(super) fn new(d: &Deployment, other: Option<Arc<dyn SessionFence>>) -> Result<Self, Error> {
        let portal = d.modules.employee_portal()?;
        let issuer = portal.clone();
        let (verifier, queue) = mpsc::sync_channel::<Verify>(1);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?;
        thread::Builder::new()
            .name("da-employee-verifier".into())
            .spawn(move || {
                while let Ok(job) = queue.recv() {
                    let result = rt.block_on(Self::verify(issuer.as_ref(), &job.employee));
                    let _ = job.result.send(result);
                }
            })?;
        Ok(Self {
            deployment: d.clone(),
            portal,
            registered: AtomicBool::new(false),
            state: Mutex::default(),
            other,
            verifier,
            wake: tokio::sync::Notify::new(),
        })
    }
    async fn verify(portal: Option<&EmployeePortal>, employee: &Employee) -> Result<(), Error> {
        let portal = portal.ok_or(Error::Configuration)?;
        if !portal.accepts(&employee.identity) || !employee.current() {
            return Err(Error::Identity);
        }
        let expiry = da::verify_employee(
            &portal.origin,
            portal.ca_pem.as_deref(),
            portal.tenant,
            employee.identity.principal_id,
            &employee.credential,
        )
        .await?;
        if expiry <= utc()? {
            return Err(Error::Expired);
        }
        Ok(())
    }
    // Only the verifier thread performs browser IO. Each result authorizes one call in
    // the same still-live native context. Expired results and superseded contexts are discarded.
    fn checked(&self, s: &mut State, employee: &Employee) -> Result<bool, Error> {
        if let Some(c) = &s.checking {
            if c.revision == s.revision && c.started.elapsed() < Duration::from_secs(4) {
                match c.result.try_recv() {
                    Ok(result) => {
                        s.checking = None;
                        result?;
                        return Ok(true);
                    }
                    Err(mpsc::TryRecvError::Empty) => return Ok(false),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        s.checking = None;
                        return Err(Error::Unavailable);
                    }
                }
            }
        }
        let (result, rx) = mpsc::sync_channel(1);
        self.verifier
            .try_send(Verify {
                employee: employee.clone(),
                result,
            })
            .map_err(|_| Error::Capacity)?;
        s.checking = Some(Check {
            revision: s.revision,
            started: Instant::now(),
            result: rx,
        });
        Ok(false)
    }
    /// All device HTTP uses the active DA worker and its current certificate.
    pub(super) async fn read_directory(
        &self,
        client: &da::Client<DeviceSecrets, SystemClock>,
    ) -> Result<(), Error> {
        let job = self.state.lock().map_err(|_| Error::Storage)?.lookup.take();
        if let Some((employee, sender)) = job {
            let result = async {
                Self::verify(self.portal.as_ref(), &employee).await?;
                let ops = client.operations()?;
                let d = ops.employee_bindings(&employee.credential).await?;
                let state = self.state.lock().map_err(|_| Error::Storage)?;
                if !employee.current()
                    || !state.employee.as_ref().is_some_and(|e| {
                        e.context == employee.context
                            && e.binding == employee.binding
                            && e.identity == employee.identity
                            && e.credential == employee.credential
                    })
                {
                    return Err(Error::Identity);
                }
                crate::service::access::own_bindings(d, &employee.subject, &employee.identity)
            }
            .await;
            let _ = sender.send(result);
        }
        Ok(())
    }
    fn store(&self) -> Result<Store, Error> {
        Store::open(
            &crate::access_worker::root(&self.deployment.execution),
            self.deployment.installation_id,
        )
        .map_err(|_| Error::Storage)
    }
    pub(super) fn offer(&self, t: &w::AccessTaskSpec) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        let now = utc()?;
        s.offers
            .retain(|_, o| o.task.expires_at.min(o.task.deadline) > now);
        if s.offers.get(&t.task_id).is_some_and(|o| &o.task == t) {
            return Ok(());
        }
        if s.offers.len() >= 128 && !s.offers.contains_key(&t.task_id) {
            return Err(Error::Capacity);
        }
        s.offers.insert(
            t.task_id,
            Offer {
                task: t.clone(),
                setup: None,
                confirmed: false,
                password: None,
            },
        );
        Ok(())
    }
    pub(super) fn confirmed(&self) -> Result<Option<(Uuid, Uuid)>, Error> {
        let s = self.state.lock().map_err(|_| Error::Unavailable)?;
        let now = utc()?;
        Ok(s.offers
            .values()
            .find(|o| {
                o.confirmed
                    && o.task.expires_at.min(o.task.deadline) > now
                    && s.employee.as_ref().is_some_and(|e| e.permits(&o.task))
            })
            .map(|o| (o.task.task_id, o.task.attempt_id)))
    }
    pub(super) fn withdraw(&self, id: Uuid, attempt: Uuid) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        if s.offers
            .get(&id)
            .is_some_and(|o| o.task.attempt_id == attempt)
        {
            s.offers.remove(&id);
        }
        Ok(())
    }
    pub(super) fn reset(&self) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        s.offers.clear();
        s.logout();
        s.revision = s.revision.checked_add(1).ok_or(Error::Capacity)?;
        Ok(())
    }
    pub(super) async fn authorize_input(&self, t: &w::AccessTaskSpec) -> Result<u64, Error> {
        let (employee, revision) = {
            let s = self.state.lock().map_err(|_| Error::Unavailable)?;
            let e = s
                .employee
                .as_ref()
                .filter(|e| e.permits(t))
                .ok_or(Error::Denied)?;
            if !s
                .offers
                .get(&t.task_id)
                .is_some_and(|o| o.task == *t && o.confirmed)
            {
                return Err(Error::Denied);
            }
            (e.clone(), s.revision)
        };
        Self::verify(self.portal.as_ref(), &employee).await?;
        Ok(revision)
    }
    /// Consume once under the native worker lock after rechecking the verified console context.
    pub(super) fn input(
        &self,
        t: &w::AccessTaskSpec,
        revision: u64,
    ) -> Result<Option<Zeroizing<String>>, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        if s.revision != revision || !s.employee.as_ref().is_some_and(|e| e.permits(t)) {
            return Err(Error::Identity);
        }
        let o = s
            .offers
            .get_mut(&t.task_id)
            .filter(|o| o.task == *t && o.confirmed)
            .ok_or(Error::Denied)?;
        if matches!(t.action, w::AccessAction::Create { .. }) && o.password.is_none() {
            return Err(Error::Denied);
        }
        if t.expires_at.min(t.deadline) <= utc()? {
            return Err(Error::Expired);
        }
        Ok(o.password.take())
    }
    fn bind(
        &self,
        peer: &Peer,
        subject: String,
        value: &NativeContextSession,
        expected: Option<&Id>,
        revision: u64,
    ) -> Result<Reply, Error> {
        let binding = peer.session_binding().map_err(|_| Error::Identity)?;
        execution_ipc::windows_identity::verify_unlocked_console(
            &subject,
            peer.session(),
            &binding,
        )
        .map_err(|_| Error::Identity)?;
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        if s.revision != revision {
            return Err(Error::Identity);
        }
        s.revision = s.revision.checked_add(1).ok_or(Error::Capacity)?;
        if expected.is_some_and(|old| {
            !s.employee.as_ref().is_some_and(|e| {
                &e.context.id == old
                    && e.subject == subject
                    && e.binding == binding
                    && e.context.generation == value.generation
            })
        }) {
            return Err(Error::Identity);
        }
        let context = NativeContextRef {
            id: value.generation.clone(),
            generation: value.generation.clone(),
        };
        let Some(user) = value.user() else {
            s.logout();
            return Ok(Reply::ContextBound { value: context });
        };
        if !self.portal.as_ref().is_some_and(|p| p.accepts(user))
            || user.principal_id.is_nil()
            || value.expires_at_unix_ms <= crate::wall()?
            || value.expires_at_unix_ms > execution_contract::MAX_SAFE_INTEGER
            || !value.credential().is_some_and(|p| {
                p.len() == 64
                    && p.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err(Error::Identity);
        }
        if !s.employee.as_ref().is_some_and(|e| {
            e.subject == subject
                && e.binding == binding
                && e.context == context
                && e.identity == *user
        }) {
            s.clear_setup();
        }
        s.employee = Some(Employee {
            credential: Arc::new(value.credential().ok_or(Error::Identity)?.clone()),
            subject,
            session: peer.session(),
            binding,
            context: context.clone(),
            identity: user.clone(),
            expires: value.expires_at_unix_ms,
        });
        Ok(Reply::ContextBound { value: context })
    }
    pub(super) fn handle(&self, peer: &Peer, request: &Request) -> Option<Reply> {
        if !matches!(
            request,
            Request::BindContext { .. } | Request::DeviceAccess { .. }
        ) {
            return None;
        }
        let revision = self.state.lock().ok()?.revision;
        let subject = peer
            .authenticate(&self.deployment.registration_clients)
            .ok()?;
        match request {
            Request::BindContext { value, expected } => Some(
                self.bind(peer, subject, value, expected.as_ref(), revision)
                    .unwrap_or(Reply::Rejected),
            ),
            Request::DeviceAccess {
                context,
                origin: ClientOrigin::Desktop {},
                mac_password: None,
                mac_confirmation: None,
                input,
            } => self
                .access(peer, &subject, context, input)
                .unwrap_or(Some(Reply::Rejected)),
            _ => None,
        }
    }
    /// Start MDM's normal authenticated listing while the DA verifier is waiting on Identity.
    /// Keep the response only for this live native context and the current bounded request.
    pub(super) fn mdm_tasks(
        &self,
        peer: &Peer,
        request: &Request,
        mdm: &mut dyn Handler,
        consume: bool,
    ) -> Vec<SetupTask> {
        let Request::DeviceAccess {
            context,
            input: Operation::Tasks {},
            ..
        } = request
        else {
            return Vec::new();
        };
        let Ok(mut s) = self.state.lock() else {
            return Vec::new();
        };
        if !s.employee.as_ref().is_some_and(|e| {
            &e.context == context
                && peer
                    .authenticate(&self.deployment.registration_clients)
                    .ok()
                    .as_ref()
                    == Some(&e.subject)
                && peer.session_binding().ok().as_ref() == Some(&e.binding)
                && e.current()
        }) {
            return Vec::new();
        }
        let revision = s.revision;
        if s.mdm_tasks
            .as_ref()
            .is_some_and(|(r, at, _)| *r == revision && at.elapsed() < Duration::from_secs(5))
        {
            return if consume {
                s.mdm_tasks
                    .take()
                    .map(|(_, _, tasks)| tasks)
                    .unwrap_or_default()
            } else {
                vec![]
            };
        }
        drop(s);
        let start = Instant::now();
        let tasks = match mdm.handle(peer, request.clone()) {
            Reply::DeviceAccess {
                value: Response::Tasks { tasks },
            } => tasks,
            _ => vec![],
        };
        let Ok(mut s) = self.state.lock() else {
            return Vec::new();
        };
        if s.revision != revision || !s.employee.as_ref().is_some_and(Employee::current) {
            return Vec::new();
        }
        if consume {
            tasks
        } else {
            s.mdm_tasks = Some((revision, start, tasks));
            vec![]
        }
    }
    fn access(
        &self,
        peer: &Peer,
        subject: &str,
        context: &NativeContextRef,
        input: &Operation,
    ) -> Result<Option<Reply>, Error> {
        if !matches!(
            input,
            Operation::Tasks {}
                | Operation::OwnBindings {}
                | Operation::WindowsStatus {}
                | Operation::Prepare { .. }
                | Operation::ConfirmFactor { .. }
        ) {
            return Ok(None);
        }
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        let Some(e) = s.employee.clone() else {
            return Ok(None);
        };
        if &e.context != context
            || e.subject != subject
            || e.binding != peer.session_binding().map_err(|_| Error::Identity)?
            || !e.current()
        {
            return Ok(None);
        }
        if matches!(input, Operation::OwnBindings {}) {
            if !self.registered.load(Ordering::Acquire) {
                return Ok(None);
            }
            if let Some((start, rx)) = &s.directory {
                match rx.try_recv() {
                    Ok(result) => {
                        s.directory = None;
                        return Ok(Some(Reply::DeviceAccess { value: result? }));
                    }
                    Err(mpsc::TryRecvError::Empty) if start.elapsed() < Duration::from_secs(12) => {
                        return Ok(Some(Reply::DaVerificationPending {}))
                    }
                    _ => {
                        s.directory = None;
                        return Err(Error::Unavailable);
                    }
                }
            }
            let (tx, rx) = mpsc::sync_channel(1);
            s.lookup = Some((e, tx));
            s.directory = Some((Instant::now(), rx));
            self.wake.notify_one();
            return Ok(Some(Reply::DaVerificationPending {}));
        }
        let root = crate::access_worker::root(&self.deployment.execution);
        let installation = self.deployment.installation_id;
        // This observation uses the current bound console SID, just like the MDM read.
        if matches!(input, Operation::WindowsStatus {}) {
            return Ok(Some(Reply::DeviceAccess {
                value: Response::WindowsStatus(crate::access_component::status(
                    &root,
                    installation,
                    subject,
                )?),
            }));
        }
        let now = utc()?;
        s.offers
            .retain(|_, o| o.task.expires_at.min(o.task.deadline) > now);
        let target = match input {
            Operation::Prepare { task } | Operation::ConfirmFactor { task, .. } => Some(*task),
            _ => None,
        };
        if target.is_some_and(|id| !s.offers.get(&id).is_some_and(|o| e.permits(&o.task))) {
            return Ok(None);
        }
        if matches!(input, Operation::Tasks {}) && !s.offers.values().any(|o| e.permits(&o.task)) {
            return Ok(Some(Reply::DeviceAccess {
                value: Response::Tasks { tasks: vec![] },
            }));
        }
        if !self.checked(&mut s, &e)? {
            return Ok(Some(Reply::DaVerificationPending {}));
        }
        if !e.current() {
            return Err(Error::Identity);
        }
        if target.is_some() {
            let status = crate::access_component::status(&root, installation, subject)?;
            if status.component != execution_ipc::device_access::WindowsComponentState::Ready {
                return Ok(Some(Reply::DeviceAccess {
                    value: Response::WindowsStatus(status),
                }));
            }
        }
        let value = match input {
            Operation::Tasks {} => Response::Tasks {
                tasks: s
                    .offers
                    .values()
                    .map(|o| &o.task)
                    .filter(|t| e.permits(t))
                    .map(|t| SetupTask {
                        mac_confirmation: None,
                        task: t.task_id,
                        attempt: t.attempt_id,
                        binding: t.binding_id,
                        create_name: match &t.action {
                            w::AccessAction::Create { name, .. } => Some(name.clone()),
                            _ => None,
                        },
                    })
                    .collect(),
            },
            Operation::Prepare { task: id } | Operation::ConfirmFactor { task: id, .. } => {
                let Some(offer) = s.offers.get_mut(id) else {
                    return Ok(None);
                };
                let t = &offer.task;
                let key = device_access::CandidateKey::for_task(device_access::Channel::Da, t);
                let mut store = self.store()?;
                if let Operation::ConfirmFactor { code, password, .. } = input {
                    let clock = device_access::windows::clock().map_err(|_| Error::Clock)?;
                    let create = matches!(t.action, w::AccessAction::Create { .. });
                    if password.is_some() != create
                        || password
                            .as_ref()
                            .is_some_and(|v| v.is_empty() || v.len() > 1024 || v.contains('\0'))
                    {
                        return Err(Error::Protocol);
                    }
                    let candidate = offer.setup.as_mut().ok_or(Error::Denied)?;
                    if !candidate
                        .confirm(code, clock)
                        .map_err(|_| Error::Protocol)?
                    {
                        return Err(Error::Denied);
                    }
                    if store.has_candidate(key).map_err(|_| Error::Storage)? {
                        if !store
                            .reconfirm(key, code, clock, &SystemProtector)
                            .map_err(|_| Error::Storage)?
                        {
                            return Err(Error::Denied);
                        }
                    } else {
                        store
                            .stage(key, candidate, &SystemProtector)
                            .map_err(|_| Error::Storage)?;
                    }
                    offer.password = password.clone();
                    offer.confirmed = true;
                    self.wake.notify_one();
                    Response::Prepared {}
                } else {
                    if offer.setup.is_none() {
                        offer.setup =
                            Some(if store.has_candidate(key).map_err(|_| Error::Storage)? {
                                store
                                    .candidate(key, &SystemProtector)
                                    .map_err(|_| Error::Storage)?
                            } else {
                                Candidate::generate().map_err(|_| Error::Unavailable)?
                            });
                    }
                    let candidate = offer.setup.as_ref().ok_or(Error::Storage)?;
                    Response::Setup {
                        task: *id,
                        secret: Zeroizing::new(
                            data_encoding::BASE32_NOPAD.encode(candidate.totp_secret()),
                        ),
                    }
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(Reply::DeviceAccess { value }))
    }
}
fn utc() -> Result<i64, Error> {
    Ok((crate::wall()? / 1000) as i64)
}
impl SessionFence for Login {
    fn revoke(&self, peer: &Peer) -> Reply {
        let other = self.other.as_ref().map(|f| f.revoke(peer));
        if let (Ok(subject), Ok(binding), Ok(mut s)) = (
            peer.authenticate(&self.deployment.registration_clients),
            peer.session_binding(),
            self.state.lock(),
        ) {
            let Some(revision) = s.revision.checked_add(1) else {
                return Reply::Unavailable;
            };
            s.revision = revision;
            if s.employee
                .as_ref()
                .is_some_and(|e| e.subject == subject && e.binding == binding)
            {
                s.logout();
            }
            Reply::ContextRevoked
        } else {
            other.unwrap_or(Reply::Rejected)
        }
    }
}
