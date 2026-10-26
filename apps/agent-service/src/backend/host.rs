use crate::SystemClock;
use execution_app::{ai_execution_gate, AccessRequest, AdmissionGate, ExecutionGate, JournalError};
use execution_app::{AppHost, Error, RequestContext, ServiceBinding};
use execution_contract::*;
use execution_runner::MaterialRegistry;
use std::sync::{Arc, Mutex};

pub(crate) struct BackendPermit {
    pub(crate) plan: FrozenExecution,
    pub(crate) start: agent_client::Start,
    pub(crate) gate: super::gate::ProductGateProof,
}
#[derive(Clone)]
pub(crate) struct EnterpriseHost {
    pub binding: ServiceBinding,
    pub root: ServiceBinding,
    pub local_actor: ActorId,
    pub namespaces: Vec<(ServiceBinding, ActorId)>,
    pub actor: ActorId,
    pub clock: SystemClock,
    pub materials: MaterialRegistry,
    pub subject: String,
    pub current: Arc<Mutex<Option<Arc<BackendPermit>>>>,
    pub local_network: Arc<Mutex<Option<crate::network_check::Permit>>>,
    pub local_settings: Arc<Mutex<Option<crate::local_settings::Permit>>>,
    pub local_read: Arc<Mutex<Option<crate::local_execution::ReadPermit>>>,
    pub local_software: Arc<Mutex<Option<Arc<crate::local_software::Permit>>>>,
    pub local_stopped: Arc<std::sync::atomic::AtomicBool>,
    pub identity_until: agent_client::RuntimeGate,
    pub revoked: Arc<std::sync::atomic::AtomicBool>,
}
impl EnterpriseHost {
    pub(crate) fn identity_ready(&self) -> bool {
        !self.revoked.load(std::sync::atomic::Ordering::Acquire)
            && self.clock.millis().is_ok_and(|now| {
                self.identity_until
                    .expires_at()
                    .is_some_and(|until| now / 1000 < until as u64)
            })
    }
    fn permit(&self, plan: &FrozenExecution) -> Result<Arc<BackendPermit>, Error> {
        if !self.identity_ready() {
            return Err(Error::Denied);
        }
        let permit = self
            .current
            .lock()
            .map_err(|_| Error::Unavailable)?
            .clone()
            .ok_or(Error::Denied)?;
        permit
            .gate
            .verify(plan, self.clock.millis().map_err(|_| Error::Clock)?)
            .map_err(|_| Error::Denied)?;
        if permit.plan.digest() != plan.digest()
            || permit.start.payload().permit() != rss_mdm_agent_wire::runtime::TaskPermit::Start
        {
            return Err(Error::Denied);
        }
        Ok(permit)
    }
}

impl AppHost for EnterpriseHost {
    fn service_binding(&self) -> Result<ServiceBinding, Error> {
        // Revocation blocks new admission, while the original journal remains available for
        // stopping/reconciliation and reporting; it never changes device or tenant identity.
        if execution_ipc::host::current_subject()? != self.subject {
            return Err(Error::Unbound);
        }
        Ok(self.root.clone())
    }
    fn binding_for_authority(&self, authority: &Authority) -> Result<ServiceBinding, Error> {
        self.service_binding()?;
        if authority == &self.root.authority {
            return Ok(self.root.clone());
        }
        if let Some((binding, _)) = self
            .namespaces
            .iter()
            .find(|(b, _)| &b.authority == authority)
        {
            return Ok(binding.clone());
        }
        Err(Error::Denied)
    }
    fn binding_for_actor(&self, actor: &ActorId) -> Result<ServiceBinding, Error> {
        if actor == &self.local_actor {
            return Ok(self.root.clone());
        }
        if let Some((binding, _)) = self.namespaces.iter().find(|(_, a)| a == actor) {
            return Ok(binding.clone());
        }
        Err(Error::Denied)
    }
    fn owns_device(&self, device: &DeviceId) -> Result<bool, Error> {
        Ok(device == &self.root.device || self.namespaces.iter().any(|(b, _)| &b.device == device))
    }
    fn authorize(
        &self,
        caller: &RequestContext,
        r: AccessRequest<'_>,
    ) -> Result<(), execution_app::JournalError> {
        if caller.actor != r.scope.actor {
            return Err(execution_app::JournalError::Denied);
        }
        self.authorize_service(r)
    }
    fn authorize_service(&self, r: AccessRequest<'_>) -> Result<(), execution_app::JournalError> {
        let binding = self
            .binding_for_actor(&r.scope.actor)
            .map_err(|_| execution_app::JournalError::Denied)?;
        if r.scope.authority != binding.authority {
            return Err(execution_app::JournalError::Denied);
        }
        Ok(())
    }
    fn reliable_now(&self) -> Result<u64, execution_app::JournalError> {
        self.clock
            .millis()
            .map_err(|_| execution_app::JournalError::Clock)
    }

    fn admit(
        &self,
        plan: &FrozenExecution,
        _: &AttemptId,
        now: u64,
    ) -> Result<AdmissionGate, JournalError> {
        self.materials
            .inspect(plan)
            .map_err(|_| JournalError::Trust)?;
        let p = plan.spec();
        let denied = || JournalError::Denied;
        if p.request.authority == self.root.authority
            && matches!(p.request.authority, Authority::Local { .. })
        {
            if self
                .local_stopped
                .load(std::sync::atomic::Ordering::Acquire)
                || p.request.actor != self.local_actor
                || p.request.target.device != self.root.device
            {
                return Err(denied());
            }
            let software = self.local_software.lock().map_err(|_| denied())?;
            let read = self.local_read.lock().map_err(|_| denied())?;
            let settings = self.local_settings.lock().map_err(|_| denied())?;
            let network = self.local_network.lock().map_err(|_| denied())?;
            let risk = if let Some(permit) = network.as_ref() {
                permit.verify(plan, now).map_err(|_| denied())?;
                RiskLevel::One
            } else if let Some(permit) = settings.as_ref() {
                permit.verify(plan, now).map_err(|_| denied())?
            } else if let Some(permit) = software.as_ref().filter(|v| v.matches(plan)) {
                permit.verify(plan, now).map_err(|_| denied())?;
                RiskLevel::Two
            } else if let Some(permit) = read.as_ref() {
                permit.verify(plan, now).map_err(|_| denied())?;
                RiskLevel::One
            } else {
                return Err(denied());
            };
            return Ok(AdmissionGate {
                execution_gate: if matches!(p.request.initiator, Initiator::Ai { .. }) {
                    ai_execution_gate(Some(risk))
                } else {
                    ExecutionGate::Direct
                },
                valid_until_unix_ms: p.validity.expires_at_unix_ms,
            });
        }
        let permit = self.permit(plan).map_err(|_| denied())?;
        Ok(AdmissionGate {
            execution_gate: ExecutionGate::Direct,
            valid_until_unix_ms: permit.gate.until(),
        })
    }
}
