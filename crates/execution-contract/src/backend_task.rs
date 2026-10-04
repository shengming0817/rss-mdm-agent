//! Public projections of already verified backend tasks; these values never carry authority.
use crate::{Digest, Id, OsSessionRef, RequestId, VersionedRef};
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

/// Protected policy classification; never accepted as a request field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum RiskLevel {
    /// Pure computation without external effects.
    Zero = 0,
    /// Bounded non-sensitive read.
    One = 1,
    /// Explicitly permitted bounded side effects; AI requires user confirmation.
    Two = 2,
    /// Destructive or security-sensitive effects; AI is blocked.
    Three = 3,
}
/// Backend-authenticated classification of the exact offer, never an IPC claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionRisk {
    /// Explicit effect classification selected by backend policy.
    pub level: RiskLevel,
    /// Exact backend risk policy revision.
    pub source: RiskSource,
    /// Exclusive trusted decision deadline.
    pub expires_at_unix_ms: u64,
}
/// Risk authority is explicit; local software rules never impersonate a Policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RiskSource {
    /// Current server Policy decision, subsequently checked against the signed Offer.
    Policy {
        /// Exact configuration revision.
        policy: VersionedRef,
    },
    /// Closed Rust rule for a verified software action.
    Software {
        /// Exact signed software definition and action.
        definition: Digest,
    },
}
/// Employee input; caller source and confirmation are supplied only by trusted native code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyChoice {
    /// Stable request allocated before submission, reused after response loss.
    pub request: RequestId,
    /// Policy from the current authorized directory.
    pub policy_id: Id,
    /// Exact current configuration revision.
    pub policy_revision: u64,
    /// Immutable execution version.
    pub policy_version: Id,
    /// Only Resource fields whose Policy source is Input.
    pub parameters: std::collections::BTreeMap<String, serde_json::Value>,
}
/// One preparation reference, before or after a signed Offer has arrived.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PreparedAction {
    /// A bounded native observation authorized by an exact current signed software definition.
    SoftwareRead {
        /// Original read identity, independent of installation execution.
        choice: SoftwareReadChoice,
        /// Exact declared software definition; this is a software risk source.
        definition: Digest,
        /// Original preparation validity.
        expires_at_unix_ms: u64,
        /// Independently collected fact, appended once and never treated as installation success.
        observation: Option<SoftwareObservation>,
    },
    /// Existing offered software selection.
    Offered {
        /// Exact signed task projection.
        offer: BackendTask,
    },
    /// Policy invocation and its eventual exact Offer, held in the same journal record.
    Policy {
        /// Immutable employee choice.
        choice: PolicyChoice,
        /// Formal backend directory DTO, including the Resource-owned schema.
        directory: Box<rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem>,
        /// Backend Run returned for the original request, absent until acceptance is known.
        run_id: Option<Id>,
        /// Eventual signed Offer; no task/attempt is invented before this exists.
        offer: Option<BackendTask>,
        /// Authenticated product employee, checked against the signed authorization.
        actor: rss_mdm_agent_wire::runtime::ScriptActor,
        /// Native product login generation.
        generation: Id,
        /// Bounded validity of the original preparation.
        expires_at_unix_ms: u64,
    },
}
impl Eq for PreparedAction {}
impl PartialEq for PreparedAction {
    fn eq(&self, other: &Self) -> bool {
        // The producer DTO has no PartialEq; compare its structural JSON, never a display name.
        serde_json::to_value(self)
            .ok()
            .zip(serde_json::to_value(other).ok())
            .is_some_and(|(a, b)| a == b)
    }
}
impl PreparedAction {
    /// Original stable request, independent of the arrival of a task.
    pub fn request(&self) -> &RequestId {
        match self {
            Self::SoftwareRead { choice, .. } => &choice.request,
            Self::Offered { offer } => &offer.request,
            Self::Policy { choice, .. } => &choice.request,
        }
    }
    /// Actual Offer only; absence cannot be treated as a fabricated task.
    pub fn offer(&self) -> Option<&BackendTask> {
        match self {
            Self::SoftwareRead { .. } => None,
            Self::Offered { offer } => Some(offer),
            Self::Policy { offer, .. } => offer.as_ref(),
        }
    }
    /// The same immutable choice; only acceptance and eventual Offer may be appended.
    pub fn same_choice(&self, other: &Self) -> bool {
        let mut a = self.clone();
        let mut b = other.clone();
        for action in [&mut a, &mut b] {
            if let Self::SoftwareRead { observation, .. } = action {
                *observation = None;
            }
            if let Self::Policy { run_id, offer, .. } = action {
                *run_id = None;
                *offer = None;
            }
        }
        a == b
            && match (self, other) {
                (
                    Self::SoftwareRead {
                        observation: old, ..
                    },
                    Self::SoftwareRead { observation, .. },
                ) => old
                    .as_ref()
                    .is_none_or(|value| observation.as_ref() == Some(value)),
                (
                    Self::Policy {
                        run_id: old_run,
                        offer: old_offer,
                        ..
                    },
                    Self::Policy { run_id, offer, .. },
                ) => {
                    old_run
                        .as_ref()
                        .is_none_or(|old| run_id.as_ref() == Some(old))
                        && old_offer
                            .as_ref()
                            .is_none_or(|old| offer.as_ref() == Some(old))
                        && offer
                            .as_ref()
                            .is_none_or(|offer| run_id.as_ref() == Some(&offer.task))
                }
                _ => true,
            }
    }
    /// Preparation validity independent of transport lease changes.
    pub fn expires_at_unix_ms(&self) -> u64 {
        match self {
            Self::SoftwareRead {
                expires_at_unix_ms, ..
            } => *expires_at_unix_ms,
            Self::Offered { offer } => u64::try_from(offer.expires_at)
                .unwrap_or(0)
                .saturating_mul(1000),
            Self::Policy {
                expires_at_unix_ms, ..
            } => *expires_at_unix_ms,
        }
    }
}
/// Sole product confirmation, persisted together with the original AI request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackendConfirmation {
    /// Actual original login, independently authenticated at confirmation and dispatch.
    pub os_session: OsSessionRef,
    /// Trusted service time at the user's explicit response.
    pub confirmed_at_unix_ms: u64,
    /// Exclusive deadline, bounded by the offer, policy and one-minute interaction limit.
    pub expires_at_unix_ms: u64,
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
    /// A bounded native observation was collected; no execution attempt or side effect exists.
    Observed,
    /// Risk-two AI action awaiting its sole explicit product confirmation.
    AwaitingConfirmation,
    /// Product gate is satisfied; this still grants no backend Start or OS authority.
    Ready,
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
    /// No backend-authenticated risk information is available.
    RiskUnknown,
    /// Backend policy prohibits this AI action.
    RiskBlocked,
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
    /// The sole immutable choice and appended backend association.
    pub action: PreparedAction,
    /// OS-authenticated original trigger; never accepted from an IPC request body.
    pub trigger: crate::BackendTrigger,
    /// Immutable trusted classification; absent for human requests or unknown AI risk.
    pub risk: Option<ExecutionRisk>,
    /// Explicit risk-two response; readiness alone never proves confirmation.
    pub confirmation: Option<BackendConfirmation>,
    /// Monotonic journal revision.
    pub revision: u64,
    /// Preparation state, separate from execution lifecycle.
    pub state: BackendRequestState,
    /// Diagnosis when preparation failed.
    pub failure: Option<BackendRequestFailure>,
}

