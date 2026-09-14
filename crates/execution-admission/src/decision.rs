use crate::VerificationError;
use execution_contract::{Digest, Id, PlanId, VersionedRef};

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
}
impl AdmissionDecision {
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
