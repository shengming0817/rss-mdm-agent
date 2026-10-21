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
    OperationProposal, OperationRequest, OperationScene, OperationStatus, ServiceError,
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
    let (mut schema, parameter_sources) = match &item.task {
        rss_mdm_agent_wire::runtime::SelfServiceTask::Script {
            parameter_schema,
            parameter_sources,
            ..
        } => (parameter_schema.clone(), parameter_sources.clone()),
        rss_mdm_agent_wire::runtime::SelfServiceTask::Software { .. }
        | rss_mdm_agent_wire::runtime::SelfServiceTask::Enrollment { .. } => (
            serde_json::json!({"type":"object","properties":{},"required":[],"additionalProperties":false}),
            std::collections::BTreeMap::new(),
        ),
    };
    let object = schema.as_object_mut().ok_or(ServiceError::InvalidInput)?;
    let input = |key: &str| {
        matches!(
            parameter_sources.get(key),
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
    project_parameter_schema(&mut schema)?;
    Ok(
        serde_json::json!({"name": policy_tool_name(item), "description": serde_json::to_string(&item.self_service.presentation).map_err(|_| ServiceError::Denied)?, "inputSchema": schema, "annotations": {"readOnlyHint": item.self_service.risk_level == 1 && matches!(item.task, rss_mdm_agent_wire::runtime::SelfServiceTask::Script {..}), "openWorldHint": false, "idempotentHint": false}}),
    )
}

// The JS transport accepts only exactly representable integers. Project that intersection;
// the service still validates resolved parameters against the untouched Resource schema.
fn project_parameter_schema(schema: &mut serde_json::Value) -> Result<(), ServiceError> {
    const SAFE: i64 = 9_007_199_254_740_991;
    let object = schema.as_object_mut().ok_or(ServiceError::InvalidInput)?;
    if object.get("type").and_then(serde_json::Value::as_str) == Some("integer") {
        for (keyword, edge, lower) in [("minimum", -SAFE, true), ("maximum", SAFE, false)] {
            let bound = object.get(keyword).and_then(serde_json::Value::as_f64);
            if bound.is_some_and(|value| {
                if lower {
                    value > SAFE as f64
                } else {
                    value < -SAFE as f64
                }
            }) {
                return Err(ServiceError::InvalidInput);
            }
            if bound.is_none_or(|value| {
                if lower {
                    value < edge as f64
                } else {
                    value > edge as f64
                }
            }) {
                object.insert(keyword.into(), edge.into());
            }
        }
    }
    if let Some(properties) = object
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    {
        for child in properties.values_mut() {
            project_parameter_schema(child)?;
        }
    }
    if let Some(items) = object.get_mut("items") {
        project_parameter_schema(items)?;
    }
    fn transport_safe(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Number(number) => number
                .as_f64()
                .is_some_and(|n| n.is_finite() && (n.fract() != 0.0 || n.abs() <= SAFE as f64)),
            serde_json::Value::Array(values) => values.iter().all(transport_safe),
            serde_json::Value::Object(values) => values.values().all(transport_safe),
            _ => true,
        }
    }
    if !transport_safe(schema) {
        return Err(ServiceError::InvalidInput);
    }
    Ok(())
}
