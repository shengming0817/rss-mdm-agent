use execution_contract::*;
use execution_lifecycle::*;

// Deterministic test evidence only, with no OS runner or production evidence.
struct TestEvidence(Observation);
impl ObservationVerifier for TestEvidence {
    fn verify(
        &self,
        p: &FrozenPlan,
        a: &AttemptId,
        e: &EvidenceRef,
        now: u64,
    ) -> Result<ObservationFacts, ObservationError> {
        if !matches!(p.spec().request.authority, Authority::Test { .. })
            || e.kind != EvidenceKind::TestResult
        {
            return Err(ObservationError::Untrusted);
        }
        Ok(ObservationFacts {
            plan_id: p.spec().plan_id.clone(),
            plan_digest: p.digest().clone(),
            attempt_id: a.clone(),
            evidence: e.clone(),
            observed_at_unix_ms: now,
            observation: self.0.clone(),
        })
    }
}
fn apply(s: Execution, name: &str, command: Command, now: u64, fact: Observation) -> Execution {
    let event = Event {
        id: EventId::new(name).unwrap(),
        expected_revision: s.snapshot().revision,
        command,
    };
    s.evaluate(event, now, &TestEvidence(fact))
        .unwrap()
        .transition
        .unwrap()
        .next
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(std::env::args().nth(1).ok_or("test plan path required")?)?;
    let limits = PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_attempts: 3,
    };
    let plan = FrozenPlan::freeze(decode_plan(&bytes, &limits)?, &limits)?;
    let bounds = Limits {
        max_snapshot_bytes: MIN_SNAPSHOT_BYTES,
    };
    let time = plan.spec().validity.not_before_unix_ms;
    let s = Execution::open(plan.clone(), time, bounds)?;
    let s = apply(s, "prepare", Command::Prepare, time, Observation::Uncertain);
    let attempt = AttemptId::new("test-attempt")?;
    let runner = Id::new("test-runner")?;
    let s = apply(
        s,
        "begin",
        Command::BeginAttempt {
            attempt_id: attempt.clone(),
            runner: runner.clone(),
            mode: ExecutionMode::Test,
        },
        time,
        Observation::Uncertain,
    );
    assert_eq!(s.phase(), Phase::Starting);
    let s = apply(s, "restart", Command::Recover, time, Observation::Uncertain);
    assert_eq!(s.directive(time)?, Directive::Reconcile);
    let evidence = |name: &str| EvidenceRef {
        reference: VersionedRef {
            id: Id::new(name).unwrap(),
            revision: Id::new("1").unwrap(),
        },
        kind: EvidenceKind::TestResult,
        runner: runner.clone(),
    };
    let s = apply(
        s,
        "exit",
        Command::Observe {
            attempt_id: attempt.clone(),
            evidence: evidence("test-exit"),
        },
        time,
        Observation::Exited { exit_code: 0 },
    );
    assert_eq!(s.directive(time)?, Directive::VerifyTarget);
    let encoded = serde_json::to_vec(s.snapshot())?;
    let s = Execution::decode(plan, &encoded, bounds)?;
    let s = apply(
        s,
        "verify",
        Command::Observe {
            attempt_id: attempt,
            evidence: evidence("test-target"),
        },
        time,
        Observation::Effect {
            assessment: EffectAssessment::Satisfied,
        },
    );
    assert_eq!(s.directive(time)?, Directive::Done);
    assert_eq!(
        s.snapshot().attempt.as_ref().unwrap().mode,
        ExecutionMode::Test
    );
    println!("execution-lifecycle: explicit test evidence; no dispatch, SQLite transaction or real platform proof");
    Ok(())
}
