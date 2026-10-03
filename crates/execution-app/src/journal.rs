use crate::*;
use execution_approval::ProfileApproval;
use execution_contract::*;
use execution_interaction::{Interaction, Reference, Spec};
use execution_lifecycle::{CommandEvent, ObservationEvent, ObservationVerifier};

/// Trusted, synchronous atomic journal operations required by the execution application.
/// Implementations own their connection and transaction. Write methods return success only
/// after a durable commit; admission callbacks run inside that same write transaction after
/// replay has been ruled out. Approval consumption, intent, claim, snapshot, audit and receipt
/// cannot be split into separately visible writes. Callbacks are bounded and non-reentrant.
/// An adapter is trusted product code: the type boundary does not prove external persistence.
pub trait JournalPort {
    /// Authority actually bound to this opened journal.
    fn authority(&self) -> &Authority;
    /// Immutable decoder/execution envelope selected when this journal was opened.
    fn input_limits(&self) -> ExecutionLimits;
    /// List bounded preparation records without creating another task queue.
    fn backend_requests(
        &self,
        actor: &execution_contract::ActorId,
        host: &impl JournalHost,
    ) -> Result<Vec<BackendRequest>, JournalError>;
    /// Read a pre-execution intent from the same protected journal and actor namespace.
    fn backend_request(
        &self,
        scope: &Scope,
        host: &impl JournalHost,
    ) -> Result<Option<BackendRequest>, JournalError>;
    /// Compare-and-append a preparation transition; it creates no execution attempt or grant.
    fn record_backend_request(
        &mut self,
        scope: &Scope,
        expected: Option<&BackendRequest>,
        next: &BackendRequest,
        host: &impl JournalHost,
    ) -> Result<(), JournalError>;
    /// Device-owner lookup, fenced by the exact current journal format and authority.
    fn contains_request(
        &self,
        request: &execution_contract::RequestId,
    ) -> Result<bool, JournalError>;
    /// Atomically append the exact facts and ownership/claim changes to the original attempt.
    /// Return those facts only after commit succeeds. Unknown commit must return
    /// OperationCommitUnknown and must not release a runner boundary.
    fn record_software_progress(
        &mut self,
        scope: &Scope,
        facts: &SoftwareProgress,
        host: &impl JournalHost,
    ) -> Result<SoftwareProgress, JournalError>;
    /// Read only protected provenance produced by completed installations in this journal.
    fn software_ownership(
        &self,
        scope: &Scope,
        host: &impl JournalHost,
    ) -> Result<Vec<execution_contract::SoftwareOwnership>, JournalError>;
    /// Read the original attempt's history without creating or replaying an invocation.
    fn software_progress(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl JournalHost,
    ) -> Result<Option<SoftwareProgress>, JournalError>;
    /// Read unconfirmed events and their evidence in one SQLite snapshot. Deliver and
    /// RunnerFact are independently authorized; ordinary result access is insufficient.
    fn delivery_evidence(
        &self,
        scope: &Scope,
        consumer: &Id,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<Vec<DeliveryEvidence>, JournalError>;
    /// Pull the oldest unconfirmed results for one authorized scope/consumer.
    /// Only durable confirmation advances delivery; partial or out-of-order processing cannot skip a result.
    fn pull_results(
        &self,
        scope: &Scope,
        consumer: &Id,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<Vec<Receipt>, JournalError>;
    /// Idempotently confirm exactly one delivered event. Confirmation creates no delivery event.
    /// Consumers persist their own effect/result before acknowledging; delivery is at least once.
    fn confirm(
        &mut self,
        scope: &Scope,
        consumer: &Id,
        event: &EventId,
        host: &impl JournalHost,
    ) -> Result<(), JournalError>;
    /// Read full audit using a separate current authorization; ordinary result access is insufficient.
    fn audit(
        &self,
        scope: &Scope,
        operation: &OperationRequestId,
        host: &impl JournalHost,
    ) -> Result<AuditRecord, JournalError>;
    /// Restore by stable business request identity after reconnect or restart. The stored scope
    /// is authenticated before any plan or state is returned. No dispatch action is recoverable.
    fn execution_by_request(
        &self,
        request: &execution_contract::RequestId,
        access: ExecutionAccess<'_>,
        host: &impl JournalHost,
    ) -> Result<ExecutionRecord, JournalError>;
    /// Retrieve an execution command's safe receipt under current Execute permission, independent
    /// of general result reading. Does not authorize a new attempt or re-run admission.
    fn execution_receipt(
        &self,
        scope: &Scope,
        op: &OperationRequestId,
        host: &impl JournalHost,
    ) -> Result<Option<Receipt>, JournalError>;
    /// Register one immutable bounded plan. No preparation, approval or runner action is implied.
    fn open_execution(
        &mut self,
        op: &OperationRequestId,
        plan: &FrozenExecution,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError>;
    /// Device service recovery scan. Every row requires independent RunnerFact authorization.
    /// This does not grant ordinary callers a cross-actor listing endpoint.
    fn device_execution_requests(
        &self,
        device: &execution_contract::DeviceId,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<ExecutionRequestPage, JournalError>;
    /// Read a bounded page only for the selected actor/device; each row requires result access.
    fn execution_requests(
        &self,
        actor: &execution_contract::ActorId,
        device: &execution_contract::DeviceId,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<ExecutionRequestPage, JournalError>;
    /// Apply a host command through historical deduplication and atomic persistence.
    /// BeginAttempt obtains decisions lazily after receipt replay is ruled out.
    /// Never reconstruct the returned dispatch action after a lost response.
    /// ```compile_fail
    /// use execution_app::{JournalPort, JournalHost, OperationRequestId, Scope};
    /// use execution_lifecycle::ObservationEvent;
    /// fn wrong<J: JournalPort>(journal: &mut J, op: &OperationRequestId, scope: &Scope, event: &ObservationEvent, host: &impl JournalHost) {
    ///     journal.apply_command(op, scope, event, &[], host);
    /// }
    /// ```
    fn apply_command(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &CommandEvent,
        bindings: &[ProfileApproval],
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError>;
    /// Verify and record an observation under RunnerFact authorization.
    /// Replay, revision and attempt checks precede verification; no approval inputs apply.
    /// ```compile_fail
    /// use execution_app::{JournalPort, JournalHost, OperationRequestId, Scope};
    /// use execution_lifecycle::{CommandEvent, ObservationVerifier};
    /// fn wrong<J: JournalPort>(journal: &mut J, op: &OperationRequestId, scope: &Scope, event: &CommandEvent, host: &impl JournalHost, verifier: &dyn ObservationVerifier) {
    ///     journal.apply_observation(op, scope, event, host, verifier);
    /// }
    /// ```
    /// ```compile_fail
    /// use execution_app::{JournalPort, JournalHost, OperationRequestId, Scope};
    /// use execution_lifecycle::ObservationEvent;
    /// fn missing<J: JournalPort>(journal: &mut J, op: &OperationRequestId, scope: &Scope, event: &ObservationEvent, host: &impl JournalHost) {
    ///     journal.apply_observation(op, scope, event, host);
    /// }
    /// ```
    fn apply_observation(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        event: &ObservationEvent,
        host: &impl JournalHost,
        verifier: &dyn ObservationVerifier,
    ) -> Result<CommitOutcome, JournalError>;
    /// Bounded owner scan, authenticating every scope before returning a request identifier.
    fn service_requests(
        &self,
        after: Option<&execution_contract::RequestId>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<Vec<execution_contract::RequestId>, JournalError>;
    /// Read the execution confirmation under the caller's existing operation access.
    /// Missing is distinct from corrupt/unavailable storage; this does not grant result access.
    fn execution_confirmation_state(
        &self,
        input: &execution_contract::FrozenExecution,
        access: ExecutionAccess<'_>,
        host: &impl JournalHost,
    ) -> Result<Option<Interaction>, JournalError>;
    /// Open an interaction bound to an existing execution scope. Answers are never approvals.
    fn open_interaction(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        spec: &Spec,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError>;
    /// Resolve answer/cancel/expiry against the current protected state inside one write transaction.
    /// No public method accepts the core's freely constructible Transition.
    fn apply_interaction(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        id: &Reference,
        command: &execution_interaction::Command,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError>;
    /// Restore a pending or terminal interaction without changing the execution task.
    fn interaction(
        &self,
        scope: &Scope,
        id: &Reference,
        host: &impl JournalHost,
    ) -> Result<Interaction, JournalError>;
    /// Read the protected CAS revision for a subsequent trust refresh. Requires trust-management
    /// access, independently of result/audit access; absence means the first refresh uses None.
    fn trust_revision(
        &self,
        scope: &Scope,
        host: &impl JournalHost,
    ) -> Result<Option<u64>, JournalError>;
    /// Atomically install a complete freshly verified snapshot using expected local head revision.
    /// Definitions are immutable; refresh cannot import, reset or refund local consumption counters.
    fn refresh_trust(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        expected: Option<u64>,
        host: &impl JournalHost,
    ) -> Result<CommitOutcome, JournalError>;
    /// Save process facts and raw byte BLOBs atomically in the existing attempt namespace.
    fn record_process(
        &mut self,
        scope: &Scope,
        facts: &ProcessEvidence,
        host: &impl JournalHost,
    ) -> Result<(), JournalError>;
    /// Internal owner reconciliation reads its previously committed facts, never creating a permit.
    fn runner_evidence(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl JournalHost,
    ) -> Result<Option<ProcessEvidence>, JournalError>;
}

/// Canonical identity digest for journal scopes, operations and events.
/// The V7 byte domain is persisted identity and stays stable when Rust owners move.
pub fn journal_fingerprint(value: &impl serde::Serialize) -> Result<String, JournalError> {
    use sha2::{Digest as _, Sha256};
    let bytes = serde_json_canonicalizer::to_vec(value).map_err(|_| JournalError::Corrupt)?;
    let digest = Sha256::digest([b"execution-sqlite/v7\0".as_slice(), &bytes].concat());
    Ok(format!("{digest:x}"))
}
/// Stable interaction identity for the one product execution confirmation.
pub fn execution_confirmation(plan: &execution_contract::FrozenExecution) -> Spec {
    Spec {
        id: Reference::new(format!("execute-{}", plan.digest().as_str())).expect("bounded digest"),
        subject: Scope::from_input(plan).interaction_subject(),
        kind: execution_interaction::Kind::ExecutionAction {
            digest: Reference::new(plan.digest().as_str()).expect("digest"),
        },
        expires_at_unix_ms: plan.spec().validity.expires_at_unix_ms.min(
            plan.spec()
                .validity
                .not_before_unix_ms
                .saturating_add(60_000),
        ),
    }
}
