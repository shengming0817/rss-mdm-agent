use execution_admission::AdmissionDecision;
use execution_approval::{ApprovalDecision, ApprovalVerifier, ProfileApproval};
use execution_contract::*;
use execution_lifecycle::DispatchAction;
use serde::{Deserialize, Serialize};

/// Static failure codes; never include SQL, paths, input payloads or provider text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Invalid explicit store limits or SQLite durability configuration.
    #[error("invalid storage configuration")]
    Configuration,
    /// Invalid operation identity, query arguments or new aggregate input; correct the call.
    #[error("invalid storage operation input")]
    InvalidInput,
    /// Authentication, scope, or requested access was rejected.
    #[error("storage access denied")]
    Denied,
    /// Requested record does not exist in the authorized scope.
    #[error("record not found")]
    NotFound,
    /// Operation content, identity or conditional revision conflicts.
    #[error("storage identity or revision conflict")]
    Conflict,
    /// Reliable time is unavailable or moved behind a committed watermark.
    #[error("reliable clock unavailable")]
    Clock,
    /// Protected trust data is missing, expired or invalid.
    #[error("trusted state unavailable")]
    Trust,
    /// Another connection holds a conflicting lock.
    #[error("database busy")]
    Busy,
    /// Logical quota, bounded input, or physical database capacity was exhausted.
    #[error("storage capacity exceeded")]
    Capacity,
    /// Database or bounded stored data is malformed.
    #[error("database corrupt")]
    Corrupt,
    /// Database identity or schema cannot be opened by this writer.
    #[error("unsupported database schema")]
    Schema,
    /// Storage protection or filesystem operation failed.
    #[error("protected storage unavailable")]
    Storage,
    /// Operation commit did not report success; query or resubmit the SAME operation ID.
    /// Never use a new ID or redispatch a runner to recover this error.
    #[error("operation commit outcome unknown")]
    OperationCommitUnknown,
    /// Confirmation commit did not report success; retry the SAME scope/consumer/event.
    /// No operation receipt exists for confirmation; retry is idempotent.
    #[error("confirmation commit outcome unknown")]
    ConfirmationCommitUnknown,
    /// Migration commit failed in an isolated bootstrap file; no database was published.
    /// Retry initialize_test with the same intended path, or diagnose storage first.
    #[error("bootstrap database not published")]
    BootstrapUnpublished,
}
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode::*;
        match value {
            rusqlite::Error::SqliteFailure(e, _) => match e.code {
                DatabaseBusy | DatabaseLocked => Self::Busy,
                DiskFull | TooBig => Self::Capacity,
                ConstraintViolation => Self::Conflict,
                DatabaseCorrupt | NotADatabase => Self::Corrupt,
                _ => Self::Storage,
            },
            rusqlite::Error::QueryReturnedNoRows => Self::NotFound,
            _ => Self::Corrupt,
        }
    }
}
/// Storage operation identity, independent of plan/request/event/attempt identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationRequestId(Id);
impl OperationRequestId {
    /// Construct a bounded ID; this grants no authority.
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        Id::new(value).map(Self).map_err(|_| Error::InvalidInput)
    }
    /// Borrow the bounded identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
