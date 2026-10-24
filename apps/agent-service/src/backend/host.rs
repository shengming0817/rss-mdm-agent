use crate::SystemClock;
use execution_admission::{
    AuthorityFacts, AuthorityVerifier, Rule, RuleEffect, SubjectFacts, VerificationError,
};
use execution_app::{AccessRequest, TrustSnapshot};
use execution_app::{
    AppHost, CapabilitySnapshot, ConfigChange, Error, RequestContext, ServiceBinding,
};
use execution_capability::*;
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
impl AuthorityVerifier for EnterpriseHost {
    fn verify(
        &self,
        plan: &FrozenExecution,
        _: &AttemptId,
    ) -> Result<AuthorityFacts, VerificationError> {
        let p = plan.spec();
        if p.request.authority == self.root.authority
            && matches!(p.request.authority, Authority::Local { .. })
        {
            if self
                .local_stopped
                .load(std::sync::atomic::Ordering::Acquire)
                || p.request.actor != self.local_actor
                || p.request.target.device != self.root.device
            {
                return Err(VerificationError::Subject);
            }
            let now = self.clock.millis().map_err(|_| VerificationError::Clock)?;
            let software = self
                .local_software
                .lock()
                .map_err(|_| VerificationError::Subject)?;
            let read = self
                .local_read
                .lock()
                .map_err(|_| VerificationError::Subject)?;
            let settings = self
                .local_settings
                .lock()
                .map_err(|_| VerificationError::Subject)?;
            let network = self
                .local_network
                .lock()
                .map_err(|_| VerificationError::Subject)?;
            let risk = if let Some(permit) = network.as_ref() {
                permit
                    .verify(plan, now)
                    .map_err(|_| VerificationError::Subject)?;
                RiskLevel::One
            } else if let Some(permit) = settings.as_ref() {
                permit
                    .verify(plan, now)
                    .map_err(|_| VerificationError::Subject)?
            } else if let Some(permit) = software.as_ref().filter(|v| v.matches(plan)) {
                permit
                    .verify(plan, now)
                    .map_err(|_| VerificationError::Subject)?;
                RiskLevel::Two
            } else if let Some(permit) = read.as_ref() {
                permit
                    .verify(plan, now)
                    .map_err(|_| VerificationError::Subject)?;
                RiskLevel::One
            } else {
                return Err(VerificationError::Subject);
            };
            return Ok(AuthorityFacts {
                verified_origin: p.request.initiator.clone(),
                risk: Some(risk),
                subject: SubjectFacts {
                    authority: self.root.authority.clone(),
                    actor: self.local_actor.clone(),
                    validity: p.validity,
                    budget: p.budget,
                },
                delegation: None,
                policy: p.policy.clone(),
                rules: vec![Rule {
                    id: p.policy.id.clone(),
                    template: plan.clone(),
                    effect: RuleEffect::Allow,
                }],
                now_unix_ms: now,
                verification_revision: p.policy.clone(),
                fresh_until_unix_ms: p.validity.expires_at_unix_ms,
            });
        }
        let grant = self.permit(plan).map_err(|_| VerificationError::Subject)?;
        let p = grant.plan.spec();
        let now = self.clock.millis().map_err(|_| VerificationError::Clock)?;
        // This template was deterministically adapted from the verified backend Start.
        // No submitted IPC/AI plan supplies an allow rule and there is no second approver.
        Ok(AuthorityFacts {
            verified_origin: p.request.initiator.clone(),
            risk: grant.gate.risk(),
            subject: SubjectFacts {
                authority: self.binding.authority.clone(),
                actor: self.actor.clone(),
                validity: p.validity,
                budget: p.budget,
            },
            delegation: None,
            policy: p.policy.clone(),
            rules: vec![Rule {
                id: p.policy.id.clone(),
                template: grant.plan.clone(),
                effect: RuleEffect::Allow,
            }],
            now_unix_ms: now,
            verification_revision: p.policy.clone(),
            fresh_until_unix_ms: grant.gate.until(),
        })
    }
}
fn inventory<T>(items: Vec<T>) -> Inventory<T> {
    Inventory {
        complete: true,
        entries: items
            .into_iter()
            .map(|capability| Entry {
                capability,
                availability: Availability::Available,
            })
            .collect(),
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
    fn capabilities(&self, plan: &FrozenExecution) -> Result<CapabilitySnapshot, Error> {
        let binding = self.binding_for_authority(&plan.spec().request.authority)?;
        self.materials.inspect(plan)?;
        let now = self.reliable_now()?;
        let p = plan.spec();
        Ok(CapabilitySnapshot {
            verified_at_unix_ms: now,
            fresh_until_unix_ms: now + 1000,
            environment: EnvironmentSnapshot {
                authority: binding.authority,
                device: binding.device,
                source: p.policy.clone(),
                platform: Some(p.request.target.platform),
                interpreters: inventory(vec![p.launch.interpreter.clone()]),
                software: inventory(if let Some(program) = p.execution.software_program() {
                    let mut kinds = Vec::new();
                    for step in &program.steps {
                        if !kinds.contains(&step.format.adapter()) {
                            kinds.push(step.format.adapter());
                        }
                    }
                    kinds
                } else {
                    vec![]
                }),
                launch_io: inventory(vec![
                    LaunchIoCapability::ControlledStdin(TextEncoding::Utf8),
                    LaunchIoCapability::CapturedText(TextEncoding::Utf8),
                    LaunchIoCapability::CapturedText(TextEncoding::Utf16Le),
                ]),
                run_as: inventory(vec![p.run_as.clone()]),
                user_sessions: inventory(match &p.run_as {
                    RunAs::User { account } => vec![account.clone()],
                    _ => vec![],
                }),
                isolation: inventory(vec![]),
            },
        })
    }
    fn trusted_snapshot(
        &self,
        plan: &FrozenExecution,
    ) -> Result<TrustSnapshot, execution_app::JournalError> {
        self.service_binding()
            .map_err(|_| execution_app::JournalError::Trust)?;
        let input = plan.spec();
        let binding = self
            .binding_for_actor(&input.request.actor)
            .map_err(|_| execution_app::JournalError::Trust)?;
        if input.request.authority != binding.authority
            || input.request.target.device != binding.device
        {
            return Err(execution_app::JournalError::Trust);
        }
        // Registration scope remains bound to this frozen journal after restart or a later
        // dispatch. The snapshot grants no attempt: AuthorityVerifier independently requires
        // the exact signed Start and its shorter deadline. Avoid a local one-second window
        // that expires while retained software materials are checked; never renew plan validity.
        Ok(TrustSnapshot {
            authorization_revision: input.policy.clone(),
            approval_revision: input.policy.clone(),
            fresh_until_unix_ms: input.validity.expires_at_unix_ms,
            approvals: vec![],
        })
    }
    fn approval_bindings(
        &self,
        _: &FrozenExecution,
    ) -> Result<Vec<execution_approval::ProfileApproval>, Error> {
        Ok(vec![])
    }
    fn configuration_change(&self, _: &ConfigChange) -> Result<(), Error> {
        Err(Error::Denied)
    }
}
