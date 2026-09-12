use crate::{
    ConversationId, EngineCapabilities, EngineConfigRef, EngineIdentity, MessageId, Name,
    ResumeBinding, ResumeUnavailable, ToolCallId, TurnId, V1,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Conversation identity and the exact engine/configuration capability snapshot that describes it.
pub struct Conversation {
    /// Conversation identity, also carried by event/command envelopes.
    pub id: ConversationId,
    /// Exact engine/version/generation binding; claims require adapter-side verification.
    pub engine: EngineIdentity,
    /// Opaque host configuration ID/revision; contains no secrets or permission overrides.
    pub config: EngineConfigRef,
    /// Capabilities claimed for this exact engine/configuration, not trusted execution evidence.
    pub capabilities: EngineCapabilities,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Conversation role only; does not authenticate a product actor.
pub enum Role {
    /// User-channel content; a role label does not authenticate its author.
    User,
    /// Model-generated untrusted content.
    Assistant,
    /// Untrusted tool-channel content, not execution evidence.
    Tool,
}
/// Untrusted text; consumers render inert text and apply their own disclosure policy.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContentPart {
    /// Inert untrusted text; HTML/terminal escape execution is a consumer concern.
    Text {
        /// Untrusted text content; render as data and never execute embedded markup.
        text: String,
    },
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Complete untrusted message belonging to one model turn.
pub struct Message {
    /// Stable message identity shared by snapshots and deltas.
    pub id: MessageId,
    /// Stable model-turn correlation, separate from execution request/attempt identities.
    pub turn_id: TurnId,
    /// Conversation role only; never a trusted product user or authority.
    pub role: Role,
    /// Untrusted text parts; consumers apply disclosure policy and render inert text.
    pub content: Vec<ContentPart>,
}
/// A proposal is data, not an instruction to an executor. The host maps it to its
/// authorization service. Even actor/approved keys inside arguments remain untrusted data.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallProposal {
    /// Proposal identity, not an execution task or approval identity.
    pub id: ToolCallId,
    /// Stable model-turn correlation, separate from execution request/attempt identities.
    pub turn_id: TurnId,
    /// Tool name to resolve through the host catalog; not a command or executable path.
    pub name: Name,
    #[serde(deserialize_with = "crate::arguments::deserialize")]
    /// Untrusted tool data with recursively unique keys; actor/approved keys grant no permission.
    pub arguments: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Protocol disposition of a tool response, independent of execution success.
pub enum ToolResponseOutcome {
    /// A result was returned to the model; not proof that an operation succeeded.
    Returned,
    /// The host rejected the proposal.
    Rejected,
    /// The host/tool could not provide a response or operation.
    Unavailable,
}
/// Model-facing protocol response, never an Execution Evidence or approval record.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallResponse {
    /// Tool proposal being answered; does not identify an execution approval.
    pub proposal_id: ToolCallId,
    /// Stable model-turn correlation, separate from execution request/attempt identities.
    pub turn_id: TurnId,
    /// The specific protocol outcome defined by the containing event or response.
    pub outcome: ToolResponseOutcome,
    /// Untrusted text parts; consumers apply disclosure policy and render inert text.
    pub content: Vec<ContentPart>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Explicit terminal model-turn outcome; never proves an OS task or side effect terminated.
pub enum TurnOutcome {
    /// The model turn completed, independently of any execution task.
    Completed,
    /// The engine explicitly acknowledged that this turn was interrupted.
    Interrupted,
    /// The model turn failed.
    Failed,
    /// The engine refused the requested turn.
    Refused,
    /// The engine terminated the turn after a model/protocol limit.
    LimitReached,
}
/// Accepted acknowledges dispatch only; it never confirms turn/OS termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CancelDispatchOutcome {
    /// Cancellation request was dispatched; more stream events may still follow.
    Accepted,
    /// The addressed turn was already terminal when cancellation was handled.
    AlreadyTerminal,
    /// The engine cannot perform this cancellation request.
    Unsupported,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
/// Actual provider continuation response, distinct from the pure resume eligibility check.
pub enum ResumeOutcome {
    /// The provider confirmed continuation on the returned binding.
    Resumed {
        /// Exact engine/version/generation binding; claims require adapter-side verification.
        engine: EngineIdentity,
    },
    /// Continuation did not occur for the stated closed reason.
    Unavailable {
        /// Closed provider/eligibility failure reason; no raw diagnostic payload.
        reason: ResumeUnavailable,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
/// Value-free stream failure classification; a stream failure is not a terminal turn outcome.
pub enum StreamErrorCode {
    /// The adapter cannot interpret or support the observed protocol.
    ProtocolUnsupported,
    /// The stream transport was lost; turn termination remains unconfirmed.
    TransportLost,
    /// A provider-level failure occurred without exposing its raw diagnostics.
    ProviderFailure,
    /// The stream contained invalid contract data.
    InvalidData,
    /// A stream input/output constraint was exceeded.
    LimitExceeded,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
/// Closed V1 stream events. Unknown variants are rejected, not treated as completion.
pub enum Event {
    /// A conversation and its exact capability binding were established.
    ConversationStarted {
        /// Conversation snapshot; its ID must match the envelope conversation ID.
        conversation: Conversation,
    },
    /// A complete message snapshot.
    Message {
        /// Complete message content for this turn; untrusted regardless of source role.
        message: Message,
    },
    /// Append-only text delta correlated by turn/message IDs.
    MessageDelta {
        /// Stable model-turn correlation, separate from execution request/attempt identities.
        turn_id: TurnId,
        /// Stable message identity used to append deltas to the same message.
        message_id: MessageId,
        /// Conversation role only; never a trusted product user or authority.
        role: Role,
        /// Untrusted text fragment. Consumers append by message ID and must not execute markup.
        text: String,
    },
    /// An untrusted model tool request awaiting host handling.
    ToolCallProposed {
        /// Model tool proposal only; the host must separately authorize any execution mapping.
        proposal: ToolCallProposal,
    },
    /// A protocol response to a model proposal, without execution authority.
    ToolCallResponded {
        /// Model-facing tool response, not a trusted execution receipt.
        response: ToolCallResponse,
    },
    /// Explicit model-turn terminal outcome.
    TurnFinished {
        /// Stable model-turn correlation, separate from execution request/attempt identities.
        turn_id: TurnId,
        /// The specific protocol outcome defined by the containing event or response.
        outcome: TurnOutcome,
    },
    /// Cancellation dispatch acknowledgment, not a terminal event.
    CancelDispatched {
        /// Stable model-turn correlation, separate from execution request/attempt identities.
        turn_id: TurnId,
        /// The specific protocol outcome defined by the containing event or response.
        outcome: CancelDispatchOutcome,
    },
    /// Provider result of an attempted continuation.
    ResumeFinished {
        /// The specific protocol outcome defined by the containing event or response.
        outcome: ResumeOutcome,
    },
    /// Non-terminal protocol error. Lost transport must not fabricate completion/interruption.
    StreamError {
        /// Stable model-turn correlation, separate from execution request/attempt identities.
        turn_id: Option<TurnId>,
        /// Closed, value-free protocol error classification.
        code: StreamErrorCode,
    },
}
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Versioned conversation event with a stable sequence and event-specific correlations.
pub struct EventEnvelope {
    /// Required current V1 discriminator; absent or unsupported versions are rejected.
    pub schema_version: V1,
    /// Stable conversation correlation; unrelated to an execution task identity.
    pub conversation_id: ConversationId,
    /// Zero-based event sequence, limited to the exact safe JSON integer range.
    pub sequence: u64,
    /// Closed event payload; only TurnFinished supplies a terminal outcome.
    pub event: Event,
}
impl EventEnvelope {
    /// Return a terminal outcome only for TurnFinished; cancellation and stream errors return None.
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
/// Host-to-engine requests; these commands do not invoke an execution runner or issue approval.
pub enum Command {
    /// Submit user-channel content; other roles are rejected on this command.
    SendMessage {
        /// Complete message content for this turn; untrusted regardless of source role.
        message: Message,
    },
    /// Request cancellation of the identified model turn.
    Cancel {
        /// Stable model-turn correlation, separate from execution request/attempt identities.
        turn_id: TurnId,
    },
    /// Request continuation using an exact engine/configuration binding.
    Resume {
        /// Exact engine/configuration binding to use for the continuation request.
        binding: ResumeBinding,
    },
    /// Return a host tool response to the model.
    RespondToTool {
        /// Model-facing tool response, not a trusted execution receipt.
        response: ToolCallResponse,
    },
}
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Versioned request associated with one conversation; validate before dispatch.
pub struct CommandEnvelope {
    /// Required current V1 discriminator; absent or unsupported versions are rejected.
    pub schema_version: V1,
    /// Stable conversation correlation; unrelated to an execution task identity.
    pub conversation_id: ConversationId,
    /// Closed host request payload; it confers no product execution authority.
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
