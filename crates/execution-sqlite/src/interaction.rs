use crate::{execution::plan_audit, journal::*, *};
use execution_contract::Decision;
use execution_interaction::{self as interaction, Command, Interaction, Reference, Spec};
use rusqlite::params;

impl Store {
    /// Open an interaction bound to an existing execution scope. Answers are never approvals.
    pub fn open_interaction(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        spec: &Spec,
        host: &impl Host,
    ) -> Result<CommitOutcome, Error> {
        if spec.subject != scope.interaction_subject() {
            return Err(Error::Denied);
        }
        let w = match self.start(
            op,
            scope,
            OperationKind::OpenInteraction,
            spec,
            Access::Interact,
            host,
        )? {
            Start::Replay(r) => return Ok(CommitOutcome::AlreadyCommitted(r)),
            Start::New(w) => w,
        };
        let plan = load_plan(&w.tx, scope, w.limits)?;
        let state = Interaction::open(spec.clone(), w.now, w.limits.interaction)
            .map_err(|_| Error::Configuration)?;
        w.tx.execute(
            "INSERT INTO interactions VALUES(?1,?2,?3,0,1)",
            params![spec.id.as_str(), scope.key(), w.bounded(state.snapshot())?],
        )?;
        let audit = plan_audit(
            &w,
            &plan,
            Decision::Proposed {},
            AuditReason::InteractionOpened,
        );
        w.finish(Outcome::Changed, 0, None, audit)
    }
    /// Resolve answer/cancel/expiry against the current protected state inside one write transaction.
    /// No public method accepts the core's freely constructible Transition.
    pub fn apply_interaction(
        &mut self,
        op: &OperationRequestId,
        scope: &Scope,
        id: &Reference,
        command: &Command,
        host: &impl Host,
    ) -> Result<CommitOutcome, Error> {
        let mut w = match self.start(
            op,
            scope,
            OperationKind::Interaction,
            &(id, command),
            Access::Interact,
            host,
        )? {
            Start::Replay(r) => return Ok(CommitOutcome::AlreadyCommitted(r)),
            Start::New(w) => w,
        };
        let plan = load_plan(&w.tx, scope, w.limits)?;
        let state = load(&w.tx, scope, id, w.limits)?;
        host.authorize(AccessRequest {
            access: Access::Interact,
            scope,
            consumer: None,
            interaction: Some((&state.snapshot().spec, command)),
        })?;
        w.refresh_time(host)?;
        let command_id = match command {
            Command::Answer { id, .. } | Command::Cancel { id } => Some(id),
            Command::CheckExpiry {} => None,
        };
        if let Some(id) = command_id {
            w.tx.execute(
                "INSERT INTO interaction_keys VALUES(?1,?2,?3)",
                params![id.as_str(), scope.key(), op.as_str()],
            )?;
        }
        let mut audit = plan_audit(
            &w,
            &plan,
            Decision::Proposed {},
            AuditReason::InteractionOpened,
        );
        let evaluation = match state.evaluate(command.clone(), w.now) {
            Ok(value) => value,
            Err(error) => {
                audit.reason = AuditReason::InteractionError(error);
                return w.finish(Outcome::Rejected, state.snapshot().revision, None, audit);
            }
        };
        let outcome = match evaluation.outcome {
            interaction::Outcome::Answered => Outcome::Answered,
            interaction::Outcome::Cancelled => Outcome::Cancelled,
            interaction::Outcome::Expired => Outcome::Expired,
            interaction::Outcome::Duplicate => Outcome::Duplicate,
            interaction::Outcome::Late => Outcome::Late,
            interaction::Outcome::NotDue => Outcome::NotDue,
        };
        audit.reason = AuditReason::Interaction(evaluation.outcome);
        let revision = if let Some(t) = evaluation.transition {
            let changed = w.tx.execute("UPDATE interactions SET snapshot=?1,revision=?2,reserve=0 WHERE id=?3 AND scope=?4 AND revision=?5",
                params![w.bounded(t.next.snapshot())?, integer(t.next.snapshot().revision)?, id.as_str(), scope.key(), integer(t.expected_revision)?])?;
            if changed != 1 {
                return Err(Error::Conflict);
            }
            t.next.snapshot().revision
        } else {
            state.snapshot().revision
        };
        w.finish(outcome, revision, None, audit)
    }
    /// Restore a pending or terminal interaction without changing the execution task.
    pub fn interaction(
        &self,
        scope: &Scope,
        id: &Reference,
        host: &impl Host,
    ) -> Result<Interaction, Error> {
        let tx = self.read(scope, Access::ReadResult, None, host)?;
        load(&tx, scope, id, self.limits)
    }
}
fn load(
    conn: &rusqlite::Connection,
    scope: &Scope,
    id: &Reference,
    limits: Limits,
) -> Result<Interaction, Error> {
    let (bytes, revision): (Vec<u8>, u64) = conn.query_row(
        "SELECT snapshot,revision FROM interactions WHERE id=?1 AND scope=?2",
        params![id.as_str(), scope.key()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let state = Interaction::decode(&bytes, limits.interaction).map_err(|_| Error::Corrupt)?;
    if state.snapshot().revision != revision
        || state.snapshot().spec.id != *id
        || state.snapshot().spec.subject != scope.interaction_subject()
    {
        return Err(Error::Corrupt);
    }
    Ok(state)
}
