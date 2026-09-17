use crate::{ExactArtifactRef, VersionedRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Exact binary and immutable calling convention. Neither reference proves availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterpreterRef {
    /// Binary identity; the adapter must verify bytes, without PATH or version fallback.
    pub artifact: ExactArtifactRef,
    /// Calling convention identity/revision; matched together with the binary.
    pub profile: VersionedRef,
}

/// One argument; artifact materialization is a typed slot, never a magic literal.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum LaunchArg {
    /// One NUL-free argument, passed without shell interpolation or secret resolution.
    Literal {
        /// Non-secret data; not a pre-quoted command fragment.
        value: String,
    },
    /// The adapter supplies the absolute path of the exact verified launch artifact.
    /// The complete argv must contain exactly one such slot.
    ArtifactPath {},
}
impl fmt::Debug for LaunchArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LaunchArg([redacted])")
    }
}

/// Expected script byte encoding, checked against exact bytes by the later adapter.
/// A declaration never authorizes transcoding, adding a BOM or changing line endings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactEncoding {
    /// UTF-8 without a byte-order mark.
    Utf8,
    /// UTF-8 with the original byte-order mark.
    Utf8Bom,
    /// UTF-16 little endian with the original byte-order mark.
    Utf16LeBom,
}

/// Explicit standard-stream text encoding; invalid sequences are diagnostic failures.
/// Output evidence retains the original bytes; decoding must not replace invalid bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TextEncoding {
    /// UTF-8 text.
    Utf8,
    /// UTF-16 little endian text.
    Utf16Le,
}

/// Noninteractive stdin only: no inherited handle, terminal, literal body or executable text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum StandardInput {
    /// No input; adapter closes the stream.
    Closed {},
    /// Resolve a bounded input under owner authorization; it may contain a secret.
    Controlled {
        /// Immutable input reference, not input/credential bytes.
        reference: VersionedRef,
        /// Required byte encoding of the materialized input.
        encoding: TextEncoding,
        /// Positive maximum bytes for this input, checked against the independent host ceiling.
        max_bytes: u64,
    },
}

/// Capture both streams as bytes; no inherited stdout/stderr or terminal.
/// All bytes, including undecodable and discarded bytes, consume the plan's output budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputSpec {
    /// Required stdout interpretation; preserve raw bytes on failure.
    pub stdout: TextEncoding,
    /// Required stderr interpretation; preserve raw bytes on failure.
    pub stderr: TextEncoding,
}
