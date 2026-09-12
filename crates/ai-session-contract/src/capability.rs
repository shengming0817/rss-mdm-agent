use crate::{Conversation, Name};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineIdentity {
    pub name: Name,
    pub version: Name,
    pub process_generation: Name,
}
/// Opaque host configuration reference. Never embeds tokens, permissions or raw provider config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineConfigRef {
    pub id: Name,
    pub revision: Name,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ContinuationScope {
    SameProcess,
    AcrossProcesses,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum Continuation {
    Supported { scope: ContinuationScope },
    Unsupported {},
    Unknown {},
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InterruptionConfirmation {
    RequestOnly,
    TerminalAcknowledged,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum Interruption {
    Supported {
        confirmation: InterruptionConfirmation,
    },
    Unsupported {},
    Unknown {},
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ToolControlMode {
    HostMediated,
    ProviderManaged,
    Disabled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum ToolControl {
    Supported { mode: ToolControlMode },
    Unsupported {},
    Unknown {},
}
/// Claims attached to a particular engine/version/generation by Conversation.
/// A decoded Supported value does not prove native tool isolation. The adapter must
/// establish that independently before the host can enable controlled operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineCapabilities {
    pub continuation: Continuation,
    pub interruption: Interruption,
    pub tool_control: ToolControl,
}
/// Host requirements, not model-supplied authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityRequirements {
    pub continuation: Option<ContinuationScope>,
    pub interruption: Option<InterruptionConfirmation>,
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
pub struct ResumeBinding {
    pub engine: EngineIdentity,
    pub config: EngineConfigRef,
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error,
)]
#[serde(rename_all = "camelCase")]
pub enum ResumeUnavailable {
    #[error("continuation unsupported")]
    Unsupported,
    #[error("continuation capability unknown")]
    UnknownCapability,
    #[error("stale process generation")]
    StaleProcessGeneration,
    #[error("engine identity changed")]
    EngineChanged,
    #[error("configuration revision changed")]
    ConfigChanged,
    #[error("session missing")]
    MissingSession,
    #[error("engine rejected continuation")]
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
