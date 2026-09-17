use execution_contract::*;
use execution_lifecycle::*;
use std::cell::Cell;

#[test]
fn restored_retry_preserves_accumulated_output_time_and_saturation() {
    let s = observe(
        started(),
        "never-dispatched",
        1200,
        Observation::NeverDispatched {
            total_output_bytes: 3000,
        },
    );
    let v = verifier(Observation::Uncertain);
    let s = apply(
        s,
        "retry",
        Command::BeginAttempt {
            attempt_id: aid("a2"),
            runner: id("test-runner"),
            mode: ExecutionMode::Test,
        },
        1300,
        &v,
    );
    let s = apply(
        s,
        "output",
        Command::Output {
            attempt_id: aid("a2"),
            total_bytes: 500,
        },
        1400,
        &v,
    );
    for state in [
        s.clone(),
        apply(
            s,
            "saturated",
            Command::Output {
                attempt_id: aid("a2"),
                total_bytes: u64::MAX,
            },
            1500,
            &v,
        ),
    ] {
        let bytes = serde_json::to_vec(state.snapshot()).unwrap();
        let restored = Execution::decode(
            plan(),
            &bytes,
            Limits {
                max_snapshot_bytes: MIN_SNAPSHOT_BYTES,
            },
        )
        .unwrap();
        assert_eq!(restored.snapshot(), state.snapshot());
        assert_eq!(restored.snapshot().attempts, 2);
        assert_eq!(restored.snapshot().first_attempt_at_unix_ms, Some(1100));
        assert_eq!(restored.snapshot().prior_output_bytes, 3000);
        assert_eq!(restored.total_output_bytes(), state.total_output_bytes());
        for time in [1500, 2100] {
            assert_eq!(restored.directive(time), state.directive(time));
        }
    }
}

#[test]
fn effect_must_follow_termination_both_live_and_after_restore() {
    struct OldEffect;
    impl ObservationVerifier for OldEffect {
        fn verify(
            &self,
            p: &FrozenPlan,
            a: &AttemptId,
            e: &EvidenceRef,
            _: u64,
        ) -> Result<ObservationFacts, ObservationError> {
            verifier(Observation::Effect {
                assessment: EffectAssessment::Satisfied,
            })
            .verify(p, a, e, 1200)
        }
    }
    let s = observe(
        started(),
        "exit",
        1300,
        Observation::Exited {
            exit_code: 0,
            total_output_bytes: 0,
        },
    );
    let command = Command::Observe {
        attempt_id: aid("a1"),
        evidence: evidence("effect"),
    };
    assert_eq!(
        s.evaluate(event(&s, "effect", command), 1400, &OldEffect)
            .unwrap_err(),
        LifecycleError::Transition
    );
    let s = observe(
        s,
        "effect",
        1400,
        Observation::Effect {
            assessment: EffectAssessment::Satisfied,
        },
    );
    let mut snapshot = s.snapshot().clone();
    snapshot
        .attempt
        .as_mut()
        .unwrap()
        .assessment
        .as_mut()
        .unwrap()
        .observed_at_unix_ms = 1200;
    assert_eq!(
        Execution::restore(
            plan(),
            snapshot,
            Limits {
                max_snapshot_bytes: MIN_SNAPSHOT_BYTES
            }
        )
        .unwrap_err(),
        LifecycleError::Snapshot
    );
}

