use execution_contract::{Authority, EvidenceRef, ExactArtifactRef, Target, VersionedRef};

/// Exact ecosystem text: no SemVer, case folding, alias lookup or normalization.
/// Accepts 1..=1024 UTF-8 bytes without control characters, including '+' and '~'.
#[derive(Clone, PartialEq, Eq)]
pub struct PackageValue(String);
impl PackageValue {
    /// Construct bounded opaque text. Resolution of moving aliases belongs to the source owner.
    pub fn new(value: impl Into<String>) -> Result<Self, DecisionError> {
        let value = value.into();
        if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
            return Err(DecisionError::Value);
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
/// Incoming desired state and all namespaces needed to correlate the host snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareIntent {
    /// Product/local/test namespace, including tenant where applicable.
    pub authority: Authority,
    /// Explicit device/platform/user scope.
    pub target: Target,
    /// Exact management-policy revision expected by this request.
    pub policy: VersionedRef,
    /// Exact package coordinates.
    pub package: PackageIdentity,
    /// Explicit target state.
    pub desired: DesiredState,
}
/// Existing software provenance; decisions never rewrite these observed facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyUse {
    /// Verified not in use as a dependency.
    Unused,
    /// Required by another installed resource.
    InUse,
    /// Dependency references have not been established.
    Unknown,
}
/// Reason to obtain new evidence rather than retry an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionCause {
    /// No suitable detection fact exists.
    NotObserved,
    /// A previous change completed; independent verification remains mandatory.
    AfterMutation,
    /// Cancellation or dispatch/install outcome is uncertain.
    UnknownEffect,
    /// A completed restart must be followed by detection.
    AfterRestart,
}
/// Host-provided detection fact for PlanningSnapshot.package and target.
/// Process exit alone cannot be used as present/absent state evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// No usable state observation; obtain new detection evidence.
    Needed(DetectionCause),
    /// Independent observation established absence.
    Absent {
        /// State/test evidence verified by the host.
        evidence: EvidenceRef,
    },
    /// Independent observation established this exact installed version.
    Present {
        /// Original ecosystem version.
        version: PackageValue,
        /// Observed ownership; never silently adopted.
        ownership: Ownership,
        /// References from other installed packages.
        dependencies: DependencyUse,
        /// State/test evidence verified by the host.
        evidence: EvidenceRef,
    },
    /// A completed detection remains ambiguous; repeating blindly is not progress.
    Indeterminate,
}
/// Result supplied by the ecosystem's comparator for one exact version pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
/// Comparator identity and exact operands; a result cannot be replayed for another version pair.
#[derive(Debug, Clone, PartialEq, Eq)]
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
/// One possible mutation, not a queued workflow step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeStrategy {
    /// Does not require removing the old installation first.
    InPlace,
    /// Requires uninstall permission and dependency safety as well as upgrade capability.
    UninstallThenInstall,
}
/// Restart behavior of the selected installer configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartBehavior {
    /// Restart is not required and cannot be initiated by the selected invocation.
    Never,
    /// May request a later restart, without initiating it itself.
    MayRequire,
    /// May restart immediately; this planner cannot coordinate such execution.
    Automatic,
}
/// Dependency effects declared by the package ecosystem; no dependency solver is implemented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyImpact {
    /// No dependency changes.
    None,
    /// Source/installer has declared dependency changes covered by management policy.
    Declared,
    /// Effects are unresolved; mutation is blocked.
    Unknown,
}
/// Installer semantics, not proof that the OS can execute or authorize it.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
/// Temporary prerequisites; real locking and scheduling remain external.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitReason {
    /// Another writer owns the exact target resource.
    ResourceBusy,
    /// The package manager is occupied.
    PackageManagerBusy,
    /// Outside the approved maintenance window.
    MaintenanceWindow,
    /// An application must be closed first.
    ApplicationBusy,
}
/// Current readiness facts. Ready never means that a future mutation has acquired locks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// No known temporary blocker.
    Ready,
    /// One explicit temporary blocker.
    Waiting(WaitReason),
    /// Host established a restart requirement; after restart, detect again.
    PendingRestart {
        /// Verified provenance of the restart requirement.
        evidence: EvidenceRef,
    },
}
/// Coherent host snapshot. Authentication/freshness belong to the host; this core checks binding.
/// No Deserialize/authority constructor or production identity adapter is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningSnapshot {
    /// Immutable snapshot identity/revision, retained in the decision.
    pub revision: VersionedRef,
    /// Must match the intent authority/tenant exactly.
    pub authority: Authority,
    /// Must match device/platform/user scope exactly.
    pub target: Target,
    /// Exact resource-management policy revision.
    pub policy: VersionedRef,
    /// Exact identity these facts describe.
    pub package: PackageIdentity,
    /// Independent installed-state facts.
    pub detection: Detection,
    /// Required for different present versions; None does not imply an ordering.
    pub comparison: Option<VersionComparison>,
    /// Selected installer semantics.
    pub installer: InstallerCapabilities,
    /// Management constraints; not an actor-level execution grant.
    pub management: ManagementConstraints,
    /// Current temporary prerequisites.
    pub readiness: Readiness,
}
/// Independent work bounds; all fields must be nonzero.
#[derive(Debug, Clone, Copy)]
pub struct PlanningLimits {
    /// Maximum UTF-8 bytes in each opaque package/version/architecture/variant field.
    pub max_text_bytes: usize,
    /// Maximum entries in the supported-operation list.
    pub max_capabilities: usize,
}
/// Static structural/binding diagnostics. Business prohibitions use Blocked instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecisionError {
    /// Invalid host workload bounds.
    #[error("invalid software planning limits")]
    Limits,
    /// Invalid opaque package text.
    #[error("invalid software coordinate")]
    Value,
    /// Workload exceeds the independent bounds.
    #[error("software planning bound exceeded")]
    Bound,
    /// Cross-target/source/policy/manager or operand inconsistency.
    #[error("inconsistent software planning snapshot")]
    Binding,
    /// Supported-operation list repeats an entry.
    #[error("duplicate installer capability")]
    Duplicate,
    /// A process-exit/test reference is being promoted into another evidence category.
    #[error("invalid software observation evidence")]
    Evidence,
}
/// Closed permanent/unknown blockers; no input values are embedded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// Required software cannot have desired absence.
    Required,
    /// No independent detection mechanism.
    DetectionUnavailable,
    /// Detection completed but is still ambiguous.
    Indeterminate,
    /// Ordering has not been established for the exact operands.
    VersionUnknown,
    /// The ecosystem cannot compare the versions.
    VersionIncomparable,
    /// Installer cannot perform the selected operation.
    UnsupportedOperation,
    /// Ownership is unknown.
    OwnershipUnknown,
    /// User-owned software is protected.
    UserOwned,
    /// Removal is prohibited.
    Removal,
    /// Other packages depend on this software, or references are unknown.
    DependencyUse,
    /// Downgrade is prohibited.
    Downgrade,
    /// Restart cannot meet management restrictions or would happen automatically.
    Restart,
    /// Dependency effects are unknown or not permitted.
    DependencyImpact,
}
/// One descriptive change. All fields are private; it cannot be executed or deserialized.
/// Real execution must acquire target-resource and manager locks, revalidate facts, and authorize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mutation {
    pub(crate) kind: MutationKind,
    pub(crate) upgrade_strategy: Option<UpgradeStrategy>,
    pub(crate) installer: ExactArtifactRef,
    pub(crate) installed_version: Option<PackageValue>,
    pub(crate) post_detection: DesiredState,
}
impl Mutation {
    /// Selected change; never an execution permit.
    pub fn kind(&self) -> MutationKind {
        self.kind
    }
    /// Selected upgrade semantics; present only for Upgrade/Downgrade.
    pub fn upgrade_strategy(&self) -> Option<UpgradeStrategy> {
        self.upgrade_strategy
    }
    /// Exact installer binary selected by the snapshot.
    pub fn installer(&self) -> &ExactArtifactRef {
        &self.installer
    }
    /// Exact observed version before the change; None only for install.
    pub fn installed_version(&self) -> Option<&PackageValue> {
        self.installed_version.as_ref()
    }
    /// Mandatory desired-state check after the change; contains the exact selected payload for Present.
    pub fn post_detection(&self) -> &DesiredState {
        &self.post_detection
    }
}
/// A single next result; no parallel status field, step queue or automatic state advancement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionOutcome {
    /// Host-provided observation meets the desire; no new state/ownership evidence is minted.
    Satisfied {
        /// Existing state/test observation.
        evidence: EvidenceRef,
        /// Existing ownership, or None for absence.
        ownership: Option<Ownership>,
    },
    /// Obtain independent state evidence, without mutating the package.
    Detect(DetectionCause),
    /// Temporary blocker; obtain a new snapshot before replanning.
    Wait(WaitReason),
    /// A separately authorized restart is required; it is not executed by this library.
    RequireRestart {
        /// Existing evidence of the requirement.
        evidence: EvidenceRef,
    },
    /// Exactly one bounded mutation with mandatory post-detection.
    Mutate(Mutation),
    /// A management restriction or unresolved fact prevents proceeding.
    Blocked(BlockReason),
}
/// Immutable, context-bound planning result, not a permit or canonical execution plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareDecision {
    pub(crate) intent: SoftwareIntent,
    pub(crate) snapshot: VersionedRef,
    pub(crate) outcome: DecisionOutcome,
}
impl SoftwareDecision {
    /// Exact original intent; scope is never inferred from the planning host.
    pub fn intent(&self) -> &SoftwareIntent {
        &self.intent
    }
    /// Snapshot revision that produced this result; a later change requires replanning.
    pub fn snapshot(&self) -> &VersionedRef {
        &self.snapshot
    }
    /// The sole decision result.
    pub fn outcome(&self) -> &DecisionOutcome {
        &self.outcome
    }
}
