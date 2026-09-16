use crate::{ApprovalOutcome, ProfileApproval};
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
}
impl ApprovalDecision {
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
