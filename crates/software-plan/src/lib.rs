//! One software decision from an explicit host snapshot; no authorization, workflow or installer.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod model;
mod planner;
pub use model::*;
pub use planner::decide;

/// Bind a pure decision to the sole canonical V3 execution plan; this does not authorize dispatch.
pub fn bind(
    decision: &SoftwareDecision,
    spec: execution_contract::PlanSpec,
    limits: &execution_contract::PlanLimits,
) -> Result<execution_contract::FrozenPlan, execution_contract::ContractError> {
    use execution_contract::{ContractError, ErrorKind, Field, Rule};
    let invalid =
        || ContractError::new(ErrorKind::InconsistentContext, Field::Plan, Rule::Mismatch);
    let software = spec.execution.software().ok_or_else(invalid)?;
    let intent = decision.intent();
    let DecisionOutcome::Mutate(mutation) = decision.outcome() else {
        return Err(invalid());
    };
    if intent.authority != spec.request.authority
        || intent.target != spec.request.target
        || intent.policy != spec.policy
        || intent.package != software.package
        || intent.desired != software.desired
        || mutation.kind() != software.mutation
        || decision.snapshot() != &software.snapshot
        || decision.installer() != &software.installer
        || decision.management() != software.management
        || decision.comparison() != software.comparison.as_ref()
    {
        return Err(invalid());
    }
    execution_contract::FrozenPlan::freeze(spec, limits)
}
