use crate::{Error, Limits};
use execution_contract::{AttemptId, FrozenExecution, SoftwareDiagnostic};
use rusqlite::{params, Connection, OptionalExtension};

pub(crate) fn claim(
    conn: &Connection,
    plan: &FrozenExecution,
    attempt: &AttemptId,
) -> Result<(), Error> {
    if let Some(program) = plan.spec().execution.software_program() {
        for key in program.lock_keys() {
            let held: Option<String> = conn
                .query_row(
                    "SELECT attempt_id FROM software_claims WHERE resource=?1",
                    [&key],
                    |r| r.get(0),
                )
                .optional()?;
            if held.as_deref().is_some_and(|a| a != attempt.as_str()) {
                return Err(Error::Busy);
            }
            conn.execute(
                "INSERT OR IGNORE INTO software_claims VALUES(?1,?2)",
                params![key, attempt.as_str()],
            )?;
        }
    }
    Ok(())
}
pub(crate) fn settle(
    conn: &Connection,
    plan: &FrozenExecution,
    next: &execution_lifecycle::Snapshot,
    limits: Limits,
) -> Result<(), Error> {
    if plan.spec().execution.software_program().is_none() {
        return Ok(());
    }
    if let Some(attempt) = next.attempt.as_ref().filter(|a| a.termination.is_some()) {
        let never = attempt.termination.as_ref().is_some_and(|v| {
            matches!(
                v.observation,
                execution_lifecycle::Observation::NeverDispatched { .. }
            )
        });
        if never
            || crate::software_progress::read(conn, &attempt.id, limits)?
                .is_some_and(|p| p.closed(plan))
        {
            conn.execute(
                "DELETE FROM software_claims WHERE attempt_id=?1",
                [attempt.id.as_str()],
            )?;
        }
    }
    Ok(())
}
pub(crate) fn diagnostic(
    conn: &Connection,
    plan: &FrozenExecution,
    attempt: Option<&AttemptId>,
    limits: Limits,
) -> Result<Option<SoftwareDiagnostic>, Error> {
    if plan.spec().execution.software_program().is_none() {
        return Ok(None);
    }
    let progress = attempt
        .map(|a| crate::software_progress::read(conn, a, limits))
        .transpose()?
        .flatten();
    Ok(Some(match progress {
        Some(p) if p.complete(plan) => SoftwareDiagnostic::DesiredStateObserved,
        Some(p) => {
            let program = plan.spec().execution.software_program().expect("program");
            let matched = p
                .checkpoints
                .iter()
                .rev()
                .find_map(|c| match c {
                    execution_contract::SoftwareCheckpoint::End {
                        step,
                        detected: Some(state),
                        ..
                    } => Some(match (program.intent, state) {
                        (
                            execution_contract::SoftwareOperation::Uninstall,
                            execution_contract::SoftwareState::Absent {},
                        ) => true,
                        (_, execution_contract::SoftwareState::Present { version }) => {
                            version == &program.steps[*step as usize].version
                        }
                        _ => false,
                    }),
                    _ => None,
                })
                .unwrap_or(false);
            if matched && !p.closed(plan) {
                SoftwareDiagnostic::CleanupPending
            } else if matched {
                SoftwareDiagnostic::DesiredStateObserved
            } else {
                SoftwareDiagnostic::DetectionUnavailable
            }
        }
        None => SoftwareDiagnostic::AwaitingDetection,
    }))
}
