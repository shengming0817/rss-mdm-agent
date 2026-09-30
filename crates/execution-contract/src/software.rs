use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Exact ecosystem text: no SemVer, case folding, alias lookup or normalization.
/// Accepts 1..=1024 UTF-8 bytes without control characters, including '+' and '~'.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct PackageValue(String);
impl PackageValue {
    /// Construct bounded opaque text. Resolution of moving aliases belongs to the source owner.
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
            return Err(ContractError::new(
                ErrorKind::InvalidValue,
                Field::Document,
                Rule::Syntax,
            ));
        }
        Ok(Self(value))
    }
    /// Exact original text; never normalized or interpreted as a version requirement.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for PackageValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PackageValue([redacted])")
    }
}
/// Exact package coordinates independent of the installed/desired version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageIdentity {
    /// Package-manager implementation/revision; no implicit manager substitution.
    pub manager: VersionedRef,
    /// Exact source configuration/revision; no public same-name fallback.
    pub source: VersionedRef,
    /// Ecosystem package identifier.
    pub package: PackageValue,
    /// Exact architecture selector, never inferred from the planning host.
    pub architecture: PackageValue,
    /// Exact variant/channel selector, never a default/latest fallback.
    pub variant: PackageValue,
}
/// Requested state; descriptive only, never an authorization decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum DesiredState {
    /// Exact selected version and immutable install payload, resolved by the source owner.
    Present {
        /// Ecosystem version, preserved verbatim.
        version: PackageValue,
        /// Exact selected package payload.
        artifact: ExactArtifactRef,
    },
    /// Absence of the exact package identity.
    Absent,
}

/// Existing software provenance; decisions never rewrite these observed facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Ownership {
    /// Pre-existing user software, protected unless explicitly permitted to modify.
    UserExisting,
    /// Organization-managed software.
    OrganizationManaged,
    /// Introduced as another package's dependency.
    DependencyIntroduced,
    /// Ownership has not been established; mutation is blocked.
    Unknown,
}
/// Whether another installed resource currently relies on this package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DependencyUse {
    /// Verified not in use as a dependency.
    Unused,
    /// Required by another installed resource.
    InUse,
    /// Dependency references have not been established.
    Unknown,
}

/// Result supplied by the ecosystem's comparator for one exact version pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum VersionRelation {
    /// Ecosystem semantics establish equivalence.
    Equal,
    /// Installed version precedes the requested version.
    Older,
    /// Installed version follows the requested version.
    Newer,
    /// Ecosystem semantics do not order these versions.
    Incomparable,
}

/// One possible mutation, not a queued workflow step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MutationKind {
    /// Install a known-absent package.
    Install,
    /// Upgrade according to the ecosystem's ordering.
    Upgrade,
    /// Explicitly permitted downgrade.
    Downgrade,
    /// Remove the exact observed package.
    Uninstall,
}
/// Preserve installer upgrade semantics; some upgrades remove the existing installation first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UpgradeStrategy {
    /// Does not require removing the old installation first.
    InPlace,
    /// Requires uninstall permission and dependency safety as well as upgrade capability.
    UninstallThenInstall,
}
/// Restart behavior of the selected installer configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RestartBehavior {
    /// Restart is not required and cannot be initiated by the selected invocation.
    Never,
    /// May request a later restart, without initiating it itself.
    MayRequire,
    /// May restart immediately; this planner cannot coordinate such execution.
    Automatic,
}
/// Dependency effects declared by the package ecosystem; no dependency solver is implemented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DependencyImpact {
    /// No dependency changes.
    None,
    /// Source/installer has declared dependency changes covered by management policy.
    Declared,
    /// Effects are unresolved; mutation is blocked.
    Unknown,
}
/// Installer semantics, not proof that the OS can execute or authorize it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallerCapabilities {
    /// Exact installer/manager binary.
    pub artifact: ExactArtifactRef,
    /// Must match PackageIdentity.manager.
    pub manager: VersionedRef,
    /// Whether this selected mechanism can independently detect the package state.
    pub can_detect: bool,
    /// Unique supported mutation kinds.
    pub operations: Vec<MutationKind>,
    /// Upgrade behavior, relevant only for Upgrade/Downgrade.
    pub upgrade_strategy: UpgradeStrategy,
    /// Selected configuration's restart semantics.
    pub restart: RestartBehavior,
    /// Declared dependency effects of the selected change mechanism.
    pub dependency_impact: DependencyImpact,
}

/// Host-supplied resource-management constraints from the bound policy revision.
/// These booleans are not actor authentication, C07 authorization or approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagementConstraints {
    /// Desired absence conflicts with this constraint even if currently absent.
    pub required: bool,
    /// Explicitly permit modifying existing user-owned software; no ownership change is inferred.
    pub allow_modify_user_owned: bool,
    /// Permit removal, including uninstall-before-upgrade.
    pub allow_remove: bool,
    /// Permit downgrade, independently of installer support.
    pub allow_downgrade: bool,
    /// Permit a separately coordinated restart if necessary.
    pub allow_restart: bool,
    /// Permit the installer's declared dependency effects.
    pub allow_dependency_changes: bool,
}

