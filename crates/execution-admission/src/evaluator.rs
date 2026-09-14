use crate::*;
use execution_contract::{
    Constraints, ExecutionBudget, ExecutionRequest, FrozenPlan, Initiator, PlanSpec, ValidityWindow,
};

// Only this verifier call can create the runtime trusted context. It has no serde/DTO constructor.
struct VerifiedContext {
    facts: AuthorityFacts,
}
impl VerifiedContext {
    fn obtain(
        plan: &FrozenPlan,
        authority: &(impl AuthorityVerifier + ?Sized),
    ) -> Result<Self, VerificationError> {
        authority.verify(plan).map(|facts| Self { facts })
    }
}
fn current(window: ValidityWindow, now: u64) -> bool {
    window.not_before_unix_ms <= now && now < window.expires_at_unix_ms
}
fn covers(parent: ValidityWindow, child: ValidityWindow) -> bool {
    parent.not_before_unix_ms <= child.not_before_unix_ms
        && child.not_before_unix_ms < child.expires_at_unix_ms
        && child.expires_at_unix_ms <= parent.expires_at_unix_ms
}
fn budget_fits(plan: ExecutionBudget, ceiling: ExecutionBudget) -> bool {
    plan.total_timeout_ms <= ceiling.total_timeout_ms
        && plan.total_output_bytes <= ceiling.total_output_bytes
        && plan.max_attempts <= ceiling.max_attempts
}
// Exhaustive patterns intentionally have no `..`: adding a contract field requires a
// decision here. Whole-value comparisons below also include future nested fields.
// ref: Rust Reference, patterns.html#struct-patterns
fn scope(spec: &PlanSpec) -> impl PartialEq + '_ {
    let PlanSpec {
        schema_version: _, // FrozenPlan already validates the sole supported version.
        plan_id: _,        // Correlation, not authority.
        request,
        launch,
        run_as,
        session_requirement,
        constraints,
        budget: _,   // Intersected independently by decide.
        validity: _, // Intersected independently by decide.
        policy: _,   // Exact verified policy checked by decide.
    } = spec;
    let ExecutionRequest {
        schema_version: _, // Validated by FrozenPlan.
        request_id: _,     // Correlation, not authority.
        initiator: _,      // Origin is authenticated by AuthorityVerifier.
        delegation: _,     // Verified and intersected independently by decide.
        authority,
        actor,
        operation,
        target,
        parameters,
    } = request;
    let Constraints {
        network,
        read_paths,
        write_paths,
        allow_child_processes,
        require_sandbox,
    } = constraints;
    (
        authority,
        actor,
        operation,
        target,
        parameters,
        launch,
        run_as,
        session_requirement,
        (
            network,
            read_paths,
            write_paths,
            allow_child_processes,
            require_sandbox,
        ),
    )
}
fn same_scope(template: &PlanSpec, plan: &PlanSpec) -> bool {
    scope(template) == scope(plan)
}
/// Evaluate one frozen plan through the trusted host seam, always returning a closed decision.
/// Missing verification, invalid rules and bounds fail as Denied, never as an approval request.
/// No counters, approval records or execution intents are changed by this call.
pub fn decide(
    plan: &FrozenPlan,
    authority: &(impl AuthorityVerifier + ?Sized),
    limits: AdmissionLimits,
) -> AdmissionDecision {
    let spec = plan.spec();
    let result = |outcome, reason, rule_ids| AdmissionDecision {
        plan_id: spec.plan_id.clone(),
        plan_digest: plan.digest().clone(),
        policy: spec.policy.clone(),
        delegation: spec.request.delegation.clone(),
        outcome,
        reason,
        rule_ids,
    };
    let deny = |reason| result(DecisionOutcome::Denied, reason, vec![]);
    if limits.max_rules == 0 {
        return deny(Reason::Limit);
    }
    let context = match VerifiedContext::obtain(plan, authority) {
        Ok(context) => context,
        Err(error) => return deny(Reason::Verification(error)),
    };
    let facts = &context.facts;
    if facts.rules.len() > limits.max_rules {
        return deny(Reason::Limit);
    }
    if facts.subject.authority != spec.request.authority
        || facts.subject.actor != spec.request.actor
    {
        return deny(Reason::SubjectMismatch);
    }
    if facts.policy != spec.policy
        || matches!(&spec.request.initiator, Initiator::Policy { policy } if policy != &facts.policy)
    {
        return deny(Reason::InvalidPolicy);
    }
    for (index, rule) in facts.rules.iter().enumerate() {
        if rule.template.spec().policy != facts.policy
            || facts.rules[..index].iter().any(|r| r.id == rule.id)
        {
            return deny(Reason::InvalidPolicy);
        }
    }
    if !current(spec.validity, facts.now_unix_ms) || !covers(facts.subject.validity, spec.validity)
    {
        return deny(Reason::Validity);
    }
    if !budget_fits(spec.budget, facts.subject.budget) {
        return deny(Reason::Budget);
    }
    match (&spec.request.delegation, &facts.delegation) {
        (None, None) => {}
        (Some(reference), Some(delegation)) if reference == &delegation.reference => {
            let scope = delegation.scope.spec();
            if scope.policy != facts.policy
                || !same_scope(scope, spec)
                || !covers(scope.validity, spec.validity)
                || !budget_fits(spec.budget, scope.budget)
            {
                return deny(Reason::Delegation);
            }
        }
        _ => return deny(Reason::Delegation),
    }
    let mut matching = Vec::new();
    for rule in &facts.rules {
        let scope = rule.template.spec();
        if !same_scope(scope, spec) || !current(scope.validity, facts.now_unix_ms) {
            continue;
        }
        // Active denial cannot be evaded by asking for a larger budget or longer plan lifetime.
        if rule.effect == RuleEffect::Deny
            || (covers(scope.validity, spec.validity) && budget_fits(spec.budget, scope.budget))
        {
            matching.push(rule);
        }
    }
    matching.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let rule_ids = matching.iter().map(|r| r.id.clone()).collect();
    if matching.iter().any(|r| r.effect == RuleEffect::Deny) {
        return result(DecisionOutcome::Denied, Reason::ExplicitDeny, rule_ids);
    }
    if matching.is_empty() {
        return deny(Reason::NoMatchingRule);
    }
    let mut profiles = matching
        .iter()
        .filter_map(|rule| match &rule.effect {
            RuleEffect::ApprovalRequired { profile } => Some(profile.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    profiles.sort_by(|a, b| {
        (a.id.as_str(), a.revision.as_str()).cmp(&(b.id.as_str(), b.revision.as_str()))
    });
    profiles.dedup();
    if profiles.is_empty() {
        result(DecisionOutcome::Allowed, Reason::RuleAllowed, rule_ids)
    } else {
        result(
            DecisionOutcome::ApprovalRequired { profiles },
            Reason::NeedsApproval,
            rule_ids,
        )
    }
}
