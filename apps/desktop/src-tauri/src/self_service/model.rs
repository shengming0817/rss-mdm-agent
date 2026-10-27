use execution_contract::RequestId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceError {
    pub code: &'static str,
    pub message: String,
}
impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ServiceError {}
pub type Result<T> = std::result::Result<T, ServiceError>;
pub fn error(code: &'static str, message: impl Into<String>) -> ServiceError {
    ServiceError {
        code,
        message: message.into(),
    }
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionRef {
    pub request_id: RequestId,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyDirectoryQuery {
    pub after: Option<String>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PolicyStatus {
    Absent,
    Preparation {
        value: Box<execution_contract::BackendRequest>,
    },
    Run {
        value: Box<rss_mdm_agent_wire::runtime::SelfServiceStatus>,
    },
}
