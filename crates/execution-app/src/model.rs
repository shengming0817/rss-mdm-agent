use execution_contract::{AttemptId, Digest, EvidenceRef, Id, PlanId, RequestId};
use execution_lifecycle::{EffectAssessment, ExecutionMode};

/// Value-free application failures. Backend paths, SQL, secrets and runner text never escape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Invalid explicit configuration or compiled hard bound.
    #[error("invalid application configuration")]
    Configuration,
    /// No authenticated product binding exists.
    #[error("trusted identity unavailable")]
    Unbound,
    /// Current access was rejected.
    #[error("access denied")]
    Denied,
    /// No result exists in the caller's authorized namespace.
    #[error("execution not found")]
    NotFound,
    /// An immutable identity or conditional revision conflicts.
    #[error("execution conflict")]
    Conflict,
    /// A bound request is malformed.
    #[error("invalid execution request")]
    InvalidInput,
    /// Reliable time or trustworthy freshness is unavailable.
    #[error("reliable time unavailable")]
    Clock,
    /// Capability is blocked, unknown, unsupported or stale.
    #[error("required capability unavailable")]
    Capability,
    /// C07/C08 rejected this attempt; this is not permission to retry it.
    #[error("attempt admission rejected")]
    AdmissionRejected,
    /// No conforming configuration is active. Reads and safe recovery remain available.
    #[error("new execution disabled")]
    Degraded,
    /// This build has no production identity/bootstrap/runner adapter.
    #[error("execution mode unsupported")]
    Unsupported,
    /// A fixed capacity was exhausted.
    #[error("execution capacity exceeded")]
    Capacity,
    /// A dependency is temporarily unavailable; no new action is implied.
    #[error("execution service unavailable")]
    Unavailable,
    /// Corrupt, unsupported or inaccessible protected storage.
    #[error("protected execution storage unavailable")]
    Storage,
    /// Query the original operation; never allocate a replacement attempt.
    #[error("operation commit outcome unknown")]
    OutcomeUnknown,
    /// Retry confirmation with the same request/consumer/event; there is no operation receipt.
    #[error("confirmation commit outcome unknown; retry the same event confirmation")]
    ConfirmationUnknown,
}
impl From<execution_sqlite::Error> for Error {
    fn from(error: execution_sqlite::Error) -> Self {
        use execution_sqlite::Error as S;
        match error {
            S::Configuration => Self::Configuration,
            S::InvalidInput => Self::InvalidInput,
            S::Denied => Self::Denied,
            S::NotFound => Self::NotFound,
            S::Conflict => Self::Conflict,
            S::Clock => Self::Clock,
            S::Trust | S::Busy => Self::Unavailable,
            S::Capacity => Self::Capacity,
            S::OperationCommitUnknown => Self::OutcomeUnknown,
            S::ConfirmationCommitUnknown => Self::ConfirmationUnknown,
            S::Corrupt | S::Schema | S::Storage | S::BootstrapUnpublished => Self::Storage,
        }
    }
}

/// Stable logical command identity, distinct from a business task's RequestId.
/// Retain this value across network retries/unknown outcomes. Generate a new value only for
/// a deliberately new owner action; doing so for advance can spend another attempt/approval.
/// ```compile_fail
/// let _: execution_app::CommandId = execution_contract::RequestId::new("task").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandId(Id);
impl CommandId {
    /// Construct a bounded identity for one explicit owner command.
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        Ok(Self(Id::new(value).map_err(|_| Error::InvalidInput)?))
    }
    /// The exact first-attempt command owned by submit. Use it to resume a registered task with
    /// no admitted attempt after a pre-commit transient failure; rejected admission stays rejected.
    pub fn initial_attempt() -> Self {
        Self(Id::new("initial").expect("static ID"))
    }
    /// Opaque value for adapter-owned durable retry tracking; not a task or authority identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Explicit bootstrap selection. Test initialization never happens as an error fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Startup {
    /// Create a new Test authority database in an existing private directory.
    CreateTest,
    /// Open an existing Test authority database, preserving every task and receipt.
    OpenTest,
    /// Fails in this S1 build; test identity is never a production fallback.
    Production,
}

/// Safe presentation of C09 facts, not a second persisted state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPhase {
    /// Registered without an admitted attempt; no execution permission is implied.
    Waiting,
    /// Intent committed, dispatch not confirmed.
    Accepted,
    /// Runner accepted the dispatch; no verified effect is implied.
    Running,
    /// Dispatch or termination needs trusted reconciliation.
    OutcomeUnknown,
    /// Runner is quiescent; assessment is still pending.
    ExecutionEnded,
    /// Explicit fixture assessment only; inspect assessment for success, failure or unknown.
    TestCompleted,
    /// Trusted evidence establishes failure before dispatch.
    FailedBeforeDispatch,
    /// Cancellation plus confirmed quiescence/no effect, or cancellation before an attempt.
    Cancelled,
}

/// Authorized value-only task projection. Never includes launch, parameters or raw output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionStatus {
    /// Original reliable business identity.
    pub operation_request_id: RequestId,
    /// Exact frozen plan identity.
    pub plan_id: PlanId,
    /// Canonical C01 plan digest.
    pub plan_digest: Digest,
    /// Current derived lifecycle phase.
    pub phase: TaskPhase,
    /// Explicit fixture provenance, also present before the first attempt.
    pub mode: ExecutionMode,
    /// Current admitted attempt, absent before admission.
    pub attempt_id: Option<AttemptId>,
    /// Total admitted attempts; retries never reset it.
    pub attempts: u32,
    /// Sticky cancellation request, independently of termination.
    pub cancel_requested: bool,
    /// Fixture assessment if recorded; Unknown is not success.
    pub assessment: Option<EffectAssessment>,
    /// Authorized evidence references only.
    pub evidence: Vec<EvidenceRef>,
}
