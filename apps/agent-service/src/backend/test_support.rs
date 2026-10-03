#![allow(dead_code)]
use crate::backend::bridge::OutputPolicy;
use agent_client::Error;
use std::sync::atomic::Ordering;
#[path = "../../../../crates/execution-app/tests/support/mod.rs"]
pub mod local;
pub struct FixtureOutput;
impl OutputPolicy for FixtureOutput {
    fn redact(&self, text: &str) -> Result<String, Error> {
        Ok(text.replace("secret-canary", "[redacted]"))
    }
}
#[derive(Clone)]
pub struct CaptureSpec {
    pub quiescent: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub quality: execution_contract::OutputQuality,
    pub end: execution_contract::ProcessEnd,
}
#[derive(Clone)]
pub struct CapturingRunner {
    pub inner: execution_app::DeterministicTestRunner,
    pub ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub capture: std::sync::Arc<std::sync::Mutex<Option<CaptureSpec>>>,
}
impl execution_app::RunnerPort for CapturingRunner {
    fn id(&self) -> execution_contract::Id {
        self.inner.id()
    }
    fn mode(&self) -> execution_lifecycle::ExecutionMode {
        execution_lifecycle::ExecutionMode::Test
    }
    fn dispatch(
        &self,
        p: execution_app::AuthorizedDispatch,
    ) -> Result<execution_app::DispatchOutcome, execution_app::Error> {
        self.ready.store(true, Ordering::SeqCst);
        self.inner.dispatch(p)
    }
    fn evidence(
        &self,
        p: &execution_contract::FrozenExecution,
        a: &execution_contract::AttemptId,
    ) -> Result<Option<execution_contract::ProcessEvidence>, execution_app::Error> {
        let capture = self.capture.lock().unwrap();
        Ok(self
            .ready
            .load(Ordering::SeqCst)
            .then(|| execution_contract::ProcessEvidence {
                content_digest: p.digest().clone(),
                attempt_id: a.clone(),
                runner: self.id(),
                scope: execution_contract::ProcessScope::ProcessGroup { owner: 1, group: 1 },
                finished: true,
                exit_code: Some(0),
                end: capture
                    .as_ref()
                    .map_or(execution_contract::ProcessEnd::Exited, |v| v.end),
                failure_kind: execution_contract::ProcessFailureKind::None,
                quiescent: capture.as_ref().is_none_or(|v| v.quiescent),
                stdout: capture.as_ref().map_or_else(
                    || b"{\"ok\":true,\"message\":\"secret-canary\"}".to_vec(),
                    |v| v.stdout.clone(),
                ),
                stderr: capture.as_ref().map_or_else(Vec::new, |v| v.stderr.clone()),
                total_output_bytes: capture
                    .as_ref()
                    .map_or(37, |v| (v.stdout.len() + v.stderr.len()) as u64),
                quality: capture
                    .as_ref()
                    .map_or(execution_contract::OutputQuality::Complete, |v| v.quality),
            }))
    }
    fn software_progress(
        &self,
        p: &execution_contract::FrozenExecution,
        a: &execution_contract::AttemptId,
    ) -> Result<Option<execution_contract::SoftwareProgress>, execution_app::Error> {
        use execution_contract::*;
        if !self.ready.load(Ordering::SeqCst) || p.spec().execution.software_program().is_none() {
            return Ok(None);
        }
        let quiet = self
            .capture
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|v| v.quiescent);
        let mut checkpoints = vec![
            SoftwareCheckpoint::Begin {
                step: 0,
                phase: SoftwarePhase::Before,
            },
            SoftwareCheckpoint::End {
                duration_ms: 0,
                step: 0,
                phase: SoftwarePhase::Before,
                process: None,
                detected: Some(SoftwareState::Present {
                    version: PackageValue::new("1.0").unwrap(),
                }),
                quiescent: quiet,
            },
        ];
        if quiet {
            checkpoints.push(SoftwareCheckpoint::Complete { step: 0 });
        }
        Ok(Some(SoftwareProgress {
            attempt_id: a.clone(),
            content_digest: p.digest().clone(),
            runner: self.id(),
            checkpoints,
            elapsed_ms: 1,
            output_bytes: 0,
        }))
    }
    fn acknowledge_software_progress(
        &self,
        _: execution_app::CommittedSoftwareProgress,
    ) -> Result<(), execution_app::Error> {
        Ok(())
    }
    fn acknowledge_capture(
        &self,
        _: &execution_contract::FrozenExecution,
        _: &execution_contract::ProcessEvidence,
    ) -> Result<(), execution_app::Error> {
        self.ready.store(false, Ordering::SeqCst);
        Ok(())
    }
    fn stop(
        &self,
        p: &execution_contract::FrozenExecution,
        a: &execution_contract::AttemptId,
    ) -> Result<(), execution_app::Error> {
        self.inner.stop(p, a)
    }
    fn observe(
        &self,
        p: &execution_contract::FrozenExecution,
        a: &execution_contract::AttemptId,
        s: execution_app::ObservationStage,
        n: u64,
    ) -> Result<Option<execution_lifecycle::ObservationFacts>, execution_app::Error> {
        if self
            .capture
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|v| !v.quiescent)
        {
            return Ok(None);
        }
        let mut value = self.inner.observe(p, a, s, n)?;
        if let Some(facts) = &mut value {
            if let execution_lifecycle::Observation::Exited {
                total_output_bytes, ..
            } = &mut facts.observation
            {
                *total_output_bytes = self
                    .capture
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map_or(37, |v| (v.stdout.len() + v.stderr.len()) as u64);
            }
        }
        Ok(value)
    }
}

pub fn adapted_plan(
    software: bool,
    device: &str,
    offer: &agent_client::Offer,
) -> execution_contract::FrozenExecution {
    use execution_contract::*;
    let original = if software {
        FrozenExecution::freeze(
            decode_execution(
                include_bytes!(
                    "../../../../crates/execution-contract/tests/fixtures/software.json"
                ),
                &execution_app::test_execution_limits(),
            )
            .unwrap(),
            &execution_app::test_execution_limits(),
        )
        .unwrap()
    } else {
        local::plan()
    };
    let mut spec = original.spec().clone();
    spec.request.request_id = crate::backend::request_id(offer).unwrap();
    spec.request.target.device = DeviceId::new(device).unwrap();
    spec.request.target.platform = Platform::Macos;
    spec.request.target.scope = TargetScope::Device {};
    spec.run_as = RunAs::System {
        platform: Platform::Macos,
    };
    if let (
        ExecutionSpec::SoftwareProgram { program },
        agent_client::wire::TaskPayload::Software(remote),
    ) = (&mut spec.execution, offer.payload())
    {
        program.definition_digest = execution_contract::Digest::new(
            remote
                .definition_digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        )
        .unwrap();
    }
    FrozenExecution::freeze(spec, &execution_app::test_execution_limits()).unwrap()
}
