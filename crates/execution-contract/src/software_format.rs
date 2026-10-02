//! Closed material semantics. The adapter name is derived, never independently supplied.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One native package identity, including resource packages and exact publisher DN.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MsixIdentity {
    /// Manifest Name.
    pub name: PackageValue,
    /// Manifest Publisher, preserved exactly.
    pub publisher: PackageValue,
    /// Native four-part version.
    pub version: [u16; 4],
    /// Native architecture: x86_64, aarch64 or neutral.
    pub architecture: PackageValue,
    /// Manifest ResourceId; empty denotes the application.
    pub resource_id: String,
}
/// A precisely selected bundle member, including its complete content identity.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MsixMember {
    /// Container-relative path.
    pub path: String,
    /// Complete package identity.
    pub identity: MsixIdentity,
    /// Complete uncompressed length.
    pub length: u64,
    /// Complete uncompressed SHA256.
    pub sha256: Digest,
}
/// MSIX container selection; no implicit architecture or language selection.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum MsixContainer {
    /// One application package.
    Package {},
    /// Exactly declared application and resource members.
    Bundle {
        /// Frozen applicable member set.
        members: Vec<MsixMember>,
    },
}
/// Native effects with separate observation and removal semantics.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum MsixDeployment {
    /// Registration for the exact user bound by the invocation and authenticated helper.
    TargetUserRegistration {},
    /// Provision/deprovision the future-user device set.
    DeviceProvisioning {},
}
/// Expected native signature, applied to immutable bytes and selected inner payloads.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SoftwareSignature {
    /// Authenticode certificate subject on one exact artifact.
    Authenticode {
        /// Exact artifact content identity.
        artifact: ExactArtifactRef,
        /// Approved native publisher.
        publisher: PackageValue,
    },
    /// Apple Developer ID TeamIdentifier on image and selected payload.
    AppleDeveloperId {
        /// Exact image or package artifact.
        artifact: ExactArtifactRef,
        /// Approved native publisher/team identity.
        publisher: PackageValue,
    },
    /// MSIX native package publisher and platform trust.
    Msix {
        /// Exact container content identity.
        artifact: ExactArtifactRef,
        /// Approved manifest publisher DN.
        publisher: PackageValue,
    },
}
/// Format-specific immutable material. Shared invocations and detection stay on the step.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SoftwareFormat {
    /// Windows Installer product.
    Msi {},
    /// macOS PackageInstaller package.
    Pkg {},
    /// Frozen WinGet export.
    Winget {
        /// Backend export identity.
        export_identity: Id,
    },
    /// Frozen Homebrew export.
    Homebrew {
        /// Backend export identity.
        export_identity: Id,
    },
    /// RSS archive with an explicit member inventory and bounded extraction.
    Bundle {
        /// Complete manifest.
        manifest: BundleManifest,
        /// Exact extraction limits.
        limits: BundleLimits,
    },
    /// Full offline executable, including exact sidecar layout.
    Exe {
        /// Stable native detector selector, excluding desired version and artifact bytes.
        ownership: Digest,
        /// Portable relative paths to exact immutable artifacts.
        layout: BTreeMap<String, ExactArtifactRef>,
    },
    /// Native package deployment.
    Msix {
        /// Container selection.
        container: MsixContainer,
        /// Expected application identity.
        identity: MsixIdentity,
        /// Exact prerequisite identities; never resolved from a store.
        dependencies: Vec<MsixIdentity>,
        /// Registration or provisioning effect.
        deployment: MsixDeployment,
        /// Minimum native OS version.
        minimum_os: [u16; 4],
        /// Require existing sideload permission.
        require_sideload: bool,
        /// Explicit permission for native unsigned deployment support.
        allow_unsigned: bool,
    },
    /// Read-only image with one selected application.
    DmgApp {
        /// Exact volume label.
        volume: PackageValue,
        /// Exact image-relative application path.
        path: String,
        /// Expected CFBundleIdentifier.
        bundle_id: PackageValue,
        /// Expected bundle version.
        version: PackageValue,
        /// Exact application basename in the selected Applications directory.
        target_name: String,
    },
    /// Read-only image with one selected PackageInstaller package.
    DmgPkg {
        /// Exact volume label.
        volume: PackageValue,
        /// Exact image-relative PKG path.
        path: String,
        /// Selected complete package length.
        length: u64,
        /// Selected complete package SHA256.
        sha256: Digest,
        /// Exact package receipt.
        receipt: PackageValue,
    },
}
impl SoftwareFormat {
    /// Derived adapter projection for presentation and platform admission.
    pub fn adapter(&self) -> SoftwareKind {
        match self {
            Self::Msi {} => SoftwareKind::Msi,
            Self::Pkg {} => SoftwareKind::Pkg,
            Self::Winget { .. } => SoftwareKind::Winget,
            Self::Homebrew { .. } => SoftwareKind::Homebrew,
            Self::Bundle { manifest, .. } => match manifest.platform {
                Platform::Windows => SoftwareKind::WindowsBundle,
                Platform::Macos | Platform::Linux => SoftwareKind::MacosBundle,
            },
            Self::Exe { .. } => SoftwareKind::Exe,
            Self::Msix { .. } => SoftwareKind::Msix,
            Self::DmgApp { .. } => SoftwareKind::DmgApp,
            Self::DmgPkg { .. } => SoftwareKind::DmgPkg,
        }
    }
    /// Exact archive inventory, only for RSS ZIP bundles.
    pub fn bundle(&self) -> Option<(&BundleManifest, &BundleLimits)> {
        match self {
            Self::Bundle { manifest, limits } => Some((manifest, limits)),
            _ => None,
        }
    }
}
/// Approved exit-code policy; reboot classification never authorizes an automatic reboot.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareExitCodes {
    /// Successful native exits, including explicitly approved nonzero values.
    pub success: std::collections::BTreeSet<i32>,
    /// Successful exits requiring separately authorized reboot handling.
    pub reboot: std::collections::BTreeSet<i32>,
}
/// Update strategy selected by the source, without inferred switches or removal discovery.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SoftwareUpgrade {
    /// Exact distinct update invocation.
    InPlace {
        /// Frozen update command.
        invocation: Box<SoftwareInvocation>,
    },
    /// Execute the explicit removal before the exact install.
    UninstallThenInstall {},
    /// Reject changes to an existing different version.
    Deny {},
}

