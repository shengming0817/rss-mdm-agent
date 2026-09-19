use crate::{ConfigChange, Error};
use execution_admission::AuthorityVerifier;
use execution_approval::ProfileApproval;
use execution_capability::EnvironmentSnapshot;
use execution_contract::{ActorId, AttemptId, Authority, DeviceId, FrozenPlan, Id};
use execution_lifecycle::{DispatchAction, ExecutionMode, ObservationFacts};
use execution_sqlite::{AccessRequest, TrustSnapshot};

/// Trusted host output; no Deserialize and no wire binding constructor is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Verified product authority, distinct from OS/provider logins.
    pub authority: Authority,
    /// Permission-bearing actor.
    pub actor: ActorId,
    /// Independently bound target device.
    pub device: DeviceId,
}
/// Verified current capability inventory, with independently checked source freshness.
pub struct CapabilitySnapshot {
    /// Exact authority/device inventory, evaluated by C06.
    pub environment: EnvironmentSnapshot,
    /// Reliable time when this snapshot was verified.
    pub verified_at_unix_ms: u64,
    /// Exclusive freshness bound.
    pub fresh_until_unix_ms: u64,
}
/// The host is trusted product code, never a model DTO decoder. Methods invoked within SQLite
/// transactions must be bounded, non-reentrant and use independently verified local snapshots.
/// C07's verifier must authenticate origin/delegation and never echo submitted plans as allow rules.
pub trait AppHost: AuthorityVerifier {
    /// Establish/recheck the actual caller binding; production failure cannot select Test identity.
    fn binding(&self) -> Result<Binding, Error>;
    /// Current read/write/answer/admin access. RunnerFact and ManageTrust are internal service actions.
    fn authorize(&self, request: AccessRequest<'_>) -> Result<(), execution_sqlite::Error>;
    /// Independently reliable UTC; uncertainty/rollback is an error.
    fn reliable_now(&self) -> Result<u64, execution_sqlite::Error>;
    /// Verified capability inventory for the bound plan, never a preview cache.
    fn capabilities(&self, plan: &FrozenPlan) -> Result<CapabilitySnapshot, Error>;
    /// Independently verified approval definitions, current policy and revocation identity.
    fn trusted_snapshot(&self, plan: &FrozenPlan)
        -> Result<TrustSnapshot, execution_sqlite::Error>;
    /// References selected by the trusted approval authority. C08 still validates every profile.
    fn approval_bindings(&self, plan: &FrozenPlan) -> Result<Vec<ProfileApproval>, Error>;
    /// Authenticate administrative rights and durably record a configuration activation.
    /// Failure must leave the previous configuration usable; no UI/AI approval boolean is accepted.
    fn configuration_change(&self, change: &ConfigChange) -> Result<(), Error>;
}

/// Only the application can construct this first-dispatch value after both current gates.
/// It cannot be cloned, decoded or reconstructed from a stored snapshot.
/// ```compile_fail
/// fn duplicate(value: execution_app::AuthorizedDispatch) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// let _: execution_app::AuthorizedDispatch = serde_json::from_str("{}").unwrap();
/// ```
pub struct AuthorizedDispatch {
    pub(crate) action: DispatchAction,
    pub(crate) plan: FrozenPlan,
}
impl AuthorizedDispatch {
    /// Inspect the authorized immutable plan to select the host's runner implementation.
    /// Reading it cannot clone or reconstruct first-dispatch authority.
    pub fn plan(&self) -> &FrozenPlan {
        &self.plan
    }
    /// Consume first-dispatch authority. Runner implementations must perform no action beforehand.
    pub fn dispatch<T>(self, run: impl FnOnce(&FrozenPlan, &DispatchAction) -> T) -> T {
        self.action.dispatch(|action| run(&self.plan, action))
    }
}
/// First delivery outcome, never an instruction to resend the same action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Runner acknowledged this exact attempt.
    Accepted,
    /// Authoritative runner rejection; still reconcile before deciding any future attempt.
    NeverDispatched,
    /// Delivery or acknowledgement is uncertain.
    OutcomeUnknown,
}
/// Separate termination and effect observations avoid treating exit zero as verified success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationStage {
    /// Establish quiescence/never dispatched/uncertainty.
    Termination,
    /// Assess the effect after quiescence was durably recorded.
    Assessment,
}
/// Trusted bounded runner seam. Only dispatch may start work; observations never replay it.
pub trait RunnerPort {
    /// Exact runner identity, immutable across a task's attempts.
    fn id(&self) -> Id;
    /// Provenance supported by this implementation.
    fn mode(&self) -> ExecutionMode;
    /// Consume the only first-delivery permission; failure or unknown must be reconciled.
    fn dispatch(&self, permit: AuthorizedDispatch) -> Result<DispatchOutcome, Error>;
    /// Request bounded stopping. Success only acknowledges the request, not termination.
    fn stop(&self, plan: &FrozenPlan, attempt: &AttemptId) -> Result<(), Error>;
    /// Return independently verified facts if available. Lost records return None, never fabricated
    /// Exited/NeverDispatched evidence inferred from the plan or lack of a visible process.
    fn observe(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        stage: ObservationStage,
        now: u64,
    ) -> Result<Option<ObservationFacts>, Error>;
}
