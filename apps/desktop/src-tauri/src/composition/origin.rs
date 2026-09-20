//! Non-secret provenance is accepted only on the desktop-owned MCP pipe.
use execution_contract::*;
use execution_mcp::ServiceError;
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Caller {
    pub tenant_id: Id,
    pub principal_id: Id,
    pub authority_id: Id,
}
#[derive(Clone)]
pub struct AiBinding {
    pub caller: Caller,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Namespace {
    tenant_id: Id,
    principal_id: Id,
    authority_id: Id,
    session_id: Id,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Origin {
    version: u8,
    namespace: Namespace,
    operation_id: Id,
    provider: Id,
    account_ref: Id,
    config: VersionedRef,
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
        let origin: Origin = serde_json::from_value(
            metadata
                .get("com.rss-mdm/ai-origin")
                .ok_or(ServiceError::Denied)?
                .clone(),
        )
        .map_err(|_| ServiceError::Denied)?;
        if origin.version != 1 {
            return Err(ServiceError::Denied);
        }
        Ok(origin.namespace.principal_id.as_str().to_owned())
    }
    pub fn bind(&self, metadata: &Map<String, Value>) -> Result<Initiator, ServiceError> {
        let origin: Origin = serde_json::from_value(
            metadata
                .get("com.rss-mdm/ai-origin")
                .ok_or(ServiceError::Denied)?
                .clone(),
        )
        .map_err(|_| ServiceError::Denied)?;
        if origin.version != 1
            || origin.namespace.tenant_id != self.caller.tenant_id
            || origin.namespace.principal_id != self.caller.principal_id
            || origin.namespace.authority_id != self.caller.authority_id
            || !["codex", "claude", "deepseek"].contains(&origin.provider.as_str())
        {
            return Err(ServiceError::Denied);
        }
        Ok(Initiator::Ai {
            provider: origin.provider,
            os_session: os_session(),
            provider_account: ProviderAccountRef {
                account: origin.account_ref,
                config: origin.config,
            },
            conversation: origin.namespace.session_id,
            tool_call: origin.operation_id,
        })
    }
    pub fn validate(&self, origin: &Initiator) -> bool {
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
                provider_account: a,
                conversation: c,
                ..
            },
            Initiator::Ai {
                provider,
                os_session,
                provider_account,
                conversation,
                ..
            },
        ) => p == provider && o == os_session && a == provider_account && c == conversation,
        _ => false,
    }
}