#[test]
fn validity_and_cancel_stop_causes_are_distinct_and_verifier_errors_survive() {
    let s = Execution::open(
        plan(),
        900,
        Limits {
            max_snapshot_bytes: MIN_SNAPSHOT_BYTES,
        },
    )
    .unwrap();
    assert_eq!(
        s.directive(900).unwrap(),
        Directive::BudgetExhausted(LimitReason::NotYetValid)
    );
    let s = started();
    assert_eq!(
        s.directive(10000).unwrap(),
        Directive::StopRunner(StopReason::Limit(LimitReason::Expired))
    );
    let s = apply(
        s,
        "cancel",
        Command::Cancel,
        1200,
        &verifier(Observation::Uncertain),
    );
    assert_eq!(
        s.directive(10000).unwrap(),
        Directive::StopRunner(StopReason::Cancelled)
    );
    struct Unavailable;
    impl ObservationVerifier for Unavailable {
        fn verify(
            &self,
            _: &FrozenPlan,
            _: &AttemptId,
            _: &EvidenceRef,
            _: u64,
        ) -> Result<ObservationFacts, ObservationError> {
            Err(ObservationError::Unavailable)
        }
    }
    let command = Command::Observe {
        attempt_id: aid("a1"),
        evidence: evidence("missing"),
    };
    assert_eq!(
        s.evaluate(event(&s, "missing", command), 1300, &Unavailable)
            .unwrap_err(),
        LifecycleError::ObservationVerification(ObservationError::Unavailable)
    );
}
fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn aid(s: &str) -> AttemptId {
    AttemptId::new(s).unwrap()
}
fn plan() -> FrozenPlan {
    plan_for(Authority::Test {
        id: id("test-authority"),
    })
}
fn plan_for(authority: Authority) -> FrozenPlan {
    let l = PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_attempts: 3,
    };
    let mut p = decode_plan(
        include_bytes!("../../execution-contract/tests/fixtures/plan.json"),
        &l,
    )
    .unwrap();
    p.request.authority = authority;
    p.budget.max_attempts = 3;
    p.validity.expires_at_unix_ms = 10000;
    FrozenPlan::freeze(p, &l).unwrap()
}
fn evidence(n: &str) -> EvidenceRef {
    EvidenceRef {
        reference: VersionedRef {
            id: id(n),
            revision: id("1"),
        },
        kind: EvidenceKind::TestResult,
        runner: id("test-runner"),
    }
}
struct Verifier {
    observation: Observation,
    calls: Cell<usize>,
    wrong_attempt: bool,
}
impl ObservationVerifier for Verifier {
    fn verify(
        &self,
        p: &FrozenPlan,
        a: &AttemptId,
        e: &EvidenceRef,
        now: u64,
    ) -> Result<ObservationFacts, ObservationError> {
        self.calls.set(self.calls.get() + 1);
        Ok(ObservationFacts {
            plan_id: p.spec().plan_id.clone(),
            plan_digest: p.digest().clone(),
            attempt_id: if self.wrong_attempt {
                aid("wrong")
            } else {
                a.clone()
            },
            evidence: e.clone(),
            observed_at_unix_ms: now,
            observation: self.observation.clone(),
        })
    }
}
fn verifier(o: Observation) -> Verifier {
    Verifier {
        observation: o,
        calls: Cell::new(0),
        wrong_attempt: false,
    }
}
fn event(s: &Execution, n: &str, command: Command) -> Event {
    Event {
        id: EventId::new(n).unwrap(),
        expected_revision: s.snapshot().revision,
        command,
    }
}
fn apply(s: Execution, n: &str, c: Command, t: u64, v: &Verifier) -> Execution {
    s.evaluate(event(&s, n, c), t, v)
        .unwrap()
        .transition
        .unwrap()
        .next()
        .clone()
}
fn prepared() -> Execution {
    let s = Execution::open(
        plan(),
        1000,
        Limits {
            max_snapshot_bytes: 16384,
        },
    )
    .unwrap();
    apply(
        s,
        "prepared",
        Command::Prepare,
        1000,
        &verifier(Observation::Exited {
            exit_code: 0,
            total_output_bytes: 0,
        }),
    )
}
fn started() -> Execution {
    apply(
        prepared(),
        "begin",
        Command::BeginAttempt {
            attempt_id: aid("a1"),
            runner: id("test-runner"),
            mode: ExecutionMode::Test,
        },
        1100,
        &verifier(Observation::Exited {
            exit_code: 0,
            total_output_bytes: 0,
        }),
    )
}

