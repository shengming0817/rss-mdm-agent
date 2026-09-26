use execution_contract::*;

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
    /// Snapshot authority or tenant differs from the intent.
    #[error("software authority mismatch")]
    Authority,
    /// Snapshot device/platform/scope differs from the intent.
    #[error("software target mismatch")]
    Target,
    /// Snapshot policy identity or revision differs from the intent.
    #[error("software policy mismatch")]
    Policy,
    /// Snapshot package/source/manager/architecture/variant differs from the intent.
    #[error("software package coordinate mismatch")]
    Package,
    /// Installer manager differs from the package manager.
    #[error("software installer manager mismatch")]
    Manager,
    /// Target user account and target OS namespaces differ.
    #[error("software target account platform mismatch")]
    TargetPlatform,
    /// A comparison requires both observed and desired present states.
    #[error("invalid software comparison context")]
    ComparisonContext,
    /// Comparison operands or identical-text relation contradict the bound states.
    #[error("software comparison operand mismatch")]
    ComparisonOperands,

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
    pub(crate) installer: InstallerCapabilities,
    pub(crate) management: ManagementConstraints,
    pub(crate) comparison: Option<VersionComparison>,
    pub(crate) intent: SoftwareIntent,
    pub(crate) snapshot: VersionedRef,
    pub(crate) outcome: DecisionOutcome,
}
impl SoftwareDecision {
    /// Management constraints actually used by the decision.
    pub fn management(&self) -> ManagementConstraints {
        self.management
    }
    /// Exact ecosystem comparison used by the decision.
    pub fn comparison(&self) -> Option<&VersionComparison> {
        self.comparison.as_ref()
    }

    /// Exact selected installer semantics used by the decision, not reconstructed by a runner.
    pub fn installer(&self) -> &InstallerCapabilities {
        &self.installer
    }
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
