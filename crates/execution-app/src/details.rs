use execution_contract::{
    Digest, ExactArtifactRef, ExecutionBudget, FrozenPlan, InterpreterRef, NetworkAccess,
    Operation, PlanId, RunAs, SessionRequirement, Target, ValidityWindow, VersionedRef, V1,
};
use schemars::JsonSchema;
use serde::Serialize;

/// Safe counts only; individual paths and network destinations remain protected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccessSummary {
    /// Whether networking is completely denied.
    pub network_denied: bool,
    /// Number of exact allowlisted destinations; never their values.
    pub network_destination_count: usize,
    /// Number of declared read paths.
    pub read_path_count: usize,
    /// Number of declared write paths.
    pub write_path_count: usize,
    /// Required child process allowance.
    pub allow_child_processes: bool,
    /// Required isolation; this is not an observation that enforcement succeeded.
    pub require_sandbox: bool,
}

/// Allowlisted view of the immutable plan; no parameters, launch inputs, secrets or audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrozenPlanSummary {
    /// Version of the frozen execution plan, independent of the AI wire version.
    pub schema_version: V1,
    /// Exact frozen plan identity.
    pub plan_id: PlanId,
    /// Digest covers the complete original plan, including omitted private values.
    pub plan_digest: Digest,
    /// Explicit device/platform/user target.
    pub target: Target,
    /// Explicit execution identity; never inferred from a provider login.
    pub run_as: RunAs,
    /// Action and exact resource revision.
    pub operation: Operation,
    /// Exact artifact revision and content hash.
    pub artifact: ExactArtifactRef,
    /// Exact interpreter selection.
    pub interpreter: InterpreterRef,
    /// Required target user session.
    pub session_requirement: SessionRequirement,
    /// Exact policy revision.
    pub policy: VersionedRef,
    /// Original validity window.
    pub validity: ValidityWindow,
    /// Cumulative plan budget; retries do not reset it.
    pub budget: ExecutionBudget,
    /// Restrictions summarized without individual private values.
    pub access: AccessSummary,
}
impl FrozenPlanSummary {
    pub(crate) fn from_plan(plan: &FrozenPlan) -> Self {
        let p = plan.spec();
        Self {
            schema_version: p.schema_version,
            plan_id: p.plan_id.clone(),
            plan_digest: plan.digest().clone(),
            target: p.request.target.clone(),
            run_as: p.run_as.clone(),
            operation: p.request.operation.clone(),
            artifact: p.launch.artifact.clone(),
            interpreter: p.launch.interpreter.clone(),
            session_requirement: p.session_requirement.clone(),
            policy: p.policy.clone(),
            validity: p.validity,
            budget: p.budget,
            access: AccessSummary {
                network_denied: matches!(p.constraints.network, NetworkAccess::Denied {}),
                network_destination_count: match &p.constraints.network {
                    NetworkAccess::Denied {} => 0,
                    NetworkAccess::Allowlist { destinations } => destinations.len(),
                },
                read_path_count: p.constraints.read_paths.len(),
                write_path_count: p.constraints.write_paths.len(),
                allow_child_processes: p.constraints.allow_child_processes,
                require_sandbox: p.constraints.require_sandbox,
            },
        }
    }
}

/// One authorized record read provides both lifecycle status and frozen plan facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionTaskDetails {
    /// Authoritative lifecycle projection.
    pub status: crate::ExecutionStatus,
    /// Redacted frozen plan bound to that lifecycle.
    pub plan: FrozenPlanSummary,
}
