use crate::*;
use execution_contract::{BackendSelection, BackendTask, TaskSubmission};
use std::future::Future;
use tokio_util::sync::CancellationToken;

/// Narrow, host-bound execution service boundary.
///
/// Implementations authenticate and bind authority/tenant, actor, device, delegation and
/// initiator out of band. Every operation rechecks its authorization and freshness. There
/// is no context DTO for a model to deserialize. UI/app owners need not depend on rmcp:
/// their composition bridge implements this trait over the same application service.
///
/// Idempotency is durable service responsibility: same namespace/operationRequestId/content
/// returns the original result, different content conflicts, and reconnect/timeout does not
/// allocate an attempt. A cancelled future/token ends a wait, not acceptance or execution.
/// Accepted effects must remain queryable even when this adapter or its process disappears.
/// Only explicitly identified test implementations may synthesize authority/evidence.
pub trait ExecutionServicePort: Send + Sync + 'static {
    /// Bind one call from bounded, untrusted MCP metadata to the authenticated connection.
    /// The composition owns metadata semantics and must reject missing/mismatched provenance.
    /// Return an immutable per-call service; never mutate shared connection identity.
    fn bind_call(
        self: &std::sync::Arc<Self>,
        metadata: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<std::sync::Arc<Self>, ServiceError>;
    /// Fail startup when no trusted binding exists. This is not a substitute for per-call checks.
    fn check_binding(&self) -> Result<(), ServiceError>;
    /// Read current verified backend offers. Their presence never substitutes for Start.
    fn tasks(
        &self,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<Vec<BackendTask>, ServiceError>> + Send;
    /// Capabilities of the bound device/context, with unknown represented explicitly.
    fn capabilities(
        &self,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<CapabilityView, ServiceError>> + Send;
    /// Accept one immutable action; return its status or required user confirmation.
    fn execute(
        &self,
        request: BackendSelection,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<TaskSubmission, ServiceError>> + Send;
    /// Authorized recovery after response loss, scoped to the trusted namespace.
    fn status(
        &self,
        request: OperationRequest,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<OperationStatus, ServiceError>> + Send;
    /// Request business cancellation. It does not promise termination or rollback.
    fn cancel(
        &self,
        request: OperationRequest,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<CancelResult, ServiceError>> + Send;
}
