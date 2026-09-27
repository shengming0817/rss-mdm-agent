use crate::{ConfigChange, Error};
use execution_admission::AuthorityVerifier;
use execution_approval::ProfileApproval;
use execution_capability::EnvironmentSnapshot;
use execution_contract::{ActorId, AttemptId, Authority, DeviceId, FrozenExecution, Id};
use execution_lifecycle::{DispatchAction, ExecutionMode, ObservationFacts};
use execution_sqlite::{AccessRequest, TrustSnapshot};

/// Trusted host output; no Deserialize and no wire binding constructor is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceBinding {
    /// Verified product authority, distinct from OS/provider logins.
    pub authority: Authority,
    /// Independently bound target device.
    pub device: DeviceId,
}
/// Caller verified by trusted ingress for one operation; never a transport/model DTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    /// Permission-bearing product actor, independent of service and provider identities.
    pub actor: ActorId,
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
    /// Establish/recheck the device authority; production failure cannot select Test identity.
    fn service_binding(&self) -> Result<ServiceBinding, Error>;
    /// Current read/write/answer/admin access for an independently verified caller.
    fn authorize(
        &self,
        caller: &RequestContext,
        request: AccessRequest<'_>,
    ) -> Result<(), execution_sqlite::Error>;
    /// Internal task observation/trust/dispatch operations, independent of UI selection.
    /// The application supplies a scope loaded from its journal, never an untrusted caller claim.
    fn authorize_service(&self, request: AccessRequest<'_>) -> Result<(), execution_sqlite::Error>;
    /// Independently reliable UTC; uncertainty/rollback is an error.
    fn reliable_now(&self) -> Result<u64, execution_sqlite::Error>;
    /// Verified capability inventory for the bound plan, never a preview cache.
    fn capabilities(&self, plan: &FrozenExecution) -> Result<CapabilitySnapshot, Error>;
    /// Independently verified approval definitions, current policy and revocation identity.
    fn trusted_snapshot(
        &self,
        plan: &FrozenExecution,
    ) -> Result<TrustSnapshot, execution_sqlite::Error>;
    /// References selected by the trusted approval authority. C08 still validates every profile.
    fn approval_bindings(&self, plan: &FrozenExecution) -> Result<Vec<ProfileApproval>, Error>;
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
    pub(crate) plan: FrozenExecution,
    pub(crate) allowance: execution_lifecycle::DispatchAllowance,
    pub(crate) issued: std::time::Instant,
    pub(crate) software_ownership: Option<execution_contract::SoftwareProvenance>,
}
impl AuthorizedDispatch {
    /// Protected provenance read from the same device journal after admission.
    pub fn software_ownership(&self) -> Option<execution_contract::SoftwareProvenance> {
        self.software_ownership.clone()
    }
    /// Inspect the authorized immutable plan to select the host's runner implementation.
    /// Reading it cannot clone or reconstruct first-dispatch authority.
    pub fn input(&self) -> &FrozenExecution {
        &self.plan
    }
    /// Inspect the remaining cumulative allowance; reading does not grant dispatch authority.
    pub fn allowance(&self) -> execution_lifecycle::DispatchAllowance {
        let mut allowance = self.allowance;
        allowance.remaining_timeout_ms = allowance
            .remaining_timeout_ms
            .saturating_sub(self.issued.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
        allowance
    }
    /// Consume first-dispatch authority. Runner implementations must perform no action beforehand.
    pub fn dispatch<T>(self, run: impl FnOnce(&FrozenExecution, &DispatchAction) -> T) -> T {
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
/// Bounded observation context loaded by the application from its authorized journal.
/// This is not an IPC DTO and never grants a new execution attempt.
#[derive(Clone, Copy)]
pub struct SoftwareObservation<'a> {
    /// Independent observation deadline, including cleanup work.
    pub deadline: std::time::Instant,
    /// Previously committed software evidence for this exact attempt.
    pub previous: Option<&'a execution_contract::SoftwareEvidence>,
    /// The journal already contains independently verified quiescence/termination.
    pub quiescent: bool,
    /// Keep historical effect facts immutable; only cleanup may advance.
    pub finalized: bool,
}
impl SoftwareObservation<'_> {
    /// A fresh bounded probe with no restored history or termination assertion.
    pub fn new(deadline: std::time::Instant) -> Self {
        Self {
            deadline,
            previous: None,
            quiescent: false,
            finalized: false,
        }
    }
}
/// Trusted bounded runner seam. Only dispatch may start work; observations never replay it.
pub trait RunnerPort {
    /// Independent software observations. Process-only runners must reject software plans.
    fn software_evidence(
        &self,
        plan: &FrozenExecution,
        _attempt: &AttemptId,
        _observation: SoftwareObservation<'_>,
    ) -> Result<Option<execution_contract::SoftwareEvidence>, Error> {
        if plan.spec().execution.software().is_some() {
            Err(Error::Unsupported)
        } else {
            Ok(None)
        }
    }

    /// Exact runner identity, immutable across a task's attempts.
    fn id(&self) -> Id;
    /// Provenance supported by this implementation.
    fn mode(&self) -> ExecutionMode;
    /// Consume the only first-delivery permission; failure or unknown must be reconciled.
    fn dispatch(&self, permit: AuthorizedDispatch) -> Result<DispatchOutcome, Error>;
    /// Latest completed capture, never a reason to redispatch. Missing capture remains unknown.
    fn evidence(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
    ) -> Result<Option<execution_contract::ProcessEvidence>, Error>;
    /// The owner has committed this final capture and its cumulative accounting. Release only
    /// the matching finished record; acknowledgements never grant a new dispatch permission.
    fn acknowledge_capture(
        &self,
        plan: &FrozenExecution,
        facts: &execution_contract::ProcessEvidence,
    ) -> Result<(), Error>;
    /// Request bounded stopping. Success only acknowledges the request, not termination.
    fn stop(&self, plan: &FrozenExecution, attempt: &AttemptId) -> Result<(), Error>;
    /// Return independently verified facts if available. Lost records return None, never fabricated
    /// Exited/NeverDispatched evidence inferred from the plan or lack of a visible process.
    fn observe(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        stage: ObservationStage,
        now: u64,
    ) -> Result<Option<ObservationFacts>, Error>;
}
