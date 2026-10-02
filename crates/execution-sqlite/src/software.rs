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
        Some(p) => {
            let program = plan.spec().execution.software_program().expect("program");
            let reboot = p.checkpoints.iter().any(|checkpoint| match checkpoint {
                execution_contract::SoftwareCheckpoint::End {
                    step,
                    phase,
                    process: Some(process),
                    ..
                } if !phase.is_observation() => {
                    program.steps[*step as usize].allow_reboot
                        && program
                            .invocation(*step as usize, *phase)
                            .is_some_and(|invocation| {
                                process.exit_code.is_some_and(|code| {
                                    invocation.exit_codes.reboot.contains(&code)
                                })
                            })
                }
                _ => false,
            });
            if reboot {
                return Ok(Some(SoftwareDiagnostic::RestartPending));
            }
            if p.complete(plan) {
                return Ok(Some(SoftwareDiagnostic::DesiredStateObserved));
            }
            let matched = p
                .checkpoints
                .iter()
                .rev()
                .find_map(|c| match c {
                    execution_contract::SoftwareCheckpoint::End {
                        step,
                        detected: Some(state),
                        ..
                    } => Some(program.satisfied(&program.steps[*step as usize], state)),
                    _ => None,
                })
                .unwrap_or(false);
            let activity_unknown = p.checkpoints.iter().any(|checkpoint| {
                matches!(
                    checkpoint,
                    execution_contract::SoftwareCheckpoint::End { phase, quiescent: false, .. }
                        if *phase != execution_contract::SoftwarePhase::Cleanup
                )
            }) || matches!(
                p.checkpoints.last(),
                Some(execution_contract::SoftwareCheckpoint::Begin { phase, .. })
                    if *phase != execution_contract::SoftwarePhase::Cleanup
            );
            if activity_unknown && !p.closed(plan) {
                SoftwareDiagnostic::DetectionUnavailable
            } else if matched && !p.closed(plan) {
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
