//! Non-secret provenance is accepted only on the desktop-owned MCP pipe.
use super::authority::id;
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
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Session {
    pub provider: Id,
    pub account_ref: Id,
    pub config: VersionedRef,
    pub profile: String,
}
#[derive(Clone)]
pub struct AiBinding {
    pub caller: Caller,
    pub session: Session,
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
pub fn os_session() -> OsSessionRef {
    OsSessionRef {
        device: DeviceId::new("fixture-device").unwrap(),
        account: OsAccountRef {
            platform: Platform::Macos,
            subject: id("fixture-user"),
        },
        session: id("fixture-session"),
    }
}
pub fn human() -> Initiator {
    Initiator::Human {
        os_session: os_session(),
    }
}
impl AiBinding {
    pub fn from_configuration(value: &Value) -> Result<Self, ServiceError> {
        let caller: Caller =
            serde_json::from_value(value["caller"].clone()).map_err(|_| ServiceError::Unbound)?;
        let session: Session =
            serde_json::from_value(value["session"].clone()).map_err(|_| ServiceError::Unbound)?;
        if caller.tenant_id.as_str() != "s1-test"
            || caller.principal_id.as_str() != "fixture-actor"
            || caller.authority_id.as_str() != "desktop-fixture"
            || !["codex", "claude", "deepseek"].contains(&session.provider.as_str())
            || !["conversation", "controlled_tools"].contains(&session.profile.as_str())
        {
            return Err(ServiceError::Unbound);
        }
        Ok(Self { caller, session })
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
            || origin.provider != self.session.provider
            || origin.account_ref != self.session.account_ref
            || origin.config != self.session.config
            || self.session.profile != "controlled_tools"
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
        matches!(origin, Initiator::Ai { provider, os_session: os, provider_account, .. } if provider == &self.session.provider && os == &os_session() && provider_account.account == self.session.account_ref && provider_account.config == self.session.config)
    }
}
pub fn same_conversation(expected: &Initiator, actual: &Initiator) -> bool {
    match (expected, actual) {
        (_, Initiator::Human { os_session: os }) => os == &os_session(),
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
