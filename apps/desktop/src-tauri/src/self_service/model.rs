use execution_contract::{BackendTask, RequestId};
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

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotQuery {
    pub after: Option<RequestId>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub available: Vec<BackendTask>,
    pub requests: Vec<execution_app::ExecutionTaskDetails>,
    pub next: Option<RequestId>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionRef {
    pub request_id: RequestId,
}
