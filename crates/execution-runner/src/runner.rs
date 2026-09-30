use crate::{materialize::Materialized, output, platform, Artifacts};
use execution_app::{AuthorizedDispatch, DispatchOutcome, Error, ObservationStage, RunnerPort};
use execution_contract::*;
use execution_lifecycle::{DispatchAllowance, ExecutionMode, Observation, ObservationFacts};
#[cfg(all(test, target_os = "macos"))]
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub(crate) mod invocation;
mod program;
mod progress;

struct Record {
    plan: FrozenExecution,
    _materials: Option<Arc<Artifacts>>,
    cancel: Arc<AtomicBool>,
    facts: Arc<Mutex<Option<ProcessEvidence>>>,
    progress: Arc<progress::Progress>,
}
/// One bounded OS runner. Its inventory is supplied by trusted host code, never IPC DTOs.
pub struct NativeRunner {
    id: Id,
    artifacts: crate::MaterialRegistry,
    records: Mutex<BTreeMap<AttemptId, Record>>,
    capacity: usize,
}

impl NativeRunner {
    /// Use the assembly's material registry while retaining one process/observation owner.
    pub fn with_materials(
        id: Id,
        artifacts: crate::MaterialRegistry,
        capacity: usize,
    ) -> Result<Self, Error> {
        if capacity == 0 || capacity > 128 {
            return Err(Error::Configuration);
        }
        Ok(Self {
            id,
            artifacts,
            records: Mutex::new(BTreeMap::new()),
            capacity,
        })
    }
    /// Fixed local materialization inventory keyed by the full frozen plan digest.
    pub fn new(
        id: Id,
        artifacts: BTreeMap<String, Artifacts>,
        capacity: usize,
    ) -> Result<Self, Error> {
        if capacity == 0 || capacity > 128 || artifacts.len() > 4096 {
            return Err(Error::Configuration);
        }
        Ok(Self {
            id,
            artifacts: crate::MaterialRegistry::from_materials(artifacts, 4096)?,
            records: Mutex::new(BTreeMap::new()),
            capacity,
        })
    }
    /// Stop physical owners without changing journal outcomes or manufacturing quiescence.
    pub fn shutdown(&self) -> Result<(), Error> {
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            let records = self.records.lock().map_err(|_| Error::Unavailable)?;
            let mut ended = true;
            for record in records.values() {
                record.cancel.store(true, Ordering::Release);
                ended &= record
                    .facts
                    .lock()
                    .map_err(|_| Error::Unavailable)?
                    .as_ref()
                    .is_some_and(|f| f.finished);
            }
            drop(records);
            if ended {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err(Error::OutcomeUnknown);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    /// Execute one already journaled system-service dispatch in this helper's actual login
    /// session. No local admission, approval, credentials or journal is created here.
    pub(crate) fn execute_delegated(
        &self,
        _connection: &crate::host::SystemConnection<'_>,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        remaining_timeout_ms: u64,
        start_before_ms: u64,
        remaining_output_bytes: u64,
    ) -> Result<DispatchOutcome, Error> {
        if !matches!(plan.spec().request.authority, Authority::Enterprise { .. })
            || !matches!(plan.spec().run_as, RunAs::User { .. })
            || !matches!(
                plan.spec().session_requirement,
                SessionRequirement::ActiveUser { .. }
            )
            || remaining_timeout_ms == 0
            || remaining_timeout_ms > plan.spec().budget.total_timeout_ms
            || remaining_output_bytes == 0
            || remaining_output_bytes > plan.spec().budget.total_output_bytes
        {
            return Err(Error::Denied);
        }
        platform::identity(&plan.spec().run_as, &plan.spec().session_requirement)?;
        let now: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Clock)?
            .as_millis()
            .try_into()
            .map_err(|_| Error::Clock)?;
        if now < plan.spec().validity.not_before_unix_ms
            || now >= plan.spec().validity.expires_at_unix_ms
        {
            return Err(Error::Clock);
        }
        if let Some(record) = self
            .records
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(attempt)
        {
            return if record.plan.digest() == plan.digest() {
                Ok(DispatchOutcome::Accepted)
            } else {
                Err(Error::Conflict)
            };
        }
        self.launch_bounded(
            plan,
            attempt,
            DispatchAllowance {
                deadline_unix_ms: plan.spec().validity.expires_at_unix_ms,
                remaining_timeout_ms: remaining_timeout_ms
                    .min(plan.spec().validity.expires_at_unix_ms - now),
                remaining_output_bytes,
            },
            Some(start_before_ms),
            Vec::new(),
        )
    }
    #[cfg(test)]
    fn launch(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        allowance: DispatchAllowance,
    ) -> Result<DispatchOutcome, Error> {
        self.launch_owned(plan, attempt, allowance, Vec::new())
    }
    fn launch_owned(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        allowance: DispatchAllowance,
        ownership: Vec<SoftwareOwnership>,
    ) -> Result<DispatchOutcome, Error> {
        let start_before = if matches!(plan.spec().request.initiator, Initiator::Backend { .. }) {
            let expiry = plan
                .spec()
                .validity
                .expires_at_unix_ms
                .checked_sub(plan.spec().budget.total_timeout_ms)
                .ok_or(Error::InvalidInput)?;
            let remaining = allowance
                .remaining_timeout_ms
                .saturating_sub(allowance.deadline_unix_ms.saturating_sub(expiry));
            Some(
                crate::host::monotonic_millis()?
                    .checked_add(remaining)
                    .ok_or(Error::Clock)?,
            )
        } else {
            None
        };
        self.launch_bounded(plan, attempt, allowance, start_before, ownership)
    }
    fn launch_bounded(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        allowance: DispatchAllowance,
        start_before: Option<u64>,
        ownership: Vec<SoftwareOwnership>,
    ) -> Result<DispatchOutcome, Error> {
        let received = Instant::now();
        let mut records = self.records.lock().map_err(|_| Error::Unavailable)?;
        if records.contains_key(attempt) {
            return Err(Error::Conflict);
        }
        if records.len() >= self.capacity {
            return Err(Error::Capacity);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let mut preparing = rejected(plan, attempt, &self.id, ProcessEnd::Unknown);
        preparing.scope = ProcessScope::Preparing {};
        preparing.finished = false;
        preparing.quiescent = false;
        preparing.quality = OutputQuality::Partial;
        let facts = Arc::new(Mutex::new(Some(preparing)));
        let progress = Arc::new(progress::Progress::new());
        let source = self.artifacts.get(plan.digest().as_str())?;
        records.insert(
            attempt.clone(),
            Record {
                _materials: source.clone(),
                plan: plan.clone(),
                cancel: cancel.clone(),
                facts: facts.clone(),
                progress: progress.clone(),
            },
        );
        drop(records); // Never hold the admission lock during filesystem or input I/O.
        let remaining = allowance
            .remaining_timeout_ms
            .saturating_sub(received.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
        if remaining == 0
            || allowance.remaining_output_bytes == 0
            || allowance.remaining_output_bytes > plan.spec().budget.total_output_bytes
            || cancel.load(Ordering::Acquire)
        {
            publish(
                &facts,
                rejected(
                    plan,
                    attempt,
                    &self.id,
                    if cancel.load(Ordering::Acquire) {
                        ProcessEnd::Cancelled
                    } else {
                        ProcessEnd::TimedOut
                    },
                ),
            );
            return Ok(DispatchOutcome::NeverDispatched);
        }
        let deadline = Instant::now() + Duration::from_millis(remaining);
        let failed_spawn = failed(plan, attempt, &self.id, ProcessFailureKind::Runtime);
        let failure_slot = facts.clone();
        let attempt = attempt.clone();
        let plan = plan.clone();
        let id = self.id.clone();
        let spawned = std::thread::Builder::new()
            .name("rss-execution-owner".into())
            .spawn(move || {
                if plan.spec().execution.software_program().is_some() {
                    match source {
                        Some(source) => program::execute(
                            source,
                            plan,
                            attempt,
                            id,
                            deadline,
                            allowance.remaining_output_bytes,
                            start_before,
                            cancel,
                            facts,
                            progress,
                            None,
                            ownership,
                        ),
                        None => publish(
                            &facts,
                            failed(&plan, &attempt, &id, ProcessFailureKind::Unbound),
                        ),
                    }
                    return;
                }
                if let Some(source) = source.as_ref().filter(|source| source.delegate.is_some()) {
                    run_delegated(
                        source,
                        &plan,
                        &attempt,
                        &id,
                        allowance.remaining_output_bytes,
                        deadline,
                        start_before,
                        &cancel,
                        &facts,
                    );
                    return;
                }
                let preparation_deadline = start_before
                    .and_then(|before| {
                        crate::host::monotonic_millis().ok().map(|now| {
                            Instant::now() + Duration::from_millis(before.saturating_sub(now))
                        })
                    })
                    .map_or(deadline, |start| start.min(deadline));
                let control = crate::software::PreparationControl {
                    deadline: preparation_deadline,
                    cancelled: cancel.clone(),
                };
                let prepared = source
                    .ok_or(Error::Unbound)
                    .and_then(|a| a.prepare(&plan, &attempt, &control));
                let materialized = match prepared {
                    Ok(value) => value,
                    Err(error) => {
                        let failure = if cancel.load(Ordering::Acquire) {
                            rejected(&plan, &attempt, &id, ProcessEnd::Cancelled)
                        } else if Instant::now() >= preparation_deadline {
                            rejected(&plan, &attempt, &id, ProcessEnd::TimedOut)
                        } else {
                            failed(&plan, &attempt, &id, classify(error))
                        };
                        publish(&facts, failure);
                        return;
                    }
                };
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    publish(
                        &facts,
                        failed(&plan, &attempt, &id, ProcessFailureKind::Runtime),
                    );
                    return;
                };
                runtime.block_on(run(
                    materialized,
                    plan,
                    attempt,
                    id,
                    (allowance.remaining_output_bytes, deadline),
                    cancel,
                    Captures {
                        start_before,
                        process: facts,
                    },
                    None,
                ));
            });
        Ok(if spawned.is_ok() {
            DispatchOutcome::Accepted
        } else {
            publish(&failure_slot, failed_spawn);
            DispatchOutcome::NeverDispatched
        })
    }
}
impl Drop for NativeRunner {
    fn drop(&mut self) {
        if let Ok(records) = self.records.lock() {
            for record in records.values() {
                record.cancel.store(true, Ordering::Release)
            }
        }
    }
}
impl RunnerPort for NativeRunner {
    fn recover_software_progress(
        &self,
        plan: &FrozenExecution,
        previous: &SoftwareProgress,
    ) -> Result<Option<SoftwareProgress>, Error> {
        if self
            .records
            .lock()
            .map_err(|_| Error::Unavailable)?
            .contains_key(&previous.attempt_id)
        {
            return Ok(None);
        }
        let Some(SoftwareCheckpoint::Begin { step, phase }) = previous.checkpoints.last() else {
            return Ok(None);
        };
        let Some(source) = self.artifacts.get(plan.digest().as_str())? else {
            return Ok(None);
        };
        let sources = source.program.get(*step as usize).ok_or(Error::Unbound)?;
        let material = if *phase == SoftwarePhase::Mutation {
            sources.mutation.as_deref()
        } else {
            sources.detection.as_deref()
        };
        let Some(connection) = material.and_then(|m| m.delegate.as_ref()) else {
            return Ok(None);
        };
        let Ok(crate::helper::Reply::Evidence {
            process: Some(mut facts),
        }) = connection.exchange(crate::helper::Command::InvocationEvidence {
            input: Box::new(plan.spec().clone()),
            attempt: previous.attempt_id.clone(),
            step: *step,
            phase: *phase,
        })
        else {
            return Ok(None);
        };
        if !facts.finished {
            return Ok(None);
        }
        if facts.content_digest != *plan.digest()
            || facts.attempt_id != previous.attempt_id
            || facts.runner.as_str() != "native-user-helper"
            || facts.stdout.len().saturating_add(facts.stderr.len()) as u64
                > plan.spec().budget.total_output_bytes
        {
            return Err(Error::Denied);
        }
        facts.runner = self.id.clone();
        let detected = if *phase == SoftwarePhase::Mutation {
            None
        } else {
            Some(program::script_detection(&facts))
        };
        let mut next = previous.clone();
        next.output_bytes = next.output_bytes.saturating_add(facts.total_output_bytes);
        next.checkpoints.push(SoftwareCheckpoint::End {
            step: *step,
            phase: *phase,
            quiescent: facts.quiescent,
            process: Some(facts),
            detected,
        });
        Ok(Some(next))
    }
    fn resume_software(&self, resume: execution_app::SoftwareResume) -> Result<(), Error> {
        resume.resume(|plan, journal, allowance, ownership| {
            let mut records = self.records.lock().map_err(|_| Error::Unavailable)?;
            if let Some(record) = records.get(&journal.attempt_id) {
                return if record.plan.digest() == plan.digest() {
                    Ok(())
                } else {
                    Err(Error::Conflict)
                };
            }
            if records.len() >= self.capacity {
                return Err(Error::Capacity);
            }
            if !journal.valid_for(&plan)
                || journal.runner != self.id
                || journal.complete(&plan)
                || !matches!(
                    journal.checkpoints.last(),
                    Some(SoftwareCheckpoint::Complete { .. })
                )
            {
                return Err(Error::Denied);
            }
            let Some(source) = self.artifacts.get(plan.digest().as_str())? else {
                return Ok(());
            };
            source.inspect(&plan)?;
            let cancel = Arc::new(AtomicBool::new(false));
            let mut initial = rejected(&plan, &journal.attempt_id, &self.id, ProcessEnd::Unknown);
            initial.scope = ProcessScope::Preparing {};
            initial.finished = false;
            initial.quiescent = false;
            let facts = Arc::new(Mutex::new(Some(initial)));
            let progress = Arc::new(progress::Progress::new());
            let attempt = journal.attempt_id.clone();
            records.insert(
                attempt.clone(),
                Record {
                    plan: plan.clone(),
                    _materials: Some(source.clone()),
                    cancel: cancel.clone(),
                    facts: facts.clone(),
                    progress: progress.clone(),
                },
            );
            let runner = self.id.clone();
            let output_limit = plan.spec().budget.total_output_bytes.min(
                journal
                    .output_bytes
                    .saturating_add(allowance.remaining_output_bytes),
            );
            let timeout = allowance.remaining_timeout_ms;
            let key = attempt.clone();
            if std::thread::Builder::new()
                .name("rss-execution-resume".into())
                .spawn(move || {
                    program::execute(
                        source,
                        plan,
                        attempt,
                        runner,
                        Instant::now() + Duration::from_millis(timeout),
                        output_limit,
                        None,
                        cancel,
                        facts,
                        progress,
                        Some(journal),
                        ownership,
                    );
                })
                .is_err()
            {
                records.remove(&key);
                return Err(Error::Unavailable);
            }
            Ok(())
        })
    }
    fn software_progress(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
    ) -> Result<Option<SoftwareProgress>, Error> {
        let records = self.records.lock().map_err(|_| Error::Unavailable)?;
        match records.get(attempt) {
            Some(record) if record.plan.digest() == plan.digest() => record.progress.pending(),
            Some(_) => Err(Error::Denied),
            None => Ok(None),
        }
    }
    fn acknowledge_software_progress(
        &self,
        receipt: execution_app::CommittedSoftwareProgress,
    ) -> Result<(), Error> {
        let records = self.records.lock().map_err(|_| Error::Unavailable)?;
        let record = records
            .get(&receipt.facts().attempt_id)
            .ok_or(Error::NotFound)?;
        if record.plan.digest() != &receipt.facts().content_digest {
            return Err(Error::Denied);
        }
        record.progress.acknowledge(receipt)
    }
    fn id(&self) -> Id {
        self.id.clone()
    }
    fn mode(&self) -> ExecutionMode {
        ExecutionMode::Real
    }
    fn dispatch(&self, permit: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
        let allowance = permit.allowance();
        let ownership = permit.software_ownership();
        permit.dispatch(|plan, action| {
            if action.mode() != ExecutionMode::Real
                || matches!(plan.spec().request.authority, Authority::Test { .. })
                || action.runner() != &self.id
                || action.content_digest() != plan.digest()
                || action.request_id() != &plan.spec().request.request_id
            {
                return Err(Error::Denied);
            }
            self.launch_owned(plan, action.attempt_id(), allowance, ownership)
        })
    }
    fn acknowledge_capture(
        &self,
        plan: &FrozenExecution,
        facts: &ProcessEvidence,
    ) -> Result<(), Error> {
        if !facts.finished || facts.content_digest != *plan.digest() {
            return Err(Error::Denied);
        }
        let mut records = self.records.lock().map_err(|_| Error::Unavailable)?;
        let mut delegate = None;
        if let Some(record) = records.get(&facts.attempt_id) {
            if record.plan.digest() != plan.digest()
                || record
                    .facts
                    .lock()
                    .map_err(|_| Error::Unavailable)?
                    .as_ref()
                    != Some(facts)
            {
                return Err(Error::Conflict);
            }
            if plan.spec().execution.software_program().is_none()
                || record.progress.complete(plan)?
            {
                if plan.spec().execution.software_program().is_none() {
                    delegate = record
                        ._materials
                        .as_ref()
                        .and_then(|source| source.delegate.clone());
                }
                records.remove(&facts.attempt_id);
            }
        }
        drop(records);
        if let Some(connection) = delegate {
            if !matches!(facts.scope, ProcessScope::Delegated { .. }) {
                let mut delegated = facts.clone();
                delegated.runner = Id::new("native-user-helper").expect("constant");
                let _ = connection.exchange(crate::helper::Command::Acknowledge {
                    input: Box::new(plan.spec().clone()),
                    process: Box::new(delegated),
                });
            }
        }
        Ok(())
    }
    fn stop(&self, plan: &FrozenExecution, attempt: &AttemptId) -> Result<(), Error> {
        let records = self.records.lock().map_err(|_| Error::Unavailable)?;
        let record = records.get(attempt).ok_or(Error::Unavailable)?;
        if record.plan.digest() != plan.digest() {
            return Err(Error::Denied);
        }
        record.cancel.store(true, Ordering::Release);
        Ok(())
    }
    fn evidence(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
    ) -> Result<Option<ProcessEvidence>, Error> {
        let records = self.records.lock().map_err(|_| Error::Unavailable)?;
        let Some(record) = records.get(attempt) else {
            drop(records);
            if plan.spec().execution.software_program().is_some() {
                return Ok(None);
            }
            if let Some(source) = self.artifacts.get(plan.digest().as_str())? {
                if let Some(connection) = &source.delegate {
                    if let Ok(crate::helper::Reply::Evidence {
                        process: Some(mut facts),
                    }) = connection.exchange(crate::helper::Command::Evidence {
                        input: Box::new(plan.spec().clone()),
                        attempt: attempt.clone(),
                    }) {
                        if facts.content_digest != *plan.digest()
                            || facts.attempt_id != *attempt
                            || facts.runner.as_str() != "native-user-helper"
                            || facts.stdout.len().saturating_add(facts.stderr.len()) as u64
                                > plan.spec().budget.total_output_bytes
                        {
                            return Err(Error::Denied);
                        }
                        facts.runner = self.id.clone();
                        return Ok(Some(*facts));
                    }
                }
            }
            return Ok(None);
        };
        if record.plan.digest() != plan.digest() {
            return Err(Error::Denied);
        }
        let facts = record.facts.lock().map_err(|_| Error::Unavailable)?.clone();
        Ok(facts)
    }
    fn observe(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        stage: ObservationStage,
        now: u64,
    ) -> Result<Option<ObservationFacts>, Error> {
        let capture = self.evidence(plan, attempt)?;
        let Some(facts) = capture else {
            return Ok(None);
        };
        if !facts.finished || !facts.quiescent {
            return Ok(None);
        }
        // Exit is not independent effect evidence. No generic script observer can assert Satisfied.
        if stage == ObservationStage::Assessment {
            return Ok(None);
        }
        let never = matches!(facts.scope, ProcessScope::NotStarted {});
        if !never && facts.exit_code.is_none() {
            return Ok(None);
        }
        Ok(Some(ObservationFacts {
            request_id: plan.spec().request.request_id.clone(),
            content_digest: plan.digest().clone(),
            attempt_id: attempt.clone(),
            observed_at_unix_ms: now,
            evidence: EvidenceRef {
                reference: VersionedRef {
                    id: Id::new(attempt.as_str()).map_err(|_| Error::InvalidInput)?,
                    revision: Id::new("process-final").unwrap(),
                },
                kind: if never {
                    EvidenceKind::StateObserved
                } else {
                    EvidenceKind::ProcessExited
                },
                runner: self.id.clone(),
            },
            observation: if never {
                Observation::NeverDispatched {
                    total_output_bytes: 0,
                }
            } else {
                Observation::Exited {
                    exit_code: facts.exit_code.unwrap(),
                    total_output_bytes: facts.total_output_bytes,
                }
            },
        }))
    }
}
#[cfg(all(test, target_os = "macos"))]
fn now() -> Result<u64, Error> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Clock)?
            .as_millis(),
    )
    .map_err(|_| Error::Clock)
}
fn rejected(
    plan: &FrozenExecution,
    attempt: &AttemptId,
    id: &Id,
    end: ProcessEnd,
) -> ProcessEvidence {
    ProcessEvidence {
        content_digest: plan.digest().clone(),
        attempt_id: attempt.clone(),
        runner: id.clone(),
        scope: ProcessScope::NotStarted {},
        finished: true,
        exit_code: None,
        end,
        failure_kind: ProcessFailureKind::None,
        quiescent: true,
        stdout: vec![],
        stderr: vec![],
        total_output_bytes: 0,
        quality: OutputQuality::Failed,
    }
}
pub(crate) fn classify(error: Error) -> ProcessFailureKind {
    match error {
        Error::Denied => ProcessFailureKind::Denied,
        Error::Unbound => ProcessFailureKind::Unbound,
        Error::Capability | Error::Degraded => ProcessFailureKind::Capability,
        Error::Unsupported => ProcessFailureKind::Unsupported,
        Error::InvalidInput | Error::Configuration => ProcessFailureKind::InvalidInput,
        Error::Capacity => ProcessFailureKind::Capacity,
        Error::Conflict => ProcessFailureKind::Conflict,
        Error::NotFound
        | Error::Clock
        | Error::Unavailable
        | Error::Storage
        | Error::UnsupportedSchema { .. }
        | Error::OutcomeUnknown
        | Error::ConfirmationUnknown => ProcessFailureKind::Unavailable,
    }
}
fn failed(
    plan: &FrozenExecution,
    attempt: &AttemptId,
    id: &Id,
    kind: ProcessFailureKind,
) -> ProcessEvidence {
    let mut facts = rejected(plan, attempt, id, ProcessEnd::Rejected);
    facts.failure_kind = kind;
    facts
}
fn fault(facts: &mut ProcessEvidence, kind: ProcessFailureKind) {
    if facts.failure_kind == ProcessFailureKind::None {
        facts.failure_kind = kind;
    }
}
fn fail_running(
    facts: &mut ProcessEvidence,
    stop_at: &mut Option<Instant>,
    kind: ProcessFailureKind,
    now: Instant,
) {
    fault(facts, kind);
    if stop_at.is_none() {
        facts.end = ProcessEnd::Unknown;
    }
    stop_at.get_or_insert(now); // Neither a later pipe error nor repeated wait errors renew shutdown time.
}
fn run_delegated(
    source: &Artifacts,
    plan: &FrozenExecution,
    attempt: &AttemptId,
    runner: &Id,
    cap: u64,
    deadline: Instant,
    start_before: Option<u64>,
    cancel: &AtomicBool,
    slot: &Mutex<Option<ProcessEvidence>>,
) {
    use crate::helper::{Command as C, Reply as R};
    let connection = source.delegate.as_ref().expect("delegate selected");
    let mut unknown = rejected(plan, attempt, runner, ProcessEnd::Unknown);
    unknown.scope = ProcessScope::Delegated {
        subject: Id::new(&connection.context().subject).expect("validated subject"),
        session: connection.context().binding.clone(),
    };
    unknown.quiescent = false;
    unknown.quality = OutputQuality::Partial;
    unknown.failure_kind = ProcessFailureKind::Unavailable;
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    if remaining == 0
        || cancel.load(Ordering::Acquire)
        || start_before
            .is_none_or(|before| crate::host::monotonic_millis().map_or(true, |now| now >= before))
    {
        publish(
            slot,
            rejected(
                plan,
                attempt,
                runner,
                if cancel.load(Ordering::Acquire) {
                    ProcessEnd::Cancelled
                } else {
                    ProcessEnd::TimedOut
                },
            ),
        );
        return;
    }
    // Exactly one Start send. A transport error is never permission to send it again.
    let _ = connection.exchange(C::Start {
        input: Box::new(plan.spec().clone()),
        interpreter: source.interpreter.clone(),
        content: source.content.clone(),
        attempt: attempt.clone(),
        timeout_ms: remaining,
        start_before_ms: start_before.unwrap_or(0),
        output_bytes: cap,
    });
    let finish_unknown = |mut facts: ProcessEvidence| {
        if matches!(facts.scope, ProcessScope::Preparing {}) {
            facts.scope = ProcessScope::Delegated {
                subject: Id::new(&connection.context().subject).expect("validated subject"),
                session: connection.context().binding.clone(),
            };
        }
        facts.finished = true;
        facts.quiescent = false;
        facts.quality = OutputQuality::Partial;
        fault(&mut facts, ProcessFailureKind::Unavailable);
        publish(slot, facts);
    };
    let mut stop_sent = false;
    let mut ending = deadline + Duration::from_secs(6);
    loop {
        if !stop_sent && (cancel.load(Ordering::Acquire) || Instant::now() >= deadline) {
            stop_sent = true;
            ending = Instant::now() + Duration::from_secs(6);
            let _ = connection.exchange(C::Stop {
                input: Box::new(plan.spec().clone()),
                attempt: attempt.clone(),
            });
        }
        if let Ok(R::Evidence {
            process: Some(facts),
        }) = connection.exchange(C::Evidence {
            input: Box::new(plan.spec().clone()),
            attempt: attempt.clone(),
        }) {
            if facts.content_digest != *plan.digest()
                || facts.attempt_id != *attempt
                || facts.runner.as_str() != "native-user-helper"
                || (facts.stdout.len() as u64).saturating_add(facts.stderr.len() as u64) > cap
            {
                finish_unknown(unknown);
                return;
            }
            let mut facts = *facts;
            facts.runner = runner.clone();
            let finished = facts.finished;
            unknown = facts.clone();
            publish(slot, facts);
            if finished {
                return;
            }
        }
        if Instant::now() >= ending {
            finish_unknown(unknown);
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn publish(slot: &Mutex<Option<ProcessEvidence>>, facts: ProcessEvidence) {
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(facts)
    }
}
struct Captures {
    start_before: Option<u64>,
    process: Arc<Mutex<Option<ProcessEvidence>>>,
}
async fn run(
    mut materialized: Materialized,
    plan: FrozenExecution,
    attempt: AttemptId,
    id: Id,
    (cap, deadline): (u64, Instant),
    cancel: Arc<AtomicBool>,
    captures: Captures,
    invocation: Option<SoftwareInvocation>,
) {
    let recipe = invocation
        .as_ref()
        .map(crate::materialize::Recipe::invocation)
        .unwrap_or_else(|| crate::materialize::Recipe::root(&plan));
    let Captures {
        start_before,
        process: shared,
    } = captures;
    let mut command = tokio::process::Command::new(&materialized.interpreter);
    command
        .args(&materialized.args)
        .env_clear()
        .envs(&materialized.env)
        .stdin(if materialized.stdin.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if let Err(error) = materialized.configure(command.as_std_mut()) {
        publish(&shared, failed(&plan, &attempt, &id, classify(error)));
        return;
    }
    let mut owner = match platform::Owner::prepare(&mut command) {
        Ok(owner) => owner,
        Err(error) => {
            publish(&shared, failed(&plan, &attempt, &id, classify(error)));
            return;
        }
    };
    if cancel.load(Ordering::Acquire)
        || Instant::now() >= deadline
        || start_before
            .is_some_and(|before| crate::host::monotonic_millis().map_or(true, |now| now >= before))
    {
        publish(
            &shared,
            rejected(
                &plan,
                &attempt,
                &id,
                if cancel.load(Ordering::Acquire) {
                    ProcessEnd::Cancelled
                } else {
                    ProcessEnd::TimedOut
                },
            ),
        );
        return;
    }
    let spawn_deadline = start_before
        .and_then(|before| {
            crate::host::monotonic_millis()
                .ok()
                .map(|now| Instant::now() + Duration::from_millis(before.saturating_sub(now)))
        })
        .map_or(deadline, |start| start.min(deadline));
    let mut child = match platform::spawn(&mut command, &mut owner, &cancel, spawn_deadline).await {
        Ok(child) => child,
        Err(_) => {
            let end = if cancel.load(Ordering::Acquire) {
                ProcessEnd::Cancelled
            } else if Instant::now() >= spawn_deadline {
                ProcessEnd::TimedOut
            } else {
                ProcessEnd::Rejected
            };
            let mut facts = rejected(&plan, &attempt, &id, end);
            if end == ProcessEnd::Rejected {
                facts.failure_kind = ProcessFailureKind::Spawn;
            }
            publish(&shared, facts);
            return;
        }
    };
    let mut facts = ProcessEvidence {
        content_digest: plan.digest().clone(),
        attempt_id: attempt,
        runner: id,
        scope: owner.scope(),
        finished: false,
        exit_code: None,
        end: ProcessEnd::Unknown,
        failure_kind: ProcessFailureKind::None,
        quiescent: false,
        stdout: vec![],
        stderr: vec![],
        total_output_bytes: 0,
        quality: OutputQuality::Partial,
    };
    let publish = |facts: &ProcessEvidence| {
        if let Ok(mut slot) = shared.lock() {
            *slot = Some(facts.clone())
        }
    };
    publish(&facts);
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let input = materialized.stdin.take();
    let stdin = child.stdin.take();
    let mut writer = tokio::spawn(async move {
        if let (Some(mut pipe), Some(bytes)) = (stdin, input) {
            pipe.write_all(bytes.bytes()).await?;
            pipe.shutdown().await?;
        }
        Ok::<(), std::io::Error>(())
    });
    let mut input_done = false;
    let mut input_failed = false;
    let mut out = [0u8; 8192];
    let mut err = [0u8; 8192];
    let mut out_done = false;
    let mut err_done = false;
    let mut stop_at = None;
    let mut killed = false;
    let mut exited = false;
    let mut next_session_check = Instant::now();
    loop {
        let clock = Instant::now();
        if stop_at.is_none() {
            let cause = if cancel.load(Ordering::Acquire) {
                Some(ProcessEnd::Cancelled)
            } else if clock >= deadline {
                Some(ProcessEnd::TimedOut)
            } else if facts.total_output_bytes >= cap {
                Some(ProcessEnd::OutputLimit)
            } else {
                None
            };
            if let Some(cause) = cause {
                facts.end = cause;
                stop_at = Some(clock);
                owner.stop();
            }
        }
        if stop_at.is_none() && clock >= next_session_check {
            next_session_check = clock + Duration::from_millis(200);
            if matches!(*recipe.session, SessionRequirement::ActiveUser { .. })
                && platform::identity(recipe.run_as, recipe.session).is_err()
            {
                fail_running(&mut facts, &mut stop_at, ProcessFailureKind::Unbound, clock);
                owner.stop();
            }
        }
        if let Some(stop) = stop_at {
            if !killed && clock.duration_since(stop) >= Duration::from_millis(200) {
                owner.terminate();
                let _ = child.start_kill();
                killed = true;
            }
            if clock.duration_since(stop) >= Duration::from_secs(2) {
                break;
            }
        }
        if !exited {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exited = true;
                    facts.exit_code = status.code();
                    if stop_at.is_none() {
                        facts.end = ProcessEnd::Exited;
                        stop_at = Some(clock)
                    }
                    owner.terminate();
                    killed = true;
                }
                Err(_) => {
                    fail_running(
                        &mut facts,
                        &mut stop_at,
                        ProcessFailureKind::Supervision,
                        clock,
                    );
                }
                Ok(None) => {}
            }
        }
        if exited && out_done && err_done && input_done {
            break;
        }
        let mut event = None;
        tokio::select! {
            result=&mut writer,if !input_done=>{
                input_done=true;
                if !matches!(result,Ok(Ok(()))){input_failed=true;fail_running(&mut facts,&mut stop_at,ProcessFailureKind::InputDelivery,Instant::now());owner.stop();}
            },
            count=stdout.read(&mut out),if !out_done=>{event=Some((true,count));},
            count=stderr.read(&mut err),if !err_done=>{event=Some((false,count));},
            _=tokio::time::sleep(Duration::from_millis(10))=>{},
        }
        if let Some((is_out, count)) = event {
            match count {
                Ok(0) => {
                    if is_out {
                        out_done = true
                    } else {
                        err_done = true
                    }
                }
                Ok(n) => {
                    let keep =
                        (cap.saturating_sub((facts.stdout.len() + facts.stderr.len()) as u64)
                            as usize)
                            .min(n);
                    facts.total_output_bytes = facts.total_output_bytes.saturating_add(n as u64);
                    if is_out {
                        facts.stdout.extend_from_slice(&out[..keep])
                    } else {
                        facts.stderr.extend_from_slice(&err[..keep])
                    }
                }
                Err(_) => {
                    fail_running(
                        &mut facts,
                        &mut stop_at,
                        ProcessFailureKind::Capture,
                        Instant::now(),
                    );
                    if is_out {
                        out_done = true
                    } else {
                        err_done = true
                    }
                }
            }
            publish(&facts);
        }
    }
    if !input_done {
        writer.abort();
        let _ = writer.await;
        input_failed = true;
        fault(&mut facts, ProcessFailureKind::InputDelivery);
    }
    owner.terminate();
    let _ = child.start_kill();
    if !exited {
        if let Ok(Ok(status)) = tokio::time::timeout(Duration::from_secs(1), child.wait()).await {
            facts.exit_code = status.code();
            exited = true
        }
    }
    if !exited {
        fault(&mut facts, ProcessFailureKind::Supervision);
    }
    facts.finished = true;
    facts.quiescent = exited && out_done && err_done && owner.quiescent();
    facts.quality = if facts.total_output_bytes > cap || facts.end == ProcessEnd::OutputLimit {
        OutputQuality::Truncated
    } else if !out_done || !err_done {
        OutputQuality::Partial
    } else if input_failed
        || facts.failure_kind != ProcessFailureKind::None
        || facts.end != ProcessEnd::Exited
        || facts.exit_code != Some(0)
    {
        OutputQuality::Failed
    } else {
        output::quality(&facts.stdout, &facts.stderr, recipe.launch.output)
    };
    if recipe.launch.interpreter.profile.id.as_str() == "native-osquery-info-v1"
        && facts.quality == OutputQuality::Complete
    {
        let valid = serde_json::from_slice::<serde_json::Value>(&facts.stdout)
            .ok()
            .is_some_and(|v| {
                v.as_array().is_some_and(|rows| {
                    rows.len() == 1
                        && rows[0].as_object().is_some_and(|row| {
                            row.len() == 1
                                && row.get("version").is_some_and(serde_json::Value::is_string)
                        })
                })
            });
        if !facts.stderr.is_empty() || !valid {
            facts.quality = OutputQuality::Failed;
        }
    }
    if facts.quality == OutputQuality::Failed
        && facts.end == ProcessEnd::Exited
        && facts.exit_code == Some(0)
    {
        fault(&mut facts, ProcessFailureKind::OutputValidation);
    }
    publish(&facts);
}
#[cfg(test)]
mod tests;

#[cfg(all(test, windows))]
mod windows_tests;

#[cfg(test)]
pub(crate) mod support;
