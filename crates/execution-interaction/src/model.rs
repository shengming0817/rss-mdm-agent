use serde::{Deserialize, Deserializer, Serialize};

/// Bounded opaque interaction-owned reference; not a task identity or authority proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Reference(String);
impl Reference {
    /// Accept 1–128 ASCII letters, digits or `-_.:`. Never store secret payloads here.
    pub fn new(value: impl Into<String>) -> Result<Self, InteractionError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
        {
            return Err(InteractionError::Reference);
        }
        Ok(Self(value))
    }
    /// Borrow the opaque reference, without resolving or authenticating it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for Reference {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
/// User confirmation cannot stand in for privacy consent or administrator authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationPurpose {
    /// Acknowledge readiness to continue.
    Continue,
    /// Acknowledge readiness to close an application; does not close anything itself.
    CloseApplication,
}
/// Exact waiting reason. Referenced choices and schemas belong to the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Kind {
    /// Ordinary user confirmation, never an elevation grant.
    UserConfirmation {
        /// The bounded purpose of confirmation.
        purpose: ConfirmationPurpose,
    },
    /// Consent for an explicitly referenced privacy scope.
    PrivacyConsent {
        /// Exact privacy scope revision reference.
        scope: Reference,
    },
    /// Wait for a separately verified administrator decision.
    AdministratorAuthorization {
        /// Exact approval request reference.
        request: Reference,
    },
    /// Wait for parameters validated by their schema owner.
    ParameterInput {
        /// Exact schema reference; no schema engine is embedded here.
        schema: Reference,
    },
    /// Wait for selection among externally defined maintenance windows.
    MaintenanceWindow {
        /// Exact available choices reference.
        options: Reference,
    },
    /// Wait for a restart choice, without scheduling or restarting.
    RestartPrompt {
        /// Exact available choices reference.
        options: Reference,
    },
}
/// Submitted data only. Even administrator records still need C07/C08 verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Response {
    /// Ordinary confirmation answer.
    Confirmation {
        /// Whether the user confirmed the requested purpose.
        accepted: bool,
    },
    /// Privacy consent answer, not an authorization grant.
    PrivacyConsent {
        /// Whether the user consented to the exact scope.
        accepted: bool,
    },
    /// Unverified administrator decision reference; never an approved boolean.
    AdministratorDecision {
        /// Exact external decision record reference.
        record: Reference,
    },
    /// External validated parameter submission reference; no raw sensitive values.
    ParameterSubmission {
        /// Exact submission reference for the requested schema.
        submission: Reference,
    },
    /// External maintenance-window selection reference.
    MaintenanceSelection {
        /// Exact selected option reference.
        selection: Reference,
    },
    /// External restart selection reference.
    RestartSelection {
        /// Exact selected option reference.
        selection: Reference,
    },
}
impl Kind {
    pub(crate) fn accepts(&self, response: &Response) -> bool {
        matches!(
            (self, response),
            (Self::UserConfirmation { .. }, Response::Confirmation { .. })
                | (Self::PrivacyConsent { .. }, Response::PrivacyConsent { .. })
                | (
                    Self::AdministratorAuthorization { .. },
                    Response::AdministratorDecision { .. }
                )
                | (
                    Self::ParameterInput { .. },
                    Response::ParameterSubmission { .. }
                )
                | (
                    Self::MaintenanceWindow { .. },
                    Response::MaintenanceSelection { .. }
                )
                | (
                    Self::RestartPrompt { .. },
                    Response::RestartSelection { .. }
                )
        )
    }
}
/// Immutable interaction identity, subject binding and waiting reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Spec {
    /// Unique interaction identity within the host's protected storage namespace.
    pub id: Reference,
    /// Exact opaque subject binding (authority/task/plan) resolved and authorized by the host.
    pub subject: Reference,
    /// Immutable waiting reason.
    pub kind: Kind,
    /// Exclusive deadline in UTC Unix milliseconds, supplied by the host.
    pub expires_at_unix_ms: u64,
}
/// One-shot status; no variant is an execution result or permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Status {
    /// No committed answer, cancellation or expiry.
    Pending,
    /// One response was committed before the deadline.
    Answered {
        /// Response idempotency identity.
        id: Reference,
        /// Submitted data, still not authorization.
        response: Response,
        /// Host-observed UTC Unix milliseconds.
        at_unix_ms: u64,
    },
    /// This interaction was explicitly cancelled; no task cancellation is implied.
    Cancelled {
        /// Cancellation idempotency identity.
        id: Reference,
        /// Host-observed UTC Unix milliseconds.
        at_unix_ms: u64,
    },
    /// Deadline elapsed without a committed answer or cancellation.
    Expired {
        /// Host-observed UTC Unix milliseconds at or after the deadline.
        at_unix_ms: u64,
    },
}
/// Sole current storage DTO. Restore validates structure; storage authenticity is external.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    /// Only version 1 is accepted; there is no legacy reader.
    pub version: u8,
    /// Immutable request and subject binding.
    pub spec: Spec,
    /// Creation time in host-observed UTC Unix milliseconds.
    pub opened_at_unix_ms: u64,
    /// Zero for pending, one for the single terminal transition.
    pub revision: u64,
    /// Current one-shot state.
    pub status: Status,
}
/// Required host bounds; no implicit unlimited mode.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum encoded snapshot bytes. Pending state must also fit its largest bounded terminal result.
    pub max_snapshot_bytes: usize,
    /// Maximum deadline minus creation time, in milliseconds.
    pub max_lifetime_ms: u64,
}
/// Static, value-free failure vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InteractionError {
    /// Invalid bound configuration.
    #[error("invalid interaction limits")]
    Configuration,
    /// Snapshot exceeds its byte or lifetime bound.
    #[error("interaction bound exceeded")]
    Limit,
    /// Invalid reference representation.
    #[error("invalid reference")]
    Reference,
    /// Malformed, unsupported or inconsistent snapshot.
    #[error("invalid interaction snapshot")]
    Snapshot,
    /// Host time precedes creation or the committed transition.
    #[error("interaction time moved backwards")]
    Clock,
    /// An answer belongs to another family of interaction.
    #[error("response kind does not match")]
    ResponseKind,
    /// Reused command ID with different content or command kind.
    #[error("interaction idempotency conflict")]
    IdempotencyConflict,
}
