use execution_contract::{
    Digest, ExactArtifactRef, ExecutionBudget, FrozenExecution, InterpreterRef, NetworkAccess,
    Operation, RequestId, RunAs, SessionRequirement, Target, ValidityWindow, VersionedRef, V4,
};
use schemars::JsonSchema;
use serde::Serialize;

/// Safe counts only; individual paths and network destinations remain protected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AccessSummary {
    /// No additional confinement beyond the selected OS account permissions.
    OsIdentity {},
    /// Explicit restrictions, never inferred from absent fields.
    Restricted {
        /// Whether networking is completely denied.
        network_denied: bool,
        /// Number of exact allowlisted destinations; never their values.
        network_destination_count: usize,
        /// Number of declared read paths.
        read_path_count: usize,
        /// Number of declared write paths.
        write_path_count: usize,
        /// Required child process allowance.
        allow_child_processes: bool,
        /// Required isolation; this is not an observation that enforcement succeeded.
        require_sandbox: bool,
    },
}

/// Allowlisted view of the immutable plan; no parameters, launch inputs, secrets or audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrozenExecutionSummary {
    /// Product authority bound to the exact plan.
    pub authority: execution_contract::Authority,
    /// Permission-bearing principal; not the model account.
    pub actor: execution_contract::ActorId,
    /// Human or AI origin, without granting execution permission.
    pub initiator: execution_contract::Initiator,
    /// Version of the frozen execution plan, independent of the AI wire version.
    pub schema_version: V4,
    /// Closed execution kind without private software paths or source inputs.
    pub execution: ExecutionSummary,
    /// Exact frozen plan identity.
    pub request_id: RequestId,
    /// Digest covers the complete original plan, including omitted private values.
    pub content_digest: Digest,
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
impl FrozenExecutionSummary {
    pub(crate) fn from_input(plan: &FrozenExecution) -> Self {
        let p = plan.spec();
        Self {
            authority: p.request.authority.clone(),
            actor: p.request.actor.clone(),
            initiator: p.request.initiator.clone(),
            schema_version: p.schema_version,
            execution: match &p.execution {
                execution_contract::ExecutionSpec::Process {} => ExecutionSummary::Process {},
                execution_contract::ExecutionSpec::Software { software } => {
                    ExecutionSummary::Software {
                        adapter: software.adapter,
                        mutation: software.mutation,
                    }
                }
            },
            request_id: p.request.request_id.clone(),
            content_digest: plan.digest().clone(),
            target: p.request.target.clone(),
            run_as: p.run_as.clone(),
            operation: p.request.operation.clone(),
            artifact: p.launch.artifact.clone(),
            interpreter: p.launch.interpreter.clone(),
            session_requirement: p.session_requirement.clone(),
            policy: p.policy.clone(),
            validity: p.validity,
            budget: p.budget,
            access: match &p.constraints {
                execution_contract::IsolationPolicy::OsIdentity {} => AccessSummary::OsIdentity {},
                execution_contract::IsolationPolicy::Restricted {
                    network,
                    read_paths,
                    write_paths,
                    allow_child_processes,
                    require_sandbox,
                } => AccessSummary::Restricted {
                    network_denied: matches!(network, NetworkAccess::Denied {}),
                    network_destination_count: match network {
                        NetworkAccess::Denied {} => 0,
                        NetworkAccess::Allowlist { destinations } => destinations.len(),
                    },
                    read_path_count: read_paths.len(),
                    write_path_count: write_paths.len(),
                    allow_child_processes: *allow_child_processes,
                    require_sandbox: *require_sandbox,
                },
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
    pub action: FrozenExecutionSummary,
}
/// Bounded authorized task list using the same detail projection as individual reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskPage {
    /// Safe frozen facts and current lifecycle state.
    pub items: Vec<ExecutionTaskDetails>,
    /// Exclusive request continuation; absent at the end.
    pub next: Option<execution_contract::RequestId>,
}

/// Safe execution semantics for task presentation; it never grants execution permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExecutionSummary {
    /// Existing process or collection execution.
    Process {},
    /// Software mutation, requiring independent installed-state verification.
    Software {
        /// Selected platform adapter.
        adapter: execution_contract::SoftwareKind,
        /// Selected mutation.
        mutation: execution_contract::MutationKind,
    },
}
