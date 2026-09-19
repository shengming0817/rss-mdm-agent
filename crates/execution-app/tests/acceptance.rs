mod support;
use execution_app::*;
use execution_contract::*;
use support::*;
fn command(value: &str) -> CommandId {
    CommandId::new(value).unwrap()
}

fn open(
    db: &Database,
    host: TestHost,
    runner: DeterministicTestRunner,
    startup: Startup,
) -> ExecutionApp<TestHost, DeterministicTestRunner> {
    ExecutionApp::start(&db.path, startup, host, runner, AppConfig::test_defaults(1)).unwrap()
}

#[test]
fn production_and_unbound_startup_fail_before_database_creation() {
    let db = Database::new();
    let host = TestHost::new();
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    assert!(matches!(
        ExecutionApp::start(
            &db.path,
            Startup::Production,
            host.clone(),
            runner.clone(),
            AppConfig::test_defaults(1)
        ),
        Err(Error::Unbound)
    ));
    assert!(!db.path.exists());
    host.state.lock().unwrap().bound = false;
    assert!(matches!(
        ExecutionApp::start(
            &db.path,
            Startup::CreateTest,
            host,
            runner,
            AppConfig::test_defaults(1)
        ),
        Err(Error::Unbound)
    ));
    assert!(!db.path.exists());
}

#[test]
fn lost_response_replay_and_reopen_preserve_one_attempt_and_dispatch() {
    let db = Database::new();
    let host = TestHost::new();
    host.state.lock().unwrap().approval = true;
    host.grant(2);
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let p = plan();
    let request = &p.spec().request.request_id;
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    let accepted = app.submit(request, &p).unwrap();
    assert_eq!(accepted.attempts, 1);
    drop(app);
    let mut app = open(&db, host.clone(), runner.clone(), Startup::OpenTest);
    assert_eq!(
        app.submit(request, &p).unwrap().attempt_id,
        accepted.attempt_id
    );
    assert_eq!(runner.dispatch_count(), 1);
    assert_eq!(db.count("approval_consumptions"), 1);
    assert_eq!(host.state.lock().unwrap().admissions, 1);
    let completed = app.reconcile(request).unwrap();
    assert_eq!(completed.phase, TaskPhase::TestCompleted);
    assert!(completed
        .evidence
        .iter()
        .all(|e| e.kind == EvidenceKind::TestResult));
    assert_eq!(app.reconcile(request).unwrap(), completed);
    let mut changed = p.spec().clone();
    changed.budget.max_attempts = 2;
    let changed = FrozenPlan::freeze(changed, &test_store_limits().plan).unwrap();
    assert_eq!(app.submit(request, &changed).unwrap_err(), Error::Conflict);
}

#[test]
fn admission_failure_and_dispatch_gate_never_call_runner() {
    for denial in [true, false] {
        let db = Database::new();
        let host = TestHost::new();
        if denial {
            host.state.lock().unwrap().allow = false;
        } else {
            host.state.lock().unwrap().block_capability_on = 3;
        }
        let runner =
            DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
        let mut app = open(&db, host, runner.clone(), Startup::CreateTest);
        let p = plan();
        let request = &p.spec().request.request_id;
        let result = app.submit(request, &p);
        if denial {
            assert_eq!(result.unwrap_err(), Error::AdmissionRejected);
        }
        assert_eq!(runner.dispatch_count(), 0);
        assert_eq!(db.count("attempts"), if denial { 0 } else { 1 });
        assert_eq!(
            app.submit(request, &p).unwrap().attempts,
            if denial { 0 } else { 1 }
        );
        assert_eq!(runner.dispatch_count(), 0);
    }
}