#[test]
fn first_commit_yields_dispatch_but_replay_and_restore_do_not() {
    let s = prepared();
    let v = verifier(Observation::Uncertain);
    let begin = event(
        &s,
        "begin",
        Command::BeginAttempt {
            attempt_id: aid("a1"),
            runner: id("test-runner"),
            mode: ExecutionMode::Test,
        },
    );
    let candidate = || {
        s.evaluate(begin.clone(), 1100, &v)
            .unwrap()
            .transition
            .unwrap()
    };
    assert_eq!(
        candidate()
            .commit(|_| Err::<CommitStatus, _>("failed"))
            .unwrap_err(),
        "failed"
    );
    assert!(candidate()
        .commit(|_| Ok::<_, ()>(CommitStatus::AlreadyCommitted))
        .unwrap()
        .is_none());
    let next = candidate().next().clone();
    let action = candidate()
        .commit(|_| Ok::<_, ()>(CommitStatus::Applied))
        .unwrap()
        .unwrap();
    let calls = Cell::new(0);
    action.dispatch(|action| {
        assert_eq!(action.attempt_id(), &aid("a1"));
        assert_eq!(action.plan_digest(), plan().digest());
        assert_eq!(action.runner(), &id("test-runner"));
        assert_eq!(action.mode(), ExecutionMode::Test);
        assert_eq!(action.committed_revision(), next.snapshot().revision);
        calls.set(calls.get() + 1);
    });
    assert_eq!(calls.get(), 1);
    assert!(next.evaluate(begin, 1200, &v).unwrap().transition.is_none());
    let restored = Execution::restore(
        plan(),
        next.snapshot().clone(),
        Limits {
            max_snapshot_bytes: MIN_SNAPSHOT_BYTES,
        },
    )
    .unwrap();
    assert_eq!(restored.directive(1200).unwrap(), Directive::Reconcile);
}
fn observe(s: Execution, n: &str, t: u64, o: Observation) -> Execution {
    apply(
        s,
        n,
        Command::Observe {
            attempt_id: aid("a1"),
            evidence: evidence(n),
        },
        t,
        &verifier(o),
    )
}

#[test]
fn terminal_output_cannot_refund_already_accounted_bytes() {
    let s = apply(
        started(),
        "output",
        Command::Output {
            attempt_id: aid("a1"),
            total_bytes: 100,
        },
        1200,
        &verifier(Observation::Uncertain),
    );
    let before = s.snapshot().clone();
    for observation in [
        Observation::Exited {
            exit_code: 0,
            total_output_bytes: 99,
        },
        Observation::NeverDispatched {
            total_output_bytes: 99,
        },
    ] {
        let command = Command::Observe {
            attempt_id: aid("a1"),
            evidence: evidence("terminal"),
        };
        assert_eq!(
            s.evaluate(event(&s, "terminal", command), 1300, &verifier(observation))
                .unwrap_err(),
            LifecycleError::Accounting
        );
        assert_eq!(s.snapshot(), &before);
        assert_eq!(s.total_output_bytes(), 100);
    }
}

