use crate::{
    Command, CommandEvent, Directive, EventOutcome, EventRecord, Execution, ExecutionMode,
};
use execution_contract::{AttemptId, Digest, Id, PlanId};

/// Outcome reported by the trusted atomic persistence owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitStatus {
    /// This invocation committed the candidate for the first time.
    Applied,
    /// The event/attempt already exists; reload its durable outcome, never dispatch again.
    AlreadyCommitted,
}

/// Candidate conditional write; only lifecycle evaluation can construct it.
#[derive(Debug)]
pub struct Transition {
    pub(crate) expected_revision: u64,
    pub(crate) next: Execution,
}
impl Transition {
    /// Expected revision within the protected plan namespace.
    pub fn expected_revision(&self) -> u64 {
        self.expected_revision
    }
    /// Candidate state for persistence, not a dispatch capability.
    pub fn next(&self) -> &Execution {
        &self.next
    }
    /// Commit through the product's trusted C18 persistence boundary.
    ///
    /// The callback must atomically validate current authorization/approval, historical
    /// event/attempt uniqueness and CAS, and persist all consumption, intent and state.
    /// Return Applied only for the first successful commit; conflicts, unknown outcomes
    /// and failed writes are errors. Idempotent replay returns AlreadyCommitted.
    /// No action survives callback failure, replay or restoration from storage.
    /// This library cannot authenticate a malicious in-process persistence callback.
    ///
    /// ref: sqlx sqlx-core/src/transaction.rs (success after commit)
    pub fn commit<E>(
        self,
        persist: impl FnOnce(&Self) -> Result<CommitStatus, E>,
    ) -> Result<Option<DispatchAction>, E> {
        if persist(&self)? != CommitStatus::Applied {
            return Ok(None);
        }
        let s = self.next.snapshot();
        let Some(EventRecord::Command(CommandEvent {
            command:
                Command::BeginAttempt {
                    attempt_id,
                    runner,
                    mode,
                },
            ..
        })) = &s.last_event
        else {
            return Ok(None);
        };
        Ok(Some(DispatchAction {
            plan_id: s.plan_id.clone(),
            plan_digest: s.plan_digest.clone(),
            attempt_id: attempt_id.clone(),
            runner: runner.clone(),
            mode: *mode,
            committed_revision: s.revision,
        }))
    }
}

/// First-delivery action released only after trusted atomic admission succeeds.
/// Dropping it loses first delivery; recovery must reconcile the durable Starting state.
/// It cannot be cloned, deserialized, or reconstructed from a Snapshot.
/// INVARIANT: EXECUTION-FIRST-DISPATCH-01 — private construction and consuming dispatch.
/// ~~~compile_fail
/// fn duplicate(action: execution_lifecycle::DispatchAction) {
///     let _ = action.clone();
/// }
/// ~~~
#[derive(Debug)]
#[must_use = "dispatch once or reconcile the admitted attempt after recovery"]
pub struct DispatchAction {
    plan_id: PlanId,
    plan_digest: Digest,
    attempt_id: AttemptId,
    runner: Id,
    mode: ExecutionMode,
    committed_revision: u64,
}
impl DispatchAction {
    /// Invoke the trusted host dispatch function once, consuming first-delivery authority.
    /// Callback failure must be reconciled, never automatically retried.
    /// The host still enforces current stop/cancellation and runner capability gates.
    /// ref: tokio tokio/src/sync/oneshot.rs (Sender::send consumes self)
    pub fn dispatch<T>(self, dispatch: impl FnOnce(&Self) -> T) -> T {
        dispatch(&self)
    }
    /// Exact committed plan.
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    /// Exact committed content identity.
    pub fn plan_digest(&self) -> &Digest {
        &self.plan_digest
    }
    /// Exact committed attempt.
    pub fn attempt_id(&self) -> &AttemptId {
        &self.attempt_id
    }
    /// Runner selected at admission.
    pub fn runner(&self) -> &Id {
        &self.runner
    }
    /// Fixed test/real provenance.
    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }
    /// Revision at which this attempt was atomically admitted.
    pub fn committed_revision(&self) -> u64 {
        self.committed_revision
    }
}

/// Pure evaluation result. Directives never call a runner.
#[derive(Debug)]
pub struct Evaluation {
    /// Applied candidate, duplicate or stale command.
    pub outcome: EventOutcome,
    /// Suggested next action based on current reliable time.
    pub directive: Directive,
    /// Optional write; CAS conflicts require reread and reevaluation.
    pub transition: Option<Transition>,
}
