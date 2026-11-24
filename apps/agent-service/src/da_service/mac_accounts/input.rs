//! A native employee confirmation is valid for one foreground session and one exact permit.
use super::*;
use execution_contract::Id;
use execution_ipc::device_access::{EmployeePortal, SetupTask};
use std::{collections::BTreeMap, sync::Mutex};
use zeroize::Zeroizing;

#[derive(Clone)]
pub(crate) struct Employee {
    pub portal: EmployeePortal,
    pub principal: Uuid,
    pub proof: Zeroizing<String>,
    pub subject: String,
    pub session: u32,
    pub binding: Id,
    pub epoch: Arc<Mutex<u64>>,
    pub generation: u64,
    pub current: Arc<AtomicBool>,
    pub expires: u64,
}
impl Employee {
    pub(super) fn permits(&self, task: &w::AccessTaskSpec) -> bool {
        (self.portal.tenant, self.portal.instance, self.principal)
            == (task.tenant_id, task.instance_id, task.principal_id)
            && self.current.load(Ordering::Acquire)
            && crate::wall().is_ok_and(|now| now < self.expires)
            && self.epoch.lock().is_ok_and(|e| *e == self.generation)
            && execution_ipc::macos_identity::verify_console(
                &self.subject,
                self.session,
                &self.binding,
            )
            .is_ok()
    }
    pub(super) async fn verify(&self, t: &w::AccessTaskSpec) -> Result<(), Error> {
        if !self.permits(t) {
            return Err(Error::Identity);
        }
        let expires = da::verify_employee(
            &self.portal.origin,
            self.portal.ca_pem.as_deref(),
            self.portal.tenant,
            self.principal,
            &self.proof,
        )
        .await?;
        if expires <= seconds()? || !self.permits(t) {
            return Err(Error::Identity);
        }
        Ok(())
    }
}
struct Input {
    task: w::AccessTaskSpec,
    owner: Employee,
    bytes: Zeroizing<String>,
    until: std::time::Instant,
}
#[derive(Default)]
struct State {
    offers: BTreeMap<Ids, w::AccessTaskSpec>,
    input: Option<Input>,
}
#[derive(Default)]
pub(crate) struct Setup {
    state: Mutex<State>,
    pub wake: tokio::sync::Notify,
}
impl Setup {
    pub fn owns(&self, id: Ids) -> Result<bool, Error> {
        Ok(self
            .state
            .lock()
            .map_err(|_| Error::Storage)?
            .offers
            .contains_key(&id))
    }
    pub fn tasks(&self, e: &Employee) -> Result<Vec<SetupTask>, Error> {
        let now = seconds()?;
        let s = self.state.lock().map_err(|_| Error::Storage)?;
        Ok(s.offers
            .values()
            .filter(|t| t.expires_at > now && e.permits(t))
            .filter_map(|t| {
                let w::AccessAction::Create { name, .. } = &t.action else {
                    return None;
                };
                Some(SetupTask {
                    task: t.task_id,
                    attempt: t.attempt_id,
                    binding: t.binding_id,
                    create_name: Some(name.clone()),
                })
            })
            .collect())
    }
    pub fn prepare(
        self: &Arc<Self>,
        id: Ids,
        e: Employee,
        password: &Zeroizing<String>,
    ) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        let t = s.offers.get(&id).ok_or(Error::NotFound)?;
        if !e.permits(t)
            || t.expires_at <= seconds()?
            || password.is_empty()
            || password.len() > 1024
            || password.chars().any(char::is_control)
        {
            return Err(Error::Denied);
        }
        let until = std::time::Instant::now() + Duration::from_secs(30);
        s.input = Some(Input {
            task: t.clone(),
            owner: e,
            bytes: password.clone(),
            until,
        });
        let setup = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep_until(until.into()).await;
            if let Some(setup) = setup.upgrade() {
                if let Ok(mut state) = setup.state.lock() {
                    if state
                        .input
                        .as_ref()
                        .is_some_and(|i| i.until <= std::time::Instant::now())
                    {
                        state.input = None;
                    }
                }
            }
        });
        self.wake.notify_one();
        Ok(())
    }
    pub fn clear_input(&self) -> Result<(), Error> {
        self.state.lock().map_err(|_| Error::Storage)?.input = None;
        Ok(())
    }
    pub fn reset(&self) -> Result<(), Error> {
        *self.state.lock().map_err(|_| Error::Storage)? = State::default();
        Ok(())
    }
    pub(super) fn withdraw(&self, id: Ids) -> Result<(), Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        s.offers.remove(&id);
        if s.input.as_ref().is_some_and(|i| ids(&i.task) == id) {
            s.input = None;
        }
        Ok(())
    }
    pub(super) fn offer(&self, t: &w::AccessTaskSpec) -> Result<(), Error> {
        let now = seconds()?;
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        s.offers.retain(|_, o| {
            o.expires_at > now && (o.task_id != t.task_id || o.attempt_id == t.attempt_id)
        });
        if s.input
            .as_ref()
            .is_some_and(|i| i.task.task_id == t.task_id && i.task != *t)
        {
            s.input = None;
        }
        if s.offers.len() >= 64 && !s.offers.contains_key(&ids(t)) {
            return Err(Error::Capacity);
        }
        s.offers.insert(ids(t), t.clone());
        Ok(())
    }
    pub(super) fn confirmed(&self) -> Result<Option<(w::AccessTaskSpec, Employee)>, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        if s.input
            .as_ref()
            .is_some_and(|i| i.until <= std::time::Instant::now() || !i.owner.permits(&i.task))
        {
            s.input = None;
        }
        Ok(s.input.as_ref().map(|i| (i.task.clone(), i.owner.clone())))
    }
    pub(super) fn take(
        &self,
        t: &w::AccessTaskSpec,
        owner: &Employee,
    ) -> Result<Zeroizing<String>, Error> {
        let mut s = self.state.lock().map_err(|_| Error::Storage)?;
        let i = s.input.as_ref().ok_or(Error::Denied)?;
        if i.task != *t
            || !Arc::ptr_eq(&owner.current, &i.owner.current)
            || i.until <= std::time::Instant::now()
            || !i.owner.permits(t)
            || seconds()? >= t.expires_at
        {
            return Err(Error::Denied);
        }
        Ok(s.input.take().ok_or(Error::Denied)?.bytes)
    }
}
