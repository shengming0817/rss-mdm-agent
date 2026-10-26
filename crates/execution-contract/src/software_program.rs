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
        /// Original native observation account.
        run_as: RunAs,
        /// Original OS session required by the observation.
        session_requirement: SessionRequirement,
    },
    /// Query the exact macOS package receipt.
    PkgReceipt {
        /// Literal receipt identifier.
        receipt: String,
        /// Expected exact version.
        version: PackageValue,
        /// Original native observation account.
        run_as: RunAs,
        /// Original OS session required by the observation.
        session_requirement: SessionRequirement,
    },
    /// Execute the backend's declared detector under its own cumulative bounds.
    Script {
        /// Exact detector invocation.
        invocation: Box<SoftwareInvocation>,
    },
}
impl SoftwareDetector {
    /// Exact observation identity; no installation recipe is required for native reads.
    pub fn context(&self) -> (&RunAs, &SessionRequirement) {
        match self {
            Self::MsiProduct {
                run_as,
                session_requirement,
                ..
            }
            | Self::PkgReceipt {
                run_as,
                session_requirement,
                ..
            } => (run_as, session_requirement),
            Self::Script { invocation } => (&invocation.run_as, &invocation.session_requirement),
        }
    }
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
    /// Complete retained material coordinates; recovery never guesses alternate paths.
    pub materials: Vec<SoftwareMaterial>,
    /// Native signature requirements on exact declared artifacts.
    pub signatures: Vec<SoftwareSignature>,
    /// Frozen strategy for an existing different version.
    pub upgrade: SoftwareUpgrade,
    /// Only reachable physical operations, keyed by their existing durable phase.
    pub invocations: std::collections::BTreeMap<SoftwarePhase, SoftwareInvocation>,
    /// Independent detector used under one shared observation budget.
    pub detection: SoftwareDetector,
    /// Permission for an exact downgrade; unknown comparisons do not imply permission.
    pub allow_downgrade: bool,
    /// Whether a required reboot may be reported for separately authorized handling.
    pub allow_reboot: bool,
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
        if phase.is_observation() {
            match &step.detection {
                SoftwareDetector::Script { invocation } => Some(invocation),
                _ => None,
            }
        } else {
            step.invocations.get(&phase)
        }
    }

    /// Desired effect, evaluated independently of installer process exits.
    pub fn satisfied(&self, step: &SoftwareProgramStep, state: &SoftwareState) -> bool {
        match self.intent {
            SoftwareOperation::Install => {
                matches!(state, SoftwareState::Present { version } if version == &step.version)
            }
            SoftwareOperation::Uninstall => matches!(state, SoftwareState::Absent {}),
            SoftwareOperation::Detect => !matches!(state, SoftwareState::Unknown { .. }),
        }
    }
    /// Ordered physical operations derived from the frozen format, intent and before-state.
    pub fn mutation_phases(
        &self,
        step: &SoftwareProgramStep,
        before: &SoftwareState,
    ) -> Vec<SoftwarePhase> {
        if self.intent == SoftwareOperation::Detect
            || matches!(before, SoftwareState::Unknown { .. })
        {
            return Vec::new();
        }
        let updating = self.intent == SoftwareOperation::Install
            && matches!(before, SoftwareState::Present { .. });
        if updating && matches!(step.upgrade, SoftwareUpgrade::Deny {}) {
            return Vec::new();
        }
        let mut phases = Vec::new();
        if step.invocations.contains_key(&SoftwarePhase::Prepare) {
            phases.push(SoftwarePhase::Prepare);
        }
        if step.invocations.contains_key(&SoftwarePhase::Attach)
            && self.intent == SoftwareOperation::Install
        {
            phases.push(SoftwarePhase::Attach);
            if step.invocations.contains_key(&SoftwarePhase::Stage) {
                phases.push(SoftwarePhase::Stage);
            }
        }
        if updating && matches!(step.upgrade, SoftwareUpgrade::UninstallThenInstall {}) {
            phases.push(SoftwarePhase::Removal);
            phases.push(SoftwarePhase::RemovalAfter);
        }
        phases.push(
            if updating && matches!(step.upgrade, SoftwareUpgrade::InPlace { .. }) {
                SoftwarePhase::Upgrade
            } else {
                SoftwarePhase::Mutation
            },
        );
        if step.invocations.contains_key(&SoftwarePhase::Cleanup) {
            phases.push(SoftwarePhase::Cleanup);
        }
        phases
    }
    /// The original phase may acquire resources that only its frozen Cleanup can close.
    pub(crate) fn acquires_resources(&self, step: u32, phase: SoftwarePhase) -> bool {
        matches!(phase, SoftwarePhase::Attach | SoftwarePhase::Prepare)
            || (phase == SoftwarePhase::Mutation
                && self.intent == SoftwareOperation::Uninstall
                && self.steps.get(step as usize).is_some_and(|s| {
                    matches!(s.format, SoftwareFormat::DmgApp { .. })
                        && s.invocations.contains_key(&SoftwarePhase::Cleanup)
                }))
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

impl std::fmt::Debug for SoftwareProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftwareProgram")
            .field("intent", &self.intent)
            .field("steps", &self.steps.len())
            .finish_non_exhaustive()
    }
}

impl SoftwareInvocation {
    /// Match approved exits without inferring installation, quiescence or reboot authority.
    pub fn succeeded(&self, facts: &ProcessEvidence, allow_reboot: bool) -> bool {
        facts.end == ProcessEnd::Exited
            && facts.failure_kind == ProcessFailureKind::None
            && facts.exit_code.is_some_and(|code| {
                self.exit_codes.success.contains(&code)
                    || (allow_reboot && self.exit_codes.reboot.contains(&code))
            })
    }
}

/// One immutable material retained by the original program, including sidecars and native images.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareMaterial {
    /// Absolute protected path selected by the trusted compiler.
    pub path: String,
    /// Exact declared bytes.
    pub artifact: ExactArtifactRef,
}

/// Facts from fixed operations; these describe native completion, never inferred installation.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoftwareWorkerResult {
    /// Bytes observed by native tools, including output consumed for independent checks.
    pub native_output_bytes: u64,
    /// Independent observation for a detector only.
    pub detected: Option<SoftwareState>,
    /// Native operation completion is known; a process group alone cannot prove this on macOS.
    pub closed: bool,
    /// Captured native tool diagnostics within the original budget.
    pub diagnostics: String,
}