// These are synthetic authenticated facts testing core category rules, not OS evidence.
fn real_started() -> (FrozenPlan, Execution) {
    let p = plan_for(Authority::Local {
        id: id("local-authority"),
    });
    let v = verifier(Observation::Uncertain);
    let s = Execution::open(
        p.clone(),
        1000,
        Limits {
            max_snapshot_bytes: MIN_SNAPSHOT_BYTES,
        },
    )
    .unwrap();
    let s = apply(s, "prepare", Command::Prepare, 1000, &v);
    let s = apply(
        s,
        "begin",
        Command::BeginAttempt {
            attempt_id: aid("a1"),
            runner: id("test-runner"),
            mode: ExecutionMode::Real,
        },
        1100,
        &v,
    );
    (p, s)
}
fn real_observe(
    s: &Execution,
    observation: Observation,
    kind: EvidenceKind,
    now: u64,
) -> Result<Evaluation, LifecycleError> {
    let mut evidence = evidence("real-evidence");
    evidence.kind = kind;
    let command = Command::Observe {
        attempt_id: aid("a1"),
        evidence,
    };
    s.evaluate(
        event(s, &format!("observe-{now}"), command),
        now,
        &verifier(observation),
    )
}
fn real_observations() -> [(Observation, EvidenceKind); 4] {
    [
        (
            Observation::Exited {
                exit_code: 0,
                total_output_bytes: 0,
            },
            EvidenceKind::ProcessExited,
        ),
        (
            Observation::NeverDispatched {
                total_output_bytes: 0,
            },
            EvidenceKind::StateObserved,
        ),
        (
            Observation::Effect {
                assessment: EffectAssessment::Satisfied,
            },
            EvidenceKind::StateObserved,
        ),
        (Observation::Uncertain, EvidenceKind::StateObserved),
    ]
}
fn real_state_for(observation: &Observation) -> (FrozenPlan, Execution) {
    let (p, s) = real_started();
    if matches!(observation, Observation::Effect { .. }) {
        let s = real_observe(
            &s,
            Observation::Exited {
                exit_code: 0,
                total_output_bytes: 0,
            },
            EvidenceKind::ProcessExited,
            1200,
        )
        .unwrap()
        .transition
        .unwrap()
        .next()
        .clone();
        (p, s)
    } else {
        (p, s)
    }
}
#[test]
fn real_evidence_categories_are_enforced_for_live_observations() {
    for (observation, expected) in real_observations() {
        let (_, s) = real_state_for(&observation);
        let before = s.snapshot().clone();
        for kind in [
            EvidenceKind::TestResult,
            EvidenceKind::ProcessExited,
            EvidenceKind::StateObserved,
        ] {
            let result = real_observe(&s, observation.clone(), kind, 1300);
            if kind == expected {
                assert_eq!(result.unwrap().outcome, EventOutcome::Applied);
            } else {
                assert_eq!(result.unwrap_err(), LifecycleError::Observation);
            }
            assert_eq!(s.snapshot(), &before);
        }
    }
}
#[test]
fn real_recorded_categories_are_revalidated_on_restore() {
    for (observation, expected) in real_observations().into_iter().take(3) {
        let (p, s) = real_state_for(&observation);
        let applied = real_observe(&s, observation.clone(), expected, 1300)
            .unwrap()
            .transition
            .unwrap();
        for kind in [
            EvidenceKind::TestResult,
            EvidenceKind::ProcessExited,
            EvidenceKind::StateObserved,
        ] {
            let mut snapshot = applied.next().snapshot().clone();
            let attempt = snapshot.attempt.as_mut().unwrap();
            let recorded = if matches!(observation, Observation::Effect { .. }) {
                attempt.assessment.as_mut().unwrap()
            } else {
                attempt.termination.as_mut().unwrap()
            };
            recorded.evidence.kind = kind;
            let result = Execution::restore(
                p.clone(),
                snapshot,
                Limits {
                    max_snapshot_bytes: MIN_SNAPSHOT_BYTES,
                },
            );
            if kind == expected {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.unwrap_err(), LifecycleError::Snapshot);
            }
        }
    }
}
#[test]
fn exit_zero_needs_verified_target_and_test_evidence_stays_test() {
    let s = observe(
        started(),
        "exit",
        1200,
        Observation::Exited {
            exit_code: 0,
            total_output_bytes: 0,
        },
    );
    assert_eq!(s.phase(), Phase::ExecutionEnded);
    assert_eq!(s.directive(1200).unwrap(), Directive::VerifyTarget);
    let s = observe(
        s,
        "target",
        1300,
        Observation::Effect {
            assessment: EffectAssessment::Satisfied,
        },
    );
    assert_eq!(s.phase(), Phase::Verified);
    assert_eq!(s.directive(1300).unwrap(), Directive::Done);
    assert_eq!(
        s.snapshot().attempt.as_ref().unwrap().mode,
        ExecutionMode::Test
    );
}
#[test]
fn no_effect_while_runner_might_live_does_not_allow_retry() {
    let s = started();
    let e = event(
        &s,
        "premature",
        Command::Observe {
            attempt_id: aid("a1"),
            evidence: evidence("check"),
        },
    );
    assert_eq!(
        s.evaluate(
            e,
            1200,
            &verifier(Observation::Effect {
                assessment: EffectAssessment::NoEffect
            })
        )
        .unwrap_err(),
        LifecycleError::Transition
    );
    let s = observe(
        s,
        "exit",
        1200,
        Observation::Exited {
            exit_code: 1,
            total_output_bytes: 0,
        },
    );
    let s = observe(
        s,
        "none",
        1300,
        Observation::Effect {
            assessment: EffectAssessment::NoEffect,
        },
    );
    assert_eq!(s.directive(1300).unwrap(), Directive::RetryEligible);
    let s = apply(
        s,
        "retry",
        Command::BeginAttempt {
            attempt_id: aid("a2"),
            runner: id("test-runner"),
            mode: ExecutionMode::Test,
        },
        1400,
        &verifier(Observation::Uncertain),
    );
    assert_eq!(s.snapshot().attempts, 2);
    assert_eq!(s.snapshot().first_attempt_at_unix_ms, Some(1100));
}
#[test]
fn cancellation_and_crash_do_not_claim_termination_or_rollback() {
    let v = verifier(Observation::Uncertain);
    let s = apply(started(), "cancel", Command::Cancel, 1200, &v);
    assert_eq!(
        s.directive(1200).unwrap(),
        Directive::StopRunner(StopReason::Cancelled)
    );
    let s = apply(s, "restart", Command::Recover, 1250, &v);
    assert_eq!(s.phase(), Phase::OutcomeUnknown);
    assert!(s.snapshot().cancel_requested);
    let s = observe(
        s,
        "exit",
        1300,
        Observation::Exited {
            exit_code: 0,
            total_output_bytes: 0,
        },
    );
    assert_eq!(s.directive(1300).unwrap(), Directive::VerifyTarget);
    let s = observe(
        s,
        "none",
        1400,
        Observation::Effect {
            assessment: EffectAssessment::NoEffect,
        },
    );
    assert_eq!(s.directive(1400).unwrap(), Directive::Done);
}
#[test]
fn duplicate_stale_and_wrong_attempt_events_do_not_verify_or_rewrite() {
    let s = started();
    let v = verifier(Observation::Exited {
        exit_code: 0,
        total_output_bytes: 0,
    });
    let e = event(
        &s,
        "exit",
        Command::Observe {
            attempt_id: aid("a1"),
            evidence: evidence("exit"),
        },
    );
    let s = s
        .evaluate(e.clone(), 1200, &v)
        .unwrap()
        .transition
        .unwrap()
        .next()
        .clone();
    let replay = s.evaluate(e.clone(), 1300, &v).unwrap();
    assert_eq!(replay.outcome, EventOutcome::Duplicate);
    assert!(replay.transition.is_none());
    assert_eq!(v.calls.get(), 1);
    let mut conflict = e.clone();
    conflict.command = Command::Cancel;
    assert_eq!(
        s.evaluate(conflict, 1300, &v).unwrap_err(),
        LifecycleError::IdempotencyConflict
    );
    let mut stale = e;
    stale.id = EventId::new("old").unwrap();
    assert_eq!(
        s.evaluate(stale, 1300, &v).unwrap().outcome,
        EventOutcome::Stale
    );
    let wrong = event(
        &s,
        "wrong",
        Command::Observe {
            attempt_id: aid("a0"),
            evidence: evidence("wrong"),
        },
    );
    assert_eq!(
        s.evaluate(wrong, 1300, &v).unwrap_err(),
        LifecycleError::Attempt
    );
    assert_eq!(v.calls.get(), 1);
}
#[test]
fn timeout_blocks_attempts_but_accepts_late_exit_and_verification() {
    let s = started();
    assert_eq!(
        s.directive(2100).unwrap(),
        Directive::StopRunner(StopReason::Limit(LimitReason::Timeout))
    );
    let s = observe(
        s,
        "late-exit",
        2500,
        Observation::Exited {
            exit_code: 0,
            total_output_bytes: 0,
        },
    );
    let s = observe(
        s,
        "late-verified",
        2600,
        Observation::Effect {
            assessment: EffectAssessment::Satisfied,
        },
    );
    assert_eq!(s.directive(2600).unwrap(), Directive::Done);
}
#[test]
fn restore_rejects_corruption_and_preserves_consumed_budget() {
    let s = started();
    let mut snap = s.snapshot().clone();
    snap.attempts = 4;
    assert!(Execution::restore(
        plan(),
        snap,
        Limits {
            max_snapshot_bytes: 16384
        }
    )
    .is_err());
    let bytes = serde_json::to_vec(s.snapshot()).unwrap();
    let restored = Execution::decode(
        plan(),
        &bytes,
        Limits {
            max_snapshot_bytes: 16384,
        },
    )
    .unwrap();
    assert_eq!(restored.snapshot(), s.snapshot());
    assert_eq!(
        restored.directive(2100).unwrap(),
        Directive::StopRunner(StopReason::Limit(LimitReason::Timeout))
    );
    let mut snap = s.snapshot().clone();
    snap.version = 0;
    assert!(Execution::restore(
        plan(),
        snap,
        Limits {
            max_snapshot_bytes: 16384
        }
    )
    .is_err());
}

