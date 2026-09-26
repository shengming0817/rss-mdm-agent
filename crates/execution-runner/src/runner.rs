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

struct Record {
    plan: FrozenPlan,
    cancel: Arc<AtomicBool>,
    facts: Arc<Mutex<Option<ProcessEvidence>>>,
}
/// One bounded OS runner. Its inventory is supplied by trusted host code, never IPC DTOs.
pub struct NativeRunner {
    id: Id,
    artifacts: BTreeMap<String, Arc<Artifacts>>,
    records: Mutex<BTreeMap<AttemptId, Record>>,
    capacity: usize,
}
impl NativeRunner {
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
            artifacts: artifacts
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect(),
            records: Mutex::new(BTreeMap::new()),
            capacity,
        })
    }
    fn launch(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        allowance: DispatchAllowance,
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
        records.insert(
            attempt.clone(),
            Record {
                plan: plan.clone(),
                cancel: cancel.clone(),
                facts: facts.clone(),
            },
        );
        drop(records); // Never hold the admission lock during filesystem or input I/O.
        let source = self.artifacts.get(plan.digest().as_str()).cloned();
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
        let failed_spawn = rejected(plan, attempt, &self.id, ProcessEnd::Rejected);
        let failure_slot = facts.clone();
        let attempt = attempt.clone();
        let plan = plan.clone();
        let id = self.id.clone();
        let spawned = std::thread::Builder::new()
            .name("rss-execution-owner".into())
            .spawn(move || {
                let prepared = source
                    .ok_or(Error::Unbound)
                    .and_then(|a| a.prepare(&plan, &attempt));
                let materialized = match prepared {
                    Ok(value) => value,
                    Err(_) => {
                        publish(&facts, rejected(&plan, &attempt, &id, ProcessEnd::Rejected));
                        return;
                    }
                };
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    publish(&facts, rejected(&plan, &attempt, &id, ProcessEnd::Rejected));
                    return;
                };
                runtime.block_on(run(
                    materialized,
                    plan,
                    attempt,
                    id,
                    (allowance.remaining_output_bytes, deadline),
                    cancel,
                    facts,
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
    fn id(&self) -> Id {
        self.id.clone()
    }
    fn mode(&self) -> ExecutionMode {
        ExecutionMode::Real
    }
    fn dispatch(&self, permit: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
        let allowance = permit.allowance();
        permit.dispatch(|plan, action| {
            if action.mode() != ExecutionMode::Real
                || matches!(plan.spec().request.authority, Authority::Test { .. })
                || action.runner() != &self.id
                || action.plan_digest() != plan.digest()
                || action.plan_id() != &plan.spec().plan_id
            {
                return Err(Error::Denied);
            }
            self.launch(plan, action.attempt_id(), allowance)
        })
    }
    fn acknowledge_capture(&self, plan: &FrozenPlan, facts: &ProcessEvidence) -> Result<(), Error> {
        if !facts.finished || facts.plan_digest != *plan.digest() {
            return Err(Error::Denied);
        }
        let mut records = self.records.lock().map_err(|_| Error::Unavailable)?;
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
            records.remove(&facts.attempt_id);
        }
        Ok(())
    }
    fn stop(&self, plan: &FrozenPlan, attempt: &AttemptId) -> Result<(), Error> {
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
        plan: &FrozenPlan,
        attempt: &AttemptId,
    ) -> Result<Option<ProcessEvidence>, Error> {
        let records = self.records.lock().map_err(|_| Error::Unavailable)?;
        let Some(record) = records.get(attempt) else {
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
        plan: &FrozenPlan,
        attempt: &AttemptId,
        stage: ObservationStage,
        now: u64,
    ) -> Result<Option<ObservationFacts>, Error> {
        let Some(facts) = self.evidence(plan, attempt)? else {
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
            plan_id: plan.spec().plan_id.clone(),
            plan_digest: plan.digest().clone(),
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
fn rejected(plan: &FrozenPlan, attempt: &AttemptId, id: &Id, end: ProcessEnd) -> ProcessEvidence {
    ProcessEvidence {
        plan_digest: plan.digest().clone(),
        attempt_id: attempt.clone(),
        runner: id.clone(),
        scope: ProcessScope::NotStarted {},
        finished: true,
        exit_code: None,
        end,
        quiescent: true,
        stdout: vec![],
        stderr: vec![],
        total_output_bytes: 0,
        quality: OutputQuality::Failed,
    }
}
fn publish(slot: &Mutex<Option<ProcessEvidence>>, facts: ProcessEvidence) {
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(facts)
    }
}
async fn run(
    mut materialized: Materialized,
    plan: FrozenPlan,
    attempt: AttemptId,
    id: Id,
    (cap, deadline): (u64, Instant),
    cancel: Arc<AtomicBool>,
    shared: Arc<Mutex<Option<ProcessEvidence>>>,
) {
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
    if materialized.configure(command.as_std_mut()).is_err() {
        publish(
            &shared,
            rejected(&plan, &attempt, &id, ProcessEnd::Rejected),
        );
        return;
    }
    let Ok(mut owner) = platform::Owner::prepare(&mut command) else {
        publish(
            &shared,
            rejected(&plan, &attempt, &id, ProcessEnd::Rejected),
        );
        return;
    };
    if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
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
    let Ok(mut child) = platform::spawn(&mut command, &mut owner, &cancel, deadline).await else {
        publish(
            &shared,
            rejected(&plan, &attempt, &id, ProcessEnd::Rejected),
        );
        return;
    };
    let mut facts = ProcessEvidence {
        plan_digest: plan.digest().clone(),
        attempt_id: attempt,
        runner: id,
        scope: owner.scope(),
        finished: false,
        exit_code: None,
        end: ProcessEnd::Unknown,
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
                    facts.end = ProcessEnd::Unknown;
                    stop_at = Some(clock);
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
                if !matches!(result,Ok(Ok(()))){input_failed=true;facts.end=ProcessEnd::Unknown;stop_at.get_or_insert(Instant::now());owner.stop();}
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
                    facts.end = ProcessEnd::Unknown;
                    stop_at.get_or_insert(Instant::now());
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
    }
    owner.terminate();
    let _ = child.start_kill();
    if !exited {
        if let Ok(Ok(status)) = tokio::time::timeout(Duration::from_secs(1), child.wait()).await {
            facts.exit_code = status.code();
            exited = true
        }
    }
    facts.finished = true;
    facts.quiescent = exited && out_done && err_done && owner.quiescent();
    facts.quality = if facts.total_output_bytes > cap || facts.end == ProcessEnd::OutputLimit {
        OutputQuality::Truncated
    } else if !out_done || !err_done {
        OutputQuality::Partial
    } else if input_failed || facts.end != ProcessEnd::Exited || facts.exit_code != Some(0) {
        OutputQuality::Failed
    } else {
        output::quality(&facts.stdout, &facts.stderr, plan.spec().launch.output)
    };
    if plan.spec().launch.interpreter.profile.id.as_str() == "native-osquery-info-v1"
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
    publish(&facts);
}
#[cfg(test)]
mod tests;
