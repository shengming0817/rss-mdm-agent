//! Bounded, user-selected DNS and TCP diagnostics. No executable input is accepted.
use crate::RequestId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One destination selected by the user, independent of backend configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkTarget {
    /// A domain or IP literal, never a URL or command.
    pub host: String,
    /// One TCP port, from 1 through 65535.
    pub port: u16,
}
/// Finite desktop operations; identity and origin are supplied by authenticated native code.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum NetworkCheckOperation {
    /// Read the installed backend host and port without network I/O.
    Defaults,
    /// Start one immutable request, or look up an exact replay.
    Run {
        /// Original operation UUID, allocated before submission.
        request: RequestId,
        /// Exact selected destination.
        target: NetworkTarget,
    },
    /// Query the original execution and retained result.
    Status {
        /// Original operation UUID.
        request: RequestId,
    },
}
/// Closed diagnostic findings; network failure is distinct from worker failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NetworkFinding {
    /// The step succeeded.
    Reachable,
    /// IP literals need no DNS lookup.
    NotRequired,
    /// No address could be resolved.
    DnsFailed,
    /// The peer refused the TCP connection.
    Refused,
    /// The OS could not reach the destination.
    Unreachable,
    /// The step exhausted its time allowance.
    TimedOut,
}
/// One actual DNS or TCP observation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkProbe {
    /// Resolved socket address for TCP, absent for DNS.
    pub address: Option<String>,
    /// Closed outcome, without raw OS errors.
    pub finding: NetworkFinding,
    /// Actual elapsed milliseconds.
    pub duration_ms: u64,
}
/// Bounded evidence from the fixed worker, not proof of application health.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkCheckResult {
    /// Destination frozen for this request.
    pub target: NetworkTarget,
    /// Actual UTC start time in milliseconds.
    pub collected_at_unix_ms: u64,
    /// DNS result or a literal-IP marker.
    pub dns: NetworkProbe,
    /// At most four actual TCP attempts.
    pub tcp: Vec<NetworkProbe>,
}