#[test]
fn retry_keeps_output_and_attempt_budgets_and_rejects_old_attempts() {
    let v = verifier(Observation::NeverDispatched {
        total_output_bytes: 0,
    });
    let s = apply(
        started(),
        "output",
        Command::Output {
            attempt_id: aid("a1"),
            total_bytes: 3000,
        },
        1150,
        &v,
    );
    let s = observe(
        s,
        "not-dispatched",
        1200,
        Observation::NeverDispatched {
            total_output_bytes: 3000,
        },
    );
    assert_eq!(s.directive(1200).unwrap(), Directive::RetryEligible);
    let s = apply(
        s,
        "retry",
        Command::BeginAttempt {
            attempt_id: aid("a2"),
            runner: id("test-runner"),
            mode: ExecutionMode::Test,
        },
        1300,
        &v,
    );
    assert_eq!(s.total_output_bytes(), 3000);
    let old = event(
        &s,
        "old-output",
        Command::Output {
            attempt_id: aid("a1"),
            total_bytes: 4000,
        },
    );
    assert_eq!(
        s.evaluate(old, 1300, &v).unwrap_err(),
        LifecycleError::Attempt
    );
    let s = apply(
        s,
        "new-output",
        Command::Output {
            attempt_id: aid("a2"),
            total_bytes: 1096,
        },
        1400,
        &v,
    );
    assert_eq!(
        s.directive(1400).unwrap(),
        Directive::StopRunner(StopReason::Limit(LimitReason::Output))
    );
    let decrease = event(
        &s,
        "decrease",
        Command::Output {
            attempt_id: aid("a2"),
            total_bytes: 100,
        },
    );
    assert_eq!(
        s.evaluate(decrease, 1400, &v).unwrap_err(),
        LifecycleError::Accounting
    );
    let s = apply(
        s,
        "exit2",
        Command::Observe {
            attempt_id: aid("a2"),
            evidence: evidence("exit2"),
        },
        1500,
        &verifier(Observation::NeverDispatched {
            total_output_bytes: 1096,
        }),
    );
    assert_eq!(
        s.directive(1500).unwrap(),
        Directive::BudgetExhausted(LimitReason::Output)
    );
    assert_eq!(s.total_output_bytes(), 4096);
}

