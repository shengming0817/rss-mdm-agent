use super::SystemClock;
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

pub(super) struct BackendPermit {
    pub plan: FrozenExecution,
    pub start: agent_client::Start,
}
#[derive(Clone)]
pub(super) struct EnterpriseHost {
    pub binding: ServiceBinding,
    pub actor: ActorId,
    pub clock: SystemClock,
    pub materials: MaterialRegistry,
    pub subject: String,
    pub current: Arc<Mutex<Option<Arc<BackendPermit>>>>,
    pub revoked: Arc<std::sync::atomic::AtomicBool>,
}
impl EnterpriseHost {
    fn permit(&self, plan: &FrozenExecution) -> Result<Arc<BackendPermit>, Error> {
        if self.revoked.load(std::sync::atomic::Ordering::Acquire) {
            return Err(Error::Denied);
        }
        let permit = self
            .current
            .lock()
            .map_err(|_| Error::Unavailable)?
            .clone()
            .ok_or(Error::Denied)?;
        if permit.plan.digest() != plan.digest()
            || permit.start.payload().permit() != agent_client::wire::TaskPermit::Start
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
        let grant = self.permit(plan).map_err(|_| VerificationError::Subject)?;
        let p = grant.plan.spec();
        let now = self.clock.millis().map_err(|_| VerificationError::Clock)?;
        // This template was deterministically adapted from the verified backend Start.
        // No submitted IPC/AI plan supplies an allow rule and there is no second approver.
        Ok(AuthorityFacts {
            verified_origin: p.request.initiator.clone(),
            risk: None,
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
            fresh_until_unix_ms: u64::try_from(grant.start.payload().expires_at())
                .map_err(|_| VerificationError::Clock)?
                .checked_mul(1000)
                .ok_or(VerificationError::Clock)?,
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
        if execution_runner::host::current_subject()? != self.subject {
            return Err(Error::Unbound);
        }
        Ok(self.binding.clone())
    }
    fn authorize(
        &self,
        caller: &RequestContext,
        r: AccessRequest<'_>,
    ) -> Result<(), execution_app::JournalError> {
        if caller.actor != self.actor {
            return Err(execution_app::JournalError::Denied);
        }
        self.authorize_service(r)
    }
    fn authorize_service(&self, r: AccessRequest<'_>) -> Result<(), execution_app::JournalError> {
        if r.scope.authority != self.binding.authority || r.scope.actor != self.actor {
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
        self.materials.inspect(plan)?;
        let now = self.reliable_now()?;
        let p = plan.spec();
        Ok(CapabilitySnapshot {
            verified_at_unix_ms: now,
            fresh_until_unix_ms: now + 1000,
            environment: EnvironmentSnapshot {
                authority: self.binding.authority.clone(),
                device: self.binding.device.clone(),
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
        if input.request.authority != self.binding.authority
            || input.request.actor != self.actor
            || input.request.target.device != self.binding.device
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