impl TryFrom<String> for PackageValue {
    type Error = ContractError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}
impl From<PackageValue> for String {
    fn from(v: PackageValue) -> Self {
        v.0
    }
}

/// The supported software adapters. No plugin names or command strings are accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SoftwareKind {
    /// Windows Installer with a fixed PowerShell entry.
    Msi,
    /// Exact Windows Package Manager source and coordinate.
    Winget,
    /// macOS installer package with a fixed shell entry.
    Pkg,
    /// Exact formula from a fixed Homebrew tap.
    Homebrew,
    /// RSS archive with a PowerShell entry.
    WindowsBundle,
    /// RSS archive with an explicit sh/Bash entry.
    MacosBundle,
}
impl SoftwareKind {
    /// OS on which this adapter can execute.
    pub fn platform(self) -> Platform {
        match self {
            Self::Msi | Self::Winget | Self::WindowsBundle => Platform::Windows,
            Self::Pkg | Self::Homebrew | Self::MacosBundle => Platform::Macos,
        }
    }
    /// Whether the payload is an RSS ZIP bundle.
    pub fn is_bundle(self) -> bool {
        matches!(self, Self::WindowsBundle | Self::MacosBundle)
    }
}
/// Extraction limits are part of the authorized software description.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleLimits {
    /// Maximum archive bytes.
    pub archive_bytes: u64,
    /// Maximum file count.
    pub files: u32,
    /// Maximum individual expanded file bytes.
    pub file_bytes: u64,
    /// Maximum total expanded bytes.
    pub expanded_bytes: u64,
    /// Maximum path component count.
    pub depth: u32,
}
/// A V4 Bundle member. Its portable path is the key in the manifest entry map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleFile {
    /// Exact uncompressed size, including zero-length members.
    pub length: u64,
    /// Exact uncompressed SHA-256 bytes, using the producer's manifest representation.
    pub sha256: [u8; 32],
}
/// Exact V4 manifest.json. Package/version/detection belong to the signed action, not a
/// second archive format. No old manifest aliases or conversion parser are retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    /// The only supported Bundle schema.
    pub schema: V1,
    /// Exact target operating system.
    pub platform: Platform,
    /// Exact common architecture selector: aarch64 or x86_64.
    pub architecture: PackageValue,
    /// Complete declared member set, excluding manifest.json.
    pub entries: std::collections::BTreeMap<String, BundleFile>,
}
/// Closed execution semantics, sharing one launch envelope and canonical plan digest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ExecutionSpec {
    /// A backend-ordered software program owned by one journal intent.
    SoftwareProgram {
        /// Complete compiled instructions; no local source catalog or enterprise grant.
        program: Box<SoftwareProgram>,
    },
    /// Existing bounded script or collection process.
    Process {},
}
impl ExecutionSpec {
    /// Ordered software instructions, when present.
    pub fn software_program(&self) -> Option<&SoftwareProgram> {
        match self {
            Self::SoftwareProgram { program } => Some(program),
            _ => None,
        }
    }
}

/// Independently observed software state; installer exit is never a value of this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SoftwareState {
    /// Declared installed target is authoritatively absent.
    Absent {},
    /// Installed bytes match one exact declared version proof.
    Present {
        /// Ecosystem version, preserved exactly.
        version: PackageValue,
    },
    /// Target is unreadable, changed concurrently, or contains unknown bytes.
    Unknown {
        /// Closed detection failure, without paths or backend text.
        reason: SoftwareDetectionFailure,
    },
}
/// Comparator identity and exact operands; a result cannot be replayed for another version pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionComparison {
    /// Exact comparator implementation/configuration revision.
    pub comparator: VersionedRef,
    /// Must equal the snapshot's installed version.
    pub installed: PackageValue,
    /// Must equal the intent's desired version.
    pub desired: PackageValue,
    /// Ecosystem result; the core performs no parsing.
    pub relation: VersionRelation,
}

/// Closed independent detection failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SoftwareDetectionFailure {
    /// Target or protected ancestor cannot be inspected.
    Unavailable,
    /// Installed bytes do not match a declared version.
    UnrecognizedVersion,
    /// The independent observation exhausted its time, cancellation or byte bound.
    BudgetExceeded,
}
/// Safe software diagnostic; contains no paths, source coordinates or captured output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SoftwareDiagnostic {
    /// No independent observation has been committed.
    AwaitingDetection,
    /// A different kernel boot has not yet been observed.
    RestartPending,
    /// Installed target cannot be inspected.
    DetectionUnavailable,
    /// Installed bytes do not match a declared version.
    UnrecognizedVersion,
    /// Detection needs another bounded observation opportunity.
    DetectionBudgetExceeded,
    /// Target matches; this is not a quiescence or final-success assertion.
    DesiredStateObserved,
    /// Independently observed target does not match the desired state.
    DesiredStateMissing,
    /// Installation is observed but its staging awaits quiescence or a cleanup retry.
    CleanupPending,
    /// Staging object ownership cannot be established; no pathname-based deletion is attempted.
    CleanupUnverified,
}