#[test]
fn cancellation_before_start_and_test_mode_cannot_be_bypassed() {
    let s = prepared();
    let v = verifier(Observation::Uncertain);
    let begin = Command::BeginAttempt {
        attempt_id: aid("a1"),
        runner: id("test-runner"),
        mode: ExecutionMode::Real,
    };
    assert_eq!(
        s.evaluate(event(&s, "real", begin), 1100, &v).unwrap_err(),
        LifecycleError::Attempt
    );
    let s = apply(s, "cancel", Command::Cancel, 1100, &v);
    assert_eq!(s.phase(), Phase::Cancelled);
    let begin = Command::BeginAttempt {
        attempt_id: aid("a1"),
        runner: id("test-runner"),
        mode: ExecutionMode::Test,
    };
    assert_eq!(
        s.evaluate(event(&s, "begin", begin), 1100, &v).unwrap_err(),
        LifecycleError::Transition
    );
    assert_eq!(s.snapshot().attempts, 0);
}

#[test]
fn never_dispatched_requires_reconciliation_and_unknown_effects_never_retry() {
    let v = verifier(Observation::NeverDispatched {
        total_output_bytes: 0,
    });
    let s = apply(
        started(),
        "dispatch",
        Command::Dispatched {
            attempt_id: aid("a1"),
        },
        1150,
        &v,
    );
    let cmd = Command::Observe {
        attempt_id: aid("a1"),
        evidence: evidence("absent"),
    };
    assert_eq!(
        s.evaluate(event(&s, "absent", cmd), 1200, &v).unwrap_err(),
        LifecycleError::Transition
    );
    let s = observe(
        s,
        "exit",
        1300,
        Observation::Exited {
            exit_code: 1,
            total_output_bytes: 0,
        },
    );
    for assessment in [EffectAssessment::Unknown, EffectAssessment::NotSatisfied] {
        let state = observe(
            s.clone(),
            "effect",
            1400,
            Observation::Effect { assessment },
        );
        assert_eq!(state.directive(1400).unwrap(), Directive::ManualReview);
    }
}