#[test]
fn unknown_dispatch_and_lost_runner_memory_never_synthesize_success() {
    let db = Database::new();
    let host = TestHost::new();
    let p = plan();
    let r = &p.spec().request.request_id;
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Unknown, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    assert_eq!(app.submit(r, &p).unwrap().phase, TaskPhase::OutcomeUnknown);
    assert_eq!(app.reconcile(r).unwrap().phase, TaskPhase::OutcomeUnknown);
    drop(app);
    let fresh =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let mut app = open(&db, host, fresh.clone(), Startup::OpenTest);
    assert_eq!(app.reconcile(r).unwrap().phase, TaskPhase::OutcomeUnknown);
    app.submit(r, &p).unwrap();
    assert_eq!(fresh.dispatch_count(), 0);
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
fn cancellation_degraded_and_new_attempt_authorization_are_separate() {
    let db = Database::new();
    let host = TestHost::new();
    let p = plan();
    let r = &p.spec().request.request_id;
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let mut app = open(&db, host, runner, Startup::CreateTest);
    app.submit(r, &p).unwrap();
    app.configuration_load_failed(2);
    assert_eq!(app.configuration_state(), ConfigState::Degraded);
    assert_eq!(
        app.advance(r, &command("retry")).unwrap_err(),
        Error::Degraded
    );
    assert!(app.cancel(r).unwrap().cancel_requested);
    assert_eq!(app.reconcile(r).unwrap().phase, TaskPhase::Cancelled);
    assert_eq!(app.status(r).unwrap().attempts, 1);
}

#[test]
fn transaction_failure_rolls_back_intent_and_approval_before_dispatch() {
    let db = Database::new();
    let host = TestHost::new();
    host.state.lock().unwrap().approval = true;
    host.grant(1);
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let mut app = open(&db, host, runner.clone(), Startup::CreateTest);
    db.sql().execute_batch("CREATE TRIGGER fail_attempt AFTER INSERT ON attempts BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;").unwrap();
    let p = plan();
    let r = &p.spec().request.request_id;
    assert!(app.submit(r, &p).is_err());
    assert_eq!(runner.dispatch_count(), 0);
    assert_eq!(db.count("attempts"), 0);
    assert_eq!(db.count("approval_consumptions"), 0);
    db.sql()
        .execute_batch("DROP TRIGGER fail_attempt;")
        .unwrap();
    assert_eq!(app.submit(r, &p).unwrap().attempts, 0);
    assert_eq!(
        app.advance(r, &CommandId::initial_attempt())
            .unwrap()
            .attempts,
        1
    );
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
fn concurrent_instances_and_result_redelivery_do_not_duplicate_effects() {
    let db = Database::new();
    let host = TestHost::new();
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    drop(open(&db, host.clone(), runner.clone(), Startup::CreateTest));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        let mut joins = vec![];
        for _ in 0..2 {
            let host = host.clone();
            let runner = runner.clone();
            let barrier = barrier.clone();
            let db = &db;
            joins.push(scope.spawn(move || {
                let mut app = open(db, host, runner, Startup::OpenTest);
                let p = plan();
                barrier.wait();
                app.submit(&p.spec().request.request_id, &p).unwrap()
            }));
        }
        for join in joins {
            join.join().unwrap();
        }
    });
    assert_eq!(runner.dispatch_count(), 1);
    assert_eq!(db.count("attempts"), 1);
    let mut app = open(&db, host.clone(), runner.clone(), Startup::OpenTest);
    let p = plan();
    let r = &p.spec().request.request_id;
    let events = app.pull_results(r, &id("ui"), 64).unwrap();
    assert_eq!(events, app.pull_results(r, &id("ui"), 64).unwrap());
    app.confirm(r, &id("ui"), &events.last().unwrap().event_id)
        .unwrap();
    assert_eq!(
        app.pull_results(r, &id("ui"), 64).unwrap().first(),
        events.first()
    );
    host.state.lock().unwrap().audit = false;
    assert_eq!(
        app.audit(r, &events[0].operation_id).unwrap_err(),
        Error::Denied
    );
    host.state.lock().unwrap().read = false;
    assert_eq!(app.status(r).unwrap_err(), Error::Denied);
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
fn ordinary_answers_and_model_approval_references_never_grant_authority() {
    use execution_interaction::{Command, ConfirmationPurpose, Kind, Reference, Response};
    let db = Database::new();
    let host = TestHost::new();
    host.state.lock().unwrap().approval = true;
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    let p = plan();
    let r = &p.spec().request.request_id;
    let reference = |s: &str| Reference::new(s).unwrap();
    assert_eq!(app.submit(r, &p).unwrap_err(), Error::AdmissionRejected);
    for (name, kind, response) in [
        (
            "confirm",
            Kind::UserConfirmation {
                purpose: ConfirmationPurpose::Continue,
            },
            Response::Confirmation { accepted: true },
        ),
        (
            "admin",
            Kind::AdministratorAuthorization {
                request: reference("approval-request"),
            },
            Response::AdministratorDecision {
                record: reference("model-claims-approved"),
            },
        ),
    ] {
        let interaction = reference(name);
        app.open_interaction(r, interaction.clone(), kind, 1900)
            .unwrap();
        let receipt = app
            .respond(
                r,
                &command(name),
                &interaction,
                &Command::Answer {
                    id: reference(&format!("answer-{name}")),
                    response,
                },
            )
            .unwrap();
        assert_eq!(receipt.outcome, execution_sqlite::Outcome::Answered);
        assert_eq!(
            app.advance(r, &command(name)).unwrap_err(),
            Error::AdmissionRejected
        );
    }
    assert_eq!(runner.dispatch_count(), 0);
    assert_eq!(db.count("approval_consumptions"), 0);
    host.grant(1);
    assert_eq!(
        app.advance(r, &command("trusted-grant")).unwrap().attempts,
        1
    );
    assert_eq!(db.count("approval_consumptions"), 1);
}

#[test]
fn retry_uses_fresh_policy_and_never_refunds_previous_attempt() {
    let db = Database::new();
    let mut host = TestHost::new();
    let mut spec = host.template.spec().clone();
    spec.budget.max_attempts = 2;
    host.template = FrozenPlan::freeze(spec, &test_store_limits().plan).unwrap();
    let p = host.template.clone();
    let r = &p.spec().request.request_id;
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::NoEffect, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    app.submit(r, &p).unwrap();
    app.reconcile(r).unwrap();
    host.state.lock().unwrap().allow = false;
    assert_eq!(
        app.advance(r, &command("retry-denied")).unwrap_err(),
        Error::AdmissionRejected
    );
    assert_eq!(app.status(r).unwrap().attempts, 1);
    host.state.lock().unwrap().allow = true;
    assert_eq!(
        app.advance(r, &command("retry-denied")).unwrap_err(),
        Error::AdmissionRejected
    );
    assert_eq!(
        app.advance(r, &command("retry-allowed")).unwrap().attempts,
        2
    );
    assert_eq!(runner.dispatch_count(), 2);
    app.reconcile(r).unwrap();
    assert_eq!(
        app.advance(r, &command("third")).unwrap_err(),
        Error::Conflict
    );
}

#[test]
fn committed_intent_process_exit_is_not_reissued_after_restart() {
    struct ExitRunner;
    impl RunnerPort for ExitRunner {
        fn id(&self) -> Id {
            id("test-runner")
        }
        fn mode(&self) -> execution_lifecycle::ExecutionMode {
            execution_lifecycle::ExecutionMode::Test
        }
        fn dispatch(&self, _: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
            std::process::exit(72)
        }
        fn stop(&self, _: &FrozenPlan, _: &AttemptId) -> Result<(), Error> {
            unreachable!()
        }
        fn observe(
            &self,
            _: &FrozenPlan,
            _: &AttemptId,
            _: ObservationStage,
            _: u64,
        ) -> Result<Option<execution_lifecycle::ObservationFacts>, Error> {
            unreachable!()
        }
    }
    const ENV: &str = "EXECUTION_APP_CRASH_FIXTURE";
    if let Some(path) = std::env::var_os(ENV) {
        let mut app = ExecutionApp::start(
            std::path::Path::new(&path),
            Startup::OpenTest,
            TestHost::new(),
            ExitRunner,
            AppConfig::test_defaults(1),
        )
        .unwrap();
        let p = plan();
        app.submit(&p.spec().request.request_id, &p).unwrap();
        unreachable!();
    }
    let db = Database::new();
    let host = TestHost::new();
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    drop(open(&db, host.clone(), runner.clone(), Startup::CreateTest));
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "committed_intent_process_exit_is_not_reissued_after_restart",
        ])
        .env(ENV, &db.path)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(72));
    let mut app = open(&db, host, runner.clone(), Startup::OpenTest);
    let p = plan();
    let r = &p.spec().request.request_id;
    assert_eq!(app.submit(r, &p).unwrap().attempts, 1);
    assert_eq!(app.reconcile(r).unwrap().phase, TaskPhase::OutcomeUnknown);
    assert_eq!(runner.dispatch_count(), 0);
}

