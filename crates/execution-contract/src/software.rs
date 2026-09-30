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
