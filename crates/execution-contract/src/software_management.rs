//! Local software presentation and intent. None of these deserializable references grants permission.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Exact observed installation in an authorized immutable data snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareRef {
    /// Service-owned collection definition.
    pub query: QueryRef,
    /// Snapshot digest and row position, independently revalidated by the service.
    pub cursor: DataCursor,
    /// Native collector's stable installation identity.
    pub installation: Id,
}
/// Installation mechanism; an installation record is not necessarily a GUI application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InstalledSoftwareKind {
    /// macOS application bundle.
    Application,
    /// macOS PackageInstaller receipt; native removal is unsupported.
    Pkg,
    /// Windows native program record, requiring native MSI/EXE qualification before mutation.
    WindowsProgram,
    /// Windows native package identity.
    Msix,
    /// Installed Homebrew formula or cask with separate source identity.
    Homebrew,
}
/// Product-owned local actions; arbitrary commands are never accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LocalSoftwareAction {
    /// Open the exact GUI application in the authenticated user's session.
    Launch,
    /// Install one exact permitted newer candidate.
    Update,
    /// Remove the exact selected installation.
    Uninstall,
    /// Recheck the exact installation without mutation.
    Detect,
}
/// Closed explanation of an unavailable local action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SoftwareBlock {
    /// The current product or organization rule protects this software.
    Protected,
    /// The corresponding local action is disabled by the protected rule document.
    RuleDisabled,
    /// Native identity, source or installation context cannot be established.
    IdentityUnknown,
    /// Data is missing, stale, incomplete or changed since selection.
    ObservationUnavailable,
    /// No allowed fixed update candidate is known.
    UpdateUnknown,
    /// The record is not a launchable GUI application.
    NoGui,
    /// The platform or native mechanism is unsupported.
    Unsupported,
}
/// Availability is a presentation fact; execution always rechecks current policy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareActionView {
    /// Closed requested operation.
    pub action: LocalSoftwareAction,
    /// Whether preparation can currently be attempted.
    pub available: bool,
    /// Accurate reason when unavailable.
    pub reason: Option<SoftwareBlock>,
    /// Exact qualified update target, when known.
    pub target_version: Option<String>,
}
/// One actual installation, distinct from enterprise catalog candidates.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledSoftware {
    /// Snapshot-bound reference, never a filesystem path selector.
    pub reference: SoftwareRef,
    /// Native installation family.
    pub kind: InstalledSoftwareKind,
    /// Display name from validated collection facts.
    pub name: String,
    /// Observed installed version; absence is not a latest selector.
    pub version: Option<String>,
    /// Exact native identity for display and correlation.
    pub native_id: String,
    /// Observed source family.
    pub origin: String,
    /// Effective installation owner/scope.
    pub scope: DataScope,
    /// Display publisher, never authorization.
    pub publisher: Option<String>,
    /// Current locally supported actions.
    pub actions: Vec<SoftwareActionView>,
}
/// Bounded read of collected software; it never performs a new inventory scan.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareRead {
    /// Existing data read, with independent snapshot pagination.
    pub read: DataRead,
}
/// Authorized software projection carrying the original observation quality.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwarePage {
    /// Physical inventories and account boundaries covered by this query, independent of delivery identity.
    pub coverage: Vec<Digest>,
    /// Service-owned source definition.
    pub query: QueryRef,
    /// Actual collection time, absent when unobserved.
    pub collected_at: Option<u64>,
    /// Original evidence quality.
    pub quality: DataQuality,
    /// Whether the original enumeration completed.
    pub enumeration_complete: bool,
    /// Whether the requested freshness bound was exceeded.
    pub stale: bool,
    /// User-visible installations; other users' private records are excluded.
    pub items: Vec<InstalledSoftware>,
    /// Next position in this exact observation, even when this page has no visible records.
    pub next: Option<DataCursor>,
}
/// Original local software intent. The service derives all executable inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSoftwareChoice {
    /// Stable UUID allocated before submission and reused after response loss.
    pub request: RequestId,
    /// Exact observed installation.
    pub software: SoftwareRef,
    /// Closed operation.
    pub action: LocalSoftwareAction,
}

/// Safe immutable preview of a locally prepared operation; executable input stays in the journal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSoftwarePrepared {
    /// Original request UUID.
    pub request: RequestId,
    /// Digest of the complete service-frozen execution.
    pub digest: Digest,
    /// Observed installation and requested action.
    pub choice: LocalSoftwareChoice,
    /// Safe software name.
    pub name: String,
    /// Original authenticated source, preserved across human confirmation.
    pub initiator: Initiator,
    /// Effective physical account class.
    pub identity: BackendIdentity,
    /// Fixed candidate version for updates; never a moving latest selector.
    pub target_version: Option<String>,
    /// Latest permitted first start in UTC milliseconds.
    pub start_before_unix_ms: u64,
}
/// Submit or query the exact service-frozen preparation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSoftwareSubmission {
    /// Original request UUID.
    pub request: RequestId,
    /// Exact preview digest; changing the selection requires a new explicit preparation.
    pub digest: Digest,
}

/// Bounded history of local software intents in the existing execution journal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareHistoryQuery {
    /// Exclusive journal continuation.
    pub after: Option<RequestId>,
    /// One through 128 records.
    pub limit: u16,
}

/// Local software requests share one native context and the existing journal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "input",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum LocalSoftwareOperation {
    /// Recover authorized preparations and original results without creating an attempt.
    History(SoftwareHistoryQuery),
    /// Retain an immutable preview without admission or dispatch.
    Prepare(LocalSoftwareChoice),
    /// Explicit human submission/confirmation of the original preview.
    Submit(LocalSoftwareSubmission),
    /// Read the original operation; never creates another attempt.
    Status(QueryRequest),
    /// Request cancellation, independently of physical termination or rollback.
    Cancel(QueryRequest),
}

pub(crate) fn valid_software_metadata(input: &ExecutionInput) -> bool {
    input.request.parameters.is_empty()
}
