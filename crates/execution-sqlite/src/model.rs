use execution_admission::AdmissionDecision;
use execution_approval::{ApprovalDecision, ApprovalVerifier, ProfileApproval};
use execution_contract::*;
use execution_lifecycle::{DispatchAction, ObservationVerifier};
use serde::{Deserialize, Serialize};

/// Static failure codes; never include SQL, paths, input payloads or provider text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Invalid explicit store limits.
    #[error("invalid storage configuration")]
    Configuration,
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
    /// Commit did not report success; query the same operation ID before proceeding.
    #[error("commit outcome unknown")]
    CommitUnknown,
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
        Id::new(value).map(Self).map_err(|_| Error::Configuration)
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
            || self.max_receipts < 8
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
pub trait Host: ObservationVerifier {
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
    /// Stable increasing cursor within this database.
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
    /// Aggregate/head revision at this operation.
    pub revision: u64,
    /// Bound attempt, if applicable.
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
/// Privileged audit projection, generated only from protected state and pure decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuditRecord {
    /// Existing C01 event, absent for trust-only operations before a plan exists.
    pub event: Option<AuditEvent>,
    /// Exact attempt, including admission events.
    pub attempt_id: Option<AttemptId>,
    /// Complete C07 matching rules.
    pub rule_ids: Vec<Id>,
    /// Closed Rust reason names, never submitted text.
    pub reason: String,
    /// C07 trusted authority revision, when evaluated.
    pub authorization_revision: Option<VersionedRef>,
    /// All consumed records; one row per distinct record.
    pub consumptions: Vec<ConsumptionAudit>,
}
