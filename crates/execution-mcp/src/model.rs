use execution_contract::{AttemptId, Digest, ExactArtifactRef, Id, PlanId, RequestId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use service_catalog::{CatalogRef, SelectedOperation};

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

/// Exact catalog selection. The arguments retain original numeric tokens until C03 validates them.
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CatalogInput {
    /// Stable business identity, independent of the MCP request ID.
    pub operation_request_id: RequestId,
    /// Exact catalog identity and digest.
    pub catalog: CatalogRef,
    /// Catalog item ID.
    pub item_id: Id,
    /// Operation variant ID.
    pub variant_id: Id,
    /// Original parameter object; schema/rules are obtained from execution_catalog.
    #[schemars(with = "serde_json::Map<String, serde_json::Value>")]
    pub arguments: Box<RawValue>,
}

/// Preview either an exact catalog choice or a previously proposed immutable candidate.
#[derive(Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum PreviewInput {
    /// Catalog parameters still require C03 validation.
    Catalog {
        /// Exact selection.
        selection: CatalogInput,
    },
    /// Candidate ownership and content must be checked by the execution service.
    Candidate {
        /// Stable operation identity.
        operation_request_id: RequestId,
        /// Exact immutable candidate.
        candidate: ExactArtifactRef,
    },
}

/// Untrusted proposal input; there is no approval, actor or permission field.
#[derive(Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ProposeInput {
    /// A directory operation using the shared parameter grammar.
    Catalog {
        /// Exact selection.
        selection: CatalogInput,
    },
    /// New UTF-8 source. The service creates the immutable artifact; the adapter never writes it.
    Script {
        /// Stable operation identity.
        operation_request_id: RequestId,
        /// Original UTF-8 script, never executed or echoed.
        source_utf8: String,
        /// Explicit interpreter identity, subject to service validation.
        interpreter: ExactArtifactRef,
    },
}

/// Validated selection passed to the service. It confers no execution permission.
pub struct CatalogCandidate {
    /// Original caller business identity.
    pub operation_request_id: RequestId,
    /// C03 validated, normalized, immutable selection.
    pub selection: SelectedOperation,
}
/// A bounded draft; only the adapter can construct one from protocol input.
pub struct ScriptDraft {
    pub(crate) operation_request_id: RequestId,
    pub(crate) source_utf8: String,
    pub(crate) interpreter: ExactArtifactRef,
}
impl ScriptDraft {
    /// Stable operation identity.
    pub fn operation_request_id(&self) -> &RequestId {
        &self.operation_request_id
    }
    /// Original UTF-8 bytes; callers must not rewrite them when freezing an artifact.
    pub fn source_utf8(&self) -> &str {
        &self.source_utf8
    }
    /// Exact requested interpreter, not authorization or availability evidence.
    pub fn interpreter(&self) -> &ExactArtifactRef {
        &self.interpreter
    }
}
/// Validated shape for proposal; authorization and artifact ownership remain in the service.
pub enum CandidateRequest {
    /// Normalized catalog parameters.
    Catalog(Box<CatalogCandidate>),
    /// Bounded original source.
    Script(ScriptDraft),
}
/// Service preview input; no model-supplied trusted context.
pub enum PreviewRequest {
    /// Shared directory selection.
    Catalog(Box<CatalogCandidate>),
    /// Service must verify the exact proposed artifact under its bound subject.
    Candidate {
        /// Stable operation identity.
        operation_request_id: RequestId,
        /// Immutable candidate.
        candidate: ExactArtifactRef,
    },
}

/// Immutable plan identity projected from the execution service's FrozenPlan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanRef {
    /// Execution-contract plan ID.
    pub plan_id: PlanId,
    /// Existing C01 canonical digest, never an adapter-specific hash.
    pub digest: Digest,
}
/// Explicit idempotent submission. No retry/attempt or approval policy can be supplied.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmitRequest {
    /// Stable business identity retained through timeout/reconnect.
    pub operation_request_id: RequestId,
    /// Exact previously frozen plan.
    pub plan: PlanRef,
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
/// An immutable candidate was recorded; it was not approved or executed.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CandidateReceipt {
    /// Stable operation identity.
    pub operation_request_id: RequestId,
    /// Service-owned candidate identity and exact content hash.
    pub candidate: ExactArtifactRef,
}
/// Safe plan preview; never returns script, secrets, or raw parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanPreview {
    /// Stable operation identity.
    pub operation_request_id: RequestId,
    /// C01 plan identity and canonical digest.
    pub plan: PlanRef,
    /// Explicit capability information from the service.
    pub capability: CapabilityView,
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
    /// The service verified the requested effect.
    Verified,
    /// Explicit test-only terminal result; not a platform effect.
    TestCompleted,
    /// Failure known to the execution authority.
    Failed,
    /// Service confirms its business cancellation condition, not rollback.
    Cancelled,
}
/// Authorized execution fact projection; accepted is never rewritten to success.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationStatus {
    /// Durable initial submission receipt; preview alone is false.
    pub submitted: bool,
    /// Durable cancellation request, independent of termination/effect status.
    pub cancel_requested: bool,
    /// Original business identity.
    pub operation_request_id: RequestId,
    /// Original frozen plan.
    pub plan: PlanRef,
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
    pub catalog_reason: Option<crate::catalog_error::CatalogErrorView>,
}
/// One output schema for both success and tool-level failure.
#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum ToolOutput<T> {
    Ok { result: T },
    Error { error: ErrorView },
}
