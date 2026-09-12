use crate::{
    ConversationId, EngineCapabilities, EngineConfigRef, EngineIdentity, MessageId, Name,
    ResumeBinding, ResumeUnavailable, ToolCallId, TurnId, V1,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Conversation {
    pub id: ConversationId,
    pub engine: EngineIdentity,
    pub config: EngineConfigRef,
    pub capabilities: EngineCapabilities,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    User,
    Assistant,
    Tool,
}
/// Untrusted text; consumers render inert text and apply their own disclosure policy.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContentPart {
    Text { text: String },
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Message {
    pub id: MessageId,
    pub turn_id: TurnId,
    pub role: Role,
    pub content: Vec<ContentPart>,
}
/// A proposal is data, not an instruction to an executor. The host maps it to its
/// authorization service. Even actor/approved keys inside arguments remain untrusted data.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallProposal {
    pub id: ToolCallId,
    pub turn_id: TurnId,
    pub name: Name,
    pub arguments: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ToolResponseOutcome {
    Returned,
    Rejected,
    Unavailable,
}
/// Model-facing protocol response, never an Execution Evidence or approval record.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallResponse {
    pub proposal_id: ToolCallId,
    pub turn_id: TurnId,
    pub outcome: ToolResponseOutcome,
    pub content: Vec<ContentPart>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TurnOutcome {
    Completed,
    Interrupted,
    Failed,
    Refused,
    LimitReached,
}
/// Accepted acknowledges dispatch only; it never confirms turn/OS termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CancelDispatchOutcome {
    Accepted,
    AlreadyTerminal,
    Unsupported,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum ResumeOutcome {
    Resumed { engine: EngineIdentity },
    Unavailable { reason: ResumeUnavailable },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StreamErrorCode {
    ProtocolUnsupported,
    TransportLost,
    ProviderFailure,
    InvalidData,
    LimitExceeded,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Event {
    ConversationStarted {
        conversation: Conversation,
    },
    Message {
        message: Message,
    },
    MessageDelta {
        turn_id: TurnId,
        message_id: MessageId,
        role: Role,
        text: String,
    },
    ToolCallProposed {
        proposal: ToolCallProposal,
    },
    ToolCallResponded {
        response: ToolCallResponse,
    },
    TurnFinished {
        turn_id: TurnId,
        outcome: TurnOutcome,
    },
    CancelDispatched {
        turn_id: TurnId,
        outcome: CancelDispatchOutcome,
    },
    ResumeFinished {
        outcome: ResumeOutcome,
    },
    /// Non-terminal protocol error. Lost transport must not fabricate completion/interruption.
    StreamError {
        turn_id: Option<TurnId>,
        code: StreamErrorCode,
    },
}
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventEnvelope {
    pub schema_version: V1,
    pub conversation_id: ConversationId,
    pub sequence: u64,
    pub event: Event,
}
impl EventEnvelope {
    pub fn terminal_outcome(&self) -> Option<TurnOutcome> {
        match self.event {
            Event::TurnFinished { outcome, .. } => Some(outcome),
            _ => None,
        }
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Command {
    SendMessage { message: Message },
    Cancel { turn_id: TurnId },
    Resume { binding: ResumeBinding },
    RespondToTool { response: ToolCallResponse },
}
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandEnvelope {
    pub schema_version: V1,
    pub conversation_id: ConversationId,
    pub command: Command,
}
macro_rules! redacted_debug {
    ($($t:ty),*) => { $(impl fmt::Debug for $t {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(concat!(stringify!($t), "([redacted])")) }
    })* };
}
redacted_debug!(
    ContentPart,
    Message,
    ToolCallProposal,
    ToolCallResponse,
    Event,
    EventEnvelope,
    Command,
    CommandEnvelope
);
