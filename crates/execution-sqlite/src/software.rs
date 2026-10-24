use crate::{Error, Limits};
use execution_contract::{AttemptId, FrozenExecution, SoftwareDiagnostic};
use rusqlite::Connection;

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
