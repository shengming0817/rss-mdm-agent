use super::*;
use crate::materialize::Recipe;

pub(super) struct ProgramRun {
    pub(super) source: Arc<Artifacts>,
    pub(super) plan: FrozenExecution,
    pub(super) attempt: AttemptId,
    pub(super) runner: Id,
    pub(super) deadline: Instant,
    pub(super) output_limit: u64,
    pub(super) first_start: Option<u64>,
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) capture: Arc<Mutex<Option<ProcessEvidence>>>,
    pub(super) progress: Arc<progress::Progress>,
    pub(super) previous: Option<SoftwareProgress>,
    pub(super) ownership: Vec<SoftwareOwnership>,
}
pub(super) fn execute(input: ProgramRun) {
    let ProgramRun {
        source,
        plan,
        attempt,
        runner,
        deadline,
        output_limit,
        first_start,
        cancel,
        capture,
        progress,
        previous,
        mut ownership,
    } = input;
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
    let prior = journal.clone();
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
            let mut phases = std::collections::VecDeque::from([SoftwarePhase::Before]);
            let mut failed_mutation = false;
            let mut invocation_elapsed = std::collections::BTreeMap::<SoftwarePhase, u64>::new();
            let mut invocation_output = std::collections::BTreeMap::<SoftwarePhase, u64>::new();
            while let Some(phase) = phases.pop_front() {
                if failed_mutation && !phase.is_observation() && phase != SoftwarePhase::Cleanup {
                    continue;
                }
                let phase_started = Instant::now();
                let command = program.invocation(index, phase);
                let recorded = prior.checkpoints.iter().find_map(|c| match c {
                    SoftwareCheckpoint::End {
                        step,
                        phase: p,
                        process,
                        detected,
                        quiescent,
                        duration_ms,
                    } if *step as usize == index && *p == phase => Some((
                        process.as_deref().cloned(),
                        detected.clone(),
                        *quiescent,
                        *duration_ms,
                    )),
                    _ => None,
                });
                let replayed = recorded.is_some();
                let (facts, detected, quiet) = if let Some(recorded) = recorded {
                    if phase.is_observation() {
                        detector_elapsed = detector_elapsed.saturating_add(recorded.3);
                        detector_output = detector_output.saturating_add(
                            recorded.0.as_ref().map_or(0, |p| p.total_output_bytes),
                        );
                    }
                    (recorded.0, recorded.1, recorded.2)
                } else {
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
                    if let Some(command) = command {
                        let material = if !phase.is_observation() {
                            sources
                                .mutations
                                .get(&phase)
                                .map(Box::as_ref)
                                .ok_or(Error::Unbound)?
                        } else {
                            sources.detection.as_deref().ok_or(Error::Unbound)?
                        };
                        let elapsed = if !phase.is_observation() {
                            invocation_elapsed.get(&phase).copied().unwrap_or(0)
                        } else {
                            detector_elapsed
                        };
                        let consumed = if !phase.is_observation() {
                            invocation_output.get(&phase).copied().unwrap_or(0)
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
                            (&plan, &attempt, &runner),
                            command,
                            (index as u32, phase),
                            (invocation_deadline, cap, first.take()),
                            cancel.clone(),
                            before.clone(),
                        )?;
                        if phase.is_observation() {
                            detector_elapsed = detector_elapsed.saturating_add(
                                invoked.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                            );
                            detector_output =
                                detector_output.saturating_add(facts.total_output_bytes);
                        }
                        if !phase.is_observation() {
                            *invocation_elapsed.entry(phase).or_default() +=
                                invoked.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                            *invocation_output.entry(phase).or_default() +=
                                facts.total_output_bytes;
                        }
                        let detection = if !phase.is_observation() {
                            None
                        } else {
                            Some(invocation_detection(command, &facts))
                        };
                        let quiet = facts.quiescent;
                        (Some(facts), detection, quiet)
                    } else if phase.is_observation() {
                        // Native read-only receipt queries do not create a child process or invent
                        // an exit status. Their observation is recorded separately in this journal.
                        let context = program
                            .invocation(index, SoftwarePhase::Mutation)
                            .unwrap_or(&step.install);
                        if matches!(context.run_as, RunAs::User { .. }) {
                            if let Some(connection) = &source.delegate {
                                connection.verify_context(
                                    &context.run_as,
                                    &context.session_requirement,
                                )?;
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
                    }
                };
                if let Some(facts) = &facts {
                    if !replayed {
                        journal.output_bytes = journal
                            .output_bytes
                            .saturating_add(facts.total_output_bytes);
                    }
                    latest = facts.clone();
                }
                if !replayed {
                    journal.checkpoints.push(SoftwareCheckpoint::End {
                        duration_ms: phase_started
                            .elapsed()
                            .as_millis()
                            .min(u128::from(u64::MAX)) as u64,
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
                }
                let material = if !phase.is_observation() {
                    sources.mutations.get(&phase).map(Box::as_ref)
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
                if !quiet {
                    return Err(Error::OutcomeUnknown);
                }
                if let Some(state) = detected {
                    if matches!(state, SoftwareState::Unknown { .. }) {
                        return Err(Error::OutcomeUnknown);
                    }
                    if phase == SoftwarePhase::RemovalAfter
                        && !matches!(state, SoftwareState::Absent {})
                    {
                        return Err(Error::OutcomeUnknown);
                    }
                    let satisfied =
                        phase != SoftwarePhase::RemovalAfter && program.satisfied(step, &state);
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
                    if phase == SoftwarePhase::Before
                        && matches!(state, SoftwareState::Present { .. })
                    {
                        let managed = ownership
                            .iter()
                            .any(|o| o.step as usize == index && o.state == state);
                        if (program.intent == SoftwareOperation::Uninstall
                            && matches!(step.format, SoftwareFormat::DmgApp { .. })
                            && !managed)
                            || (step.existing != ExistingSoftware::AllowUserExisting && !managed)
                            || (program.intent == SoftwareOperation::Install
                                && !step.allow_downgrade
                                && !crate::software::native_detection::not_downgrade(step, &state))
                        {
                            return Err(Error::Denied);
                        }
                    }
                    if phase == SoftwarePhase::Before {
                        let mutations = program.mutation_phases(step, &state);
                        if mutations.is_empty() {
                            return Err(Error::Denied);
                        }
                        phases.extend(mutations);
                        phases.push_back(SoftwarePhase::After);
                    }
                    before = Some(state);
                } else if !phase.is_observation() {
                    let facts = facts.as_ref().ok_or(Error::OutcomeUnknown)?;
                    mutation_succeeded &= command
                        .ok_or(Error::InvalidInput)?
                        .succeeded(facts, step.allow_reboot);
                    failed_mutation |= !mutation_succeeded;
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
        latest.quiescent = journal.closed(&plan);
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
    (plan, attempt, runner): (&FrozenExecution, &AttemptId, &Id),
    invocation: &SoftwareInvocation,
    (step, phase): (u32, SoftwarePhase),
    (deadline, cap, first_start): (Instant, u64, Option<u64>),
    cancel: Arc<AtomicBool>,
    before: Option<SoftwareState>,
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
            before: before.clone(),
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
    let mut material = material.clone();
    if matches!(invocation.launch.stdin, StandardInput::Controlled { .. })
        && invocation.launch.interpreter.profile.id.as_str() == "native-software-worker"
    {
        material.controlled_input = Some(Arc::new(BeforeInput {
            digest: plan.digest().clone(),
            attempt: attempt.clone(),
            step,
            state: before.ok_or(Error::Denied)?,
        }));
    }
    let prepared =
        material.prepare_recipe(plan, attempt, Recipe::invocation(invocation), &control)?;
    runtime.block_on(run(
        prepared,
        (plan.clone(), attempt.clone(), runner.clone()),
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

pub(super) fn invocation_detection(
    invocation: &SoftwareInvocation,
    facts: &ProcessEvidence,
) -> SoftwareState {
    if invocation.launch.interpreter.profile.id.as_str() == "native-software-worker" {
        if facts.end == ProcessEnd::Exited
            && facts.exit_code == Some(0)
            && facts.quality == OutputQuality::Complete
        {
            if let Ok(result) = serde_json::from_slice::<SoftwareWorkerResult>(&facts.stdout) {
                if result.closed {
                    if let Some(state) = result.detected {
                        return state;
                    }
                }
            }
        }
        return SoftwareState::Unknown {
            reason: SoftwareDetectionFailure::Unavailable,
        };
    }
    script_detection(facts)
}

pub(crate) struct BeforeInput {
    pub(crate) digest: Digest,
    pub(crate) attempt: AttemptId,
    pub(crate) step: u32,
    pub(crate) state: SoftwareState,
}
impl crate::InputResolver for BeforeInput {
    fn resolve(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        reference: &VersionedRef,
        max_bytes: u64,
    ) -> Result<crate::InputBytes, Error> {
        if plan.digest() != &self.digest
            || attempt != &self.attempt
            || reference.id.as_str() != "software-before"
            || reference.revision.as_str() != self.step.to_string()
            || matches!(self.state, SoftwareState::Unknown { .. })
        {
            return Err(Error::Denied);
        }
        let bytes = serde_json::to_vec(&self.state).map_err(|_| Error::InvalidInput)?;
        if bytes.len() as u64 > max_bytes {
            return Err(Error::Capacity);
        }
        Ok(crate::InputBytes::new(bytes))
    }
}
