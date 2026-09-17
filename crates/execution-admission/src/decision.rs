use crate::VerificationError;
use execution_contract::{AttemptId, Digest, Id, PlanId, VersionedRef};

/// Authorization result only. None of these variants is an execution permit or consumed approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionOutcome {
    /// The verified policy authorizes this plan; other execution gates still apply.
    Allowed,
    /// Not authorized; user confirmation cannot turn this into permission.
    Denied,
    /// Conditional eligibility for all listed approval profiles, bound to the returned plan digest.
    ApprovalRequired {
        /// Exact profiles sorted by ID/revision with duplicates removed.
        profiles: Vec<VersionedRef>,
    },
}
/// Stable reasons without payloads, paths, parameters or provider error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Trusted authorization snapshot has no remaining freshness window.
    StaleVerification,
    /// Invalid evaluator configuration or excessive rule count.
    Limit,
    /// Trusted adapter could not verify required facts.
    Verification(VerificationError),
    /// Authenticated authority/actor differs from the request.
    SubjectMismatch,
    /// Current policy does not match plan or rule version, or rule IDs collide.
    InvalidPolicy,
    /// The plan or subject window is not valid for this decision.
    Validity,
    /// A plan exceeds an actor-wide budget ceiling.
    Budget,
    /// Delegation is missing, extra, mismatched or outside its exact scope/bounds.
    Delegation,
    /// No complete allow or conditional-approval rule matches.
    NoMatchingRule,
    /// At least one active matching rule explicitly denies this plan.
    ExplicitDeny,
    /// All mandatory authorization gates passed with an allow rule.
    RuleAllowed,
    /// One or more rules explicitly require subsequent approval.
    NeedsApproval,
}
/// Immutable decision bound to the evaluated plan, never deserializable as a permission.
/// ```compile_fail
/// let _: execution_admission::AdmissionDecision = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionDecision {
    pub(crate) plan_id: PlanId,
    pub(crate) plan_digest: Digest,
    pub(crate) policy: VersionedRef,
    pub(crate) delegation: Option<VersionedRef>,
    pub(crate) outcome: DecisionOutcome,
    pub(crate) reason: Reason,
    pub(crate) rule_ids: Vec<Id>,
    pub(crate) attempt_id: AttemptId,
    pub(crate) validity: Option<AdmissionValidity>,
}
/// Verified authorization snapshot, privately constructed by C07.
/// Persistence must recheck this revision and exclusive deadline at atomic admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionValidity {
    pub(crate) revision: VersionedRef,
    pub(crate) verified_at_unix_ms: u64,
    pub(crate) valid_until_unix_ms: u64,
}
impl AdmissionValidity {
    /// Subject/delegation/policy/revocation snapshot that must still be current.
    pub fn revision(&self) -> &VersionedRef {
        &self.revision
    }
    /// Reliable host time at verification; commit must not move backwards.
    pub fn verified_at_unix_ms(&self) -> u64 {
        self.verified_at_unix_ms
    }
    /// Exclusive deadline capped by the frozen plan's validity.
    pub fn valid_until_unix_ms(&self) -> u64 {
        self.valid_until_unix_ms
    }
    /// Check reliable commit time and the current trusted authority revision.
    pub fn is_current(&self, now: u64, revision: &VersionedRef) -> bool {
        revision == &self.revision
            && self.verified_at_unix_ms <= now
            && now < self.valid_until_unix_ms
    }
}
impl AdmissionDecision {
    /// Exact attempt authenticated through the authority port. It cannot be rebound.
    pub fn attempt_id(&self) -> &AttemptId {
        &self.attempt_id
    }
    /// Verified freshness identity; denied decisions never carry one.
    pub fn validity(&self) -> Option<&AdmissionValidity> {
        self.validity.as_ref()
    }
    /// Exact plan identity; no newer plan is implicitly substituted.
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    /// Exact plan digest, covering identities, execution content, constraints and all bounds.
    pub fn plan_digest(&self) -> &Digest {
        &self.plan_digest
    }
    /// Policy reference claimed by the evaluated plan; InvalidPolicy means it was not verified.
    pub fn policy(&self) -> &VersionedRef {
        &self.policy
    }
    /// Delegation reference claimed by the plan; denial does not authenticate that reference.
    pub fn delegation(&self) -> Option<&VersionedRef> {
        self.delegation.as_ref()
    }
    /// Closed authorization outcome.
    pub fn outcome(&self) -> &DecisionOutcome {
        &self.outcome
    }
    /// Stable explanation without leaking submitted content.
    pub fn reason(&self) -> Reason {
        self.reason
    }
    /// Matching rule IDs sorted and deduplicated; empty when no rules were applied.
    pub fn rule_ids(&self) -> &[Id] {
        &self.rule_ids
    }
}
