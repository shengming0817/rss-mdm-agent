use crate::{
    database::bounded_blob,
    execution::load_execution,
    journal::{authorize, decode, encode},
    *,
};
use execution_contract::{AttemptId, ProcessEvidence};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

impl Store {
    /// Persist bounded runner evidence in the existing journal. It cannot create an attempt,
    /// change authority, or manufacture lifecycle termination/effect observations.
    pub fn record_process(
        &mut self,
        scope: &Scope,
        facts: &ProcessEvidence,
        host: &impl Host,
    ) -> Result<(), Error> {
        self.check_scope(scope)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::database::ensure_current(&tx, &self.authority, self.limits)?;
        authorize(host, Access::RunnerFact, scope, None)?;
        let (_, execution, _) = load_execution(&tx, scope, self.limits)?;
        let attempt = execution
            .snapshot()
            .attempt
            .as_ref()
            .ok_or(Error::Conflict)?;
        let retained = facts
            .stdout
            .len()
            .checked_add(facts.stderr.len())
            .ok_or(Error::Capacity)? as u64;
        if facts.attempt_id != attempt.id
            || facts.plan_digest != *execution.plan().digest()
            || facts.runner != attempt.runner
            || retained > facts.total_output_bytes
            || retained > execution.plan().spec().budget.total_output_bytes
            || (facts.quiescent && !facts.finished)
        {
            return Err(Error::InvalidInput);
        }
        let bytes = encode(facts, self.limits.max_record_bytes)?;
        let old: Option<Vec<u8>> = tx
            .query_row(
                &format!(
                    "SELECT {} FROM process_evidence WHERE attempt_id=?1",
                    bounded_blob("body", self.limits.max_record_bytes)
                ),
                [facts.attempt_id.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(old) = old {
            let previous: ProcessEvidence = decode(&old, self.limits.max_record_bytes)?;
            if previous == *facts {
                return Ok(());
            }
            if previous.finished
                || previous.plan_digest != facts.plan_digest
                || previous.scope != facts.scope
                || previous.runner != facts.runner
                || previous.total_output_bytes > facts.total_output_bytes
                || !facts.stdout.starts_with(&previous.stdout)
                || !facts.stderr.starts_with(&previous.stderr)
            {
                return Err(Error::Conflict);
            }
        }
        tx.execute("INSERT INTO process_evidence(attempt_id,body) VALUES(?1,?2) ON CONFLICT(attempt_id) DO UPDATE SET body=excluded.body", params![facts.attempt_id.as_str(), bytes])?;
        tx.commit().map_err(|_| Error::OperationCommitUnknown)
    }
    /// Read evidence only under current result permission and the exact plan scope.
    pub fn process_evidence(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl Host,
    ) -> Result<Option<ProcessEvidence>, Error> {
        let tx = self.read(scope, Access::ReadResult, None, host)?;
        let bytes: Option<Vec<u8>> = tx.query_row(&format!("SELECT {} FROM process_evidence p JOIN attempts a ON a.attempt_id=p.attempt_id WHERE a.scope=?1 AND p.attempt_id=?2", bounded_blob("body", self.limits.max_record_bytes)), params![scope.key(), attempt.as_str()], |r| r.get(0)).optional()?;
        bytes
            .map(|b| decode(&b, self.limits.max_record_bytes))
            .transpose()
    }
}
