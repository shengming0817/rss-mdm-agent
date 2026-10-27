use crate::*;
use execution_contract::*;
use execution_interaction::{Interaction, Reference, Spec};
use execution_lifecycle::{CommandEvent, ObservationEvent, ObservationVerifier};

/// Trusted, synchronous atomic journal operations required by the execution application.
/// Implementations own their connection and transaction. Write methods return success only
/// after a durable commit; admission callbacks run inside that same write transaction after
/// replay has been ruled out. Intent, claim, snapshot, audit and receipt
/// cannot be split into separately visible writes. Callbacks are bounded and non-reentrant.
/// An adapter is trusted product code: the type boundary does not prove external persistence.
pub trait JournalPort {
    /// Adapter-owned connection-local reclamation checkpoint, with no serialization contract.
    type MaterialCheckpoint: Copy + Eq;
    /// Capture local and external changes for a reclamation pass.
    fn material_checkpoint(&self) -> Result<Self::MaterialCheckpoint, JournalError>;
    /// Whether changes could have weakened a completed or partial reclamation proof.
    fn material_proof_current(
        &self,
        checkpoint: Self::MaterialCheckpoint,
    ) -> Result<bool, JournalError>;
    /// Authenticate all rows in one bounded page, including independent delivery permission.
    fn material_page(
        &self,
        after: Option<&RequestId>,
        limit: usize,
        consumer: &Id,
        host: &impl JournalHost,
    ) -> Result<Vec<ServiceMaterialFact>, JournalError>;
    /// Authenticate one associated request independently of the history cursor.
    fn material_fact(
        &self,
        request: &RequestId,
        consumer: &Id,
        host: &impl JournalHost,
    ) -> Result<ServiceMaterialFact, JournalError>;
    /// Inspect a bounded segment of committed receipts without restarting historical proof.
    /// External connection changes reject the continuation; no mutation is performed.
    fn material_changes(
        &self,
        checkpoint: Self::MaterialCheckpoint,
        limit: usize,
        consumer: &Id,
        host: &impl JournalHost,
    ) -> Result<ServiceMaterialChanges<Self::MaterialCheckpoint>, JournalError>;
    /// Fence concurrent writers while the sole product owner performs a small physical deletion.
    /// The callback does not access this journal; an error preserves remaining materials.
    fn with_material_proof<T>(
        &self,
        checkpoint: Self::MaterialCheckpoint,
        action: impl FnOnce() -> Result<T, Error>,
    ) -> Result<T, Error>;

    /// Resolve the exact immutable backend task/attempt from the original execution plan.
    fn remote_request(
        &self,
        task: &Id,
        attempt: &Id,
        host: &impl JournalHost,
    ) -> Result<Option<RequestId>, JournalError>;
    /// Find the exact execution holding one resource; validates its stored scope and RunnerFact access.
    /// This service-only lookup never scans execution history or grants execution permission.
    fn resource_owner(
        &self,
        key: &str,
        host: &impl JournalHost,
    ) -> Result<Option<RequestId>, JournalError>;

    /// Authority actually bound to this opened journal.
    fn authority(&self) -> &Authority;
    /// Immutable decoder/execution envelope selected when this journal was opened.
    fn input_limits(&self) -> ExecutionLimits;
    /// Stable descending time/identity discovery across both original history tables.
    fn task_history_page(
        &self,
        authority: &Authority,
        actor: &ActorId,
        query: &TaskHistoryQuery,
        host: &impl JournalHost,
    ) -> Result<Vec<TaskHistoryPosition>, JournalError>;
    /// List bounded preparation records without creating another task queue.
    fn backend_requests(
        &self,
        authority: &Authority,
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
    /// Atomically append the exact facts and claim changes to the original attempt.
    /// Return those facts only after commit succeeds. Unknown commit must return
    /// OperationCommitUnknown and must not release a runner boundary.
    fn record_software_progress(
        &mut self,
        scope: &Scope,
        facts: &SoftwareProgress,
        host: &impl JournalHost,
    ) -> Result<SoftwareProgress, JournalError>;
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
    /// Original preparation row and execution receipt watermarks, read in one owner transaction.
    fn task_watermarks(&self) -> Result<(u64, u64), JournalError>;
    /// Stable first-receipt keyset; later execution updates cannot move tasks across pages.
    fn task_requests(
        &self,
        before: Option<u64>,
        limit: usize,
        host: &impl JournalHost,
    ) -> Result<RecentRequestPage, JournalError>;
    /// Current protected confirmation state of the original execution.
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