pub(crate) fn relative_material_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.contains(':')
                && !part.ends_with(['.', ' '])
        })
}
impl SoftwareFormat {
    pub(crate) fn valid_for(&self, step: &SoftwareProgramStep) -> bool {
        match self {
            Self::Msi {} | Self::Pkg {} | Self::Winget { .. } | Self::Homebrew { .. } => true,
            Self::Bundle { manifest, limits } => {
                matches!(manifest.platform, Platform::Windows | Platform::Macos)
                    && manifest.architecture == step.architecture
                    && !manifest.entries.is_empty()
                    && manifest.entries.len() <= 4096
                    && manifest.entries.keys().all(|p| relative_material_path(p))
                    && limits.archive_bytes > 0
                    && limits.files > 0
                    && limits.files <= 4096
                    && limits.file_bytes > 0
                    && limits.expanded_bytes > 0
                    && limits.depth > 0
                    && limits.depth <= 32
            }
            Self::Exe { layout, .. } => {
                let keys = layout
                    .keys()
                    .map(|p| p.to_ascii_lowercase())
                    .collect::<std::collections::BTreeSet<_>>();
                !layout.is_empty()
                    && layout.len() <= 64
                    && keys.len() == layout.len()
                    && layout.keys().all(|p| relative_material_path(p))
                    && layout.values().any(|a| a == &step.payload)
            }
            Self::Msix {
                identity,
                container,
                dependencies,
                deployment,
                allow_unsigned,
                ..
            } => {
                let valid = |v: &MsixIdentity| {
                    matches!(v.architecture.as_str(), "neutral" | "aarch64" | "x86_64")
                        && (v.architecture.as_str() == "neutral"
                            || v.architecture == step.architecture)
                        && v.resource_id.len() <= 1024
                        && !v.resource_id.chars().any(char::is_control)
                };
                valid(identity)
                    && identity.resource_id.is_empty()
                    && dependencies.len() <= 64
                    && dependencies.iter().all(valid)
                    && match container {
                        MsixContainer::Package {} => true,
                        MsixContainer::Bundle { members } => {
                            !members.is_empty()
                                && members.len() <= 64
                                && members.iter().all(|m| {
                                    relative_material_path(&m.path)
                                        && m.length > 0
                                        && valid(&m.identity)
                                })
                                && members
                                    .iter()
                                    .filter(|m| m.identity.resource_id.is_empty())
                                    .count()
                                    == 1
                                && members.iter().any(|m| &m.identity == identity)
                                && members
                                    .iter()
                                    .map(|m| m.path.to_ascii_lowercase())
                                    .collect::<std::collections::BTreeSet<_>>()
                                    .len()
                                    == members.len()
                        }
                    }
                    && match deployment {
                        MsixDeployment::DeviceProvisioning {} => {
                            !allow_unsigned && matches!(step.install.run_as, RunAs::System { .. })
                        }
                        MsixDeployment::TargetUserRegistration {} => {
                            matches!(step.install.run_as, RunAs::User { .. })
                        }
                    }
            }
            Self::DmgApp {
                path,
                target_name,
                version,
                ..
            } => {
                relative_material_path(path)
                    && path.ends_with(".app")
                    && relative_material_path(target_name)
                    && !target_name.contains('/')
                    && target_name.ends_with(".app")
                    && version == &step.version
            }
            Self::DmgPkg { path, length, .. } => {
                relative_material_path(path)
                    && path.ends_with(".pkg")
                    && *length > 0
                    && matches!(step.install.run_as, RunAs::System { .. })
            }
        }
    }
}
