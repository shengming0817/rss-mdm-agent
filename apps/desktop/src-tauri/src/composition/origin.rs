//! Non-secret provenance is accepted only on the desktop-owned MCP pipe.
use execution_contract::*;
use execution_mcp::ServiceError;
use serde_json::{Map, Value};

#[derive(Clone)]
pub struct Caller {
    pub tenant_id: Id,
    pub principal_id: Id,
    pub authority_id: Id,
}
#[derive(Clone)]
pub struct AiBinding {
    pub caller: Caller,
}
use crate::self_service::fixtures::os_session;
impl AiBinding {
    pub fn for_user(user_id: &str) -> Result<Self, ServiceError> {
        Ok(Self {
            caller: Caller {
                tenant_id: Id::new("test-users").unwrap(),
                principal_id: Id::new(user_id).map_err(|_| ServiceError::Unbound)?,
                authority_id: Id::new("desktop-fixture").unwrap(),
            },
        })
    }
    pub fn principal(metadata: &Map<String, Value>) -> Result<String, ServiceError> {
        let origin: ai_session_contract::ExecutionOrigin = serde_json::from_value(
            metadata
                .get("com.rss-mdm/ai-origin")
                .ok_or(ServiceError::Denied)?
                .clone(),
        )
        .map_err(|_| ServiceError::Denied)?;
        Ok(origin.namespace.principal_id.as_str().to_owned())
    }
    pub fn bind(&self, metadata: &Map<String, Value>) -> Result<Initiator, ServiceError> {
        let origin: ai_session_contract::ExecutionOrigin = serde_json::from_value(
            metadata
                .get("com.rss-mdm/ai-origin")
                .ok_or(ServiceError::Denied)?
                .clone(),
        )
        .map_err(|_| ServiceError::Denied)?;
        if origin.namespace.tenant_id.as_str() != self.caller.tenant_id.as_str()
            || origin.namespace.principal_id.as_str() != self.caller.principal_id.as_str()
            || origin.namespace.authority_id.as_str() != self.caller.authority_id.as_str()
        {
            return Err(ServiceError::Denied);
        }
        Ok(Initiator::Ai {
            provider: Id::new(origin.provider.to_string()).map_err(|_| ServiceError::Denied)?,
            os_session: os_session(),
            config: VersionedRef {
                id: Id::new(origin.config.id.as_str()).map_err(|_| ServiceError::Denied)?,
                revision: Id::new(origin.config.revision.as_str())
                    .map_err(|_| ServiceError::Denied)?,
            },
            conversation: Id::new(origin.namespace.session_id.as_str())
                .map_err(|_| ServiceError::Denied)?,
            tool_call: Id::new(origin.operation_id.as_str()).map_err(|_| ServiceError::Denied)?,
        })
    }
    pub fn validate(origin: &Initiator) -> bool {
        matches!(origin, Initiator::Ai { provider, os_session: os, .. } if ["codex", "claude", "deepseek"].contains(&provider.as_str()) && os == &os_session())
    }
}
pub fn same_conversation(expected: &Initiator, actual: &Initiator) -> bool {
    match (expected, actual) {
        (
            Initiator::Human {
                os_session: expected,
            },
            Initiator::Human { os_session: actual },
        ) => expected == actual,
        (
            Initiator::Ai {
                provider: p,
                os_session: o,
                config: a,
                conversation: c,
                ..
            },
            Initiator::Ai {
                provider,
                os_session,
                config,
                conversation,
                ..
            },
        ) => p == provider && o == os_session && a == config && c == conversation,
        _ => false,
    }
}
