use execution_contract::*;
use execution_lifecycle::DispatchAction;
use serde::{Deserialize, Serialize};

/// Static failure codes; never include SQL, paths, input payloads or provider text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JournalError {
    /// Invalid explicit store limits or journal envelope.
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
    /// The journal cannot acquire exclusive write ownership.
    #[error("journal busy")]
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
}
/// Storage operation identity, independent of plan/request/event/attempt identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationRequestId(Id);
impl OperationRequestId {
    /// Construct a bounded ID; this grants no authority.
    pub fn new(value: impl Into<String>) -> Result<Self, JournalError> {
        Id::new(value)
            .map(Self)
            .map_err(|_| JournalError::InvalidInput)
    }
    /// Borrow the bounded identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
/// Exact storage scope. Claims must be authenticated by JournalHost, even on reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    /// Authority including the explicit enterprise tenant, where applicable.
    pub authority: Authority,
    /// Permission-bearing actor, not the model/provider account.
    pub actor: ActorId,
    /// Immutable execution plan identity.
    pub request_id: RequestId,
}
impl Scope {
    /// Derive claims from a frozen plan; JournalHost still authenticates them independently.
    pub fn from_input(plan: &FrozenExecution) -> Self {
        Self {
            authority: plan.spec().request.authority.clone(),
            actor: plan.spec().request.actor.clone(),
            request_id: plan.spec().request.request_id.clone(),
        }
    }
    /// Opaque interaction subject bound to the complete scope, not just a display ID.
    pub fn interaction_subject(&self) -> execution_interaction::Reference {
        execution_interaction::Reference::new(
            crate::journal_fingerprint(self).expect("finite scope"),
        )
        .expect("hex digest")
    }
}
/// Closed product access requests. JournalHost is responsible for current authenticated grants.
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
    /// Apply the exact request-purpose access rule, including submission replay access.
    pub fn authorize(self, scope: &Scope, host: &impl JournalHost) -> Result<(), JournalError> {
        let (access, consumer) = match self {
            Self::Submission => (Access::Create, None),
            Self::Execute => (Access::Execute, None),
            Self::RunnerFact => (Access::RunnerFact, None),
            Self::Interact => (Access::Interact, None),
            Self::Result => (Access::ReadResult, None),
            Self::Audit => (Access::ReadAudit, None),
            Self::Delivery(consumer) => (Access::Deliver, Some(consumer)),
        };
        let result = host.authorize(AccessRequest {
            access,
            scope,
            consumer,
            interaction: None,
        });
        if matches!(self, Self::Submission) && result == Err(JournalError::Denied) {
            return host.authorize(AccessRequest {
                access: Access::ReadResult,
                scope,
                consumer: None,
                interaction: None,
            });
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
}
/// A coherent protected task and its latest durable admission result, read in one transaction.
pub struct ExecutionRecord {
    /// Core state; restoration never creates a dispatch permission.
    pub execution: execution_lifecycle::Execution,
    /// Latest non-stale admission result, excluding all sensitive audit fields.
    pub admission: Option<AdmissionStatus>,
    /// Redacted process facts, present only for authorized result/owner reads.
    pub process: Option<execution_contract::ProcessSummary>,
    /// Authorized, value-free diagnostic from the software observation journal.
    pub software: Option<execution_contract::SoftwareDiagnostic>,
}
/// Verification input for access checks. JournalHost must not trust claims because they deserialize.
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
/// Product execution requirement, independent of enterprise approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionGate {
    /// Authorized without an additional interaction.
    Direct,
    /// Exact product confirmation is required.
    Confirmation,
    /// No execution is permitted.
    Blocked,
}
/// Map the existing closed AI risk classification to product interaction.
pub fn ai_execution_gate(risk: Option<RiskLevel>) -> ExecutionGate {
    match risk {
        Some(RiskLevel::Zero | RiskLevel::One) => ExecutionGate::Direct,
        Some(RiskLevel::Two) => ExecutionGate::Confirmation,
        Some(RiskLevel::Three) | None => ExecutionGate::Blocked,
    }
}
/// Trusted product result for this synchronous admission call; never a wire value.
pub struct AdmissionGate {
    /// Product interaction requirement.
    pub execution_gate: ExecutionGate,
    /// Exclusive deadline, rechecked after the host callback in the write transaction.
    pub valid_until_unix_ms: u64,
}
/// Product trust boundary. No production signer, identity adapter or runner is supplied.
/// Implementations are trusted code, not DTO decoders. The store cannot protect against a
/// malicious in-process JournalHost or local administrator. Calls under a write transaction must be
/// bounded, non-reentrant and use already verified local facts (no network calls).
pub trait JournalHost {
    /// Authenticate the actual caller and independently authorize the exact requested scope.
    /// Interact additionally verifies responder rights and answer/reference bindings.
    fn authorize(&self, request: AccessRequest<'_>) -> Result<(), JournalError>;
    /// Reliable UTC Unix milliseconds; uncertainty and rollback must return Clock.
    fn reliable_now(&self) -> Result<u64, JournalError>;
    /// Verify the product permit for a new attempt in this SQLite transaction.
    /// Replay does not invoke this callback or return another first-dispatch right.
    fn admit(
        &self,
        _plan: &FrozenExecution,
        _attempt: &AttemptId,
        _now: u64,
    ) -> Result<AdmissionGate, JournalError> {
        Err(JournalError::Trust)
    }
}
/// Durable operation family, independent of the command's success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
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
    /// Required product permit or confirmation was not satisfied.
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

/// One authorized read snapshot for a delivery consumer; never grants execution.
#[derive(Debug)]
pub struct DeliveryEvidence {
    /// Exact source event, not a delivery checkpoint.
    pub receipt: Receipt,
    /// Immutable local input used to bind evidence.
    pub input: std::sync::Arc<FrozenExecution>,
    /// Committed process capture for current_attempt, independent of receipt.attempt_id.
    pub process: Option<std::sync::Arc<execution_contract::ProcessEvidence>>,
    /// Ordered phase facts from the same attempt journal, independent of result delivery.
    pub software_progress: Option<std::sync::Arc<execution_contract::SoftwareProgress>>,
    /// Current journal attempt, independent of a preceding delivery receipt.
    pub current_attempt: Option<AttemptId>,
    /// Authoritative no-process terminal fact, never inferred from missing capture.
    pub terminal: Option<DeliveryTerminal>,
    /// Reliable timestamp of the current durable journal facts.
    pub observed_at_unix_ms: u64,
}

/// Safe terminal projection proved by the execution journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryTerminal {
    /// Cancellation before any attempt, or cancellation with verified NoEffect.
    Cancelled,
    /// Persisted admission denial or independently observed NeverDispatched.
    NeverDispatched,
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
/// Stable storage reasons; core classifications use explicit serde adapters, never Debug text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum AuditReason {
    /// A frozen execution was registered.
    ExecutionOpened,
    /// Execution event processing began.
    ExecutionEvent,
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
/// Privileged audit projection, generated only from protected state and pure decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuditRecord {
    /// Stop acknowledgement/failure only; independent of termination and effect evidence.
    pub stop_outcome: Option<execution_lifecycle::StopOutcome>,
    /// Existing C01 execution event, absent before a plan exists.
    pub event: Option<AuditEvent>,
    /// Exact submitted attempt, including rejected admission.
    pub attempt_id: Option<AttemptId>,
    /// Exact first-delivery diagnosis, independent of subsequent lifecycle rejection/staleness.
    /// Never proof of termination, no effect or permission to dispatch again.
    pub dispatch_cause: Option<execution_lifecycle::DispatchCause>,
    /// Stable closed storage/core classification.
    pub reason: AuditReason,
    /// Whether this product admission passed.
    pub admission: Option<bool>,
}