/// Exact storage scope. Claims must be authenticated by Host, even on reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    /// Authority including the explicit enterprise tenant, where applicable.
    pub authority: Authority,
    /// Permission-bearing actor, not the model/provider account.
    pub actor: ActorId,
    /// Immutable execution plan identity.
    pub plan_id: PlanId,
}
impl Scope {
    /// Derive claims from a frozen plan; Host still authenticates them independently.
    pub fn from_plan(plan: &FrozenPlan) -> Self {
        Self {
            authority: plan.spec().request.authority.clone(),
            actor: plan.spec().request.actor.clone(),
            plan_id: plan.spec().plan_id.clone(),
        }
    }
    /// Opaque interaction subject bound to the complete scope, not just a display ID.
    pub fn interaction_subject(&self) -> execution_interaction::Reference {
        execution_interaction::Reference::new(crate::journal::hash(self).expect("finite scope"))
            .expect("hex digest")
    }
    pub(crate) fn key(&self) -> String {
        self.interaction_subject().as_str().into()
    }
}
/// Explicit resource bounds, supplied by the product rather than inferred from a database.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Frozen plan decoder limits.
    pub plan: PlanLimits,
    /// Lifecycle snapshot limit, including terminal-state headroom.
    pub lifecycle: execution_lifecycle::Limits,
    /// Interaction bounds.
    pub interaction: execution_interaction::Limits,
    /// Maximum records/profiles in one trust update or admission.
    pub max_approvals: usize,
    /// Maximum encoded receipt/audit/trust record bytes.
    pub max_record_bytes: usize,
    /// Maximum retained receipts plus reserved terminal receipt slots.
    pub max_receipts: u64,
    /// Per-connection SQLite page ceiling; physical disk exhaustion may occur earlier.
    pub max_database_pages: u32,
    /// Maximum distinct delivery consumers per scope.
    pub max_consumers: u32,
    /// Maximum results returned by one query.
    pub max_batch: usize,
    /// Bounded SQLite lock wait; zero is a valid nonblocking configuration.
    pub busy_timeout_ms: u32,
}
impl Limits {
    pub(crate) fn validate(self) -> Result<(), Error> {
        if self.max_approvals == 0
            || self.max_approvals > 128
            || self.max_batch == 0
            || self.max_batch > 4096
            || self.max_receipts < 9
            || self.max_receipts > i64::MAX as u64
            || self.max_database_pages == 0
            || self.max_consumers == 0
            || self.max_consumers > 128
            || self.max_record_bytes < 65_536
            || self.max_record_bytes > 16 * 1024 * 1024
            || self.lifecycle.max_snapshot_bytes < execution_lifecycle::MIN_SNAPSHOT_BYTES
            || self.lifecycle.max_snapshot_bytes > self.max_record_bytes
            || self.interaction.max_snapshot_bytes == 0
            || self.interaction.max_snapshot_bytes > self.max_record_bytes
            || self.interaction.max_lifetime_ms == 0
            || self.plan.max_input_bytes > self.max_record_bytes
            || self.busy_timeout_ms > 60_000
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
}
/// Closed product access requests. Host is responsible for current authenticated grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Register a frozen plan.
    Create,
    /// Submit user/product lifecycle commands.
    Execute,
    /// Submit runner output, dispatch or evidence/recovery facts.
    RunnerFact,
    /// Create or answer an interaction, including responder and response-reference validation.
    Interact,
    /// Read a result or replay a receipt independently of old execution authorization.
    ReadResult,
    /// Read full admission/audit details.
    ReadAudit,
    /// Refresh independently verified authority/approval definitions.
    ManageTrust,
    /// Pull/ack events for this exact consumer identity.
    Deliver,
}
/// Exact purpose of request-based resolution. Delivery always carries a consumer; resolving a
/// task never implicitly grants ReadResult or skips the eventual operation's own authorization.
#[derive(Debug, Clone, Copy)]
pub enum ExecutionAccess<'a> {
    /// Submission response, including read-only replay of a previously registered task.
    Submission,
    /// Execute/cancel or retrieve this action's safe response.
    Execute,
    /// Service-owned runner reconciliation.
    RunnerFact,
    /// Interaction creation/answer; responder checks still occur at the write boundary.
    Interact,
    /// Ordinary task/result read.
    Result,
    /// Privileged audit read.
    Audit,
    /// Pull/confirm for this exact consumer.
    Delivery(&'a Id),
}
impl ExecutionAccess<'_> {
    pub(crate) fn authorize(self, scope: &Scope, host: &impl Host) -> Result<(), Error> {
        let (access, consumer) = match self {
            Self::Submission => (Access::Create, None),
            Self::Execute => (Access::Execute, None),
            Self::RunnerFact => (Access::RunnerFact, None),
            Self::Interact => (Access::Interact, None),
            Self::Result => (Access::ReadResult, None),
            Self::Audit => (Access::ReadAudit, None),
            Self::Delivery(consumer) => (Access::Deliver, Some(consumer)),
        };
        let result = crate::journal::authorize(host, access, scope, consumer);
        if matches!(self, Self::Submission) && result == Err(Error::Denied) {
            return crate::journal::authorize(host, Access::ReadResult, scope, None);
        }
        result
    }
}
/// Value-only admission result; policy/rule/approver details remain in privileged audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AdmissionStatus {
    /// This admission committed an attempt, not an execution effect.
    Admitted,
    /// Current admission/commit conditions denied an attempt.
    Denied,
    /// Policy requires independently verified approval which is not satisfied.
    ApprovalRequired,
}
/// A coherent protected task and its latest durable admission result, read in one transaction.
pub struct ExecutionRecord {
    /// Core state; restoration never creates a dispatch permission.
    pub execution: execution_lifecycle::Execution,
    /// Latest non-stale admission result, excluding all sensitive audit fields.
    pub admission: Option<AdmissionStatus>,
}
/// Verification input for access checks. Host must not trust claims because they deserialize.
pub struct AccessRequest<'a> {
    /// Requested capability.
    pub access: Access,
    /// Exact claimed scope.
    pub scope: &'a Scope,
    /// Delivery consumer, only present for Deliver.
    pub consumer: Option<&'a Id>,
    /// Submitted interaction command/reference for responder validation.
    pub interaction: Option<(
        &'a execution_interaction::Spec,
        &'a execution_interaction::Command,
    )>,
}
/// Immutable authenticated approval definition; local usage is intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalDefinition {
    /// Exact record version.
    pub reference: VersionedRef,
    /// Authenticated approving actor.
    pub approver: ActorId,
    /// Exact approved plan.
    pub plan_id: PlanId,
    /// Complete canonical plan digest.
    pub plan_digest: Digest,
    /// All profiles authorized by this record.
    pub profiles: Vec<VersionedRef>,
    /// Exclusive validity interval.
    pub validity: ValidityWindow,
    /// Maximum committed attempts, never a refundable counter.
    pub max_uses: u32,
}
/// Current trusted disposition of an immutable approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalState {
    /// Currently usable, subject to all other checks.
    Active,
    /// Explicitly revoked.
    Revoked,
    /// Current status cannot be established.
    Unknown,
}
/// Complete currently verified approval entry, not a user answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedApproval {
    /// Immutable authenticated definition.
    pub definition: ApprovalDefinition,
    /// Current status.
    pub state: ApprovalState,
}
/// Complete product-verified trust snapshot. Only Host can supply it to storage.
/// This DTO itself is not a permission and does not contain local usage counters.
#[derive(Debug, Clone)]
pub struct TrustSnapshot {
    /// Subject/delegation/policy/revocation identity used by C07.
    pub authorization_revision: VersionedRef,
    /// Approval trust/revocation identity used by C08.
    pub approval_revision: VersionedRef,
    /// Exclusive freshness bound; reliable time is supplied independently.
    pub fresh_until_unix_ms: u64,
    /// Complete current record set; omitted records become unavailable.
    pub approvals: Vec<TrustedApproval>,
}
/// Both pure decisions are retained so audit never reconstructs or loses C07 rule evidence.
pub struct AdmissionGate {
    /// Full current C07 decision for the exact plan/attempt.
    pub admission: AdmissionDecision,
    /// Full C08 decision, including the NotRequired path.
    pub approval: ApprovalDecision,
}
/// Product trust boundary. No production signer, identity adapter or runner is supplied.
/// Implementations are trusted code, not DTO decoders. The store cannot protect against a
/// malicious in-process Host or local administrator. Calls under a write transaction must be
/// bounded, non-reentrant and use already verified local facts (no network calls).
pub trait Host {
    /// Authenticate the actual caller and independently authorize the exact requested scope.
    /// Interact additionally verifies responder rights and answer/reference bindings.
    fn authorize(&self, request: AccessRequest<'_>) -> Result<(), Error>;
    /// Reliable UTC Unix milliseconds; uncertainty and rollback must return Clock.
    fn reliable_now(&self) -> Result<u64, Error>;
    /// Verify issuer rights, source authenticity, policy/revocation and return one coherent snapshot.
    fn trusted_snapshot(&self, _scope: &Scope) -> Result<TrustSnapshot, Error> {
        Err(Error::Trust)
    }
    /// Obtain fresh full decisions for a NEW attempt. The verifier exposes protected definitions
    /// and current local consumption counters in the same SQLite transaction.
    /// Replay never calls this method and never needs a fresh execution approval.
    fn admit(
        &self,
        _plan: &FrozenPlan,
        _attempt: &AttemptId,
        _bindings: &[ProfileApproval],
        _approvals: &dyn ApprovalVerifier,
        _now: u64,
    ) -> Result<AdmissionGate, Error> {
        Err(Error::Trust)
    }
}
/// Durable operation family, independent of the command's success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    /// Trust snapshot update.
    Trust,
    /// Frozen execution registration.
    OpenExecution,
    /// Execution event.
    Execution,
    /// Interaction registration.
    OpenInteraction,
    /// Answer/cancel/expiry command.
    Interaction,
}
/// A bounded authorized page of existing executions, with an exclusive continuation.
pub struct ExecutionRequestPage {
    /// Request identities belonging to the requested actor and device.
    pub requests: Vec<execution_contract::RequestId>,
    /// Last returned identity when another page exists.
    pub next: Option<execution_contract::RequestId>,
}
/// Stable business result; none of these variants is a dispatch permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Operation changed its protected aggregate.
    Changed,
    /// A previous core command already exists.
    Duplicate,
    /// An execution revision was stale.
    Stale,
    /// Another interaction command already won.
    Late,
    /// Interaction deadline has not elapsed.
    NotDue,
    /// Interaction was answered.
    Answered,
    /// Interaction was cancelled.
    Cancelled,
    /// Interaction expired; not consent.
    Expired,
    /// Required admission/approval was not satisfied.
    Rejected,
}
/// Immutable accepted API result and reliable event. Sensitive audit is queried separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    /// Stable increasing ordering key within this database; never a delivery checkpoint.
    pub sequence: u64,
    /// Stable event identity, reused on every pull/replay.
    pub event_id: EventId,
    /// Original idempotency key.
    pub operation_id: OperationRequestId,
    /// Authenticated bound scope.
    pub scope: Scope,
    /// Operation family.
    pub kind: OperationKind,
    /// Durable result, including unchanged/rejected business results.
    pub outcome: Outcome,
    /// Safe admission projection, present only for a non-stale admission decision.
    pub admission: Option<AdmissionStatus>,
    /// Aggregate/head revision at this operation.
    pub revision: u64,
    /// Exact submitted attempt for attempt-bearing commands; absent for other commands.
    pub attempt_id: Option<AttemptId>,
    /// Trusted recorded receipt time.
    pub occurred_at_unix_ms: u64,
}
/// The first-commit result. DispatchAction is neither serializable nor recoverable.
#[derive(Debug)]
pub enum CommitOutcome {
    /// This invocation committed for the first time.
    Applied {
        /// Durable receipt (also queryable after a lost response).
        receipt: Receipt,
        /// Present only for the first successful BeginAttempt commit.
        first_dispatch: Option<DispatchAction>,
    },
    /// Already committed; always contains only the original durable result.
    AlreadyCommitted(Receipt),
}
impl CommitOutcome {
    /// Borrow the durable result without obtaining execution permission.
    pub fn receipt(&self) -> &Receipt {
        match self {
            Self::Applied { receipt, .. } | Self::AlreadyCommitted(receipt) => receipt,
        }
    }
}
/// One exact approval consumption in an admission audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConsumptionAudit {
    /// Exact record version.
    pub approval: VersionedRef,
    /// Verified approving actor.
    pub approver: ActorId,
    /// Previous consumed uses.
    pub used_before: u32,
    /// Post-commit uses.
    pub used_after: u32,
    /// Previous consumption CAS.
    pub revision_before: u64,
    /// Committed consumption CAS.
    pub revision_after: u64,
    /// Trusted approval verification/revocation revision.
    pub verification_revision: VersionedRef,
}
/// Stable storage reasons; core classifications use explicit serde adapters, never Debug text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum AuditReason {
    /// Current protected trust was refreshed.
    TrustRefreshed,
    /// A frozen execution was registered.
    ExecutionOpened,
    /// Execution event processing began.
    ExecutionEvent,
    /// No protected trust head was available.
    TrustUnavailable,
    /// Both decisions were evaluated; inspect their structured projections.
    AdmissionEvaluated,
    /// Current transaction checks rejected the evaluated gate.
    CommitGateRejected,
    /// Lifecycle rejected the event.
    LifecycleError(#[serde(with = "LifecycleErrorWire")] execution_lifecycle::LifecycleError),
    /// Core next-step advice, never execution permission.
    Lifecycle(#[serde(with = "DirectiveWire")] execution_lifecycle::Directive),
    /// Interaction was registered.
    InteractionOpened,
    /// Interaction rejected the command.
    InteractionError(
        #[serde(with = "InteractionErrorWire")] execution_interaction::InteractionError,
    ),
    /// Interaction result classification.
    Interaction(#[serde(with = "InteractionOutcomeWire")] execution_interaction::Outcome),
}
/// Serializable evidence of verification time, not a reconstructible authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionValidity {
    /// Source authorization identity.
    pub revision: VersionedRef,
    /// Verification time.
    pub verified_at_unix_ms: u64,
    /// Exclusive commit deadline.
    pub valid_until_unix_ms: u64,
}
impl From<&execution_admission::AdmissionValidity> for DecisionValidity {
    fn from(v: &execution_admission::AdmissionValidity) -> Self {
        Self {
            revision: v.revision().clone(),
            verified_at_unix_ms: v.verified_at_unix_ms(),
            valid_until_unix_ms: v.valid_until_unix_ms(),
        }
    }
}
/// Complete C07 decision projection, including claimed identity on denial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdmissionAudit {
    /// Evaluated plan identity.
    pub plan_id: PlanId,
    /// Evaluated canonical plan digest.
    pub plan_digest: Digest,
    /// Evaluated attempt.
    pub attempt_id: AttemptId,
    /// Claimed policy; denial does not authenticate it.
    pub policy: VersionedRef,
    /// Claimed delegation; denial does not authenticate it.
    pub delegation: Option<VersionedRef>,
    /// Closed C07 outcome.
    #[serde(with = "AdmissionOutcomeWire")]
    pub outcome: execution_admission::DecisionOutcome,
    /// Closed C07 reason.
    #[serde(with = "AdmissionReasonWire")]
    pub reason: execution_admission::Reason,
    /// Exact matching rules.
    pub rule_ids: Vec<Id>,
    /// Verified authorization freshness, absent on denial.
    pub validity: Option<DecisionValidity>,
}
/// Profile mapping; the containing audit field distinguishes submitted and verified mappings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalBindingAudit {
    /// Required or claimed profile.
    pub profile: VersionedRef,
    /// Exact referenced approval.
    pub record: VersionedRef,
}
impl From<&ProfileApproval> for ApprovalBindingAudit {
    fn from(b: &ProfileApproval) -> Self {
        Self {
            profile: b.profile.clone(),
            record: b.record.clone(),
        }
    }
}
/// C08 candidate consumption evidence, not committed usage or permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingConsumptionAudit {
    /// Exact record.
    pub approval: VersionedRef,
    /// Counter required by the decision.
    pub expected_uses: u32,
    /// CAS required by the decision.
    pub expected_consumption_revision: u64,
    /// Source trust identity.
    pub verification_revision: VersionedRef,
    /// Exclusive decision deadline.
    pub valid_until_unix_ms: u64,
}
/// Complete C08 decision projection. It cannot be converted to ApprovalDecision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalAudit {
    /// Evaluated plan.
    pub plan_id: PlanId,
    /// Evaluated digest.
    pub plan_digest: Digest,
    /// Evaluated attempt.
    pub attempt_id: AttemptId,
    /// Closed C08 result and rejection reason.
    #[serde(with = "ApprovalOutcomeWire")]
    pub outcome: execution_approval::ApprovalOutcome,
    /// Verified mappings, empty unless C08 was satisfied.
    pub bindings: Vec<ApprovalBindingAudit>,
    /// Candidate consumptions; only AuditRecord.consumptions proves local application.
    pub pending_consumptions: Vec<PendingConsumptionAudit>,
    /// Inherited C07 freshness.
    pub admission_validity: Option<DecisionValidity>,
}
/// Protected head used for evaluation, independent of submitted claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustAudit {
    /// Current C07 revision.
    pub authorization_revision: VersionedRef,
    /// Current C08 revision.
    pub approval_revision: VersionedRef,
    /// Exclusive protected snapshot freshness.
    pub fresh_until_unix_ms: u64,
}
/// Record read from protected storage before consumption. Not proof of current applicability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtectedApprovalAudit {
    /// Authenticated immutable definition, including approver.
    pub definition: ApprovalDefinition,
    /// Protected disposition, possibly unknown or revoked.
    pub state: ApprovalState,
    /// Committed uses before this evaluation.
    pub used: u32,
    /// Consumption CAS before this evaluation.
    pub consumption_revision: u64,
}
/// Privileged audit projection, generated only from protected state and pure decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuditRecord {
    /// Stop acknowledgement/failure only; independent of termination and effect evidence.
    pub stop_outcome: Option<execution_lifecycle::StopOutcome>,
    /// Existing C01 event, absent for trust-only operations before a plan exists.
    pub event: Option<AuditEvent>,
    /// Exact submitted attempt, including rejected admission.
    pub attempt_id: Option<AttemptId>,
    /// Exact first-delivery diagnosis, independent of subsequent lifecycle rejection/staleness.
    /// Never proof of termination, no effect or permission to dispatch again.
    pub dispatch_cause: Option<execution_lifecycle::DispatchCause>,
    /// Stable closed storage/core classification.
    pub reason: AuditReason,
    /// Protected trust head at evaluation, absent if unavailable.
    pub trust: Option<TrustAudit>,
    /// Full C07 decision if evaluated.
    pub admission: Option<AdmissionAudit>,
    /// Full C08 decision if evaluated.
    pub approval: Option<ApprovalAudit>,
    /// Untrusted submitted references, retained even when verification rejects them.
    pub submitted_approvals: Vec<ApprovalBindingAudit>,
    /// Distinct records resolved from protected storage, never synthesized from submitted values.
    pub protected_approvals: Vec<ProtectedApprovalAudit>,
    /// All applied consumptions, one per distinct record.
    pub consumptions: Vec<ConsumptionAudit>,
}

