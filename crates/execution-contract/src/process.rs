//! Bounded process facts. These DTOs carry evidence, never dispatch authority.
use crate::{AttemptId, Digest, Id};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
/// Immutable terminal capture persisted beside the existing execution attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessEvidence {
    /// Exact immutable plan, not a bare task identifier.
    pub plan_digest: Digest,
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