#[test]
fn verifier_binding_clock_and_evidence_failures_are_closed() {
    struct BadFacts(u8);
    impl ObservationVerifier for BadFacts {
        fn verify(
            &self,
            p: &FrozenPlan,
            a: &AttemptId,
            e: &EvidenceRef,
            _: u64,
        ) -> Result<ObservationFacts, ObservationError> {
            let mut f = ObservationFacts {
                plan_id: p.spec().plan_id.clone(),
                plan_digest: p.digest().clone(),
                attempt_id: a.clone(),
                evidence: e.clone(),
                observed_at_unix_ms: 1200,
                observation: Observation::Exited {
                    exit_code: 0,
                    total_output_bytes: 0,
                },
            };
            match self.0 {
                0 => return Err(ObservationError::Untrusted),
                1 => f.plan_id = PlanId::new("other").unwrap(),
                2 => f.plan_digest = Digest::new("ab".repeat(32)).unwrap(),
                3 => f.attempt_id = aid("other"),
                4 => f.evidence = evidence("other"),
                5 => f.observed_at_unix_ms = 1099,
                _ => f.observed_at_unix_ms = 1201,
            }
            Ok(f)
        }
    }
    let s = started();
    for case in 0..7 {
        let cmd = Command::Observe {
            attempt_id: aid("a1"),
            evidence: evidence("exit"),
        };
        assert_eq!(
            s.evaluate(event(&s, "exit", cmd), 1200, &BadFacts(case))
                .unwrap_err(),
            if case == 0 {
                LifecycleError::ObservationVerification(ObservationError::Untrusted)
            } else {
                LifecycleError::Observation
            },
            "case {case}"
        );
    }
    for kind in [EvidenceKind::ProcessExited, EvidenceKind::StateObserved] {
        let mut e = evidence("exit");
        e.kind = kind;
        let cmd = Command::Observe {
            attempt_id: aid("a1"),
            evidence: e,
        };
        assert_eq!(
            s.evaluate(
                event(&s, "exit", cmd),
                1200,
                &verifier(Observation::Exited {
                    exit_code: 0,
                    total_output_bytes: 0
                })
            )
            .unwrap_err(),
            LifecycleError::Observation
        );
    }
    assert_eq!(s.directive(1099).unwrap_err(), LifecycleError::Clock);
    let mut future = event(&s, "future", Command::Cancel);
    future.expected_revision += 1;
    assert_eq!(
        s.evaluate(future, 1200, &verifier(Observation::Uncertain))
            .unwrap_err(),
        LifecycleError::Revision
    );
}

#[test]
fn max_attempts_and_saturating_output_never_replenish_budget() {
    let v = verifier(Observation::NeverDispatched {
        total_output_bytes: 0,
    });
    let mut s = started();
    for n in 1..=3 {
        s = apply(
            s,
            &format!("end-{n}"),
            Command::Observe {
                attempt_id: aid(&format!("a{n}")),
                evidence: evidence(&format!("end-{n}")),
            },
            1100 + n * 10,
            &v,
        );
        if n < 3 {
            s = apply(
                s,
                &format!("begin-{}", n + 1),
                Command::BeginAttempt {
                    attempt_id: aid(&format!("a{}", n + 1)),
                    runner: id("test-runner"),
                    mode: ExecutionMode::Test,
                },
                1105 + n * 10,
                &v,
            );
        }
    }
    assert_eq!(
        s.directive(1140).unwrap(),
        Directive::BudgetExhausted(LimitReason::Attempts)
    );
    assert_eq!(s.snapshot().attempts, 3);
    let s = apply(
        started(),
        "huge-output",
        Command::Output {
            attempt_id: aid("a1"),
            total_bytes: u64::MAX,
        },
        1200,
        &v,
    );
    assert_eq!(s.total_output_bytes(), u64::MAX);
    assert_eq!(
        s.directive(1200).unwrap(),
        Directive::StopRunner(StopReason::Limit(LimitReason::Output))
    );
}

#[test]
fn strict_snapshot_and_minimum_capacity_preserve_terminal_evidence() {
    let s = started();
    let mut json = serde_json::to_value(s.snapshot()).unwrap();
    json["legacyStatus"] = "running".into();
    assert!(Execution::decode(
        plan(),
        &serde_json::to_vec(&json).unwrap(),
        Limits {
            max_snapshot_bytes: 16384
        }
    )
    .is_err());
    for case in 0..7 {
        let mut snap = s.snapshot().clone();
        match case {
            0 => snap.first_attempt_at_unix_ms = None,
            1 => snap.attempt.as_mut().unwrap().mode = ExecutionMode::Real,
            2 => snap.attempt.as_mut().unwrap().number = 0,
            3 => snap.prior_output_bytes = 1,
            4 => snap.last_event.as_mut().unwrap().expected_revision = u64::MAX,
            5 => snap.updated_at_unix_ms = 1099,
            _ => snap.attempt.as_mut().unwrap().accepted_at_unix_ms = 2100,
        }
        assert!(
            Execution::restore(
                plan(),
                snap,
                Limits {
                    max_snapshot_bytes: 16384
                }
            )
            .is_err(),
            "case {case}"
        );
    }
    let long = "a".repeat(128);
    let v = verifier(Observation::Exited {
        exit_code: i32::MIN,
        total_output_bytes: 0,
    });
    let s = apply(
        prepared(),
        "long-begin",
        Command::BeginAttempt {
            attempt_id: aid(&long),
            runner: id("test-runner"),
            mode: ExecutionMode::Test,
        },
        1100,
        &v,
    );
    let e = EvidenceRef {
        reference: VersionedRef {
            id: id(&long),
            revision: id(&long),
        },
        kind: EvidenceKind::TestResult,
        runner: id("test-runner"),
    };
    let s = apply(
        s,
        &long,
        Command::Observe {
            attempt_id: aid(&long),
            evidence: e.clone(),
        },
        1200,
        &v,
    );
    let s = apply(
        s,
        "effect",
        Command::Observe {
            attempt_id: aid(&long),
            evidence: e,
        },
        1300,
        &verifier(Observation::Effect {
            assessment: EffectAssessment::Satisfied,
        }),
    );
    assert_eq!(s.directive(1300).unwrap(), Directive::Done);
    assert!(serde_json::to_vec(s.snapshot()).unwrap().len() < MIN_SNAPSHOT_BYTES);
}

