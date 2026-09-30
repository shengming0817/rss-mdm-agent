//! AI provenance bound by the desktop-owned pipe; OS identity comes from native IPC.
use execution_contract::{Id, VersionedRef};
use execution_mcp::ServiceError;
use execution_runner::host::ClientOrigin;
use serde_json::{Map, Value};
pub fn bind(
    context: &ai_session_contract::UserContext,
    metadata: &Map<String, Value>,
) -> Result<ClientOrigin, ServiceError> {
    let bytes = serde_json::to_vec(
        metadata
            .get("com.rss-mdm/ai-origin")
            .ok_or(ServiceError::Denied)?,
    )
    .map_err(|_| ServiceError::Denied)?;
    let record = ai_session_contract::decode(
        &bytes,
        &ai_session_contract::Limits {
            max_bytes: 16384,
            max_text_bytes: 8192,
            max_depth: 16,
            max_nodes: 1024,
        },
    )
    .map_err(|_| ServiceError::Denied)?;
    let ai_session_contract::WireRecord::ExecutionOrigin(origin) = record else {
        return Err(ServiceError::Denied);
    };
    let expected = super::credentials::credential_caller(context);
    if origin.user_generation != context.generation
        || origin.namespace.tenant_id != expected.tenant_id
        || origin.namespace.principal_id != expected.principal_id
        || origin.namespace.authority_id != expected.authority_id
    {
        return Err(ServiceError::Denied);
    }
    Ok(ClientOrigin::Ai {
        config: VersionedRef {
            id: Id::new(origin.config.id.as_str()).map_err(|_| ServiceError::Denied)?,
            revision: Id::new(origin.config.revision.as_str()).map_err(|_| ServiceError::Denied)?,
        },
        conversation: Id::new(origin.namespace.session_id.as_str())
            .map_err(|_| ServiceError::Denied)?,
        tool_call: Id::new(origin.operation_id.as_str()).map_err(|_| ServiceError::Denied)?,
    })
}
