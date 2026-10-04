//! Bounded controlled-execution MCP adapter. Execution authority belongs to the service.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod config;
mod ingress;
mod model;
mod output;
mod port;
mod server;
mod transport;
pub use config::McpLimits;
pub use model::{
    CancelDisposition, CancelResult, CapabilityState, CapabilityView, OperationPhase,
    OperationRequest, OperationStatus, ServiceError,
};
pub use port::ExecutionServicePort;

/// Exact exported tool definitions for host-owned product instructions and drift checks.
/// The implementation and this projection share the same Rust schema owner.
pub fn tool_definitions() -> serde_json::Value {
    serde_json::to_value(server::tools()).expect("static tool schema serializes")
}

use rmcp::ServiceExt;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::sync::CancellationToken;

/// A single host-bound controlled tools service; construct a new instance per connection.
/// The host owns authentication and protects rmcp metadata/result tracing in its subscriber.
pub struct ExecutionMcp<S> {
    service: Arc<S>,
    limits: McpLimits,
}
struct SessionGuard(Arc<transport::Session>);
impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.close();
    }
}
impl<S: ExecutionServicePort> ExecutionMcp<S> {
    /// Check host limits and the service's out-of-band identity binding before accepting input.
    pub fn new(service: Arc<S>, limits: McpLimits) -> Result<Self, ServiceError> {
        limits.validate()?;
        service.check_binding()?;
        Ok(Self { service, limits })
    }
    /// Serve a host-owned stdio/byte-stream connection until EOF, stop or protocol failure.
    ///
    /// Input errors fail closed. All stream writes and shutdown waits are bounded. Dropping
    /// or stopping this session never cancels an accepted business operation. The service
    /// must retain durable requests independently of this future/process.
    /// Clean EOF/host stop returns `Ok`; protocol, budget and I/O failures return a static error.
    pub async fn serve<R, W>(
        self,
        reader: R,
        writer: W,
        stop: CancellationToken,
    ) -> Result<(), ServiceError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let session = transport::Session::new(&self.limits, stop.child_token());
        let _guard = SessionGuard(session.clone());
        let transport =
            transport::BoundedTransport::new(reader, writer, session.clone(), self.limits.clone());
        let handler = transport::BoundedService {
            handler: server::Handler {
                service: self.service,
                limits: self.limits.clone(),
            },
            session: session.clone(),
        };
        let initialized = tokio::time::timeout(
            self.limits.request_timeout,
            handler.serve_with_ct(transport, session.stop.clone()),
        )
        .await;
        let result = match initialized {
            Ok(Ok(running)) => running
                .waiting()
                .await
                .map(|_| ())
                .map_err(|_| ServiceError::Unavailable),
            _ => Err(ServiceError::Unavailable),
        };
        session.close();
        // Quiesce service futures independently of rmcp's transport-loop completion.
        let _drained = tokio::time::timeout(
            self.limits.request_timeout,
            session
                .slots
                .clone()
                .acquire_many_owned(self.limits.in_flight as u32),
        )
        .await
        .map_err(|_| ServiceError::Unavailable)?
        .map_err(|_| ServiceError::Unavailable)?;
        session.failure().map_or(result, Err)
    }
}

/// Deterministic MCP identity bound to Policy configuration and immutable execution version.
pub fn policy_tool_name(item: &rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem) -> String {
    use sha2::{Digest, Sha256};
    let hash = format!(
        "{:x}",
        Sha256::digest(format!("{}:{}", item.policy_revision, item.policy_version))
    );
    format!("policy_{}_{}", item.policy_id.simple(), &hash[..16])
}
/// Project only Input fields from the Resource-owned schema; full resolved validation remains in the service.
pub fn policy_tool(
    item: &rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem,
) -> Result<serde_json::Value, ServiceError> {
    use rss_mdm_agent_wire::runtime::ScriptParameterSource;
    if !item.self_service.published
        || !item.self_service.allow_ai
        || !matches!(item.self_service.risk_level, 1 | 2)
    {
        return Err(ServiceError::Denied);
    }
    let mut schema = item.parameter_schema.clone();
    let object = schema.as_object_mut().ok_or(ServiceError::InvalidInput)?;
    let input = |key: &str| {
        matches!(
            item.parameter_sources.get(key),
            Some(ScriptParameterSource::Input {})
        )
    };
    if let Some(properties) = object
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    {
        properties.retain(|key, _| input(key));
    }
    if let Some(required) = object
        .get_mut("required")
        .and_then(serde_json::Value::as_array_mut)
    {
        required.retain(|value| value.as_str().is_some_and(input));
    }
    object.insert("additionalProperties".into(), false.into());
    Ok(
        serde_json::json!({"name": policy_tool_name(item), "description": format!("{}\n{}\nPrerequisites: {}\nSide effects: {}", item.self_service.display_name, item.self_service.description, item.self_service.prerequisites, item.self_service.side_effects), "inputSchema": schema, "annotations": {"readOnlyHint": item.self_service.risk_level == 1, "openWorldHint": false, "idempotentHint": false}}),
    )
}
