use execution_contract::{AttemptId, Digest, Id, RequestId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Static safe errors. An implementation must never attach raw backend diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum ServiceError {
    /// Invalid host configuration.
    #[error("invalid limits")]
    InvalidLimits,
    /// No authenticated host/service binding; startup must fail.
    #[error("unbound service")]
    Unbound,
    /// Subject/delegation/target is not authorized.
    #[error("denied")]
    Denied,
    /// Object absent within the authorized namespace; do not reveal another namespace.
    #[error("not found")]
    NotFound,
    /// Stable operation ID already names different content.
    #[error("content conflict")]
    Conflict,
    /// Exact reference or trusted authorization is no longer current.
    #[error("expired reference")]
    Expired,
    /// Capability not implemented; never weaken required protection.
    #[error("unsupported")]
    Unsupported,
    /// Service unavailable; submission may require querying the original ID.
    #[error("unavailable")]
    Unavailable,
    /// Acceptance cannot be determined; query/retry the original operationRequestId.
    #[error("outcome unknown")]
    OutcomeUnknown,
    /// Invalid request data.
    #[error("invalid input")]
    InvalidInput,
    /// A configured byte, collection or concurrency budget was exceeded.
    #[error("limit exceeded")]
    Limit,
    /// Current RPC wait ended. This is not business cancellation.
    #[error("wait cancelled")]
    Cancelled,
}

/// Authorized lookup/cancellation within the host-bound namespace.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationRequest {
    /// Business identity, never inferred from a JSON-RPC ID.
    pub operation_request_id: RequestId,
}
/// Empty arguments for current directory and capability queries.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Empty {}
/// Capability projection; an enum is not an execution permit.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityState {
    /// The service has matching known capability facts.
    Supported,
    /// Policy or a known constraint blocks the operation.
    Blocked,
    /// Required capability is not implemented.
    Unsupported,
    /// No sufficient facts.
    Unknown,
}
/// Safe capability summary for the bound context.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityView {
    /// Aggregate availability, not authorization.
    pub state: CapabilityState,
    /// Static/opaque reason identifiers, not raw backend messages.
    pub reasons: Vec<Id>,
}
/// Read-only service projection. This crate implements no execution state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum OperationPhase {
    /// Durable acceptance only.
    Accepted,
    /// Waiting for policy/approval/user conditions.
    Waiting,
    /// Service reports execution underway.
    Running,
    /// Execution ended; effects have not necessarily been verified.
    ExecutionEnded,
    /// Outcome needs authoritative reconciliation.
    OutcomeUnknown,
    /// An independent assessment was recorded; inspect mode and assessment.
    Verified,
    /// Failure known to the execution authority.
    Failed,
    /// Service confirms its business cancellation condition, not rollback.
    Cancelled,
}
/// Authorized execution fact projection; accepted is never rewritten to success.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    /// Execution provenance; a verified test result is not a real platform effect.
    pub mode: execution_lifecycle::ExecutionMode,
    /// Redacted root process progress; not full-scope termination or effect proof.
    pub process: Option<execution_contract::ProcessSummary>,
    /// Independent effect assessment, never inferred from process exit.
    pub assessment: Option<execution_lifecycle::EffectAssessment>,
    /// Durable cancellation request, independent of termination/effect status.
    pub cancel_requested: bool,
    /// Original business identity.
    pub operation_request_id: RequestId,
    /// Exact content identity: signed offer revision during preparation, frozen input thereafter.
    pub content_digest: Digest,
    /// Service-owned current phase.
    pub phase: OperationPhase,
    /// Service-owned attempt identity, if an attempt exists.
    pub attempt_id: Option<AttemptId>,
    /// Authorized evidence references only.
    pub evidence: Vec<Id>,
}
/// Business cancellation acceptance, independent of RPC cancellation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CancelDisposition {
    /// Request recorded; does not assert process termination.
    Requested,
    /// Operation was already terminal.
    AlreadyTerminal,
}
/// Service cancellation result, retaining the current authoritative status.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CancelResult {
    /// Cancellation acceptance classification.
    pub disposition: CancelDisposition,
    /// Current operation fact projection.
    pub operation: OperationStatus,
}

/// Protocol error payload; optional catalog diagnostic is produced only from static C03 errors.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ErrorView {
    pub code: ServiceError,
}
/// One output schema for both success and tool-level failure.
#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum ToolOutput<T> {
    Ok { result: T },
    Error { error: ErrorView },
}
