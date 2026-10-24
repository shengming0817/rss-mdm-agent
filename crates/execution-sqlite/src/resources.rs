//! Exact physical resource exclusion, shared by every authority in this installation.
use crate::{Error, Limits};
use execution_contract::{AttemptId, FrozenExecution};
use rusqlite::{params, Connection, OptionalExtension};

pub(crate) fn claim(
    conn: &Connection,
    plan: &FrozenExecution,
    attempt: &AttemptId,
) -> Result<(), Error> {
    for key in plan
        .exclusive_resource_keys()
        .map_err(|_| Error::InvalidInput)?
    {
        let held: Option<String> = conn
            .query_row(
                "SELECT attempt_id FROM resource_claims WHERE resource=?1",
                [&key],
                |r| r.get(0),
            )
            .optional()?;
        if held.as_deref().is_some_and(|a| a != attempt.as_str()) {
            return Err(Error::Busy);
        }
        conn.execute(
            "INSERT OR IGNORE INTO resource_claims VALUES(?1,?2)",
            params![key, attempt.as_str()],
        )?;
    }
    Ok(())
}
pub(crate) fn settle(
    conn: &Connection,
    plan: &FrozenExecution,
    next: &execution_lifecycle::Snapshot,
    limits: Limits,
) -> Result<(), Error> {
    if plan
        .exclusive_resource_keys()
        .map_err(|_| Error::InvalidInput)?
        .is_empty()
    {
        return Ok(());
    }
    if let Some(attempt) = next.attempt.as_ref().filter(|a| a.termination.is_some()) {
        let never = attempt.termination.as_ref().is_some_and(|v| {
            matches!(
                v.observation,
                execution_lifecycle::Observation::NeverDispatched { .. }
            )
        });
        let process_closed = plan.spec().execution.software_program().is_none()
            && matches!(
                attempt.termination.as_ref().map(|v| &v.observation),
                Some(
                    execution_lifecycle::Observation::Exited { .. }
                        | execution_lifecycle::Observation::Quiescent { .. }
                )
            );
        if never
            || process_closed
            || crate::software_progress::read(conn, &attempt.id, limits)?
                .is_some_and(|p| p.closed(plan))
        {
            conn.execute(
                "DELETE FROM resource_claims WHERE attempt_id=?1",
                [attempt.id.as_str()],
            )?;
        }
    }
    Ok(())
}

impl crate::Store {
    pub(crate) fn sql_resource_owner(
        &self,
        key: &str,
        host: &impl execution_app::JournalHost,
    ) -> Result<Option<execution_contract::RequestId>, Error> {
        crate::database::ensure_current(&self.conn, &self.authority, self.limits)?;
        if key.is_empty() || key.len() > 256 {
            return Err(Error::InvalidInput);
        }
        let request:Option<String>=self.conn.query_row("SELECT e.request_id FROM resource_claims r JOIN attempts a ON a.attempt_id=r.attempt_id JOIN executions e ON e.scope=a.scope WHERE r.resource=?1",[key],|r|r.get(0)).optional()?;
        let Some(request) = request else {
            return Ok(None);
        };
        let request = execution_contract::RequestId::new(request).map_err(|_| Error::Corrupt)?;
        let record = self.sql_execution_by_request(
            &request,
            execution_app::ExecutionAccess::RunnerFact,
            host,
        )?;
        if !record
            .execution
            .input()
            .exclusive_resource_keys()
            .map_err(|_| Error::Corrupt)?
            .iter()
            .any(|k| k == key)
        {
            return Err(Error::Corrupt);
        }
        Ok(Some(request))
    }
}
