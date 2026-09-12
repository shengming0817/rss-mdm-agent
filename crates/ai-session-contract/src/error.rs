use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable category independent of provider error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// Malformed JSON or a structure outside the current contract.
    Encoding,
    /// A numeric contract version is not supported.
    UnsupportedVersion,
    /// A value violates a deterministic contract rule.
    InvalidValue,
    /// Untrusted data exceeds a supplied bound.
    LimitExceeded,
    /// Host-supplied limits are invalid.
    InvalidConfiguration,
    /// Related contract values disagree.
    InconsistentContext,
}

/// Closed, value-free location of the failed contract constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    /// Whole session envelope encoding.
    Document,
    /// Local AI schema version.
    Version,
    /// Opaque engine/config/tool name.
    Reference,
    /// Conversation identity or correlation.
    Conversation,
    /// Turn identity.
    Turn,
    /// Message identity.
    Message,
    /// Tool proposal identity.
    ToolCall,
    /// Whole encoded envelope byte count.
    InputBytes,
    /// Text byte budget, including accumulated content parts.
    TextBytes,
    /// Content-part item budget.
    ContentParts,
    /// Encoded tool-argument byte budget.
    ArgumentBytes,
    /// Tool-argument nesting depth.
    ArgumentDepth,
    /// Tool-argument value-node count.
    ArgumentNodes,
    /// Tool-argument object.
    Arguments,
    /// Event ordering sequence number.
    Sequence,
    /// Host-to-engine message direction.
    MessageRole,
}

/// Closed validation rule; never contains a supplied value or dynamic key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rule {
    /// JSON syntax or a closed schema shape is invalid.
    Syntax,
    /// Only the current V1 version is accepted.
    Version,
    /// An opaque identifier violates its documented alphabet or length.
    Identifier,
    /// A required positive value is zero.
    NonZero,
    /// An encoded value or text exceeds its byte budget.
    ByteLimit,
    /// Nested input exceeds the supported depth.
    DepthLimit,
    /// Input contains too many value nodes.
    NodeLimit,
    /// A collection exceeds its item budget.
    CollectionLimit,
    /// A numeric value lies outside its supported exact range.
    NumericRange,
    /// A JSON object contains a repeated member name.
    DuplicateKey,
    /// A required collection or value is empty.
    Empty,
    /// Related identifiers or platforms do not match.
    Mismatch,
    /// A command carries a role disallowed for that direction.
    Direction,
}

/// A bounded diagnostic of kind, static field and rule; carries no input, key, payload or source chain.
/// Diagnostic values do not establish identity, authorization or execution evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[error("{kind:?} at {field:?}: {rule:?}")]
pub struct ContractError {
    kind: ErrorKind,
    field: Field,
    rule: Rule,
}
impl ContractError {
    /// Construct a value-free diagnostic from this owner's closed vocabulary.
    pub const fn new(kind: ErrorKind, field: Field, rule: Rule) -> Self {
        Self { kind, field, rule }
    }
    /// Return the failure category, including the distinction between configuration and input limits.
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }
    /// Return the static contract location; never exposes a user-supplied member name.
    pub const fn field(&self) -> Field {
        self.field
    }
    /// Return the precise failed rule without disclosing the rejected value.
    pub const fn rule(&self) -> Rule {
        self.rule
    }
    pub(crate) fn for_serde(self) -> CodecDiagnostic {
        CodecDiagnostic(self)
    }
    pub(crate) fn from_serde(error: serde_json::Error) -> Self {
        // Serde erases custom error types into text. Recover only our reserved,
        // closed diagnostic record; ordinary serde/provider messages are never returned.
        let message = error.to_string();
        if let Some(record) = message.strip_prefix(PREFIX) {
            let record = record
                .split_once(" at line ")
                .map_or(record, |(record, _)| record);
            if let Ok(owned) = serde_json::from_str(record) {
                return owned;
            }
        }
        Self::new(ErrorKind::Encoding, Field::Document, Rule::Syntax)
    }
}
const PREFIX: &str = "ai-session-contract-diagnostic:";
pub(crate) struct CodecDiagnostic(ContractError);
impl fmt::Display for CodecDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(PREFIX)?;
        f.write_str(&serde_json::to_string(&self.0).map_err(|_| fmt::Error)?)
    }
}
