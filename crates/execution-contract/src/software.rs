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
pub enum UpgradeStrategy {
    /// Does not require removing the old installation first.
    InPlace,
    /// Requires uninstall permission and dependency safety as well as upgrade capability.
    UninstallThenInstall,
}
/// Restart behavior of the selected installer configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
/// Independent file state for a declared ecosystem version; not an installer receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareVersionProof {
    /// Original ecosystem version.
    pub version: PackageValue,
    /// Expected bytes of the independently installed file.
    pub sha256: Digest,
}
/// Bounded independent detection. Unknown bytes are indeterminate, never absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareDetection {
    /// Absolute installed file location; checked against the explicit target platform.
    pub path: String,
    /// Exact allowed version proofs, with unique versions and hashes.
    pub versions: Vec<SoftwareVersionProof>,
    /// Maximum bytes read for one detection.
    pub max_bytes: u64,
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
/// A declared file in the bundle. Directories are implicit; links are never supported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleFile {
    /// Portable slash-separated relative path.
    pub path: String,
    /// Exact uncompressed bytes.
    pub sha256: Digest,
}
/// Exact bundle manifest, embedded once inside the archive as manifest.json.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleManifest {
    /// Exact package coordinate.
    pub package: PackageIdentity,
    /// Exact package version.
    pub version: PackageValue,
    /// Payload files, excluding this manifest.
    pub files: Vec<BundleFile>,
    /// Fixed install.ps1 or install.sh entry.
    pub install: String,
    /// Explicit relative uninstall entry, if removal is supported.
    pub uninstall: Option<String>,
    /// Independent installed-state detection declaration.
    pub detection: SoftwareDetection,
}
/// Typed software execution facts. These are declarations, never approval or identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareSpec {
    /// Closed adapter selection.
    pub adapter: SoftwareKind,
    /// Exact source, manager, package, architecture and variant.
    pub package: PackageIdentity,
    /// Explicit target state.
    pub desired: DesiredState,
    /// One admitted mutation; detection and replanning must agree before launch.
    pub mutation: MutationKind,
    /// Original planning snapshot reference.
    pub snapshot: VersionedRef,
    /// Immutable package/archive/formula/manifest, also retained for declared uninstall.
    pub payload: ExactArtifactRef,
    /// Fixed native package-manager binary, separate from the script interpreter.
    pub manager: ExactArtifactRef,
    /// Fixed backend source selector; not a URL or a fallback source.
    pub source: PackageValue,
    /// Exact installed resource identity (e.g. MSI product code or fully qualified formula).
    pub resource: PackageValue,
    /// Absolute installed-state detector.
    pub detection: SoftwareDetection,
    /// Removal entry, bound by exact content digest; None means unsupported.
    pub uninstall: Option<ExactArtifactRef>,
    /// Resource-management constraints, not execution approval.
    pub management: ManagementConstraints,
    /// Declared dependency effects; unresolved effects block execution.
    pub dependencies: DependencyImpact,
    /// Exact ecosystem comparison from the planning snapshot; operands rechecked under lock.
    pub comparison: Option<VersionComparison>,
    /// Exact bundle extraction bounds, required only for bundle adapters.
    pub bundle: Option<BundleLimits>,
}
/// Closed execution semantics, sharing one launch envelope and canonical plan digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ExecutionSpec {
    /// Existing bounded script or collection process.
    Process {},
    /// A software mutation with mandatory independent detection.
    Software {
        /// Software semantics cannot be lowered to generic process parameters.
        software: Box<SoftwareSpec>,
    },
}
impl ExecutionSpec {
    /// Software descriptor, if this is a software plan.
    pub fn software(&self) -> Option<&SoftwareSpec> {
        match self {
            Self::Process {} => None,
            Self::Software { software } => Some(software),
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
    Unknown {},
}
/// Software observations bound to the existing attempt, stored in the same journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareEvidence {
    /// Existing attempt identity.
    pub attempt_id: AttemptId,
    /// Complete canonical plan digest.
    pub plan_digest: Digest,
    /// Runner identity.
    pub runner: Id,
    /// Detection before mutation, absent when history was lost.
    pub before: Option<SoftwareState>,
    /// Latest independent detection, not a terminal assertion.
    pub detected: SoftwareState,
    /// Installer explicitly requested a restart, not an instruction to reboot.
    pub restart_required: bool,
}
impl SoftwareSpec {
    /// OS-wide manager/resource keys, independent of tenant, source and actor.
    /// A single device journal must be shared by system/user product helpers.
    pub fn lock_keys(&self) -> [String; 2] {
        use sha2::{Digest as _, Sha256};
        let family = match self.adapter {
            SoftwareKind::Msi | SoftwareKind::Winget => "windows-installers",
            SoftwareKind::Pkg => "macos-installer",
            SoftwareKind::Homebrew => "homebrew",
            SoftwareKind::WindowsBundle | SoftwareKind::MacosBundle => "rss-bundle",
        };
        let path = if self.adapter.platform() == Platform::Windows {
            self.detection.path.replace('\\', "/").to_ascii_lowercase()
        } else {
            self.detection.path.clone()
        };
        [
            format!("manager-{family}"),
            format!("resource-{:x}", Sha256::digest(path.as_bytes())),
        ]
    }
    /// Whether independent state satisfies this exact desired state.
    pub fn satisfied(&self, state: &SoftwareState) -> bool {
        match (&self.desired, state) {
            (DesiredState::Absent, SoftwareState::Absent {}) => true,
            (
                DesiredState::Present {
                    version: desired, ..
                },
                SoftwareState::Present { version },
            ) => version == desired,
            _ => false,
        }
    }
}

/// Comparator identity and exact operands; a result cannot be replayed for another version pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

/// Protected provenance supplied by the journal, not a deserializable authorization claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareProvenance {
    /// Owner established by a committed, independently verified installation.
    pub ownership: Ownership,
    /// Last verified installed state; mismatches invalidate automatic ownership reuse.
    pub state: Option<SoftwareState>,
}
