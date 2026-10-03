use crate::{
    database::bounded_blob,
    execution::load_execution,
    journal::{authorize, decode, encode},
    *,
};
use execution_contract::{AttemptId, SoftwareProgress};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

impl Store {
    /// Append software phase facts atomically to the original attempt, before physical dispatch.
    pub(crate) fn sql_record_software_progress(
        &mut self,
        scope: &Scope,
        facts: &SoftwareProgress,
        host: &impl JournalHost,
    ) -> Result<SoftwareProgress, Error> {
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
        if let Some(previous) = read(&tx, &attempt.id, self.limits)? {
            if !facts.extends(&previous) {
                return Err(Error::Conflict);
            }
        }
        tx.execute("INSERT INTO software_progress(attempt_id,body) VALUES(?1,?2) ON CONFLICT(attempt_id) DO UPDATE SET body=excluded.body",
            params![attempt.id.as_str(), encode(facts, self.limits.max_record_bytes)?])?;
        crate::software::settle(&tx, &plan, execution.snapshot(), self.limits)?;
        tx.commit().map_err(|_| Error::OperationCommitUnknown)?;
        Ok(facts.clone())
    }
    /// Read the original attempt's history without creating or replaying an invocation.
    pub(crate) fn sql_software_progress(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl JournalHost,
    ) -> Result<Option<SoftwareProgress>, Error> {
        let tx = self.read(scope, Access::RunnerFact, None, host)?;
        let belongs: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM attempts WHERE scope=?1 AND attempt_id=?2)",
            params![scope.interaction_subject().as_str(), attempt.as_str()],
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
