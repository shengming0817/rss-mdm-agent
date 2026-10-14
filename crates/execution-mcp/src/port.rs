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
    /// Native discovery of current AI-authorized Policy tools. The production consumer uses the formal directory.
    fn policy_tools(
        &self,
        _: CancellationToken,
    ) -> impl Future<Output = Result<Vec<serde_json::Value>, ServiceError>> + Send {
        async { Ok(Vec::new()) }
    }
    /// Invoke a current Policy descriptor with a Host-owned original request identity.
    fn invoke_policy(
        &self,
        _: &str,
        _: serde_json::Map<String, serde_json::Value>,
        _: CancellationToken,
    ) -> impl Future<Output = Result<execution_contract::BackendRequest, ServiceError>> + Send {
        async { Err(ServiceError::Denied) }
    }
    /// Prepare a registered operation without dispatch or confirmation.
    fn prepare_operation(
        &self,
        _: OperationProposal,
        _: CancellationToken,
    ) -> impl Future<Output = Result<execution_contract::BackendRequest, ServiceError>> + Send {
        async { Err(ServiceError::Unsupported) }
    }
    /// Structured employee-authorized result for the original Policy request.
    fn policy_status(
        &self,
        _: OperationRequest,
        _: CancellationToken,
    ) -> impl Future<Output = Result<serde_json::Value, ServiceError>> + Send {
        async { Err(ServiceError::Denied) }
    }
    /// Bounded on-demand native software observation without an execution attempt.
    fn observe_software(
        &self,
        _: execution_contract::SoftwareReadSelection,
        _: CancellationToken,
    ) -> impl Future<Output = Result<execution_contract::BackendRequest, ServiceError>> + Send {
        async { Err(ServiceError::Denied) }
    }
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
