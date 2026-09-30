use super::*;
use crate::materialize::Recipe;

pub(super) fn execute(
    source: Arc<Artifacts>,
    plan: FrozenExecution,
    attempt: AttemptId,
    runner: Id,
    deadline: Instant,
    output_limit: u64,
    first_start: Option<u64>,
    cancel: Arc<AtomicBool>,
    capture: Arc<Mutex<Option<ProcessEvidence>>>,
    progress: Arc<progress::Progress>,
    previous: Option<SoftwareProgress>,
    mut ownership: Vec<SoftwareOwnership>,
) {
    let started = Instant::now();
    let mut journal = previous.unwrap_or_else(|| SoftwareProgress {
        attempt_id: attempt.clone(),
        content_digest: plan.digest().clone(),
        runner: runner.clone(),
        checkpoints: Vec::new(),
        elapsed_ms: 0,
        output_bytes: 0,
    });
    let elapsed = journal.elapsed_ms;
    let completed = journal
        .checkpoints
        .iter()
        .filter(|c| matches!(c, SoftwareCheckpoint::Complete { .. }))
        .count();
    let mut latest = rejected(&plan, &attempt, &runner, ProcessEnd::Unknown);
    latest.quiescent = false;
    let result = (|| -> Result<(), Error> {
        let program = plan
            .spec()
            .execution
            .software_program()
            .ok_or(Error::InvalidInput)?;
        if source.program.len() != program.steps.len() {
            return Err(Error::Unbound);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?;
        let mut first = first_start;
        for (index, step) in program.steps.iter().enumerate().skip(completed) {
            let sources = &source.program[index];
            let _leases = sources
                .files
                .iter()
                .map(|(path, artifact)| crate::materialize::verify_material(path, &artifact.sha256))
                .collect::<Result<Vec<_>, _>>()?;
            let mut detector_elapsed = 0u64;
            let mut detector_output = 0u64;
            let mut before = None;
            let mut step_quiescent = true;
            let mut mutation_succeeded = true;
            for phase in [
                SoftwarePhase::Before,
                SoftwarePhase::Mutation,
                SoftwarePhase::After,
            ] {
                if cancel.load(Ordering::Acquire)
                    || Instant::now() >= deadline
                    || journal.output_bytes >= output_limit
                {
                    return Err(Error::OutcomeUnknown);
                }
                journal.checkpoints.push(SoftwareCheckpoint::Begin {
                    step: index as u32,
                    phase,
                });
                commit(
                    &mut journal,
                    (started, elapsed),
                    &progress,
                    deadline,
                    &cancel,
                )?;
                if first.is_some_and(|before| {
                    crate::host::monotonic_millis().map_or(true, |now| now >= before)
                }) {
                    return Err(Error::Clock);
                }
                let command = program.invocation(index, phase);
                let (facts, detected, quiet) = if let Some(command) = command {
                    let material = if phase == SoftwarePhase::Mutation {
                        sources.mutation.as_deref().ok_or(Error::Unbound)?
                    } else {
                        sources.detection.as_deref().ok_or(Error::Unbound)?
                    };
                    let elapsed = if phase == SoftwarePhase::Mutation {
                        0
                    } else {
                        detector_elapsed
                    };
                    let consumed = if phase == SoftwarePhase::Mutation {
                        0
                    } else {
                        detector_output
                    };
                    let remaining = command.timeout_ms.saturating_sub(elapsed);
                    let cap = command
                        .output_bytes
                        .saturating_sub(consumed)
                        .min(output_limit.saturating_sub(journal.output_bytes));
                    if remaining == 0 || cap == 0 {
                        return Err(Error::Capacity);
                    }
                    let invoked = Instant::now();
                    let invocation_deadline =
                        deadline.min(invoked + Duration::from_millis(remaining));
                    let facts = invoke(
                        &runtime,
                        material,
                        &plan,
                        &attempt,
                        &runner,
                        command,
                        index as u32,
                        phase,
                        invocation_deadline,
                        cap,
                        first.take(),
                        cancel.clone(),
                    )?;
                    if phase != SoftwarePhase::Mutation {
                        detector_elapsed = detector_elapsed.saturating_add(
                            invoked.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                        );
                        detector_output = detector_output.saturating_add(facts.total_output_bytes);
                    }
                    let detection = if phase == SoftwarePhase::Mutation {
                        None
                    } else {
                        Some(script_detection(&facts))
                    };
                    let quiet = facts.quiescent;
                    (Some(facts), detection, quiet)
                } else if phase != SoftwarePhase::Mutation {
                    // Native read-only receipt queries do not create a child process or invent
                    // an exit status. Their observation is recorded separately in this journal.
                    let context = program
                        .invocation(index, SoftwarePhase::Mutation)
                        .unwrap_or(&step.install);
                    if matches!(context.run_as, RunAs::User { .. }) {
                        if let Some(connection) = &source.delegate {
                            connection
                                .verify_context(&context.run_as, &context.session_requirement)?;
                        } else {
                            crate::platform::identity(
                                &context.run_as,
                                &context.session_requirement,
                            )?;
                        }
                    }
                    if cancel.load(Ordering::Acquire)
                        || Instant::now() >= deadline
                        || first.is_some_and(|end| {
                            crate::host::monotonic_millis().map_or(true, |now| now >= end)
                        })
                    {
                        return Err(Error::Clock);
                    }
                    first = None;
                    (
                        None,
                        Some(crate::software::native_detection(
                            &step.detection,
                            &context.run_as,
                        )),
                        true,
                    )
                } else {
                    return Err(Error::InvalidInput);
                };
                if let Some(facts) = &facts {
                    journal.output_bytes = journal
                        .output_bytes
                        .saturating_add(facts.total_output_bytes);
                    latest = facts.clone();
                }
                journal.checkpoints.push(SoftwareCheckpoint::End {
                    step: index as u32,
                    phase,
                    process: facts.clone().map(Box::new),
                    detected: detected.clone(),
                    quiescent: quiet,
                });
                commit(
                    &mut journal,
                    (started, elapsed),
                    &progress,
                    deadline,
                    &cancel,
                )?;
                let material = if phase == SoftwarePhase::Mutation {
                    sources.mutation.as_deref()
                } else {
                    sources.detection.as_deref()
                };
                if let Some(connection) = material.and_then(|m| m.delegate.as_ref()) {
                    let _ = connection.exchange(crate::helper::Command::InvocationAck {
                        input: Box::new(plan.spec().clone()),
                        attempt: attempt.clone(),
                        step: index as u32,
                        phase,
                    });
                }
                step_quiescent &= quiet;
                if !quiet && phase != SoftwarePhase::Mutation {
                    return Err(Error::OutcomeUnknown);
                }
                if let Some(state) = detected {
                    if matches!(state, SoftwareState::Unknown { .. }) {
                        return Err(Error::OutcomeUnknown);
                    }
                    let satisfied = match program.intent {
                        SoftwareOperation::Install => {
                            matches!(&state, SoftwareState::Present { version } if *version == step.version)
                        }
                        SoftwareOperation::Uninstall => matches!(state, SoftwareState::Absent {}),
                        SoftwareOperation::Detect => true,
                    };
                    if satisfied && step_quiescent && mutation_succeeded {
                        journal
                            .checkpoints
                            .push(SoftwareCheckpoint::Complete { step: index as u32 });
                        commit(
                            &mut journal,
                            (started, elapsed),
                            &progress,
                            deadline,
                            &cancel,
                        )?;
                        let own = before.as_ref().is_some_and(|b| {
                            matches!(b, SoftwareState::Absent {})
                                || ownership
                                    .iter()
                                    .any(|o| o.step as usize == index && &o.state == b)
                        });
                        if own && program.intent == SoftwareOperation::Install {
                            let key = step.ownership_key(program.intent);
                            for (future, other) in program.steps.iter().enumerate().skip(index + 1)
                            {
                                if other.ownership_key(program.intent) == key {
                                    ownership.retain(|o| o.step as usize != future);
                                    ownership.push(SoftwareOwnership {
                                        step: future as u32,
                                        state: state.clone(),
                                    });
                                }
                            }
                        }
                        break;
                    }
                    if phase == SoftwarePhase::After {
                        return Err(Error::OutcomeUnknown);
                    }
                    if matches!(state, SoftwareState::Present { .. }) {
                        let managed = ownership
                            .iter()
                            .any(|o| o.step as usize == index && o.state == state);
                        if (step.existing != ExistingSoftware::AllowUserExisting && !managed)
                            || (program.intent == SoftwareOperation::Install
                                && !step.allow_downgrade
                                && !crate::software::native_detection::not_downgrade(step, &state))
                        {
                            return Err(Error::Denied);
                        }
                    }
                    before = Some(state);
                } else if phase == SoftwarePhase::Mutation {
                    let facts = facts.as_ref().ok_or(Error::OutcomeUnknown)?;
                    mutation_succeeded = facts.end == ProcessEnd::Exited
                        && facts.exit_code == Some(0)
                        && facts.failure_kind == ProcessFailureKind::None;
                    if before.is_none() {
                        return Err(Error::OutcomeUnknown);
                    }
                }
            }
        }
        Ok(())
    })();
    // Retain the last observed root exit even when the overall sequence is unknown.
    latest.finished = true;
    latest.total_output_bytes = journal.output_bytes;
    if result.is_ok() && latest.exit_code.is_none() {
        latest.quality = OutputQuality::Complete;
        latest.failure_kind = ProcessFailureKind::None;
    }
    if let Err(error) = result {
        latest.quiescent = false;
        latest.quality = OutputQuality::Partial;
        fault(&mut latest, classify(error));
    }
    publish(&capture, latest);
}
fn commit(
    facts: &mut SoftwareProgress,
    (started, elapsed): (Instant, u64),
    progress: &progress::Progress,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), Error> {
    facts.elapsed_ms =
        elapsed.saturating_add(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
    progress.checkpoint(facts.clone(), deadline, cancel)
}
pub(super) fn script_detection(facts: &ProcessEvidence) -> SoftwareState {
    if facts.end == ProcessEnd::Exited
        && facts.exit_code == Some(0)
        && facts.quality == OutputQuality::Complete
    {
        if let Ok(state) = serde_json::from_slice(&facts.stdout) {
            return state;
        }
    }
    SoftwareState::Unknown {
        reason: SoftwareDetectionFailure::Unavailable,
    }
}
fn invoke(
    runtime: &tokio::runtime::Runtime,
    material: &Artifacts,
    plan: &FrozenExecution,
    attempt: &AttemptId,
    runner: &Id,
    invocation: &SoftwareInvocation,
    step: u32,
    phase: SoftwarePhase,
    deadline: Instant,
    cap: u64,
    first_start: Option<u64>,
    cancel: Arc<AtomicBool>,
) -> Result<ProcessEvidence, Error> {
    if let Some(connection) = &material.delegate {
        use crate::helper::{Command, Reply};
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        // A failed send/ack is ambiguous. Never resend Invoke; ask about this original key.
        let submitted = connection.exchange(Command::Invoke {
            input: Box::new(plan.spec().clone()),
            attempt: attempt.clone(),
            step,
            phase,
            interpreter: material.interpreter.clone(),
            content: material.content.clone(),
            timeout_ms: remaining,
            output_bytes: cap,
            first_start,
        });
        if let Err(error @ (Error::Denied | Error::Capacity)) = submitted {
            let mut facts = rejected(plan, attempt, runner, ProcessEnd::Rejected);
            facts.finished = true;
            facts.quiescent = true;
            facts.failure_kind = classify(error);
            return Ok(facts);
        }
        let mut latest = rejected(plan, attempt, runner, ProcessEnd::Unknown);
        latest.scope = ProcessScope::Delegated {
            subject: Id::new(&connection.context().subject).map_err(|_| Error::Unbound)?,
            session: connection.context().binding.clone(),
        };
        latest.quiescent = false;
        latest.quality = OutputQuality::Partial;
        let mut stop_sent = false;
        loop {
            if !stop_sent && (cancel.load(Ordering::Acquire) || Instant::now() >= deadline) {
                stop_sent = true;
                let _ = connection.exchange(Command::InvocationStop {
                    input: Box::new(plan.spec().clone()),
                    attempt: attempt.clone(),
                    step,
                    phase,
                });
            }
            if let Ok(Reply::Evidence {
                process: Some(mut facts),
            }) = connection.exchange(Command::InvocationEvidence {
                input: Box::new(plan.spec().clone()),
                attempt: attempt.clone(),
                step,
                phase,
            }) {
                if facts.content_digest != *plan.digest()
                    || facts.attempt_id != *attempt
                    || facts.runner.as_str() != "native-user-helper"
                    || facts.stdout.len().saturating_add(facts.stderr.len()) as u64 > cap
                {
                    return Err(Error::Denied);
                }
                facts.runner = runner.clone();
                latest = *facts;
                if latest.finished {
                    return Ok(latest);
                }
            }
            if Instant::now() >= deadline + Duration::from_secs(6) {
                latest.finished = true;
                latest.quiescent = false;
                latest.quality = OutputQuality::Partial;
                return Ok(latest);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let slot = Arc::new(Mutex::new(None));
    let control = crate::software::PreparationControl {
        deadline,
        cancelled: cancel.clone(),
    };
    let prepared =
        material.prepare_recipe(plan, attempt, Recipe::invocation(invocation), &control)?;
    runtime.block_on(run(
        prepared,
        plan.clone(),
        attempt.clone(),
        runner.clone(),
        (cap, deadline),
        cancel,
        Captures {
            start_before: first_start,
            process: slot.clone(),
        },
        Some(invocation.clone()),
    ));
    let facts = slot
        .lock()
        .map_err(|_| Error::Unavailable)?
        .clone()
        .ok_or(Error::OutcomeUnknown)?;
    Ok(facts)
}
