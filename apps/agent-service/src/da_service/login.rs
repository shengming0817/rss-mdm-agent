//! Desktop setup holds secrets only for the authenticated console login and exact DA attempt.
use super::*;
use device_access::{Candidate, Store, SystemProtector};
use execution_contract::{Id, NativeContextRef};
use execution_ipc::{
    device_access::{Operation, Response, SetupTask},
    host::{ClientOrigin, NativeContextSession},
};
use rss_mdm_agent_wire::runtime as w;
use std::sync::Mutex;
use zeroize::Zeroizing;

struct Employee {
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
    employee: Option<Employee>,
    task: Option<w::AccessTaskSpec>,
    setup: Option<Candidate>,
    confirmed: bool,
    password: Option<Zeroizing<String>>,
}
impl State {
    fn clear_setup(&mut self) {
        self.setup = None;
        self.confirmed = false;
        self.password = None;
    }
    fn logout(&mut self) {
        self.clear_setup();
        self.employee = None;
    }
}
pub(super) struct Login {
    deployment: Deployment,
    state: Mutex<State>,
    other: Option<Arc<dyn SessionFence>>,
}
impl Login {
    pub(super) fn new(d: &Deployment, other: Option<Arc<dyn SessionFence>>) -> Self {
        Self {
            deployment: d.clone(),
            state: Mutex::default(),
            other,
        }
    }
    fn store(&self) -> Result<Store, Error> {
        Store::open(
            &crate::access_worker::root(&self.deployment.execution),
            self.deployment.installation_id,
        )
        .map_err(|_| Error::Storage)
    }
    pub(super) fn offer(&self, t: Option<&w::AccessTaskSpec>) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        if s.task.as_ref() != t {
            s.clear_setup();
            s.task = t.cloned();
        }
        Ok(())
    }
    /// Release one confirmed creation password while the same console/employee binding is live.
    pub(super) fn input(&self, t: &w::AccessTaskSpec) -> Result<Option<Zeroizing<String>>, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        if s.task.as_ref() != Some(t)
            || !s.confirmed
            || !s.employee.as_ref().is_some_and(|e| e.permits(t))
        {
            return Err(Error::Denied);
        }
        Ok(s.password.take())
    }
    fn bind(
        &self,
        peer: &Peer,
        subject: String,
        value: &NativeContextSession,
        expected: Option<&Id>,
    ) -> Result<Reply, Error> {
        let backend = self.deployment.modules.da.backend()?;
        let binding = peer.session_binding().map_err(|_| Error::Identity)?;
        execution_ipc::windows_identity::verify_unlocked_console(
            &subject,
            peer.session(),
            &binding,
        )
        .map_err(|_| Error::Identity)?;
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
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
        if user.tenant_id != backend.tenant
            || user.instance_id != backend.instance
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
        let subject = peer
            .authenticate(&self.deployment.registration_clients)
            .ok()?;
        match request {
            Request::BindContext { value, expected } => Some(
                self.bind(peer, subject, value, expected.as_ref())
                    .unwrap_or(Reply::Rejected),
            ),
            Request::DeviceAccess {
                context,
                origin: ClientOrigin::Desktop {},
                input,
            } => self
                .access(peer, &subject, context, input)
                .unwrap_or(Some(Reply::Rejected)),
            _ => None,
        }
    }
    fn access(
        &self,
        peer: &Peer,
        subject: &str,
        context: &NativeContextRef,
        input: &Operation,
    ) -> Result<Option<Reply>, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Unavailable)?;
        let e = s.employee.as_ref().ok_or(Error::Identity)?;
        if &e.context != context
            || e.subject != subject
            || e.binding != peer.session_binding().map_err(|_| Error::Identity)?
            || !e.current()
        {
            return Err(Error::Identity);
        }
        let task = s
            .task
            .as_ref()
            .filter(|t| {
                e.permits(t)
                    && matches!(t.windows, Some(w::AccessWindowsPlan::Initialize { .. }))
                    && t.expires_at > utc().unwrap_or(i64::MAX)
            })
            .cloned();
        let value = match input {
            Operation::Tasks {} => Response::Tasks {
                tasks: task
                    .iter()
                    .map(|t| SetupTask {
                        task: t.task_id,
                        binding: t.binding_id,
                        create_name: match &t.action {
                            w::AccessAction::Create { name, .. } => Some(name.clone()),
                            _ => None,
                        },
                    })
                    .collect(),
            },
            Operation::Prepare { task: id } | Operation::ConfirmFactor { task: id, .. } => {
                let Some(t) = task.filter(|t| t.task_id == *id) else {
                    return Ok(None);
                };
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
                    let candidate = s.setup.as_mut().ok_or(Error::Denied)?;
                    if !candidate
                        .confirm(code, clock)
                        .map_err(|_| Error::Protocol)?
                    {
                        return Err(Error::Denied);
                    }
                    if store.has_candidate(*id).map_err(|_| Error::Storage)? {
                        if !store
                            .reconfirm(*id, code, clock, &SystemProtector)
                            .map_err(|_| Error::Storage)?
                        {
                            return Err(Error::Denied);
                        }
                    } else {
                        store
                            .stage(*id, candidate, &SystemProtector)
                            .map_err(|_| Error::Storage)?;
                    }
                    s.password = password.clone();
                    s.confirmed = true;
                    Response::Prepared {}
                } else {
                    if s.setup.is_none() {
                        s.setup = Some(if store.has_candidate(*id).map_err(|_| Error::Storage)? {
                            store
                                .candidate(*id, &SystemProtector)
                                .map_err(|_| Error::Storage)?
                        } else {
                            Candidate::generate().map_err(|_| Error::Unavailable)?
                        });
                    }
                    let candidate = s.setup.as_ref().ok_or(Error::Storage)?;
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
