use crate::{ApprovalOutcome, ProfileApproval};
use execution_admission::AdmissionValidity;
use execution_contract::{AttemptId, Digest, PlanId, VersionedRef};

/// Candidate consume-one CAS, committed together with its attempt intent by C18.
/// The authority namespace comes from the identically bound frozen plan.
/// Re-evaluation is not consumption; cloning never grants another use.
/// INVARIANT: APPROVAL-CONSUMPTION-01 — only successful evaluation constructs this value.
/// ~~~compile_fail
/// let _: execution_approval::ConsumptionIntent = serde_json::from_str("{}").unwrap();
/// ~~~
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumptionIntent {
    pub(crate) plan_id: PlanId,
    pub(crate) plan_digest: Digest,
    pub(crate) attempt_id: AttemptId,
    pub(crate) approval: VersionedRef,
    pub(crate) expected_consumption_revision: u64,
    pub(crate) expected_uses: u32,
    pub(crate) verification_revision: VersionedRef,
    pub(crate) valid_until_unix_ms: u64,
}
impl ConsumptionIntent {
    /// Exact frozen plan identity.
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    /// Exact canonical plan digest.
    pub fn plan_digest(&self) -> &Digest {
        &self.plan_digest
    }
    /// Stable attempt identity used for durable idempotency.
    pub fn attempt_id(&self) -> &AttemptId {
        &self.attempt_id
    }
    /// Exact approval record revision.
    pub fn approval(&self) -> &VersionedRef {
        &self.approval
    }
    /// Consumption CAS revision that must still match at commit.
    pub fn expected_consumption_revision(&self) -> u64 {
        self.expected_consumption_revision
    }
    /// Counter value that must still match; the transaction adds exactly one.
    pub fn expected_uses(&self) -> u32 {
        self.expected_uses
    }
    /// Trust/revocation snapshot that must still be acceptable at commit.
    pub fn verification_revision(&self) -> &VersionedRef {
        &self.verification_revision
    }
    /// Exclusive deadline; persistence must reject stale intents.
    pub fn valid_until_unix_ms(&self) -> u64 {
        self.valid_until_unix_ms
    }
}
/// Bound approval result, never a cached authorization.
/// C19 must obtain a fresh C07 decision for every new attempt.
/// ~~~compile_fail
/// let _: execution_approval::ApprovalDecision = serde_json::from_str("{}").unwrap();
/// ~~~
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecision {
    pub(crate) plan_id: PlanId,
    pub(crate) plan_digest: Digest,
    pub(crate) attempt_id: AttemptId,
    pub(crate) outcome: ApprovalOutcome,
    pub(crate) bindings: Vec<ProfileApproval>,
    pub(crate) consumptions: Vec<ConsumptionIntent>,
    pub(crate) admission_validity: Option<AdmissionValidity>,
}
impl ApprovalDecision {
    /// Authorization identity inherited from C07, including the no-approval path.
    pub fn admission_validity(&self) -> Option<&AdmissionValidity> {
        self.admission_validity.as_ref()
    }
    /// Required C18 check within the same transaction as lifecycle/consumption CAS.
    /// The current authority revision and time must come from the trusted storage host.
    /// This validates the immutable identity; it does not perform or prove persistence.
    pub fn valid_for_commit(
        &self,
        plan: &execution_contract::FrozenPlan,
        attempt: &AttemptId,
        now: u64,
        authority_revision: &VersionedRef,
    ) -> bool {
        self.plan_id == plan.spec().plan_id
            && &self.plan_digest == plan.digest()
            && &self.attempt_id == attempt
            && matches!(
                self.outcome,
                ApprovalOutcome::NotRequired | ApprovalOutcome::Satisfied
            )
            && self
                .admission_validity
                .as_ref()
                .is_some_and(|v| v.is_current(now, authority_revision))
            && self
                .consumptions
                .iter()
                .all(|c| now < c.valid_until_unix_ms)
    }
    /// Exact plan identity evaluated.
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    /// Exact canonical plan digest evaluated.
    pub fn plan_digest(&self) -> &Digest {
        &self.plan_digest
    }
    /// Attempt for which pending consumptions were calculated.
    pub fn attempt_id(&self) -> &AttemptId {
        &self.attempt_id
    }
    /// Closed approval applicability outcome.
    pub fn outcome(&self) -> &ApprovalOutcome {
        &self.outcome
    }
    /// Verified profile mapping, sorted by profile; empty unless satisfied.
    pub fn bindings(&self) -> &[ProfileApproval] {
        &self.bindings
    }
    /// One pending consumption per distinct record, sorted by reference.
    pub fn consumptions(&self) -> &[ConsumptionIntent] {
        &self.consumptions
    }
}