#[test]
fn human_ai_and_policy_use_the_same_actor_authorization_and_test_provenance() {
    let template = plan();
    let Initiator::Human { os_session } = template.spec().request.initiator.clone() else {
        panic!()
    };
    for initiator in [
        template.spec().request.initiator.clone(),
        Initiator::Ai {
            provider: id("fixture-provider"),
            os_session,
            provider_account: ProviderAccountRef {
                account: id("provider-login-not-authority"),
                config: reference("ai-config"),
            },
            conversation: id("conversation"),
            tool_call: id("tool"),
        },
        Initiator::Policy {
            policy: template.spec().policy.clone(),
        },
    ] {
        for allow in [false, true] {
            let db = Database::new();
            let host = TestHost::new();
            host.state.lock().unwrap().allow = allow;
            let runner =
                DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16)
                    .unwrap();
            let mut spec = template.spec().clone();
            spec.request.initiator = initiator.clone();
            let p = FrozenPlan::freeze(spec, &test_store_limits().plan).unwrap();
            let r = &p.spec().request.request_id;
            let mut app = open(&db, host, runner.clone(), Startup::CreateTest);
            if allow {
                app.submit(r, &p).unwrap();
                assert_eq!(
                    app.reconcile(r).unwrap().mode,
                    execution_lifecycle::ExecutionMode::Test
                );
            } else {
                assert_eq!(app.submit(r, &p).unwrap_err(), Error::AdmissionRejected);
            }
            assert_eq!(runner.dispatch_count(), usize::from(allow));
        }
    }
}

