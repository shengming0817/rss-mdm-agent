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
    pub current: Arc<Mutex<Option<u64>>>,
    pub console: fn(&str, u32, &Id) -> Result<(), execution_app::Error>,
    pub expires: u64,
}
impl Employee {
    pub(super) fn admit<T>(
        &self,
        task: &w::AccessTaskSpec,
        action: impl FnOnce() -> Result<T, Error>,
    ) -> Result<T, Error> {
        let epoch = self.epoch.lock().map_err(|_| Error::Unavailable)?;
        let current = self.current.lock().map_err(|_| Error::Unavailable)?;
        let now = crate::wall()?;
        if *epoch != self.generation
            || current.is_none_or(|until| now >= until)
            || now >= self.expires
            || (now / 1000) as i64 >= task.expires_at.min(task.deadline)
            || (self.portal.tenant, self.portal.instance, self.principal)
                != (task.tenant_id, task.instance_id, task.principal_id)
            || (self.console)(&self.subject, self.session, &self.binding).is_err()
        {
            return Err(Error::Identity);
        }
        // Logout takes epoch, replacement takes current. Keep both through the actual native write.
        action()
    }
    pub(super) fn permits(&self, task: &w::AccessTaskSpec) -> bool {
        self.admit(task, || Ok(())).is_ok()
    }
    pub(super) async fn verify(&mut self, t: &w::AccessTaskSpec) -> Result<(), Error> {
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
        self.expires = self
            .expires
            .min((expires.max(0) as u64).saturating_mul(1000));
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
        if s.input
            .as_ref()
            .is_some_and(|i| i.until > std::time::Instant::now() && i.owner.permits(&i.task))
        {
            return Err(Error::Conflict);
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
}

pub(super) struct NativeInput<'a> {
    pub setup: &'a Setup,
    pub task: &'a w::AccessTaskSpec,
    pub employee: &'a Employee,
}
impl NativeInput<'_> {
    fn with_input<T>(
        &self,
        use_input: impl FnOnce(&mut Option<Input>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut state = self.setup.state.lock().map_err(|_| Error::Storage)?;
        self.employee.admit(self.task, || {
            let i = state.input.as_ref().ok_or(Error::Denied)?;
            if i.task != *self.task
                || !Arc::ptr_eq(&self.employee.current, &i.owner.current)
                || i.until <= std::time::Instant::now()
            {
                return Err(Error::Denied);
            }
            use_input(&mut state.input)
        })
    }
}
impl creation::Credential for NativeInput<'_> {
    fn admit<T>(&self, action: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        self.with_input(|_| action())
    }
    fn send(&mut self, sink: &mut dyn std::io::Write) -> Result<(), Error> {
        self.with_input(|slot| {
            let input = slot.take().ok_or(Error::Denied)?;
            sink.write_all(input.bytes.as_bytes())?;
            sink.write_all(b"\n")?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access_worker::macos::creation::Credential;

    fn employee(t: &w::AccessTaskSpec) -> Employee {
        let expiry = crate::wall().unwrap() + 60_000;
        Employee {
            portal: EmployeePortal::configured(
                t.tenant_id,
                t.instance_id,
                Some(("https://employee.example", None)),
            )
            .unwrap()
            .unwrap(),
            principal: t.principal_id,
            proof: Zeroizing::new("fixture".into()),
            subject: "501".into(),
            session: 1,
            binding: Id::new("console").unwrap(),
            epoch: Arc::new(Mutex::new(1)),
            generation: 1,
            current: Arc::new(Mutex::new(Some(expiry))),
            expires: expiry,
            console: |_, _, _| Ok(()),
        }
    }
    #[tokio::test]
    async fn employee_confirmation_is_exclusive_and_fenced_at_native_submission() {
        let t = super::super::tests::task();
        let mut second = t.clone();
        second.task_id = Uuid::new_v4();
        second.attempt_id = Uuid::new_v4();
        let setup = Arc::new(Setup::default());
        let e = employee(&t);
        let pw = Zeroizing::new("private-test-input".into());
        setup.offer(&t).unwrap();
        setup.offer(&second).unwrap();
        assert_eq!(setup.tasks(&e).unwrap().len(), 2);
        let mut foreign = e.clone();
        foreign.principal = Uuid::new_v4();
        assert!(setup.tasks(&foreign).unwrap().is_empty());
        assert!(setup.prepare(ids(&t), foreign, &pw).is_err());
        assert!(setup
            .prepare((t.task_id, second.attempt_id), e.clone(), &pw)
            .is_err());
        setup.prepare(ids(&t), e.clone(), &pw).unwrap();
        assert_eq!(
            setup.prepare(ids(&second), e.clone(), &pw),
            Err(Error::Conflict)
        );
        let mut native = NativeInput {
            setup: &setup,
            task: &t,
            employee: &e,
        };
        native.admit(|| Ok(())).unwrap();
        // A same-login refresh updates the lifetime of this same confirmation.
        *e.current.lock().unwrap() = Some(crate::wall().unwrap() + 31_000);
        let mut bytes = Vec::new();
        native.send(&mut bytes).unwrap();
        assert_eq!(bytes, b"private-test-input\n");
        assert!(native.send(&mut bytes).is_err());
        setup.prepare(ids(&t), e.clone(), &pw).unwrap();
        native.admit(|| Ok(())).unwrap();
        *e.epoch.lock().unwrap() += 1;
        assert!(native.send(&mut Vec::new()).is_err());
        assert!(native
            .admit::<()>(|| panic!("native write after logout"))
            .is_err_and(|err| err == Error::Identity));
    }
}
