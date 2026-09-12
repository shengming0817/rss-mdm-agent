use crate::{Conversation, Name};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Exact engine version and process generation associated with a capability or resume claim.
pub struct EngineIdentity {
    /// Provider/engine name used with the exact version to interpret capabilities.
    pub name: Name,
    /// Exact adapter-observed engine version; no implicit upgrade or fallback.
    pub version: Name,
    /// Opaque identity for the engine process generation; stale generations may not resume.
    pub process_generation: Name,
}
/// Opaque host configuration reference. Never embeds tokens, permissions or raw provider config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineConfigRef {
    /// Opaque host configuration identity, not configuration contents.
    pub id: Name,
    /// Exact host configuration revision used for the conversation.
    pub revision: Name,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Process lifetime across which the adapter claims conversation continuation is possible.
pub enum ContinuationScope {
    /// Continuation requires the same engine process generation.
    SameProcess,
    /// Continuation may be requested across generations for the same engine/config revision.
    AcrossProcesses,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
/// Continuation capability: a supported scope, explicitly unsupported, or not established.
pub enum Continuation {
    /// The adapter claims the specified continuation scope.
    Supported {
        /// Engine-process lifetime across which continuation is claimed to work.
        scope: ContinuationScope,
    },
    /// Continuation is explicitly unavailable.
    Unsupported {},
    /// Continuation support has not been established; cannot satisfy a requirement.
    Unknown {},
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// What the engine protocol can acknowledge about a cancellation request.
pub enum InterruptionConfirmation {
    /// The protocol can acknowledge dispatch, without a terminal confirmation guarantee.
    RequestOnly,
    /// The protocol can explicitly acknowledge an interrupted terminal turn.
    TerminalAcknowledged,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
/// Interruption capability; a dispatch acknowledgment alone is not confirmed termination.
pub enum Interruption {
    /// The adapter claims this cancellation acknowledgment strength.
    Supported {
        /// Protocol acknowledgment strength; request acceptance is weaker than a terminal acknowledgment.
        confirmation: InterruptionConfirmation,
    },
    /// Interruption is explicitly unavailable.
    Unsupported {},
    /// Interruption support has not been established.
    Unknown {},
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Who mediates model tools. Only independently verified host mediation supports controlled use.
pub enum ToolControlMode {
    /// Tools are claimed to be mediated by the host; actual bypass isolation still needs proof.
    HostMediated,
    /// The provider manages tools; cannot satisfy a host-mediation requirement.
    ProviderManaged,
    /// Tools are disabled for this conversation; ordinary dialogue may remain available.
    Disabled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
/// Tool-control capability; deserialization never verifies the claim or grants execution permission.
pub enum ToolControl {
    /// The adapter claims the specified tool-control mode.
    Supported {
        /// Declared tool mediation mode; no mode value itself proves safety.
        mode: ToolControlMode,
    },
    /// This tool-control capability is unavailable.
    Unsupported {},
    /// Tool mediation has not been established; controlled operation must not be enabled.
    Unknown {},
}
/// Claims attached to a particular engine/version/generation by Conversation.
/// A decoded Supported value does not prove native tool isolation. The adapter must
/// establish that independently before the host can enable controlled operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineCapabilities {
    /// Claimed continuation scope for the containing conversation binding.
    pub continuation: Continuation,
    /// Claimed cancellation acknowledgment strength for the containing conversation.
    pub interruption: Interruption,
    /// Claimed tool mediation mode; the host independently verifies that no native bypass exists.
    pub tool_control: ToolControl,
}
/// Host requirements, not model-supplied authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityRequirements {
    /// Minimum continuation scope; None leaves this axis unconstrained.
    pub continuation: Option<ContinuationScope>,
    /// Minimum acknowledgment strength; None leaves this axis unconstrained.
    pub interruption: Option<InterruptionConfirmation>,
    /// Require independently established host-mediated tools when negotiating capabilities.
    pub host_tool_control: bool,
}
impl EngineCapabilities {
    /// Pure capability negotiation only; does not issue a permit or verify these claims.
    pub fn satisfies(&self, required: CapabilityRequirements) -> bool {
        let continuation = match (required.continuation, self.continuation) {
            (None, _) => true,
            (Some(want), Continuation::Supported { scope }) => {
                scope == want || scope == ContinuationScope::AcrossProcesses
            }
            _ => false,
        };
        let interruption = match (required.interruption, self.interruption) {
            (None, _) => true,
            (Some(want), Interruption::Supported { confirmation }) => {
                confirmation == want
                    || confirmation == InterruptionConfirmation::TerminalAcknowledged
            }
            _ => false,
        };
        continuation
            && interruption
            && (!required.host_tool_control
                || self.tool_control
                    == ToolControl::Supported {
                        mode: ToolControlMode::HostMediated,
                    })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Candidate engine/configuration binding for a saved conversation; no credentials are embedded.
pub struct ResumeBinding {
    /// Exact engine/version/generation binding; claims require adapter-side verification.
    pub engine: EngineIdentity,
    /// Opaque host configuration ID/revision; contains no secrets or permission overrides.
    pub config: EngineConfigRef,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error,
)]
#[serde(rename_all = "camelCase")]
/// Closed reason why requesting or completing continuation is unavailable.
pub enum ResumeUnavailable {
    #[error("continuation unsupported")]
    /// The conversation does not support continuation.
    Unsupported,
    #[error("continuation capability unknown")]
    /// Continuation capability has not been established.
    UnknownCapability,
    #[error("stale process generation")]
    /// Same-process continuation was requested on another generation.
    StaleProcessGeneration,
    #[error("engine identity changed")]
    /// Engine name or version differs from the stored binding.
    EngineChanged,
    #[error("configuration revision changed")]
    /// Host configuration identity/revision differs from the stored binding.
    ConfigChanged,
    #[error("session missing")]
    /// The provider has no matching saved conversation.
    MissingSession,
    #[error("engine rejected continuation")]
    /// The provider rejected the continuation request.
    EngineRejected,
}
/// Check whether a stored conversation can be *requested* on this binding.
/// Passing this check does not prove the engine actually resumed it.
pub fn check_resume(
    conversation: &Conversation,
    candidate: &ResumeBinding,
) -> Result<(), ResumeUnavailable> {
    if conversation.engine.name != candidate.engine.name
        || conversation.engine.version != candidate.engine.version
    {
        return Err(ResumeUnavailable::EngineChanged);
    }
    if conversation.config != candidate.config {
        return Err(ResumeUnavailable::ConfigChanged);
    }
    match conversation.capabilities.continuation {
        Continuation::Unsupported {} => Err(ResumeUnavailable::Unsupported),
        Continuation::Unknown {} => Err(ResumeUnavailable::UnknownCapability),
        Continuation::Supported {
            scope: ContinuationScope::SameProcess,
        } if conversation.engine.process_generation != candidate.engine.process_generation => {
            Err(ResumeUnavailable::StaleProcessGeneration)
        }
        Continuation::Supported { .. } => Ok(()),
    }
}