// Core enum serialization remains exhaustive.
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

impl AuditRecord {
    /// Set the reliable timestamp used by the final transaction gate.
    pub fn occurred_at(&mut self, now: u64) {
        if let Some(event) = &mut self.event {
            event.occurred_at_unix_ms = now;
        }
    }
}

/// One authenticated GC input; no process output or UI details are returned.
pub struct ServiceMaterialFact {
    /// Original bounded input, restored exactly once by the journal adapter.
    pub input: FrozenExecution,
    /// Current terminal state and an empty durable delivery backlog for this consumer.
    pub reclaimable: bool,
}
/// A bounded complete-journal page; foreign or corrupt rows abort instead of being filtered out.
pub struct ServiceMaterialPage {
    /// Authenticated original inputs and their minimum retention facts.
    pub items: Vec<ServiceMaterialFact>,
    /// Strict keyset continuation; absence means all remaining rows were inspected.
    pub next: Option<RequestId>,
}

/// Bounded receipt-backed updates to a material proof; checkpoints are adapter-local.
pub struct ServiceMaterialChanges<C> {
    /// Authenticated current facts for changed execution scopes.
    pub items: Vec<ServiceMaterialFact>,
    /// Continuation checkpoint; usable for deletion only once complete is true.
    pub checkpoint: C,
    /// All committed changes through this checkpoint were inspected.
    pub complete: bool,
}

/// Immutable first-record coordinate in the original journal, never an array offset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskHistoryPosition {
    /// Original request identity breaks equal-time ties.
    pub request: RequestId,
    /// Immutable preparation time or first execution receipt time.
    pub recorded_at_unix_ms: u64,
}
/// Bounded discovery over the original preparation and execution tables.
pub struct TaskHistoryQuery {
    /// Exclusive descending time/identity coordinate.
    pub after: Option<TaskHistoryPosition>,
    /// Inclusive preparation insertion watermark.
    pub preparations: u64,
    /// Inclusive execution receipt watermark.
    pub receipts: u64,
    /// Inclusive wall-clock snapshot bound.
    pub as_of: u64,
    /// Maximum raw candidates, including subsequently invisible records.
    pub limit: usize,
}

/// Internal owner-only activity cursor, authenticated before exposure to product callers.
pub struct RecentRequest {
    /// Original business request.
    pub request: execution_contract::RequestId,
    /// Exclusive durable order.
    pub sequence: u64,
    /// Trusted receipt time.
    pub recorded_at_unix_ms: u64,
}

/// A bounded raw receipt scan; the continuation advances even when no activity is visible.
pub struct RecentRequestPage {
    /// At most one latest encountered activity per request in this scan.
    pub items: Vec<RecentRequest>,
    /// Exclusive last scanned receipt, including filtered records.
    pub next: Option<u64>,
}
