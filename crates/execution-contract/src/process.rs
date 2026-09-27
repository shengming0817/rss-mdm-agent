//! Bounded process facts. These DTOs carry evidence, never dispatch authority.
use crate::{AttemptId, Digest, Id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// OS scope correlation only; a restored identifier never grants termination rights.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProcessScope {
    /// The live owner is preparing the admitted attempt; no termination fact is implied.
    Preparing {},
    /// The runner rejected preparation before any process could be created.
    NotStarted {},
    /// Cooperative process group. Group absence does not prove escaped descendants are gone.
    ProcessGroup {
        /// Live owner PID, for diagnostics only.
        owner: u32,
        /// Group identity, for diagnostics only.
        group: u32,
    },
    /// Windows kernel job with an exact service/session namespace.
    JobObject {
        /// Random per-attempt job name.
        name: String,
        /// OS session associated with the job.
        session: u32,
    },
}
/// Why a bounded process owner stopped collecting output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProcessEnd {
    /// Preparation was rejected before any target process started.
    Rejected,
    /// Root process exited; this is not an effect observation.
    Exited,
    /// Explicit cancellation requested.
    Cancelled,
    /// Remaining wall time exhausted.
    TimedOut,
    /// Combined output allowance exhausted.
    OutputLimit,
    /// Read, supervision or termination failure.
    Unknown,
}
/// Quality of the captured result, independent from the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum OutputQuality {
    /// All required output was decoded and structurally valid.
    Complete,
    /// Capture was truncated by the output budget.
    Truncated,
    /// Encoding, JSON shape, row bound or process outcome failed.
    Failed,
    /// A process or pipe is still unaccounted for.
    Partial,
}
/// Closed, value-free mechanism diagnosis, independent of cancellation, exit and effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProcessFailureKind {
    /// No mechanism failure was observed; not a statement of business success.
    None,
    /// An immutable input or artifact did not satisfy access/integrity requirements.
    Denied,
    /// The requested OS identity or controlled input binding is absent.
    Unbound,
    /// The declared restriction cannot be enforced.
    Capability,
    /// A profile or encoding is not supported on this platform.
    Unsupported,
    /// Supplied input, encoding or configuration is malformed.
    InvalidInput,
    /// A materialization, input or owner capacity limit was exceeded.
    Capacity,
    /// An attempt or immutable resource already has a conflicting owner.
    Conflict,
    /// A required platform resource could not be acquired.
    Unavailable,
    /// The owner thread/runtime could not be initialized.
    Runtime,
    /// The target could not be created or its initial thread resumed.
    Spawn,
    /// Controlled stdin could not be delivered completely.
    InputDelivery,
    /// A standard output/error stream could not be read completely.
    Capture,
    /// Root process status could not be observed reliably.
    Supervision,
    /// Captured output did not meet its declared encoding or structure.
    OutputValidation,
}
/// Immutable terminal capture persisted beside the existing execution attempt.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessEvidence {
    /// Exact immutable plan, not a bare task identifier.
    pub content_digest: Digest,
    /// Exact admitted attempt.
    pub attempt_id: AttemptId,
    /// Runner provenance.
    pub runner: Id,
    /// Scope held by the live owner during execution.
    pub scope: ProcessScope,
    /// Capture is final; later evidence cannot rewrite it.
    pub finished: bool,
    /// Root process exit status when available.
    pub exit_code: Option<i32>,
    /// Stop classification, separate from termination proof.
    pub end: ProcessEnd,
    /// First mechanism failure. Required even when no failure has occurred.
    pub failure_kind: ProcessFailureKind,
    /// True only if the whole delegated activity is proven quiescent.
    pub quiescent: bool,
    /// Raw bounded stdout. Never projected into ordinary task summaries.
    pub stdout: Vec<u8>,
    /// Raw bounded stderr.
    pub stderr: Vec<u8>,
    /// All bytes read, including discarded bytes.
    pub total_output_bytes: u64,
    /// Encoding and structure quality, not authorization to publish fields.
    pub quality: OutputQuality,
}

impl std::fmt::Debug for ProcessEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessEvidence")
            .field("attempt", &self.attempt_id)
            .field("finished", &self.finished)
            .field("quality", &self.quality)
            .finish_non_exhaustive()
    }
}
/// Ordinary result projection. No raw output, paths, process identifiers or secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessSummary {
    /// Root capture completed; not proof of all descendants terminating.
    pub finished: bool,
    /// Root exit code, independently from effect or scope quiescence.
    pub exit_code: Option<i32>,
    /// Stop/failure classification.
    pub end: ProcessEnd,
    /// First mechanism failure. Required even when no failure has occurred.
    pub failure_kind: ProcessFailureKind,
    /// Explicit full-scope proof; false means unproven.
    pub quiescent: bool,
    /// Bytes observed including discarded bytes.
    pub total_output_bytes: u64,
    /// Capture/decoding quality, not a business success bit.
    pub quality: OutputQuality,
}
impl ProcessEvidence {
    /// Project only ordinary result fields, without raw diagnostics.
    pub fn summary(&self) -> ProcessSummary {
        ProcessSummary {
            finished: self.finished,
            exit_code: self.exit_code,
            end: self.end,
            failure_kind: self.failure_kind,
            quiescent: self.quiescent,
            total_output_bytes: self.total_output_bytes,
            quality: self.quality,
        }
    }
}
