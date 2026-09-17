use execution_contract::{
    Authority, DeviceId, Digest, InterpreterRef, OsAccountRef, Platform, RunAs, TextEncoding,
    VersionedRef,
};

/// Availability of a specifically observed capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// Observed and currently available.
    Available,
    /// Known capability, currently blocked (for example, an inactive user session).
    Blocked,
}
/// One exact inventory fact; not an authorization proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<T> {
    /// Exact observed capability identity.
    pub capability: T,
    /// Current availability of that identity.
    pub availability: Availability,
}
/// A bounded-by-matcher inventory with an explicit observation coverage claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory<T> {
    /// True means absence is known unsupported; false means absence remains unknown.
    pub complete: bool,
    /// Unique exact entries; duplicates are invalid even when availability agrees.
    pub entries: Vec<Entry<T>>,
}
/// Required isolation mechanisms; declarations never prove their actual enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolation {
    /// Can deny all network access.
    NetworkDenied,
    /// Can enforce the plan's exact network allowlist.
    NetworkAllowlist,
    /// Can confine reads to the plan's declared paths, including denying reads for an empty list.
    ReadPaths,
    /// Can confine writes to the plan's declared paths, including denying writes for an empty list.
    WritePaths,
    /// Can prevent child processes.
    ChildProcessesDenied,
    /// Can enforce the required sandbox boundary.
    Sandbox,
}
/// Explicit runner stream support; it does not prove actual enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchIoCapability {
    /// Can resolve authorized input references and enforce their byte limit and encoding.
    ControlledStdin(TextEncoding),
    /// Can capture raw bytes and strictly decode this encoding while retaining invalid-byte evidence.
    CapturedText(TextEncoding),
}
/// Explicit snapshot facts for exactly one authority-scoped device. No system probing is performed.
#[derive(Debug, Clone)]
pub struct EnvironmentSnapshot {
    /// Exact authority/tenant namespace containing the device; IDs alone are not globally unique.
    pub authority: Authority,
    /// Device these facts describe, matched against the frozen target.
    pub device: DeviceId,
    /// Exact observation source/revision for correlation; does not authenticate the source.
    pub source: VersionedRef,
    /// Observed target platform; None means not established.
    pub platform: Option<Platform>,
    /// Interpreter artifact identities, including exact revision and content digest.
    pub interpreters: Inventory<InterpreterRef>,
    /// Stream mechanisms/encodings established by the snapshot provider.
    pub launch_io: Inventory<LaunchIoCapability>,
    /// Available target execution identities; independent of the originating login.
    pub run_as: Inventory<RunAs>,
    /// Active or currently blocked target user sessions, keyed by exact account.
    pub user_sessions: Inventory<OsAccountRef>,
    /// Available isolation mechanisms, as claimed by the trusted host's snapshot provider.
    pub isolation: Inventory<Isolation>,
}
/// Stable check ordering and diagnostic axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    /// Snapshot-to-target device binding.
    Target,
    /// Target OS namespace.
    Platform,
    /// Exact interpreter identity.
    Interpreter,
    /// Required controlled stdin encoding and delivery.
    StandardInput,
    /// Required stdout capture/decoding.
    StandardOutput,
    /// Required stderr capture/decoding.
    StandardError,
    /// Whole I/O inventory, for structural errors.
    LaunchIo,
    /// Requested execution identity.
    RunAs,
    /// Required active target user session.
    UserSession,
    /// Required network restrictions.
    Network,
    /// Read-path confinement.
    ReadPaths,
    /// Write-path confinement.
    WritePaths,
    /// Child-process denial when required.
    ChildProcesses,
    /// Additional mandatory sandbox boundary.
    Sandbox,
    /// Whole isolation inventory, for structural errors.
    Isolation,
}
/// Closed results, ordered from supported to most decisive negative result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchStatus {
    /// All required facts are observed available.
    Supported,
    /// Known capability is currently unavailable.
    Blocked,
    /// Required fact has not been established.
    Unknown,
    /// Complete observation establishes the required capability is unsupported.
    Unsupported,
}
/// One required check; omitted optional dimensions do not imply capability claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityCheck {
    /// Stable capability axis.
    pub dimension: Dimension,
    /// Explicit result for this requirement.
    pub status: MatchStatus,
}
/// Pure result bound to the exact plan and snapshot, never a permit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchReport {
    /// Exact frozen-plan digest checked by this call.
    pub plan_digest: Digest,
    /// Exact snapshot reference supplied by the caller.
    pub snapshot: VersionedRef,
    /// Unsupported > Unknown > Blocked > Supported, retaining all checks below.
    pub status: MatchStatus,
    /// All required axes in deterministic order, including target binding.
    pub checks: Vec<CapabilityCheck>,
}
/// Explicit workload limit for inventory validation and matching.
#[derive(Debug, Clone, Copy)]
pub struct MatchLimits {
    /// Maximum total entries across all inventories; must be nonzero.
    pub max_entries: usize,
}
/// Closed errors; no snapshot values or provider diagnostics are exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MatchError {
    /// Invalid host-provided workload limit.
    #[error("invalid capability limits")]
    Configuration,
    /// Snapshot exceeds the entry budget.
    #[error("capability inventory bound exceeded")]
    Limit,
    /// An inventory contains the same key more than once.
    #[error("duplicate capability in {0:?}")]
    Duplicate(Dimension),
}
