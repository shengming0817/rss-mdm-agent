//! Public projections of already verified backend tasks; these values never carry authority.
use crate::{Digest, Id, RequestId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Exact displayed backend offer, without executable input or authorization claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendTask {
    /// Backend task identity.
    pub task: Id,
    /// Backend attempt identity; reconnect never allocates a new attempt.
    pub attempt: Id,
    /// Full signed offer revision.
    pub revision: Digest,
    /// Deterministic original execution request used for status after response loss.
    pub request: RequestId,
    /// Safe display text selected by the backend.
    pub title: String,
    /// Complete safe action preview, immutable with the offer revision.
    pub summary: BackendTaskSummary,
    /// Exclusive UTC Unix expiry in seconds.
    pub expires_at: i64,
    /// Requires an explicit local user action before requesting Start.
    pub user_initiated: bool,
}
/// A selection references exact backend content and carries no script or local catalog model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendSelection {
    /// Original request shown with the offer, used unchanged for recovery.
    pub request: RequestId,
    /// Backend task identity.
    pub task: Id,
    /// Original backend attempt.
    pub attempt: Id,
    /// Revision shown to the user/model.
    pub revision: Digest,
}
/// Selection acknowledgement, distinct from persisted execution or effect completion.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskSubmission {
    /// Original request to query after a timeout or disconnect.
    pub request: RequestId,
    /// A human still needs to confirm the displayed task in the desktop.
    pub confirmation_required: bool,
}

/// Safe preview bound to the complete signed offer revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum BackendTaskSummary {
    /// Fixed backend script; executable input is not exposed to the UI.
    Script {
        /// Account class selected by the backend.
        identity: BackendIdentity,
    },
    /// Complete ordered software action preview.
    Software {
        /// Install, uninstall or detection intent.
        intent: crate::SoftwareOperation,
        /// Ordered prerequisites and final package.
        steps: Vec<BackendStepSummary>,
    },
}
/// Display-only account class, never OS identity evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BackendIdentity {
    /// System service execution.
    System,
    /// Bound active user helper execution.
    User,
}
/// Exact software coordinate displayed before user confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendStepSummary {
    /// Exact package selected by the backend.
    pub package: String,
    /// Exact version; no implicit latest.
    pub version: String,
    /// Execution account class.
    pub identity: BackendIdentity,
}
/// Preparation state only; none of these values authorizes physical execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BackendRequestState {
    /// AI proposal awaiting explicit human confirmation.
    Proposed,
    /// Human confirmation has been committed.
    Selected,
    /// Preparing or obtaining the original backend Start.
    Submitting,
    /// Preparation failed before an execution intent existed.
    Failed,
    /// The local proposal/selection was withdrawn.
    Cancelled,
}
/// Closed preparation diagnosis without backend bodies or local paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BackendRequestFailure {
    /// A required local resource or backend Start could not be obtained.
    PreparationFailed,
    /// The service restarted before committing an execution intent.
    Interrupted,
    /// The original offer expired.
    Expired,
    /// Backend authorization was revoked.
    Revoked,
}
/// User intent retained in the sole execution journal before a frozen execution exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendRequest {
    /// Exact offer and its display summary.
    pub offer: BackendTask,
    /// OS-authenticated original trigger; never accepted from an IPC request body.
    pub trigger: crate::BackendTrigger,
    /// Monotonic journal revision.
    pub revision: u64,
    /// Preparation state, separate from execution lifecycle.
    pub state: BackendRequestState,
    /// Diagnosis when preparation failed.
    pub failure: Option<BackendRequestFailure>,
}
