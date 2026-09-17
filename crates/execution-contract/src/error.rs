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
    /// A plan exceeds its execution allowance or has an invalid time window.
    InvalidBudget,
}

/// Closed, value-free location of the failed contract constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    /// Whole local plan or audit encoding.
    Document,
    /// Local schema version.
    Version,
    /// An opaque reference identifier.
    Identifier,
    /// Product actor reference.
    Actor,
    /// Device reference.
    Device,
    /// Execution request identity.
    Request,
    /// Execution plan identity.
    Plan,
    /// Execution attempt identity.
    Attempt,
    /// Audit event identity.
    Event,
    /// SHA-256 digest representation.
    Digest,
    /// Whole encoded input or frozen-plan byte count.
    InputBytes,
    /// Nested JSON depth.
    Depth,
    /// Total JSON value-node count.
    Nodes,
    /// String/key byte budget.
    StringBytes,
    /// Per-array/object item budget.
    CollectionItems,
    /// Plan-wide elapsed execution time budget.
    Timeout,
    /// Plan-wide output byte budget.
    OutputBytes,
    /// Plan-wide attempt-count budget.
    Attempts,
    /// Plan start/end time window.
    Validity,
    /// Target device/user context.
    Target,
    /// Target execution account context.
    RunAs,
    /// Required target user-session context.
    Session,
    /// Launch working-directory declaration.
    WorkingDirectory,
    /// Launch argument vector.
    Arguments,
    /// Controlled standard-input byte allowance.
    StandardInput,
    /// Explicit launch environment.
    Environment,
    /// Network allowlist collection.
    Network,
    /// Normalized DNS/IP destination host.
    NetworkHost,
    /// Explicit destination port.
    NetworkPort,
    /// Declared filesystem read paths.
    ReadPaths,
    /// Declared filesystem write paths.
    WritePaths,
    /// Observation evidence references.
    AuditEvidence,
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
    /// Expected exactly 64 lowercase SHA-256 hex characters.
    Digest,
    /// Expected an absolute path without NUL or dot components.
    AbsolutePath,
    /// A process argument or JSON key contains a NUL byte.
    Nul,
    /// Expected exactly one typed artifact path in the argument vector.
    ArtifactSlot,
    /// Expected a portable ASCII environment variable name.
    EnvironmentName,
    /// Environment literals must be NUL-free strings; secrets remain references.
    EnvironmentValue,
    /// Environment names collide under the target platform comparison.
    CaseCollision,
    /// Expected a standalone DNS/IP host without URL components.
    HostSyntax,
    /// The validity start must precede its end.
    TimeOrder,
    /// A requested execution allowance exceeds the host maximum.
    BudgetLimit,
    /// Test authority cannot claim real process or state evidence.
    TestEvidence,
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
const PREFIX: &str = "execution-contract-diagnostic:";
pub(crate) struct CodecDiagnostic(ContractError);
impl fmt::Display for CodecDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(PREFIX)?;
        f.write_str(&serde_json::to_string(&self.0).map_err(|_| fmt::Error)?)
    }
}
