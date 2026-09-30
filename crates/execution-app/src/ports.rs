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
    pub(crate) ownership: Vec<execution_contract::SoftwareOwnership>,
    pub(crate) action: DispatchAction,
    pub(crate) plan: FrozenExecution,
    pub(crate) allowance: execution_lifecycle::DispatchAllowance,
    pub(crate) issued: std::time::Instant,
}
impl AuthorizedDispatch {
    /// Previously completed package states from the same protected journal.
    pub fn software_ownership(&self) -> Vec<execution_contract::SoftwareOwnership> {
        self.ownership.clone()
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
/// Trusted bounded runner seam. Only dispatch may start work; observations never replay it.
pub trait RunnerPort {
    /// Resume only a known completed step boundary in the existing intent. An unfinished
    /// Begin cannot be turned into a new invocation by recovery.
    fn resume_software(&self, _resume: SoftwareResume) -> Result<(), Error> {
        Ok(())
    }
    /// Query the original physical invocation after losing its owner; this must not dispatch.
    fn recover_software_progress(
        &self,
        _plan: &FrozenExecution,
        _previous: &execution_contract::SoftwareProgress,
    ) -> Result<Option<execution_contract::SoftwareProgress>, Error> {
        Ok(None)
    }
    /// Pending phase boundary of the original software attempt. No operation may cross an
    /// unacknowledged boundary; missing live ownership never means permission to replay it.
    fn software_progress(
        &self,
        _plan: &FrozenExecution,
        _attempt: &AttemptId,
    ) -> Result<Option<execution_contract::SoftwareProgress>, Error> {
        Ok(None)
    }
    /// Release the exact boundary only after the execution journal committed it.
    fn acknowledge_software_progress(
        &self,
        _receipt: execution_sqlite::CommittedSoftwareProgress,
    ) -> Result<(), Error> {
        Err(Error::Unsupported)
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

/// Journal-derived continuation of an original software sequence; never a first-start permit.
pub struct SoftwareResume {
    pub(crate) ownership: Vec<execution_contract::SoftwareOwnership>,
    pub(crate) plan: FrozenExecution,
    pub(crate) progress: execution_contract::SoftwareProgress,
    pub(crate) allowance: execution_lifecycle::DispatchAllowance,
}
impl SoftwareResume {
    /// Consume the bounded continuation after the application verified the stored boundary.
    pub fn resume<T>(
        self,
        run: impl FnOnce(
            FrozenExecution,
            execution_contract::SoftwareProgress,
            execution_lifecycle::DispatchAllowance,
            Vec<execution_contract::SoftwareOwnership>,
        ) -> T,
    ) -> T {
        run(self.plan, self.progress, self.allowance, self.ownership)
    }
}