// Remote serde derives keep the existing core enums authoritative and exhaustively checked.
// They serialize audit evidence only; no permission-bearing core object gains Deserialize.
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "execution_admission::DecisionOutcome",
    tag = "kind",
    rename_all = "camelCase"
)]
enum AdmissionOutcomeWire {
    Allowed,
    Denied,
    ApprovalRequired { profiles: Vec<VersionedRef> },
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "execution_admission::VerificationError",
    rename_all = "camelCase"
)]
enum AdmissionVerificationWire {
    Subject,
    Policy,
    Delegation,
    Clock,
    Revocation,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "execution_admission::Reason", rename_all = "camelCase")]
enum AdmissionReasonWire {
    StaleVerification,
    Limit,
    Verification(
        #[serde(with = "AdmissionVerificationWire")] execution_admission::VerificationError,
    ),
    SubjectMismatch,
    InvalidPolicy,
    Validity,
    Budget,
    Delegation,
    NoMatchingRule,
    ExplicitDeny,
    RuleAllowed,
    NeedsApproval,
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "execution_approval::VerificationError",
    rename_all = "camelCase"
)]
enum ApprovalVerificationWire {
    Signature,
    Issuer,
    Unavailable,
    Clock,
    Revocation,
    Policy,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "execution_approval::Reason", rename_all = "camelCase")]
