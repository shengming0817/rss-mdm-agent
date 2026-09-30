use crate::{
    database::bounded_blob,
    execution::load_execution,
    journal::{authorize, decode, encode},
    *,
};
use execution_contract::{AttemptId, SoftwareProgress};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

/// Receipt constructible only after the existing journal commits the exact checkpoint.
/// A deserialized runner message cannot manufacture permission to cross a phase boundary.
pub struct CommittedSoftwareProgress(SoftwareProgress);
impl CommittedSoftwareProgress {
    /// Inspect the exact committed facts; this receipt grants no new business attempt.
    pub fn facts(&self) -> &SoftwareProgress {
        &self.0
    }
}
impl Store {
    /// Append software phase facts atomically to the original attempt, before physical dispatch.
    pub fn record_software_progress(
        &mut self,
        scope: &Scope,
        facts: &SoftwareProgress,
        host: &impl Host,
    ) -> Result<CommittedSoftwareProgress, Error> {
        self.check_scope(scope)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        authorize(host, Access::RunnerFact, scope, None)?;
        let (plan, execution, _) = load_execution(&tx, scope, self.limits)?;
        let attempt = execution
            .snapshot()
            .attempt
            .as_ref()
            .ok_or(Error::Conflict)?;
        if facts.attempt_id != attempt.id
            || facts.runner != attempt.runner
            || !facts.valid_for(&plan)
        {
            return Err(Error::InvalidInput);
        }
        let previous_count = if let Some(previous) = read(&tx, &attempt.id, self.limits)? {
            if !facts.extends(&previous) {
                return Err(Error::Conflict);
            }
            previous.checkpoints.len()
        } else {
            0
        };
        tx.execute("INSERT INTO software_progress(attempt_id,body) VALUES(?1,?2) ON CONFLICT(attempt_id) DO UPDATE SET body=excluded.body",
            params![attempt.id.as_str(), encode(facts, self.limits.max_record_bytes)?])?;
        ownership(&tx, &plan, facts, previous_count, self.limits)?;
        tx.commit().map_err(|_| Error::OperationCommitUnknown)?;
        Ok(CommittedSoftwareProgress(facts.clone()))
    }
    /// Read only protected provenance produced by completed installations in this journal.
    pub fn software_ownership(
        &self,
        scope: &Scope,
        host: &impl Host,
    ) -> Result<Vec<execution_contract::SoftwareOwnership>, Error> {
        let tx = self.read(scope, Access::RunnerFact, None, host)?;
        let (plan, _, _) = load_execution(&tx, scope, self.limits)?;
        let mut result = Vec::new();
        if let Some(program) = plan.spec().execution.software_program() {
            for (index, step) in program.steps.iter().enumerate() {
                if let Some(state) = owned(&tx, &step.ownership_key(program.intent), self.limits)? {
                    result.push(execution_contract::SoftwareOwnership {
                        step: index as u32,
                        state,
                    });
                }
            }
        }
        Ok(result)
    }
    /// Read the original attempt's history without creating or replaying an invocation.
    pub fn software_progress(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl Host,
    ) -> Result<Option<SoftwareProgress>, Error> {
        let tx = self.read(scope, Access::RunnerFact, None, host)?;
        let belongs: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM attempts WHERE scope=?1 AND attempt_id=?2)",
            params![scope.key(), attempt.as_str()],
            |r| r.get(0),
        )?;
        if !belongs {
            return Ok(None);
        }
        let value = read(&tx, attempt, self.limits)?;
        let (plan, _, _) = load_execution(&tx, scope, self.limits)?;
        if value.as_ref().is_some_and(|v| !v.valid_for(&plan)) {
            return Err(Error::Corrupt);
        }
        Ok(value)
    }
}
pub(crate) fn read(
    conn: &rusqlite::Connection,
    attempt: &AttemptId,
    limits: Limits,
) -> Result<Option<SoftwareProgress>, Error> {
    let body: Option<Vec<u8>> = conn
        .query_row(
            &format!(
                "SELECT {} FROM software_progress WHERE attempt_id=?1",
                bounded_blob("body", limits.max_record_bytes)
            ),
            [attempt.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    let value = body
        .map(|b| decode::<SoftwareProgress>(&b, limits.max_record_bytes))
        .transpose()?;
    if value.as_ref().is_some_and(|v| v.attempt_id != *attempt) {
        return Err(Error::Corrupt);
    }
    Ok(value)
}

fn owned(
    conn: &rusqlite::Connection,
    key: &str,
    limits: Limits,
) -> Result<Option<execution_contract::SoftwareState>, Error> {
    let bytes: Option<Vec<u8>> = conn
        .query_row(
            &format!(
                "SELECT {} FROM software_ownership WHERE resource=?1",
                bounded_blob("body", limits.max_record_bytes)
            ),
            [key],
            |r| r.get(0),
        )
        .optional()?;
    let value = bytes
        .map(|b| decode::<execution_contract::SoftwareState>(&b, limits.max_record_bytes))
        .transpose()?;
    if value
        .as_ref()
        .is_some_and(|v| !matches!(v, execution_contract::SoftwareState::Present { .. }))
    {
        return Err(Error::Corrupt);
    }
    Ok(value)
}
fn ownership(
    conn: &rusqlite::Connection,
    plan: &execution_contract::FrozenExecution,
    facts: &SoftwareProgress,
    from: usize,
    limits: Limits,
) -> Result<(), Error> {
    use execution_contract::{
        SoftwareCheckpoint as C, SoftwareOperation as O, SoftwarePhase as P, SoftwareState as S,
    };
    let program = plan
        .spec()
        .execution
        .software_program()
        .ok_or(Error::InvalidInput)?;
    for checkpoint in facts.checkpoints.iter().skip(from) {
        match checkpoint {
            C::End {
                step,
                phase: P::Before,
                detected: Some(state),
                ..
            } if !matches!(state, S::Unknown { .. }) => {
                let key = program.steps[*step as usize].ownership_key(program.intent);
                if owned(conn, &key, limits)?
                    .as_ref()
                    .is_some_and(|old| old != state)
                {
                    conn.execute("DELETE FROM software_ownership WHERE resource=?1", [key])?;
                }
            }
            C::Complete { step } => {
                let key = program.steps[*step as usize].ownership_key(program.intent);
                if program.intent == O::Detect {
                    continue;
                }
                let before = facts
                    .checkpoints
                    .iter()
                    .find_map(|c| match c {
                        C::End {
                            step: index,
                            phase: P::Before,
                            detected: Some(state),
                            ..
                        } if step == index => Some(state),
                        _ => None,
                    })
                    .ok_or(Error::Corrupt)?;
                let after = facts
                    .checkpoints
                    .iter()
                    .rev()
                    .find_map(|c| match c {
                        C::End {
                            step: index,
                            detected: Some(state),
                            ..
                        } if step == index => Some(state),
                        _ => None,
                    })
                    .ok_or(Error::Corrupt)?;
                if matches!(after, S::Absent {}) {
                    conn.execute("DELETE FROM software_ownership WHERE resource=?1", [key])?;
                } else if matches!(before, S::Absent {})
                    || owned(conn, &key, limits)?.as_ref() == Some(before)
                {
                    conn.execute("INSERT INTO software_ownership(resource,attempt_id,body) VALUES(?1,?2,?3) ON CONFLICT(resource) DO UPDATE SET attempt_id=excluded.attempt_id,body=excluded.body",
                        params![key, facts.attempt_id.as_str(), encode(after, limits.max_record_bytes)?])?;
                }
            }
            _ => (),
        }
    }
    Ok(())
}
