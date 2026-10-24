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