#[test]
fn scope_conflicts_capability_failure_and_expiry_are_fail_closed() {
    let db = Database::new();
    let host = TestHost::new();
    let p = plan();
    let r = &p.spec().request.request_id;
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    host.state.lock().unwrap().capability = false;
    assert_eq!(app.submit(r, &p).unwrap_err(), Error::Capability);
    assert_eq!(app.status(r).unwrap().attempts, 0);
    host.state.lock().unwrap().capability = true;
    app.advance(r, &CommandId::initial_attempt()).unwrap();
    host.state.lock().unwrap().now = 2000;
    let stopped = app.reconcile(r).unwrap();
    assert_eq!(
        stopped.assessment,
        Some(execution_lifecycle::EffectAssessment::NoEffect)
    );
    assert_eq!(
        app.advance(r, &command("expired")).unwrap_err(),
        Error::Capability
    );
    host.state.lock().unwrap().actor = ActorId::new("other-actor").unwrap();
    assert_eq!(app.status(r).unwrap_err(), Error::Denied);
    assert_eq!(app.submit(r, &p).unwrap_err(), Error::Denied);
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
fn dispatch_gate_rechecks_cancel_and_deadline_after_capability_verification() {
    for cancellation in [false, true] {
        let db = Database::new();
        let mut host = TestHost::new();
        let mut spec = host.template.spec().clone();
        spec.budget.total_timeout_ms = 10;
        host.template = FrozenPlan::freeze(spec, &test_store_limits().plan).unwrap();
        let p = host.template.clone();
        let r = p.spec().request.request_id.clone();
        let runner =
            DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
        let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
        let path = db.path.clone();
        let other_host = host.clone();
        let other_runner = runner.clone();
        let other_request = r.clone();
        host.state.lock().unwrap().capability_hook = Some((
            3,
            std::sync::Arc::new(move || {
                if cancellation {
                    let mut other = ExecutionApp::start(
                        &path,
                        Startup::OpenTest,
                        other_host.clone(),
                        other_runner.clone(),
                        AppConfig::test_defaults(1),
                    )
                    .unwrap();
                    assert!(other.cancel(&other_request).unwrap().cancel_requested);
                } else {
                    other_host.state.lock().unwrap().now = 1010;
                }
            }),
        ));
        app.submit(&r, &p).unwrap();
        assert_eq!(runner.dispatch_count(), 0, "cancellation={cancellation}");
    }
}

#[test]
fn stale_cancel_is_rebased_and_not_permanently_replayed_as_success() {
    let db = Database::new();
    let host = TestHost::new();
    let p = plan();
    let r = p.spec().request.request_id.clone();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    app.submit(&r, &p).unwrap();
    let path = db.path.clone();
    let other_host = host.clone();
    let other_runner = runner.clone();
    let other_request = r.clone();
    host.state.lock().unwrap().read_hook = Some(std::sync::Arc::new(move || {
        let mut other = ExecutionApp::start(
            &path,
            Startup::OpenTest,
            other_host.clone(),
            other_runner.clone(),
            AppConfig::test_defaults(1),
        )
        .unwrap();
        other.reconcile(&other_request).unwrap(); // durable Recover advances the revision of the running wait
    }));
    assert!(app.cancel(&r).unwrap().cancel_requested);
    assert!(app.cancel(&r).unwrap().cancel_requested);
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
fn read_permission_cannot_invoke_runner_recovery() {
    let db = Database::new();
    let host = TestHost::new();
    let p = plan();
    let r = &p.spec().request.request_id;
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    let accepted = app.submit(r, &p).unwrap();
    host.state.lock().unwrap().now = 2000; // recovery would request stop if authorized
    host.state.lock().unwrap().runner_facts = false;
    assert_eq!(app.reconcile(r).unwrap_err(), Error::Denied);
    assert!(runner
        .observe(
            &p,
            accepted.attempt_id.as_ref().unwrap(),
            ObservationStage::Termination,
            2000
        )
        .unwrap()
        .is_none());
}

#[test]
fn commit_unknown_recovery_classifications_are_not_interchangeable() {
    assert_eq!(
        Error::from(execution_sqlite::Error::OperationCommitUnknown),
        Error::OutcomeUnknown
    );
    assert_eq!(
        Error::from(execution_sqlite::Error::ConfirmationCommitUnknown),
        Error::ConfirmationUnknown
    );
}

#[test]
fn same_observation_can_commit_after_a_concurrent_cancel_advances_revision() {
    let db = Database::new();
    let host = TestHost::new();
    let p = plan();
    let r = p.spec().request.request_id.clone();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    app.submit(&r, &p).unwrap();
    let path = db.path.clone();
    let other_host = host.clone();
    let other_runner = runner.clone();
    let other_request = r.clone();
    host.state.lock().unwrap().read_hook = Some(std::sync::Arc::new(move || {
        let mut other = ExecutionApp::start(
            &path,
            Startup::OpenTest,
            other_host.clone(),
            other_runner.clone(),
            AppConfig::test_defaults(1),
        )
        .unwrap();
        other.cancel(&other_request).unwrap();
    }));
    app.reconcile(&r).unwrap();
    let completed = app.reconcile(&r).unwrap();
    assert_eq!(completed.phase, TaskPhase::Cancelled);
    assert_eq!(completed.evidence.len(), 2);
    assert_eq!(runner.dispatch_count(), 1);
}
