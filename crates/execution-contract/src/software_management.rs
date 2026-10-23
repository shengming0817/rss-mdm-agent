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
    /// macOS PackageInstaller receipt; removal requires a trusted recipe.
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
    /// Native identity, source or installation context cannot be established.
    IdentityUnknown,
    /// Data is missing, stale, incomplete or changed since selection.
    ObservationUnavailable,
    /// No fixed trusted installation/removal recipe is registered.
    RecipeUnavailable,
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
