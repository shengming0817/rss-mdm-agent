use crate::*;
use service_catalog::{CatalogRef, FrozenCatalog};
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
    /// Fail startup when no trusted binding exists. This is not a substitute for per-call checks.
    fn check_binding(&self) -> Result<(), ServiceError>;
    /// Return a bounded authorized directory, optionally matching an exact historical reference.
    /// Its selection and projection semantics must be identical to the human UI.
    fn catalog(
        &self,
        reference: Option<CatalogRef>,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<FrozenCatalog, ServiceError>> + Send;
    /// Capabilities of the bound device/context, with unknown represented explicitly.
    fn capabilities(
        &self,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<CapabilityView, ServiceError>> + Send;
    /// Freeze/preview without authorizing execution or consuming approval.
    fn preview(
        &self,
        request: PreviewRequest,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<PlanPreview, ServiceError>> + Send;
    /// Record an immutable candidate with stable content conflict semantics.
    fn propose(
        &self,
        request: CandidateRequest,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<CandidateReceipt, ServiceError>> + Send;
    /// Atomically accept/replay under the bound identity. No new attempt on duplicate delivery.
    fn submit(
        &self,
        request: SubmitRequest,
        wait: CancellationToken,
    ) -> impl Future<Output = Result<OperationStatus, ServiceError>> + Send;
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
