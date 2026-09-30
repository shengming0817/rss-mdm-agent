use crate::{
    database::bounded_blob,
    execution::load_execution,
    journal::{authorize, decode, encode},
    *,
};
use execution_contract::{AttemptId, ProcessEvidence, ProcessSummary};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

pub(crate) fn capture(
    conn: &Connection,
    attempt: &AttemptId,
    limits: Limits,
) -> Result<Option<ProcessEvidence>, Error> {
    let output =
        usize::try_from(limits.input.max_output_bytes).map_err(|_| Error::Configuration)?;
    let sql = format!(
        "SELECT {},{},{} FROM process_evidence WHERE attempt_id=?1",
        bounded_blob("body", limits.max_record_bytes),
        bounded_blob("stdout", output),
        bounded_blob("stderr", output)
    );
    let row: Option<(Vec<u8>, Vec<u8>, Vec<u8>)> = conn
        .query_row(&sql, [attempt.as_str()], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?;
    row.map(|(body, stdout, stderr)| {
        let mut facts: ProcessEvidence = decode(&body, limits.max_record_bytes)?;
        if facts.attempt_id != *attempt
            || !facts.stdout.is_empty()
            || !facts.stderr.is_empty()
            || stdout.len().saturating_add(stderr.len()) > output
            || ((stdout.len() + stderr.len()) as u64) > facts.total_output_bytes
        {
            return Err(Error::Corrupt);
        }
        facts.stdout = stdout;
        facts.stderr = stderr;
        Ok(facts)
    })
    .transpose()
}
pub(crate) fn summary(
    conn: &Connection,
    attempt: &AttemptId,
    limits: Limits,
) -> Result<Option<ProcessSummary>, Error> {
    let body: Option<Vec<u8>> = conn
        .query_row(
            &format!(
                "SELECT {} FROM process_evidence WHERE attempt_id=?1",
                bounded_blob("body", limits.max_record_bytes)
            ),
            [attempt.as_str()],
            |r| r.get(0),
        )
        .optional()?;
    body.map(|bytes| {
        let facts: ProcessEvidence = decode(&bytes, limits.max_record_bytes)?;
        if facts.attempt_id != *attempt || !facts.stdout.is_empty() || !facts.stderr.is_empty() {
            return Err(Error::Corrupt);
        }
        Ok(facts.summary())
    })
    .transpose()
}
impl Store {
    /// Save process facts and raw byte BLOBs atomically in the existing attempt namespace.
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
            || facts.content_digest != *execution.input().digest()
            || facts.runner != attempt.runner
            || retained > facts.total_output_bytes
            || retained > execution.input().spec().budget.total_output_bytes
            || (facts.quiescent && !facts.finished)
        {
            return Err(Error::InvalidInput);
        }
        if let Some(previous) = capture(&tx, &facts.attempt_id, self.limits)? {
            if previous == *facts {
                return Ok(());
            }
            let incomplete = previous.finished
                && !previous.quiescent
                && previous.end == execution_contract::ProcessEnd::Unknown
                && previous.quality == execution_contract::OutputQuality::Partial;
            if (previous.failure_kind != execution_contract::ProcessFailureKind::None
                && !(incomplete
                    && previous.failure_kind
                        == execution_contract::ProcessFailureKind::Unavailable)
                && previous.failure_kind != facts.failure_kind)
                || (previous.finished && !incomplete)
                || previous
                    .exit_code
                    .is_some_and(|exit| facts.exit_code != Some(exit))
                || previous.content_digest != facts.content_digest
                || (previous.scope != facts.scope
                    && !matches!(
                        previous.scope,
                        execution_contract::ProcessScope::Preparing {}
                            | execution_contract::ProcessScope::Delegated { .. }
                    ))
                || previous.runner != facts.runner
                || previous.total_output_bytes > facts.total_output_bytes
                || !facts.stdout.starts_with(&previous.stdout)
                || !facts.stderr.starts_with(&previous.stderr)
            {
                return Err(Error::Conflict);
            }
        }
        let mut metadata = facts.clone();
        metadata.stdout.clear();
        metadata.stderr.clear();
        let body = encode(&metadata, self.limits.max_record_bytes)?;
        tx.execute("INSERT INTO process_evidence(attempt_id,body,stdout,stderr) VALUES(?1,?2,?3,?4) ON CONFLICT(attempt_id) DO UPDATE SET body=excluded.body,stdout=excluded.stdout,stderr=excluded.stderr",params![facts.attempt_id.as_str(),body,&facts.stdout,&facts.stderr])?;
        tx.commit().map_err(|_| Error::OperationCommitUnknown)
    }
    /// Raw capture requires privileged audit access, never ordinary result permission.
    pub fn process_evidence(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl Host,
    ) -> Result<Option<ProcessEvidence>, Error> {
        self.read_process(scope, attempt, Access::ReadAudit, host)
    }
    /// Internal owner reconciliation reads its previously committed facts, never creating a permit.
    pub fn runner_evidence(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        host: &impl Host,
    ) -> Result<Option<ProcessEvidence>, Error> {
        self.read_process(scope, attempt, Access::RunnerFact, host)
    }
    fn read_process(
        &self,
        scope: &Scope,
        attempt: &AttemptId,
        access: Access,
        host: &impl Host,
    ) -> Result<Option<ProcessEvidence>, Error> {
        let tx = self.read(scope, access, None, host)?;
        let belongs: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM attempts WHERE scope=?1 AND attempt_id=?2)",
            params![scope.key(), attempt.as_str()],
            |r| r.get(0),
        )?;
        if !belongs {
            return Ok(None);
        }
        let facts = capture(&tx, attempt, self.limits)?;
        let (plan, _, _) = load_execution(&tx, scope, self.limits)?;
        if facts
            .as_ref()
            .is_some_and(|f| &f.content_digest != plan.digest())
        {
            return Err(Error::Corrupt);
        }
        Ok(facts)
    }
}
