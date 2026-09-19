use execution_contract::{AttemptId, Digest, EventId, EvidenceRef, FrozenPlan, Id, PlanId};
use serde::{Deserialize, Serialize};

pub(crate) const SNAPSHOT_VERSION: u8 = 2;

/// Explicit execution provenance; test effects never become real effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionMode {
    /// Explicit fixture runner only.
    Test,
    /// Real runner, verified by the later production host.
    Real,
}
/// Preparation is independent of process and effect observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Preparation {
    /// Request has been received.
    Received,
    /// Host preparation has completed.
    Prepared,
    /// Waiting for an external prerequisite.
    Waiting,
}
/// Dispatch progress, never proof of process termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum DispatchState {
    /// Intent accepted; dispatch/spawn not confirmed.
    Starting,
    /// Host submitted the attempt; not proof of any target effect.
    Dispatched,
    /// Restart or uncertain runner outcome requires reconciliation.
    Unknown {
        /// Sticky dispatch history; uncertainty never erases a confirmed dispatch.
        dispatched: bool,
    },
}
impl DispatchState {
    pub(crate) fn was_dispatched(self) -> bool {
        matches!(self, Self::Dispatched | Self::Unknown { dispatched: true })
    }
    pub(crate) fn uncertain(self) -> Self {
        Self::Unknown {
            dispatched: self.was_dispatched(),
        }
    }
}
/// Verified assessment of the whole controlled attempt, not text from tool output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EffectAssessment {
    /// Quiescent attempt has no side effects or pending external work.
    NoEffect,
    /// Independent target verification confirms the desired state.
    Satisfied,
    /// Target is not satisfied; side effects may nevertheless exist.
    NotSatisfied,
    /// Effect cannot be established.
    Unknown,
}
/// Facts produced by the observation port. Serializable only as protected journal data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Observation {
    /// Runner and all delegated activity are quiescent; an exit code alone proves no goal.
    Exited {
        /// Observed process/test outcome, not a convergence result.
        exit_code: i32,
        /// Final byte count for this attempt, including buffered and discarded output.
        total_output_bytes: u64,
    },
    /// Trusted dispatch reconciliation proves this attempt was never dispatched.
    NeverDispatched {
        /// Final attempt diagnostic output, including any work before failed dispatch.
        total_output_bytes: u64,
    },
    /// Independent verification after termination.
    Effect {
        /// Assessment with an explicit unknown/no-effect distinction.
        assessment: EffectAssessment,
    },
    /// Neither termination nor effects can currently be confirmed.
    Uncertain,
}
/// Authenticated port output, obtained only by calling ObservationVerifier.
/// INVARIANT: LIFECYCLE-OBSERVATION-01 — references cannot directly create verified facts.
/// ~~~compile_fail
/// let _: execution_lifecycle::ObservationFacts = serde_json::from_str("{}").unwrap();
/// ~~~
#[derive(Debug, Clone)]
pub struct ObservationFacts {
    /// Verified exact plan identity.
    pub plan_id: PlanId,
    /// Verified exact plan digest.
    pub plan_digest: Digest,
    /// Verified attempt identity.
    pub attempt_id: AttemptId,
    /// Verified reference, category and runner.
    pub evidence: EvidenceRef,
    /// Actual reliable observation time, not the caller's receipt time.
    pub observed_at_unix_ms: u64,
    /// Verified fact; an exit requires all activity controlled by this attempt to be quiescent.
    pub observation: Observation,
}
/// Product/runner-owned verification boundary, not a DTO decoder.
/// Verify provenance, plan/attempt/runner binding and observation freshness independently.
/// Exited must establish quiescence of the entire controlled attempt, including delegated work;
/// a shell's exit while child work continues must return Uncertain instead.
/// NeverDispatched requires authoritative dispatch evidence, not absence of a visible process.
/// Both terminal observations must settle the final output count, including undelivered bytes.
/// This port does not protect against a malicious in-process host.
pub trait ObservationVerifier {
    /// Resolve and verify one exact reference at the supplied reliable receipt time.
    fn verify(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        evidence: &EvidenceRef,
        now_unix_ms: u64,
    ) -> Result<ObservationFacts, ObservationError>;
}
/// Closed external verification failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationError {
    /// Source authentication or evidence validation failed.
    Untrusted,
    /// Evidence cannot be read or reconciled.
    Unavailable,
}
/// Recorded verified observation; authenticity of restored storage belongs to C18.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordedObservation {
    /// Reference only, no raw script output or credentials.
    pub evidence: EvidenceRef,
    /// Reliable time of the observation.
    pub observed_at_unix_ms: u64,
    /// Fact authenticated when accepted.
    pub observation: Observation,
}
/// The current attempt; earlier details belong to the journal, not an unbounded in-memory log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttemptSnapshot {
    /// Stable durable attempt identity.
    pub id: AttemptId,
    /// Monotonic admitted attempt number, starting at one.
    pub number: u32,
    /// Time at which the intent was admitted, not a process-start claim.
    pub accepted_at_unix_ms: u64,
    /// Exact runner identity used for all attempts of this plan.
    pub runner: Id,
    /// Test/real provenance, fixed after the first attempt.
    pub mode: ExecutionMode,
    /// Host dispatch progress.
    pub dispatch: DispatchState,
    /// Last first-delivery failure diagnosis, separate from execution/effect evidence.
    pub dispatch_cause: Option<DispatchCause>,
    /// Last stop request acknowledgement, never proof of termination.
    pub stop_outcome: Option<StopOutcome>,
    /// Verified quiescence or authoritative never-dispatched fact.
    pub termination: Option<RecordedObservation>,
    /// Independent post-termination target/effect assessment.
    pub assessment: Option<RecordedObservation>,
    /// Cumulative output including discarded bytes; final and immutable after termination.
    pub output_bytes: u64,
}
/// Closed first-delivery diagnostics. These are not observations of termination or effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DispatchCause {
    /// Required capability or its independently verified freshness is unavailable.
    CapabilityUnavailable,
    /// A reliable clock could not be established at the gate.
    ClockUnavailable,
    /// A durable cancellation was seen before first delivery.
    Cancelled,
    /// A specific plan validity or cumulative budget bound prevents delivery.
    Limit(LimitReason),
    /// The first-commit revision is no longer current.
    StaleRevision,
    /// Current runner identity or provenance does not match the admitted action.
    RunnerMismatch,
    /// Current service configuration does not permit new delivery.
    ConfigurationUnavailable,
    /// Current host binding or Execute authorization was rejected.
    AuthorityUnavailable,
    /// The lifecycle no longer permits initial delivery for another reason.
    LifecycleChanged,
    /// Runner reported rejection; reconciliation still needs authoritative evidence.
    RunnerRejected,
    /// Delivery or acknowledgement could not be established.
    DeliveryUnknown,
    /// Runner returned a closed error; provider text is never persisted here.
    RunnerError,
}
/// Closed stop request diagnostics. Neither variant is a termination/effect observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StopOutcome {
    /// The runner acknowledged the stop request, not termination.
    Acknowledged,
    /// The runner returned a failure; trusted observations may still be available.
    Failed,
}
/// Host commands. They never perform I/O or constitute dispatch permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Command {
    /// Mark preparation complete or release an external wait.
    Prepare,
    /// Suspend preparation on an external prerequisite.
    Wait,
    /// Propose durable admission of a new attempt; C19 must first reauthorize it.
    BeginAttempt {
        /// Unique ID, checked for historical uniqueness by C18.
        attempt_id: AttemptId,
        /// Exact runner selected by the host.
        runner: Id,
        /// Explicit test/real execution mode.
        mode: ExecutionMode,
    },
    /// Record host dispatch, without claiming OS process start.
    Dispatched {
        /// Current attempt only.
        attempt_id: AttemptId,
    },
    /// Retain a first-delivery diagnostic while requiring reconciliation, never refunding an
    /// attempt or treating a runner rejection report as proof of quiescence/no effect.
    DispatchUnconfirmed {
        /// Exact attempted delivery, retained in the audit even if stale or rejected.
        attempt_id: AttemptId,
        /// Static, value-free diagnosis distinct from a generic host restart.
        cause: DispatchCause,
    },
    /// Record a stop request response without modifying dispatch/termination/effect facts.
    StopReported {
        /// Exact current attempt.
        attempt_id: AttemptId,
        /// Value-free port response.
        outcome: StopOutcome,
    },
    /// Request cancellation; does not terminate or roll back any effects.
    Cancel,
    /// Mark active execution uncertain after a host restart.
    Recover,
    /// Account monotonic runner output from the trusted host, never UI/AI.
    /// After termination, late partial counts are acknowledged without reducing the
    /// settled total; counts above the authenticated final total are rejected.
    Output {
        /// Current attempt only.
        attempt_id: AttemptId,
        /// Total observed bytes for this attempt, including truncated/discarded output.
        total_bytes: u64,
    },
}
/// Stable command identity plus expected journal revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandEvent {
    /// Stable id for this command and its direct retries.
    pub id: EventId,
    /// Revision on which this command was computed.
    pub expected_revision: u64,
    /// Closed command vocabulary.
    pub command: Command,
}
/// An evidence reference to resolve through the trusted observation boundary.
/// This input is not a verified fact and cannot be evaluated as a host command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationEvent {
    /// Stable id for this observation and its direct retries.
    pub id: EventId,
    /// Revision on which this observation was submitted.
    pub expected_revision: u64,
    /// Exact current attempt.
    pub attempt_id: AttemptId,
    /// Untrusted reference until verified.
    pub evidence: EvidenceRef,
}
/// One recorded input for persistence and exact retry comparison, never an execution entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "event",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum EventRecord {
    /// A host command.
    Command(CommandEvent),
    /// An observation reference whose facts were verified before application.
    Observation(ObservationEvent),
}
impl EventRecord {
    /// Stable identity shared by commands and observations.
    pub fn id(&self) -> &EventId {
        match self {
            Self::Command(e) => &e.id,
            Self::Observation(e) => &e.id,
        }
    }
    /// Revision against which the input was evaluated.
    pub fn expected_revision(&self) -> u64 {
        match self {
            Self::Command(e) => e.expected_revision,
            Self::Observation(e) => e.expected_revision,
        }
    }
}
/// Sole current journal format; no legacy readers or automatic empty-state fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    /// Exactly version 2, with a tagged command or observation record.
    pub version: u8,
    /// Bound frozen plan identity.
    pub plan_id: PlanId,
    /// Bound canonical plan digest.
    pub plan_digest: Digest,
    /// Creation time in reliable UTC Unix milliseconds.
    pub opened_at_unix_ms: u64,
    /// Last committed receipt time, also the durable clock watermark.
    pub updated_at_unix_ms: u64,
    /// Monotonic conditional-write revision.
    pub revision: u64,
    /// Preparation facts before the first attempt.
    pub preparation: Preparation,
    /// Sticky cancellation request; not evidence of termination.
    pub cancel_requested: bool,
    /// First attempt admission time; retries and waits never reset it.
    pub first_attempt_at_unix_ms: Option<u64>,
    /// Number of admitted intents, including attempts that never dispatched.
    pub attempts: u32,
    /// Output from previous attempts, never reset or refunded.
    pub prior_output_bytes: u64,
    /// Current attempt, retained after termination.
    pub attempt: Option<AttemptSnapshot>,
    /// Most recently committed input for bounded direct-retry detection.
    pub last_event: Option<EventRecord>,
}
/// Explicit snapshot envelope. The minimum reserves space for any valid bounded terminal state.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Must be at least MIN_SNAPSHOT_BYTES; larger inputs are rejected before decoding.
    pub max_snapshot_bytes: usize,
}
/// Minimum envelope for the fixed-size references and bounded observation slots.
pub const MIN_SNAPSHOT_BYTES: usize = 16_384;
/// Derived presentation phase, not a second persisted state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No preparation yet.
    Received,
    /// Ready for admission checks.
    Prepared,
    /// Waiting for a prerequisite.
    Waiting,
    /// Intent admitted; dispatch is not confirmed.
    Starting,
    /// Host dispatched work; effects are not yet known.
    Running,
    /// Reconciliation required.
    OutcomeUnknown,
    /// Quiescent execution still requires independent effect verification.
    ExecutionEnded,
    /// Verified target/effect result; mode still distinguishes test from real.
    Verified,
    /// Trusted dispatch evidence confirms failure before dispatch.
    FailedBeforeDispatch,
    /// Cancelled before any attempt was admitted.
    Cancelled,
}
/// Explicit reason a validity or cumulative budget bound prevents work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LimitReason {
    /// Plan validity has not begun.
    NotYetValid,
    /// Plan validity has expired.
    Expired,
    /// Total time since first admitted intent is exhausted.
    Timeout,
    /// Total output, including discarded bytes, is exhausted.
    Output,
    /// Maximum admitted attempts have been used.
    Attempts,
}
/// Cause to retain in the host's stop audit, independently of termination evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// A cancellation request was accepted.
    Cancelled,
    /// A plan bound prevents further execution.
    Limit(LimitReason),
}
/// Derived next action, never a permit or an automatically invoked operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Directive {
    /// Complete host preparation.
    Prepare,
    /// Wait for an external prerequisite or running attempt.
    Wait,
    /// Reauthorize and propose the first attempt.
    Ready,
    /// Reauthorize and explicitly propose another attempt.
    RetryEligible,
    /// Query trusted runner/dispatch state before any new action.
    Reconcile,
    /// Request bounded runner termination; no termination or rollback is implied.
    StopRunner(StopReason),
    /// Independently verify effects after confirmed quiescence.
    VerifyTarget,
    /// No automatic continuation is justified.
    ManualReview,
    /// No new attempt is allowed by time/output/attempt bounds.
    BudgetExhausted(LimitReason),
    /// No further execution required; inspect the recorded assessment/mode.
    Done,
}
/// Event processing without conflating unchanged results and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    /// Candidate conditional update; not yet committed.
    Applied,
    /// Latest command is being replayed exactly.
    Duplicate,
    /// Older revision; journal history decides any further deduplication.
    Stale,
}
/// Stable errors without untrusted data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    /// Invalid envelope.
    #[error("invalid lifecycle limits")]
    Configuration,
    /// Snapshot is malformed or violates invariants.
    #[error("invalid lifecycle snapshot")]
    Snapshot,
    /// Encoded state exceeds its explicit envelope.
    #[error("lifecycle bound exceeded")]
    Limit,
    /// Reliable time moved behind a committed watermark.
    #[error("lifecycle clock moved backwards")]
    Clock,
    /// Future or exhausted revision.
    #[error("invalid lifecycle revision")]
    Revision,
    /// Current command ID reused with different content.
    #[error("lifecycle idempotency conflict")]
    IdempotencyConflict,
    /// Wrong, repeated or unsupported attempt binding.
    #[error("invalid lifecycle attempt")]
    Attempt,
    /// Transition is not justified by current facts.
    #[error("invalid lifecycle transition")]
    Transition,
    /// Evidence binding, category or freshness failed after port verification.
    #[error("invalid lifecycle observation")]
    Observation,
    /// Trusted port failure, retaining classification without provider text.
    #[error("lifecycle observation verification failed: {0:?}")]
    ObservationVerification(ObservationError),
    /// Host tried to reduce cumulative accounting.
    #[error("invalid lifecycle accounting")]
    Accounting,
}
