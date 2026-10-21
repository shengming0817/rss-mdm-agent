//! Host-authorized periodic reads reuse the native process owner, outside command dispatch.
//! ref: tokio tokio/src/time/interval.rs (skip missed ticks, no catch-up execution).
use crate::{Artifacts, NativeRunner};
use agent_client::telemetry::Configuration;
mod authorization;
use execution_app::Error;
use execution_contract::*;
use execution_lifecycle::DispatchAllowance;
use std::sync::atomic::Ordering;

/// Single-flight physical collector. The trusted service must verify its signed lease first.
/// This adapter cannot accept IPC input or claim a remote task/command receipt.
pub struct PeriodicRunner {
    runner: NativeRunner,
    active: Option<AttemptId>,
}
impl PeriodicRunner {
    /// Allocate an independent process slot so collection cannot occupy command permits.
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            runner: NativeRunner::new(
                Id::new("periodic-collector").map_err(|_| Error::Configuration)?,
                Default::default(),
                1,
            )?,
            active: None,
        })
    }
    /// Launch an immutable Policy read prepared by the trusted service.
    pub fn start(
        &mut self,
        configuration: &Configuration,
        plan: FrozenExecution,
        artifacts: Artifacts,
        now_ms: u64,
    ) -> Result<(), Error> {
        if self.active.is_some()
            || !matches!(plan.spec().request.initiator, Initiator::Policy { .. })
            || plan.spec().request.operation.action.as_str() != "telemetry-sample"
            || !matches!(plan.spec().execution, ExecutionSpec::Process {})
            || plan.spec().validity.not_before_unix_ms > now_ms
            || plan.spec().validity.expires_at_unix_ms <= now_ms
            || !matches!(plan.spec().run_as, RunAs::System { .. })
            || artifacts.delegate.is_some()
        {
            return Err(Error::Denied);
        }
        authorization::verify(configuration, &plan, &artifacts, now_ms)?;
        self.start_admitted(plan, artifacts, now_ms)
    }
    fn start_admitted(
        &mut self,
        plan: FrozenExecution,
        artifacts: Artifacts,
        now_ms: u64,
    ) -> Result<(), Error> {
        let timeout = plan
            .spec()
            .budget
            .total_timeout_ms
            .min(plan.spec().validity.expires_at_unix_ms - now_ms);
        let attempt = AttemptId::new(plan.spec().request.request_id.as_str())
            .map_err(|_| Error::InvalidInput)?;
        self.runner.artifacts.register(&plan, artifacts)?;
        if let Err(error) = self.runner.launch_bounded(
            &plan,
            &attempt,
            DispatchAllowance {
                deadline_unix_ms: plan.spec().validity.expires_at_unix_ms,
                remaining_timeout_ms: timeout,
                remaining_output_bytes: plan.spec().budget.total_output_bytes,
            },
            Some(execution_ipc::host::monotonic_millis()?.saturating_add(timeout)),
        ) {
            self.runner.artifacts.retire(plan.digest())?;
            return Err(error);
        }
        self.active = Some(attempt);
        Ok(())
    }
    /// Obtain completed, physically quiescent output exactly once and release its slot.
    pub fn take(&mut self) -> Result<Option<ProcessEvidence>, Error> {
        let Some(id) = &self.active else {
            return Ok(None);
        };
        let mut records = self.runner.records.lock().map_err(|_| Error::Unavailable)?;
        let record = records.get_mut(id).ok_or(Error::Unavailable)?;
        let result = record.facts.lock().map_err(|_| Error::Unavailable)?.clone();
        let Some(result) = result.filter(|v| v.finished && v.quiescent) else {
            return Ok(None);
        };
        let digest = record.plan.digest().clone();
        // Final capture may precede the owner's last material lease release. Keep the
        // completed record retryable until retirement succeeds; never strand active on a missing record.
        drop(record._materials.take());
        match self.runner.artifacts.retire(&digest) {
            Ok(()) => (),
            Err(Error::Conflict) => return Ok(None),
            Err(error) => return Err(error),
        }
        records.remove(id);
        drop(records);
        self.active = None;
        Ok(Some(result))
    }
    /// Request cancellation; completion still requires quiescence evidence.
    pub fn cancel(&self) -> Result<(), Error> {
        let records = self.runner.records.lock().map_err(|_| Error::Unavailable)?;
        if let Some(record) = self.active.as_ref().and_then(|id| records.get(id)) {
            record.cancel.store(true, Ordering::Release);
        }
        Ok(())
    }
    /// Whether the owner still retains a physical operation.
    pub fn active(&self) -> bool {
        self.active.is_some()
    }
    /// Bounded real cleanup through the same owner used by command executions.
    pub fn shutdown(&mut self) -> Result<(), Error> {
        self.runner.shutdown()
    }
}
impl Drop for PeriodicRunner {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn cancelled_owner_is_taken_only_after_quiescence_and_slot_is_reusable() {
        let fixture = crate::runner::tests::fixture(
            "sleep 2\nprintf '{}'",
            vec![LaunchArg::ArtifactPath {}],
            1024,
            5000,
        );
        let materials = fixture
            .runner
            .artifacts
            .get(fixture.plan.digest().as_str())
            .unwrap()
            .unwrap();
        let mut runner = PeriodicRunner::new().unwrap();
        let start_fixture = |runner: &mut PeriodicRunner| {
            let plan = &fixture.plan;
            let attempt = AttemptId::new(plan.spec().request.request_id.as_str()).unwrap();
            // Explicit unprivileged material fixture, just as NativeRunner's OS mechanics tests.
            // Production registration still requires protected System-owned material.
            runner.runner.artifacts.entries.lock().unwrap().insert(
                plan.digest().as_str().into(),
                std::sync::Arc::new((*materials).clone()),
            );
            runner.runner.launch_bounded(
                plan,
                &attempt,
                DispatchAllowance {
                    deadline_unix_ms: plan.spec().validity.expires_at_unix_ms,
                    remaining_timeout_ms: 5000,
                    remaining_output_bytes: 1024,
                },
                None,
            )?;
            runner.active = Some(attempt);
            Ok::<_, Error>(())
        };
        start_fixture(&mut runner).unwrap();
        assert!(runner.active());
        assert!(start_fixture(&mut runner).is_err());
        runner.cancel().unwrap();
        runner.shutdown().unwrap();
        let id = runner.active.clone().unwrap();
        let facts = runner.runner.records.lock().unwrap()[&id].facts.clone();
        facts.lock().unwrap().as_mut().unwrap().quiescent = false;
        assert!(runner.take().unwrap().is_none());
        assert!(runner.active());
        facts.lock().unwrap().as_mut().unwrap().quiescent = true;
        let lease = runner
            .runner
            .artifacts
            .get(fixture.plan.digest().as_str())
            .unwrap()
            .unwrap();
        assert!(runner.take().unwrap().is_none());
        assert!(runner.active());
        drop(lease);
        let take = |runner: &mut PeriodicRunner| {
            for _ in 0..100 {
                if let Some(e) = runner.take().unwrap() {
                    return e;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!("finished owner did not release material lease");
        };
        assert!(take(&mut runner).quiescent);
        assert!(runner.take().unwrap().is_none());
        assert!(!runner.active());
        start_fixture(&mut runner).unwrap();
        runner.cancel().unwrap();
        runner.shutdown().unwrap();
        assert!(take(&mut runner).quiescent);
    }
}
