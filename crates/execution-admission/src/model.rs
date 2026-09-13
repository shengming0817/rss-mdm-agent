use execution_contract::{
    ActorId, Authority, ExecutionBudget, FrozenPlan, Id, ValidityWindow, VersionedRef,
};

/// Authentication and actor-wide limits delivered by the trusted adapter, not an input DTO.
#[derive(Debug, Clone)]
pub struct SubjectFacts {
    /// Authenticated authority/tenant namespace, independently matched to the plan claim.
    pub authority: Authority,
    /// Authenticated permission-bearing actor, not inferred from OS or provider login.
    pub actor: ActorId,
    /// Current actor authorization window in UTC Unix milliseconds.
    pub validity: ValidityWindow,
    /// Actor-wide plan budget ceiling, not a replenishable per-attempt allowance.
    pub budget: ExecutionBudget,
}
/// Verified delegation facts. Issuer legitimacy and current granting rights are checked by the host.
#[derive(Debug, Clone)]
pub struct DelegationFacts {
    /// Exact authenticated delegation identity and revision.
    pub reference: VersionedRef,
    /// Exact scope plus budget/window ceiling; this template cannot supply a base permission.
    pub scope: FrozenPlan,
}
/// Explicit rule disposition; no model risk label or implicit approval heuristic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEffect {
    /// Permit the whole matching plan within the rule's budget/window ceiling.
    Allow,
    /// Deny matching execution content while the rule is active, regardless of requested budget.
    Deny,
    /// Eligible for separately verified approval; not a grant and not an ordinary denial.
    ApprovalRequired {
        /// Exact approval profile; all matching required profiles must subsequently be satisfied.
        profile: VersionedRef,
    },
}
/// Trusted policy entry expressed through existing canonical plan values, not another digest format.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Unique rule identity for stable, value-free decision explanations.
    pub id: Id,
    /// Exact execution scope and budget/window ceiling. Correlation IDs and initiator do not grant rights.
    pub template: FrozenPlan,
    /// Explicit effect selected by the trusted policy owner.
    pub effect: RuleEffect,
}
/// Adapter output only: no Deserialize and no direct `decide(plan, facts)` entry point.
/// A FrozenPlan proves normalization only; the verifier must independently obtain these rules.
/// ```compile_fail
/// let _: execution_admission::AuthorityFacts = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct AuthorityFacts {
    /// Authenticated actor and current actor-wide ceilings.
    pub subject: SubjectFacts,
    /// Authenticated delegation if and only if the request references one.
    pub delegation: Option<DelegationFacts>,
    /// Current trusted policy version, matched against every rule and the request.
    pub policy: VersionedRef,
    /// Whole rule set from that trusted version, not supplied by the model or tool output.
    pub rules: Vec<Rule>,
    /// Reliable host UTC Unix milliseconds after freshness/rollback checks by the verifier.
    pub now_unix_ms: u64,
}
/// The only host trust seam. Implementations are part of the product's trusted computing base.
///
/// Verify the exact incoming plan's actor and authority independently of its DTO claims;
/// authenticate and bind OS/provider origin provenance; reject stale or unknown revocation/time;
/// verify delegation issuer authority, recipient and current scope; load rules from the protected
/// policy authority. Never build an allow rule by echoing an untrusted incoming plan.
/// Implementations must return an error for missing facts. This library supplies no production
/// identity adapter and cannot protect against an intentionally malicious in-process host.
pub trait AuthorityVerifier {
    /// Obtain verified facts for this exact plan. Static failures are always mapped to Denied.
    fn verify(&self, plan: &FrozenPlan) -> Result<AuthorityFacts, VerificationError>;
}
/// Unavailable/invalid trusted input, without provider text or sensitive values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VerificationError {
    /// Subject or origin-account authentication/binding failed.
    #[error("subject verification unavailable")]
    Subject,
    /// Protected policy cannot be loaded or verified.
    #[error("policy verification unavailable")]
    Policy,
    /// Delegation issuer, recipient, scope or signature cannot be verified.
    #[error("delegation verification unavailable")]
    Delegation,
    /// Reliable time cannot be established, including clock rollback.
    #[error("reliable clock unavailable")]
    Clock,
    /// Required current revocation information is missing or stale.
    #[error("revocation verification unavailable")]
    Revocation,
}
/// Explicit work bound for the current rule set.
#[derive(Debug, Clone, Copy)]
pub struct AdmissionLimits {
    /// Maximum number of rules; must be nonzero.
    pub max_rules: usize,
}
