//! Ordered software instructions compiled once from the backend definition.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One backend software intent, independent of individual process exit codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SoftwareOperation {
    /// Install each exact version in the supplied order.
    Install,
    /// Independently observe each declared package.
    Detect,
    /// Execute each explicit removal in the supplied order.
    Uninstall,
}
/// Source-authorized handling of existing installations; this is not local approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ExistingSoftware {
    /// Existing installations require provenance in this device journal.
    ManagedOnly,
    /// The backend explicitly permits modifying an existing user installation.
    AllowUserExisting,
}
/// Exact invocation of a fixed native profile or a declared script.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareInvocation {
    /// Complete immutable launch inputs, with literal arguments and pinned artifacts.
    pub launch: LaunchSpec,
    /// Actual execution account selected from the observed OS context.
    pub run_as: RunAs,
    /// Exact login when a user context is required.
    pub session_requirement: SessionRequirement,
    /// Shared timeout budget across all uses of this invocation in the attempt.
    pub timeout_ms: u64,
    /// Shared diagnostic byte budget; observations and recovery do not replenish it.
    pub output_bytes: u64,
    /// Exact successful and reboot-required exits authorized by the source.
    pub exit_codes: SoftwareExitCodes,
}
/// Independent V4 detection rules; no installer exit is a detection result.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SoftwareDetector {
    /// Query the exact product in the invocation's Windows installation context.
    MsiProduct {
        /// Canonical braced product GUID.
        product_code: String,
        /// Expected exact version.
        version: PackageValue,
    },
    /// Query the exact macOS package receipt.
    PkgReceipt {
        /// Literal receipt identifier.
        receipt: String,
        /// Expected exact version.
        version: PackageValue,
    },
    /// Execute the backend's declared detector under its own cumulative bounds.
    Script {
        /// Exact detector invocation.
        invocation: Box<SoftwareInvocation>,
    },
}
/// One executable step; dependency selection and ordering have already happened on the server.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareProgramStep {
    /// Closed native material semantics; adapter identity is derived.
    pub format: SoftwareFormat,
    /// Exact package coordinate, without a local source catalog.
    pub package: PackageValue,
    /// Exact version, also required for uninstall; there is no latest selector.
    pub version: PackageValue,
    /// Common architecture selector from the device-bound backend task.
    pub architecture: PackageValue,
    /// Exact primary package, formula, manifest or Bundle bytes.
    pub payload: ExactArtifactRef,
    /// Native signature requirements on exact declared artifacts.
    pub signatures: Vec<SoftwareSignature>,
    /// Frozen strategy for an existing different version.
    pub upgrade: SoftwareUpgrade,
    /// Frozen install/update invocation.
    pub install: SoftwareInvocation,
    /// Explicit frozen removal; absence means unsupported.
    pub uninstall: Option<SoftwareInvocation>,
    /// Independent detector used under one shared observation budget.
    pub detection: SoftwareDetector,
    /// Permission to change existing user installations, supplied by the backend.
    pub existing: ExistingSoftware,
    /// Permission for an exact downgrade; unknown comparisons do not imply permission.
    pub allow_downgrade: bool,
    /// Whether a required reboot may be reported for separately authorized handling.
    pub allow_reboot: bool,
}
impl SoftwareProgramStep {
    /// Installer completion only; independent detection and quiescence still gate the step.
    /// MSI reboot codes never authorize this client to initiate a reboot.
    pub fn mutation_succeeded(&self, facts: &ProcessEvidence) -> bool {
        facts.end == ProcessEnd::Exited
            && facts.failure_kind == ProcessFailureKind::None
            && facts.exit_code.is_some_and(|code| {
                self.install.exit_codes.success.contains(&code)
                    || (self.allow_reboot && self.install.exit_codes.reboot.contains(&code))
            })
    }
}
/// A single backend attempt and one execution intent, containing every ordered step.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareProgram {
    /// Backend digest of the complete V4 step definition.
    pub definition_digest: Digest,
    /// Fixed operation selected by the backend.
    pub intent: SoftwareOperation,
    /// Original ordered prerequisites followed by the requested package.
    pub steps: Vec<SoftwareProgramStep>,
}
impl SoftwareProgram {
    /// Select exact immutable invocation inputs from the original program.
    pub fn invocation(&self, step: usize, phase: SoftwarePhase) -> Option<&SoftwareInvocation> {
        let step = self.steps.get(step)?;
        match phase {
            SoftwarePhase::Mutation => match self.intent {
                SoftwareOperation::Install => Some(&step.install),
                SoftwareOperation::Uninstall => step.uninstall.as_ref(),
                SoftwareOperation::Detect => None,
            },
            SoftwarePhase::Before | SoftwarePhase::After => match &step.detection {
                SoftwareDetector::Script { invocation } => Some(invocation),
                _ => None,
            },
        }
    }
    /// Agent serialization keys, deliberately independent of tenant and package-name aliases.
    /// These claims never promise exclusion of unrelated OS writers.
    pub fn lock_keys(&self) -> Vec<String> {
        let mut keys = std::collections::BTreeSet::new();
        for step in &self.steps {
            keys.insert(format!(
                "manager-{}",
                match step.format.adapter() {
                    SoftwareKind::Msi | SoftwareKind::Exe | SoftwareKind::Winget =>
                        "windows-installers",
                    SoftwareKind::Pkg | SoftwareKind::DmgPkg => "macos-installer",
                    SoftwareKind::Homebrew => "homebrew",
                    SoftwareKind::Msix => "windows-package-deployment",
                    SoftwareKind::DmgApp => "macos-applications",
                    SoftwareKind::WindowsBundle | SoftwareKind::MacosBundle => "rss-bundle",
                }
            ));
        }
        keys.into_iter().collect()
    }
}

/// Same-journal provenance, never a caller-supplied ownership or approval declaration.
#[derive(Clone)]
pub struct SoftwareOwnership {
    /// Step whose logical package/context matches the previously completed installation.
    pub step: u32,
    /// Last independently verified state; any different observation invalidates this provenance.
    pub state: SoftwareState,
}
impl SoftwareProgramStep {
    /// Stable native resource identity across version updates, independent of display aliases.
    pub fn ownership_key(&self, operation: SoftwareOperation) -> String {
        use sha2::{Digest as _, Sha256};
        let detector = match &self.detection {
            SoftwareDetector::MsiProduct { product_code, .. } => product_code.clone(),
            SoftwareDetector::PkgReceipt { receipt, .. } => receipt.clone(),
            SoftwareDetector::Script { invocation } => format!(
                "{:x}",
                Sha256::digest(
                    serde_json_canonicalizer::to_vec(&(&invocation.launch, &invocation.run_as))
                        .expect("closed detector")
                )
            ),
        };
        let context = if operation == SoftwareOperation::Uninstall {
            self.uninstall.as_ref().unwrap_or(&self.install)
        } else {
            &self.install
        };
        let bytes = serde_json_canonicalizer::to_vec(&(
            self.format.adapter(),
            &self.package,
            &self.architecture,
            &context.run_as,
            detector,
        ))
        .expect("closed package identity");
        format!("{:x}", Sha256::digest(bytes))
    }
}

impl std::fmt::Debug for SoftwareProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftwareProgram")
            .field("intent", &self.intent)
            .field("steps", &self.steps.len())
            .finish_non_exhaustive()
    }
}
