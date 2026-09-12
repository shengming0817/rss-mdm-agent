use crate::{ActorId, DeviceId, Digest, Id, PlanId, RequestId, V1};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionedRef {
    pub id: Id,
    pub revision: Id,
}

/// Separate namespaces: none of these serializable references authenticates its issuer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Authority {
    Local { id: Id },
    Enterprise { id: Id, tenant: Id },
    Test { id: Id },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Initiator {
    Human {},
    Ai {
        provider: Id,
        conversation: Id,
        tool_call: Id,
    },
    Policy {
        policy: VersionedRef,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OsAccountRef {
    pub platform: Platform,
    pub subject: Id,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum TargetScope {
    Device {},
    User { account: OsAccountRef },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub device: DeviceId,
    pub platform: Platform,
    pub scope: TargetScope,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RunAs {
    User { account: OsAccountRef },
    System { platform: Platform },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operation {
    pub action: Id,
    pub resource: VersionedRef,
}

/// Literal values are untrusted data. Long-lived credentials MUST use a versioned secret reference.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum InputValue {
    Literal {
        #[serde(deserialize_with = "crate::validation::unique_json")]
        value: serde_json::Value,
    },
    Secret {
        reference: VersionedRef,
    },
}
impl fmt::Debug for InputValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InputValue([redacted])")
    }
}

/// Intent data. The authority/actor are claims to be authenticated by the host, never credentials.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionRequest {
    pub schema_version: V1,
    pub request_id: RequestId,
    pub authority: Authority,
    pub actor: ActorId,
    pub initiator: Initiator,
    pub delegation: Option<VersionedRef>,
    pub target: Target,
    pub operation: Operation,
    #[serde(deserialize_with = "crate::validation::unique_map")]
    pub parameters: BTreeMap<String, InputValue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactArtifactRef {
    pub resource: VersionedRef,
    pub sha256: Digest,
}

/// Exact launch description only. No PATH lookup, download or spawning is performed by this crate.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchSpec {
    pub artifact: ExactArtifactRef,
    pub interpreter: ExactArtifactRef,
    pub argv: Vec<String>,
    pub cwd: String,
    #[serde(deserialize_with = "crate::validation::unique_map")]
    pub env: BTreeMap<String, InputValue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum NetworkAccess {
    Denied {},
    Allowlist { destinations: Vec<String> },
}
/// Required restrictions are declarations; platform enforcement is owned by the runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Constraints {
    pub network: NetworkAccess,
    pub read_paths: Vec<String>,
    pub write_paths: Vec<String>,
    pub allow_child_processes: bool,
    pub require_sandbox: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionBudget {
    /// Total elapsed wall time from the first attempt start, including retries/backoff/waits.
    /// Pre-execution approval waits are excluded; retries never replenish this budget.
    pub total_timeout_ms: u64,
    /// Aggregate stdout/stderr/result bytes across all attempts, including discarded output.
    pub total_output_bytes: u64,
    pub max_attempts: u32,
}
/// UTC Unix milliseconds. The host separately validates current time and revocation freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidityWindow {
    pub not_before_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

/// The sole canonical execution description, including its original request.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanSpec {
    pub schema_version: V1,
    pub plan_id: PlanId,
    pub request: ExecutionRequest,
    pub launch: LaunchSpec,
    pub run_as: RunAs,
    pub constraints: Constraints,
    pub budget: ExecutionBudget,
    pub validity: ValidityWindow,
    pub policy: VersionedRef,
}
macro_rules! redacted_debug {
    ($($t:ty),*) => { $(impl fmt::Debug for $t {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(concat!(stringify!($t), "([redacted])")) }
    })* };
}
redacted_debug!(ExecutionRequest, LaunchSpec, PlanSpec);
