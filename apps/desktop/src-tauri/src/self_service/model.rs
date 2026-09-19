use execution_contract::{Digest, Id, PlanId, RequestId};
use execution_interaction::Kind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use service_catalog::{CatalogKind, CatalogRef, DisplayStatus, Parameter, ResourceBinding};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum FieldInput {
    Text { value: String },
    Integer { value: String },
    Boolean { value: bool },
    SecretReference { id: String, revision: String },
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub instance_id: String,
    pub request_id: RequestId,
    pub revision: u32,
    pub catalog: CatalogRef,
    pub item_id: Id,
    pub variant_id: Id,
    pub fields: BTreeMap<String, FieldInput>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Submission {
    pub instance_id: String,
    pub request_id: RequestId,
    pub plan_id: PlanId,
    pub digest: Digest,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Answer {
    Confirmation {
        accepted: bool,
    },
    PrivacyConsent {
        accepted: bool,
    },
    Choice {
        selection: String,
    },
    Parameters {
        fields: BTreeMap<String, FieldInput>,
    },
    Cancel {},
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reply {
    pub instance_id: String,
    pub request_id: RequestId,
    pub interaction_id: String,
    pub command_id: execution_interaction::Reference,
    pub answer: Answer,
}
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
impl From<service_catalog::CatalogError> for ServiceError {
    fn from(value: service_catalog::CatalogError) -> Self {
        error("catalog", format!("目录校验失败：{value}"))
    }
}
impl From<execution_interaction::InteractionError> for ServiceError {
    fn from(value: execution_interaction::InteractionError) -> Self {
        error("interaction", format!("交互未接纳：{value}"))
    }
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "CatalogItem")]
pub struct CatalogView {
    pub catalog: CatalogRef,
    pub item_id: Id,
    pub variant_id: Id,
    pub kind: CatalogKind,
    pub name: String,
    pub description: String,
    pub category: String,
    pub resource: ResourceBinding,
    pub fields: BTreeMap<Id, Parameter>,
    pub input_schema: serde_json::Value,
    pub display: DisplayStatus,
    pub reason: String,
    pub availability: Availability,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "Plan")]
pub struct PlanView {
    pub request_id: RequestId,
    pub revision: u32,
    pub plan_id: PlanId,
    pub digest: Digest,
    pub item_id: Id,
    pub title: String,
    pub action: Id,
    pub resource: ResourceBinding,
    pub target: String,
    pub run_as: String,
    pub network: String,
    pub data_scope: String,
    pub permission: String,
    pub parameters: Vec<ParameterSummary>,
    pub expires_at_unix_ms: u64,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct ParameterSummary {
    pub label: String,
    pub state: &'static str,
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "Interaction")]
pub struct InteractionView {
    pub id: String,
    pub kind: Kind,
    pub status: InteractionStatus,
    pub message: &'static str,
    pub expires_at_unix_ms: u64,
    pub options: Vec<Choice>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestView {
    pub plan: PlanView,
    pub status: RequestStatus,
    pub message: &'static str,
    pub interactions: Vec<InteractionView>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub mode: ServiceMode,
    pub instance_id: String,
    pub target_label: &'static str,
    pub catalog: Vec<CatalogView>,
    pub requests: Vec<RequestView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    Listed,
    Withdrawn,
    Expired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ServiceMode {
    Fixture,
    S1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InteractionStatus {
    Pending,
    Answered,
    Cancelled,
    Expired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RequestStatus {
    Waiting,
    Approval,
    Complete,
    Stopped,
    RestartRequired,
    UnknownEffect,
}