enum ApprovalReasonWire {
    Limit,
    PlanMismatch,
    AdmissionDenied,
    Bindings,
    Verification(#[serde(with = "ApprovalVerificationWire")] execution_approval::VerificationError),
    Context,
    PlanNotYetValid,
    PlanExpired,
    StaleVerification,
    StaleAdmission,
    ApprovalNotYetValid,
    ApprovalExpired,
    Record,
    Revoked,
    StatusUnknown,
    Exhausted,
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "execution_approval::ApprovalOutcome",
    rename_all = "camelCase"
)]
enum ApprovalOutcomeWire {
    NotRequired,
    Satisfied,
    Rejected(#[serde(with = "ApprovalReasonWire")] execution_approval::Reason),
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "execution_lifecycle::ObservationError",
    rename_all = "camelCase"
)]
enum ObservationErrorWire {
    Unavailable,
    Untrusted,
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "execution_lifecycle::LifecycleError",
    rename_all = "camelCase"
)]
enum LifecycleErrorWire {
    Configuration,
    Snapshot,
    Limit,
    Clock,
    Revision,
    IdempotencyConflict,
    Attempt,
    Transition,
    Observation,
    ObservationVerification(
        #[serde(with = "ObservationErrorWire")] execution_lifecycle::ObservationError,
    ),
    Accounting,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "execution_lifecycle::LimitReason", rename_all = "camelCase")]
enum LimitReasonWire {
    NotYetValid,
    Expired,
    Timeout,
    Output,
    Attempts,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "execution_lifecycle::StopReason", rename_all = "camelCase")]
enum StopReasonWire {
    Cancelled,
    Limit(#[serde(with = "LimitReasonWire")] execution_lifecycle::LimitReason),
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "execution_lifecycle::Directive", rename_all = "camelCase")]
enum DirectiveWire {
    Prepare,
    Wait,
    Ready,
    RetryEligible,
    Reconcile,
    StopRunner(#[serde(with = "StopReasonWire")] execution_lifecycle::StopReason),
    VerifyTarget,
    ManualReview,
    BudgetExhausted(#[serde(with = "LimitReasonWire")] execution_lifecycle::LimitReason),
    Done,
}
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "execution_interaction::InteractionError",
    rename_all = "camelCase"
)]
enum InteractionErrorWire {
    Configuration,
    Limit,
    Reference,
    Snapshot,
    Clock,
    ResponseKind,
    IdempotencyConflict,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "execution_interaction::Outcome", rename_all = "camelCase")]
enum InteractionOutcomeWire {
    Answered,
    Cancelled,
    Expired,
    Duplicate,
    Late,
    NotDue,
}
