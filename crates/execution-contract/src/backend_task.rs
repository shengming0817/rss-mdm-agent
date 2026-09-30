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