impl BackendRequest {
    /// Actual native login bound to the original employee action.
    pub fn trigger_os_session(&self) -> Option<&Id> {
        match &self.trigger {
            crate::BackendTrigger::Human { os_session }
            | crate::BackendTrigger::Ai { os_session, .. } => Some(&os_session.session),
            _ => None,
        }
    }

    /// Check stored product-gate shape. DTO validity alone never authenticates risk or consent.
    pub fn valid_product_gate(&self) -> bool {
        use BackendRequestState as S;
        let terminal = matches!(self.state, S::Cancelled | S::Failed);
        match &self.trigger {
            crate::BackendTrigger::Human { .. } => {
                self.risk.is_none()
                    && self.confirmation.is_none()
                    && self.state != S::AwaitingConfirmation
            }
            crate::BackendTrigger::Ai { os_session, .. } => {
                let Some(risk) = &self.risk else {
                    return terminal && self.confirmation.is_none();
                };
                if risk.expires_at_unix_ms == 0 {
                    return false;
                }
                match risk.level {
                    RiskLevel::One => {
                        self.confirmation.is_none() && self.state != S::AwaitingConfirmation
                    }
                    RiskLevel::Two => match &self.confirmation {
                        None => terminal || self.state == S::AwaitingConfirmation,
                        Some(answer) => {
                            answer.os_session == *os_session
                                && answer.confirmed_at_unix_ms < answer.expires_at_unix_ms
                                && answer.expires_at_unix_ms
                                    <= answer.confirmed_at_unix_ms.saturating_add(60_000)
                                && answer.expires_at_unix_ms <= risk.expires_at_unix_ms
                                && answer.expires_at_unix_ms <= self.action.expires_at_unix_ms()
                                && self.state != S::AwaitingConfirmation
                        }
                    },
                    RiskLevel::Zero | RiskLevel::Three => terminal && self.confirmation.is_none(),
                }
            }
            crate::BackendTrigger::Automatic {} => false,
        }
    }
}

/// Model-visible selection for a finite native software detector; no arbitrary selector or command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareReadSelection {
    /// Exact current signed offer projection.
    pub selection: BackendSelection,
    /// One declared step in that definition.
    pub step: u32,
}
/// Native human read input with its preallocated stable request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareReadChoice {
    /// Original query identity.
    pub request: RequestId,
    /// The sole immutable query input.
    pub input: SoftwareReadSelection,
}
/// Fresh native software fact, with collection time and no installer result claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareObservation {
    /// Completeness of this finite native observation; failed leaves the state Unknown.
    pub quality: crate::OutputQuality,
    /// Time of actual observation from the service clock.
    pub collected_at_unix_ms: u64,
    /// Exact declared package.
    pub package: String,
    /// Exact declared desired version, separate from the observed version.
    pub expected_version: String,
    /// Native receipt or product observation; unknown remains explicit.
    pub state: crate::SoftwareState,
}