#[test]
fn final_output_is_settled_with_termination_before_retry() {
    let v = verifier(Observation::Uncertain);
    let s = apply(
        started(),
        "partial",
        Command::Output {
            attempt_id: aid("a1"),
            total_bytes: 100,
        },
        1150,
        &v,
    );
    let s = observe(
        s,
        "exit",
        1200,
        Observation::Exited {
            exit_code: 1,
            total_output_bytes: 4000,
        },
    );
    assert_eq!(s.total_output_bytes(), 4000);
    let s = apply(
        s,
        "late-partial",
        Command::Output {
            attempt_id: aid("a1"),
            total_bytes: 3000,
        },
        1250,
        &v,
    );
    assert_eq!(s.total_output_bytes(), 4000);
    let s = observe(
        s,
        "no-effect",
        1300,
        Observation::Effect {
            assessment: EffectAssessment::NoEffect,
        },
    );
    let s = apply(
        s,
        "retry",
        Command::BeginAttempt {
            attempt_id: aid("a2"),
            runner: id("test-runner"),
            mode: ExecutionMode::Test,
        },
        1400,
        &v,
    );
    assert_eq!(s.total_output_bytes(), 4000);
    let old = event(
        &s,
        "late-final",
        Command::Output {
            attempt_id: aid("a1"),
            total_bytes: 4000,
        },
    );
    assert_eq!(
        s.evaluate(old, 1400, &v).unwrap_err(),
        LifecycleError::Attempt
    );
    let s = apply(
        s,
        "next-output",
        Command::Output {
            attempt_id: aid("a2"),
            total_bytes: 96,
        },
        1450,
        &v,
    );
    assert_eq!(
        s.directive(1450).unwrap(),
        Directive::StopRunner(StopReason::Limit(LimitReason::Output))
    );
    let mut corrupt = s.snapshot().clone();
    corrupt.attempt.as_mut().unwrap().termination = Some(RecordedObservation {
        evidence: evidence("final"),
        observed_at_unix_ms: 1450,
        observation: Observation::Exited {
            exit_code: 0,
            total_output_bytes: 95,
        },
    });
    assert!(Execution::restore(
        plan(),
        corrupt,
        Limits {
            max_snapshot_bytes: MIN_SNAPSHOT_BYTES
        }
    )
    .is_err());
}

#[test]
fn recovery_cannot_erase_dispatch_history() {
    let v = verifier(Observation::Uncertain);
    let s = apply(
        started(),
        "dispatch",
        Command::Dispatched {
            attempt_id: aid("a1"),
        },
        1150,
        &v,
    );
    for uncertain in [
        Command::Recover,
        Command::Observe {
            attempt_id: aid("a1"),
            evidence: evidence("uncertain"),
        },
    ] {
        let s = apply(s.clone(), "uncertain", uncertain, 1200, &v);
        let cmd = Command::Observe {
            attempt_id: aid("a1"),
            evidence: evidence("absent"),
        };
        assert_eq!(
            s.evaluate(
                event(&s, "absent", cmd),
                1300,
                &verifier(Observation::NeverDispatched {
                    total_output_bytes: 0
                })
            )
            .unwrap_err(),
            LifecycleError::Transition
        );
    }
}
