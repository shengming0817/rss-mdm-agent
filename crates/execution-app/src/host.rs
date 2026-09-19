use crate::*;
use execution_contract::{AttemptId, EvidenceRef, FrozenPlan};
use execution_lifecycle::{ObservationError, ObservationFacts, ObservationVerifier};
use execution_sqlite::{self as db, AccessRequest, AdmissionGate, Scope, TrustSnapshot};

pub(crate) struct Host<'a, H> {
    pub inner: &'a H,
    pub binding: &'a Binding,
    pub config: Option<AppConfig>,
    pub plan: Option<&'a FrozenPlan>,
    pub observation: Option<&'a ObservationFacts>,
}
pub(crate) fn capabilities(
    host: &impl AppHost,
    plan: &FrozenPlan,
    config: AppConfig,
) -> Result<(), Error> {
    let snapshot = host.capabilities(plan)?;
    let now = host.reliable_now()?;
    let budget = plan.spec().budget;
    if snapshot.verified_at_unix_ms > now
        || now >= snapshot.fresh_until_unix_ms
        || budget.total_timeout_ms > config.max_timeout_ms
        || budget.total_output_bytes > config.max_output_bytes
    {
        return Err(Error::Capability);
    }
    let report = execution_capability::match_capabilities(
        plan,
        &snapshot.environment,
        execution_capability::MatchLimits {
            max_entries: config.max_capability_entries,
        },
    )
    .map_err(|_| Error::Capability)?;
    if report.status != execution_capability::MatchStatus::Supported {
        return Err(Error::Capability);
    }
    Ok(())
}
impl<H: AppHost> ObservationVerifier for Host<'_, H> {
    fn verify(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        evidence: &EvidenceRef,
        now: u64,
    ) -> Result<ObservationFacts, ObservationError> {
        let facts = self.observation.ok_or(ObservationError::Unavailable)?;
        if facts.plan_id != plan.spec().plan_id
            || &facts.plan_digest != plan.digest()
            || &facts.attempt_id != attempt
            || &facts.evidence != evidence
            || facts.observed_at_unix_ms > now
        {
            return Err(ObservationError::Untrusted);
        }
        Ok(facts.clone())
    }
}
impl<H: AppHost> db::Host for Host<'_, H> {
    fn authorize(&self, request: AccessRequest<'_>) -> Result<(), db::Error> {
        let actual = self.inner.binding().map_err(|_| db::Error::Denied)?;
        if &actual != self.binding
            || request.scope.authority != actual.authority
            || request.scope.actor != actual.actor
        {
            return Err(db::Error::Denied);
        }
        self.inner.authorize(request)
    }
    fn reliable_now(&self) -> Result<u64, db::Error> {
        self.inner.reliable_now()
    }
    fn trusted_snapshot(&self, scope: &Scope) -> Result<TrustSnapshot, db::Error> {
        let plan = self.plan.ok_or(db::Error::Trust)?;
        if &Scope::from_plan(plan) != scope {
            return Err(db::Error::Trust);
        }
        self.inner.trusted_snapshot(plan)
    }
    fn admit(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        bindings: &[execution_approval::ProfileApproval],
        approvals: &dyn execution_approval::ApprovalVerifier,
        _now: u64,
    ) -> Result<AdmissionGate, db::Error> {
        let config = self.config.ok_or(db::Error::Trust)?;
        capabilities(self.inner, plan, config).map_err(|_| db::Error::Trust)?;
        let admission = execution_admission::decide(
            plan,
            attempt,
            self.inner,
            execution_admission::AdmissionLimits {
                max_rules: config.max_rules,
            },
        );
        let approval = execution_approval::evaluate(
            plan,
            &admission,
            bindings,
            approvals,
            execution_approval::ApprovalLimits {
                max_profiles: config.max_profiles,
                max_records: config.max_profiles,
            },
        );
        Ok(AdmissionGate {
            admission,
            approval,
        })
    }
}
