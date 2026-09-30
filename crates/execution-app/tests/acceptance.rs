mod support;
use execution_app::*;
use execution_contract::*;
use support::*;
fn command(value: &str) -> CommandId {
    CommandId::new(value).unwrap()
}

#[test]
fn service_delivery_requires_both_delivery_and_evidence_access() {
    let db = Database::new();
    let host = TestHost::new();
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let mut app = open(&db, host.clone(), runner, Startup::CreateTest);
    let p = plan();
    let request = &p.spec().request.request_id;
    app.request_execution(&caller(), &p).unwrap();
    app.reconcile(request).unwrap();
    let consumer = id("agent-delivery");
    let items = app.service_delivery(request, &consumer, 64).unwrap();
    assert!(!items.is_empty());
    assert!(items.iter().all(|v| v.input.digest() == p.digest()));
    host.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::Deliver]);
    assert!(matches!(
        app.service_delivery(request, &consumer, 64),
        Err(Error::Denied)
    ));
    host.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::RunnerFact]);
    assert!(matches!(
        app.service_delivery(request, &consumer, 64),
        Err(Error::Denied)
    ));
    host.state.lock().unwrap().accesses = None;
    for item in items {
        app.service_confirm(request, &consumer, &item.receipt.event_id)
            .unwrap();
    }
    assert!(app
        .service_delivery(request, &consumer, 64)
        .unwrap()
        .is_empty());
}

#[test]
fn one_shot_request_replay_never_dispatches() {
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    let p = plan();
    assert_eq!(runner.dispatch_count(), 0);
    assert_eq!(app.request_execution(&caller(), &p).unwrap().attempts, 1);
    app.configuration_load_failed(2);
    host.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::ReadResult]);
    assert_eq!(app.request_execution(&caller(), &p).unwrap().attempts, 1);
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
fn task_details_are_authorized_frozen_and_redacted() {
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let p = plan();
    let r = &p.spec().request.request_id;
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    app.request_execution(&caller(), &p).unwrap();
    let details = app.task_details(&caller(), r).unwrap();
    assert_eq!(details.status, app.status(&caller(), r).unwrap());
    assert_eq!(&details.action.content_digest, p.digest());
    assert_eq!(details.action.target, p.spec().request.target);
    assert_eq!(details.action.run_as, p.spec().run_as);
    assert_eq!(details.action.actor, p.spec().request.actor);
    assert_eq!(details.action.initiator, p.spec().request.initiator);
    let json = serde_json::to_string(&details).unwrap();
    for field in [
        "parameters",
        "argv",
        "cwd",
        "env",
        "stdin",
        "delegation",
        "approvalBindings",
        "readPaths",
        "writePaths",
    ] {
        assert!(!json.contains(&format!("\"{field}\":")), "{field}");
    }
    drop(app);
    let app = open(&db, host.clone(), runner, Startup::OpenTest);
    assert_eq!(
        app.task_details(&caller(), r).unwrap().action,
        details.action
    );
    host.state.lock().unwrap().read = false;
    assert_eq!(app.task_details(&caller(), r).unwrap_err(), Error::Denied);
    host.state.lock().unwrap().read = true;
    host.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::ReadAudit]);
    assert_eq!(app.task_details(&caller(), r).unwrap_err(), Error::Denied);
}

#[test]
fn task_details_recheck_current_binding_after_authorized_record_read() {
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let p = plan();
    let r = &p.spec().request.request_id;
    let mut app = open(&db, host.clone(), runner, Startup::CreateTest);
    app.request_execution(&caller(), &p).unwrap();
    let changed = host.clone();
    host.state.lock().unwrap().read_hook = Some(std::sync::Arc::new(move || {
        changed.state.lock().unwrap().bound = false;
    }));
    assert_eq!(app.task_details(&caller(), r).unwrap_err(), Error::Unbound);
}

#[test]
fn action_permissions_do_not_require_result_reading() {
    let db = Database::new();
    let host = TestHost::new();
    host.state.lock().unwrap().read = false;
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let p = plan();
    let r = &p.spec().request.request_id;
    let mut app = open(&db, host.clone(), runner, Startup::CreateTest);
    app.request_execution(&caller(), &p).unwrap();
    app.request_execution(&caller(), &p).unwrap();
    app.reconcile(r).unwrap();
    app.cancel(&caller(), r).unwrap();
    assert_eq!(app.status(&caller(), r).unwrap_err(), Error::Denied);
    let results = app.pull_results(&caller(), r, &id("consumer"), 64).unwrap();
    app.confirm(&caller(), r, &id("consumer"), &results[0].event_id)
        .unwrap();
    app.audit(&caller(), r, &results[0].operation_id).unwrap();
    use execution_sqlite::Access;
    host.state.lock().unwrap().accesses = Some(vec![Access::ReadAudit]);
    app.audit(&caller(), r, &results[0].operation_id).unwrap();
    assert_eq!(app.cancel(&caller(), r).unwrap_err(), Error::Denied);
    assert_eq!(app.reconcile(r).unwrap_err(), Error::Denied);
    assert_eq!(
        app.pull_results(&caller(), r, &id("consumer"), 64)
            .unwrap_err(),
        Error::Denied
    );
    host.state.lock().unwrap().accesses = Some(vec![Access::Deliver]);
    host.state.lock().unwrap().consumer = Some(id("consumer"));
    app.pull_results(&caller(), r, &id("consumer"), 64).unwrap();
    app.confirm(&caller(), r, &id("consumer"), &results[0].event_id)
        .unwrap();
    assert_eq!(
        app.pull_results(&caller(), r, &id("other"), 64)
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(
        app.confirm(&caller(), r, &id("other"), &results[0].event_id)
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(
        app.audit(&caller(), r, &results[0].operation_id)
            .unwrap_err(),
        Error::Denied
    );
    host.state.lock().unwrap().accesses = Some(vec![Access::Interact]);
    use execution_interaction::{Command, ConfirmationPurpose, Kind, Reference, Response};
    let interaction = Reference::new("question").unwrap();
    app.open_interaction(
        &caller(),
        r,
        interaction.clone(),
        Kind::UserConfirmation {
            purpose: ConfirmationPurpose::Continue,
        },
        1900,
    )
    .unwrap();
    let answer = Command::Answer {
        id: Reference::new("answer").unwrap(),
        response: Response::Confirmation { accepted: true },
    };
    app.respond(&caller(), r, &command("answer"), &interaction, &answer)
        .unwrap();
    app.respond(&caller(), r, &command("answer"), &interaction, &answer)
        .unwrap();
    assert_eq!(
        app.interaction(&caller(), r, &interaction).unwrap_err(),
        Error::Denied
    );
    host.state.lock().unwrap().read = true;
    host.state.lock().unwrap().accesses = Some(vec![Access::ReadResult]);
    app.request_execution(&caller(), &p).unwrap(); // read-only replay still cannot execute or consume approvals
    assert_eq!(
        app.retry_execution(&caller(), r, &command("new-attempt"))
            .unwrap_err(),
        Error::Denied
    );
}

#[test]
fn execute_only_cancel_and_runner_fact_only_reconcile_are_separate() {
    use execution_sqlite::Access;
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let p = plan();
    let r = &p.spec().request.request_id;
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    let accepted = app.request_execution(&caller(), &p).unwrap();
    host.state.lock().unwrap().accesses = Some(vec![Access::Execute]);
    let cancelled = app.cancel(&caller(), r).unwrap();
    assert!(cancelled.cancel_requested);
    assert_eq!(cancelled.stop_outcome, None);
    assert!(runner
        .observe(
            &p,
            accepted.attempt_id.as_ref().unwrap(),
            ObservationStage::Termination,
            1000
        )
        .unwrap()
        .is_none());
    assert_eq!(app.reconcile(r).unwrap_err(), Error::Denied);
    host.state.lock().unwrap().accesses = Some(vec![Access::RunnerFact]);
    let result = app.reconcile(r).unwrap();
    assert_eq!(result.phase, TaskPhase::Cancelled);
    assert_eq!(
        result.stop_outcome,
        Some(execution_lifecycle::StopOutcome::Acknowledged)
    );
    assert_eq!(result.evidence.len(), 2);
    assert_eq!(app.cancel(&caller(), r).unwrap_err(), Error::Denied);
}

#[test]
fn failed_stop_diagnostic_write_still_allows_terminal_fact_writes() {
    let db = Database::new();
    let host = TestHost::new();
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let p = plan();
    let r = &p.spec().request.request_id;
    let mut app = open(&db, host, runner, Startup::CreateTest);
    app.request_execution(&caller(), &p).unwrap();
    app.cancel(&caller(), r).unwrap();
    db.sql().execute_batch("CREATE TRIGGER fail_stop BEFORE UPDATE OF snapshot ON executions WHEN json_extract(CAST(NEW.snapshot AS TEXT), '$.lastEvent.event.command.kind')='stopReported' BEGIN SELECT RAISE(ABORT, 'fixture stop write failure'); END;").unwrap();
    assert_eq!(app.reconcile(r).unwrap_err(), Error::Conflict);
    let status = app.status(&caller(), r).unwrap();
    assert!(status.cancel_requested);
    assert_eq!(status.phase, TaskPhase::Verified);
    assert_eq!(status.evidence.len(), 2);
    assert_eq!(status.stop_outcome, None); // failed persistence is reported, never claimed successful
}

#[test]
fn dispatch_gate_detects_reconciliation_winning_the_revision() {
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let p = plan();
    let r = p.spec().request.request_id.clone();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    let path = db.path.clone();
    let other_host = host.clone();
    let other_runner = runner.clone();
    let other_request = r.clone();
    host.state.lock().unwrap().capability_hook = Some((
        3,
        std::sync::Arc::new(move || {
            let mut other = ExecutionApp::start(
                &path,
                Startup::OpenTest,
                other_host.clone(),
                other_runner.clone(),
                AppConfig::test_defaults(1),
            )
            .unwrap();
            assert_eq!(
                other.reconcile(&other_request).unwrap().phase,
                TaskPhase::OutcomeUnknown
            );
        }),
    ));
    let status = app.request_execution(&caller(), &p).unwrap();
    assert_eq!(
        status.dispatch_cause,
        Some(execution_lifecycle::DispatchCause::StaleRevision)
    );
    assert_eq!(runner.dispatch_count(), 0);
    assert_eq!(status.assessment, None);
}

#[test]
fn admission_rejection_is_consistent_on_replay_and_restart() {
    for approval in [false, true] {
        let db = Database::new();
        let host = TestHost::new();
        host.state.lock().unwrap().allow = approval;
        host.state.lock().unwrap().approval = approval;
        let runner =
            DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
        let p = plan();
        let r = &p.spec().request.request_id;
        let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
        let initial = app.request_execution(&caller(), &p);
        let expected = if approval {
            TaskPhase::ApprovalRequired
        } else {
            TaskPhase::AdmissionDenied
        };
        assert_eq!(initial.as_ref().unwrap().phase, expected);
        host.state.lock().unwrap().audit = false;
        assert_eq!(app.status(&caller(), r), initial);
        assert_eq!(app.request_execution(&caller(), &p), initial);
        assert_eq!(app.status(&caller(), r), initial);
        drop(app);
        let mut app = open(&db, host, runner.clone(), Startup::OpenTest);
        assert_eq!(app.request_execution(&caller(), &p), initial);
        assert_eq!(runner.dispatch_count(), 0);
    }
}

#[test]
fn newer_schema_retains_read_only_startup_diagnostic() {
    let db = Database::new();
    let host = TestHost::new();
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    drop(open(&db, host.clone(), runner.clone(), Startup::CreateTest));
    db.sql().execute_batch("PRAGMA user_version=999;").unwrap();
    let before = std::fs::read(&db.path).unwrap();
    let error = match ExecutionApp::start(
        &db.path,
        Startup::OpenTest,
        host,
        runner.clone(),
        AppConfig::test_defaults(1),
    ) {
        Ok(_) => panic!("a newer schema must not produce an app/execute/confirm handle"),
        Err(error) => error,
    };
    assert_eq!(
        format!("{error:?}"),
        "UnsupportedSchema { found: 999, supported: 5 }"
    );
    assert_eq!(std::fs::read(&db.path).unwrap(), before);
    assert_eq!(runner.dispatch_count(), 0);
}

#[test]
fn stop_failure_does_not_prevent_termination_and_effect_observation() {
    struct StopFailure(DeterministicTestRunner);
    impl RunnerPort for StopFailure {
        fn id(&self) -> Id {
            self.0.id()
        }
        fn mode(&self) -> execution_lifecycle::ExecutionMode {
            self.0.mode()
        }
        fn dispatch(&self, permit: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
            self.0.dispatch(permit)
        }
        fn evidence(
            &self,
            _plan: &execution_contract::FrozenExecution,
            _attempt: &execution_contract::AttemptId,
        ) -> Result<Option<execution_contract::ProcessEvidence>, execution_app::Error> {
            Ok(None)
        }
        fn acknowledge_capture(
            &self,
            _: &execution_contract::FrozenExecution,
            _: &execution_contract::ProcessEvidence,
        ) -> Result<(), execution_app::Error> {
            Ok(())
        }
        fn stop(&self, _: &FrozenExecution, _: &AttemptId) -> Result<(), Error> {
            Err(Error::Unavailable)
        }
        fn observe(
            &self,
            plan: &FrozenExecution,
            attempt: &AttemptId,
            stage: ObservationStage,
            now: u64,
        ) -> Result<Option<execution_lifecycle::ObservationFacts>, Error> {
            self.0.observe(plan, attempt, stage, now)
        }
    }
    let db = Database::new();
    let host = TestHost::new();
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let p = plan();
    let r = &p.spec().request.request_id;
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host.clone(),
        StopFailure(runner),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    app.request_execution(&caller(), &p).unwrap();
    host.state.lock().unwrap().now = 2000;
    let status = app.reconcile(r).unwrap();
    assert_eq!(status.phase, TaskPhase::Verified);
    assert_eq!(status.evidence.len(), 2);
    assert_eq!(
        status.stop_outcome,
        Some(execution_lifecycle::StopOutcome::Failed)
    );
    let diagnostics: Vec<_> = app
        .pull_results(&caller(), r, &id("diagnostic"), 64)
        .unwrap()
        .into_iter()
        .map(|r| {
            app.audit(&caller(), &p.spec().request.request_id, &r.operation_id)
                .unwrap()
        })
        .filter(|a| a.stop_outcome.is_some())
        .collect();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].attempt_id, status.attempt_id);
    assert_eq!(diagnostics[0].stop_outcome, status.stop_outcome);
    drop(app);
    let fresh =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let app = open(&db, host, fresh, Startup::OpenTest);
    assert_eq!(app.status(&caller(), r).unwrap(), status);
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
    let accepted = app.request_execution(&caller(), &p).unwrap();
    assert_eq!(accepted.attempts, 1);
    let (response, disconnected) = std::sync::mpsc::channel();
    drop(disconnected);
    assert!(response.send(accepted.clone()).is_err());
    assert_eq!(app.reconcile(request).unwrap().phase, TaskPhase::Verified);
    drop(app);
    let mut app = open(&db, host.clone(), runner.clone(), Startup::OpenTest);
    assert_eq!(
        app.request_execution(&caller(), &p).unwrap().attempt_id,
        accepted.attempt_id
    );
    assert_eq!(runner.dispatch_count(), 1);
    assert_eq!(db.count("approval_consumptions"), 1);
    assert_eq!(host.state.lock().unwrap().admissions, 2);
    let completed = app.reconcile(request).unwrap();
    assert_eq!(completed.phase, TaskPhase::Verified);
    assert!(completed
        .evidence
        .iter()
        .all(|e| e.kind == EvidenceKind::TestResult));
    assert_eq!(app.reconcile(request).unwrap(), completed);
    let mut changed = p.spec().clone();
    changed.budget.max_attempts = 2;
    let changed = FrozenExecution::freeze(changed, &test_store_limits().input).unwrap();
    assert_eq!(
        app.request_execution(&caller(), &changed).unwrap_err(),
        Error::Conflict
    );
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
        let result = app.request_execution(&caller(), &p);
        if denial {
            assert_eq!(result.unwrap().phase, TaskPhase::AdmissionDenied);
        }
        assert_eq!(runner.dispatch_count(), 0);
        assert_eq!(db.count("attempts"), if denial { 0 } else { 1 });
        assert_eq!(
            app.request_execution(&caller(), &p).unwrap().attempts,
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
    assert_eq!(
        app.request_execution(&caller(), &p).unwrap().phase,
        TaskPhase::OutcomeUnknown
    );
    assert_eq!(app.reconcile(r).unwrap().phase, TaskPhase::OutcomeUnknown);
    drop(app);
    let fresh =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
    let mut app = open(&db, host, fresh.clone(), Startup::OpenTest);
    assert_eq!(app.reconcile(r).unwrap().phase, TaskPhase::OutcomeUnknown);
    app.request_execution(&caller(), &p).unwrap();
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
    app.request_execution(&caller(), &p).unwrap();
    app.configuration_load_failed(2);
    assert_eq!(app.configuration_state(), ConfigState::Degraded);
    assert_eq!(
        app.retry_execution(&caller(), r, &command("retry"))
            .unwrap_err(),
        Error::Degraded
    );
    assert!(app.cancel(&caller(), r).unwrap().cancel_requested);
    assert_eq!(app.reconcile(r).unwrap().phase, TaskPhase::Cancelled);
    assert_eq!(app.status(&caller(), r).unwrap().attempts, 1);
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
    assert!(app.request_execution(&caller(), &p).is_err());
    assert_eq!(runner.dispatch_count(), 0);
    assert_eq!(db.count("attempts"), 0);
    assert_eq!(db.count("approval_consumptions"), 0);
    db.sql()
        .execute_batch("DROP TRIGGER fail_attempt;")
        .unwrap();
    assert_eq!(app.request_execution(&caller(), &p).unwrap().attempts, 1);
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
                app.request_execution(&caller(), &p).unwrap()
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
    let events = app.pull_results(&caller(), r, &id("ui"), 64).unwrap();
    assert_eq!(
        events,
        app.pull_results(&caller(), r, &id("ui"), 64).unwrap()
    );
    app.confirm(&caller(), r, &id("ui"), &events.last().unwrap().event_id)
        .unwrap();
    assert_eq!(
        app.pull_results(&caller(), r, &id("ui"), 64)
            .unwrap()
            .first(),
        events.first()
    );
    host.state.lock().unwrap().audit = false;
    assert_eq!(
        app.audit(&caller(), r, &events[0].operation_id)
            .unwrap_err(),
        Error::Denied
    );
    host.state.lock().unwrap().read = false;
    assert_eq!(app.status(&caller(), r).unwrap_err(), Error::Denied);
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
    assert_eq!(
        app.request_execution(&caller(), &p).unwrap().phase,
        TaskPhase::ApprovalRequired
    );
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
        app.open_interaction(&caller(), r, interaction.clone(), kind, 1900)
            .unwrap();
        let receipt = app
            .respond(
                &caller(),
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
            app.retry_execution(&caller(), r, &command(name))
                .unwrap()
                .admission,
            Some(execution_sqlite::AdmissionStatus::ApprovalRequired)
        );
    }
    assert_eq!(runner.dispatch_count(), 0);
    assert_eq!(db.count("approval_consumptions"), 0);
    host.grant(1);
    assert_eq!(
        app.retry_execution(&caller(), r, &command("trusted-grant"))
            .unwrap()
            .attempts,
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
    host.template = FrozenExecution::freeze(spec, &test_store_limits().input).unwrap();
    let p = host.template.clone();
    let r = &p.spec().request.request_id;
    let runner =
        DeterministicTestRunner::new(id("test-runner"), TestScenario::NoEffect, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    app.request_execution(&caller(), &p).unwrap();
    app.reconcile(r).unwrap();
    host.state.lock().unwrap().allow = false;
    assert_eq!(
        app.retry_execution(&caller(), r, &command("retry-denied"))
            .unwrap()
            .admission,
        Some(execution_sqlite::AdmissionStatus::Denied)
    );
    assert_eq!(app.status(&caller(), r).unwrap().attempts, 1);
    host.state.lock().unwrap().allow = true;
    assert_eq!(
        app.retry_execution(&caller(), r, &command("retry-denied"))
            .unwrap()
            .admission,
        Some(execution_sqlite::AdmissionStatus::Denied)
    );
    assert_eq!(
        app.retry_execution(&caller(), r, &command("retry-allowed"))
            .unwrap()
            .attempts,
        2
    );
    assert_eq!(runner.dispatch_count(), 2);
    app.reconcile(r).unwrap();
    assert_eq!(
        app.retry_execution(&caller(), r, &command("third"))
            .unwrap_err(),
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
        fn evidence(
            &self,
            _plan: &execution_contract::FrozenExecution,
            _attempt: &execution_contract::AttemptId,
        ) -> Result<Option<execution_contract::ProcessEvidence>, execution_app::Error> {
            Ok(None)
        }
        fn acknowledge_capture(
            &self,
            _: &execution_contract::FrozenExecution,
            _: &execution_contract::ProcessEvidence,
        ) -> Result<(), execution_app::Error> {
            Ok(())
        }
        fn stop(&self, _: &FrozenExecution, _: &AttemptId) -> Result<(), Error> {
            unreachable!()
        }
        fn observe(
            &self,
            _: &FrozenExecution,
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
        app.request_execution(&caller(), &p).unwrap();
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
    assert_eq!(app.request_execution(&caller(), &p).unwrap().attempts, 1);
    assert_eq!(app.reconcile(r).unwrap().phase, TaskPhase::OutcomeUnknown);
    assert_eq!(runner.dispatch_count(), 0);
}

#[test]
fn human_ai_and_policy_use_the_same_actor_authorization_and_test_provenance() {
    let template = FrozenExecution::freeze(
        decode_execution(
            include_bytes!("../../execution-contract/tests/fixtures/plan.json"),
            &test_store_limits().input,
        )
        .unwrap(),
        &test_store_limits().input,
    )
    .unwrap();
    let Initiator::Human { os_session } = template.spec().request.initiator.clone() else {
        panic!()
    };
    for initiator in [
        template.spec().request.initiator.clone(),
        Initiator::Ai {
            provider: id("fixture-provider"),
            os_session,
            config: reference("ai-config"),
            conversation: id("conversation"),
            tool_call: id("tool"),
        },
        Initiator::Policy {
            policy: template.spec().policy.clone(),
        },
    ] {
        for allow in [false, true] {
            let db = Database::new();
            let mut host = TestHost::new();
            host.state.lock().unwrap().allow = allow;
            let runner =
                DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16)
                    .unwrap();
            let mut spec = template.spec().clone();
            spec.request.initiator = initiator.clone();
            let p = FrozenExecution::freeze(spec, &test_store_limits().input).unwrap();
            host.template = p.clone();
            let r = &p.spec().request.request_id;
            let mut app = open(&db, host, runner.clone(), Startup::CreateTest);
            if allow {
                app.request_execution(&caller(), &p).unwrap();
                if matches!(initiator, Initiator::Human { .. }) {
                    app.confirm_execution(&caller(), r, p.digest(), true)
                        .unwrap();
                }
                assert_eq!(
                    app.reconcile(r).unwrap().mode,
                    execution_lifecycle::ExecutionMode::Test
                );
            } else {
                assert_eq!(
                    app.request_execution(&caller(), &p).unwrap().phase,
                    TaskPhase::AdmissionDenied
                );
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
    assert_eq!(
        app.request_execution(&caller(), &p).unwrap_err(),
        Error::Capability
    );
    assert_eq!(app.status(&caller(), r).unwrap().attempts, 0);
    host.state.lock().unwrap().capability = true;
    app.request_execution(&caller(), &p).unwrap();
    host.state.lock().unwrap().now = 2000;
    let stopped = app.reconcile(r).unwrap();
    assert_eq!(
        stopped.assessment,
        Some(execution_lifecycle::EffectAssessment::NoEffect)
    );
    assert_eq!(
        app.retry_execution(&caller(), r, &command("expired"))
            .unwrap_err(),
        Error::Capability
    );
    let other = RequestContext {
        actor: ActorId::new("other-actor").unwrap(),
    };
    assert_eq!(app.status(&other, r).unwrap_err(), Error::Denied);
    assert_eq!(
        app.request_execution(&other, &p).unwrap_err(),
        Error::Denied
    );
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
fn dispatch_gate_rechecks_cancel_and_deadline_after_capability_verification() {
    for cancellation in [false, true] {
        let db = Database::new();
        let mut host = TestHost::new();
        let mut spec = host.template.spec().clone();
        spec.budget.total_timeout_ms = 10;
        host.template = FrozenExecution::freeze(spec, &test_store_limits().input).unwrap();
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
                    assert!(
                        other
                            .cancel(&caller(), &other_request)
                            .unwrap()
                            .cancel_requested
                    );
                } else {
                    other_host.state.lock().unwrap().now = 1010;
                }
            }),
        ));
        app.request_execution(&caller(), &p).unwrap();
        assert_eq!(runner.dispatch_count(), 0, "cancellation={cancellation}");
        let causes: Vec<_> = app
            .pull_results(&caller(), &r, &id("diagnostic"), 64)
            .unwrap()
            .into_iter()
            .filter_map(|receipt| {
                app.audit(&caller(), &r, &receipt.operation_id)
                    .unwrap()
                    .dispatch_cause
            })
            .collect();
        assert_eq!(causes.len(), 1);
        assert_eq!(
            causes[0],
            if cancellation {
                execution_lifecycle::DispatchCause::Cancelled
            } else {
                execution_lifecycle::DispatchCause::Limit(execution_lifecycle::LimitReason::Timeout)
            }
        );
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
    app.request_execution(&caller(), &p).unwrap();
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
    assert!(app.cancel(&caller(), &r).unwrap().cancel_requested);
    assert!(app.cancel(&caller(), &r).unwrap().cancel_requested);
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
    let accepted = app.request_execution(&caller(), &p).unwrap();
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
    app.request_execution(&caller(), &p).unwrap();
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
        other.cancel(&caller(), &other_request).unwrap();
    }));
    app.reconcile(&r).unwrap();
    let completed = app.reconcile(&r).unwrap();
    assert_eq!(completed.phase, TaskPhase::Cancelled);
    assert_eq!(completed.evidence.len(), 2);
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
fn unconfirmed_dispatch_audit_preserves_attempt_and_closed_cause_after_restart() {
    use execution_lifecycle::DispatchCause;
    for (scenario, gate_failure, expected) in [
        (
            TestScenario::Complete,
            true,
            DispatchCause::CapabilityUnavailable,
        ),
        (
            TestScenario::RejectBeforeDispatch,
            false,
            DispatchCause::RunnerRejected,
        ),
        (TestScenario::Unknown, false, DispatchCause::DeliveryUnknown),
        (TestScenario::Unavailable, false, DispatchCause::RunnerError),
    ] {
        let db = Database::new();
        let host = TestHost::new();
        let p = plan();
        let r = &p.spec().request.request_id;
        if gate_failure {
            host.state.lock().unwrap().block_capability_on = 3;
        }
        let runner = DeterministicTestRunner::new(id("test-runner"), scenario, 16).unwrap();
        let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
        let accepted = app.request_execution(&caller(), &p).unwrap();
        assert_eq!(accepted.phase, TaskPhase::OutcomeUnknown);
        drop(app);
        let app = open(&db, host, runner, Startup::OpenTest);
        let records = app
            .pull_results(&caller(), r, &id("audit-consumer"), 64)
            .unwrap();
        let mut causes = vec![];
        for receipt in records {
            let audit = app.audit(&caller(), r, &receipt.operation_id).unwrap();
            if let Some(cause) = audit.dispatch_cause {
                assert_eq!(audit.attempt_id, accepted.attempt_id);
                assert_eq!(receipt.attempt_id, accepted.attempt_id);
                causes.push(cause);
            }
        }
        assert_eq!(causes, vec![expected]);
        assert_eq!(
            app.status(&caller(), r).unwrap().dispatch_cause,
            Some(expected)
        );
    }
}

#[test]
fn dispatch_gate_clock_and_authority_failures_remain_distinct_after_restart() {
    use execution_lifecycle::DispatchCause;
    for (error, expected) in [
        (Error::Clock, DispatchCause::ClockUnavailable),
        (Error::Denied, DispatchCause::AuthorityUnavailable),
        (Error::Degraded, DispatchCause::ConfigurationUnavailable),
    ] {
        let db = Database::new();
        let host = TestHost::new();
        let other = host.clone();
        host.state.lock().unwrap().capability_hook = Some((
            3,
            std::sync::Arc::new(move || {
                other.state.lock().unwrap().capability_error = Some(error);
            }),
        ));
        let runner =
            DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16).unwrap();
        let p = plan();
        let r = &p.spec().request.request_id;
        let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
        let status = app.request_execution(&caller(), &p).unwrap();
        assert_eq!(status.dispatch_cause, Some(expected));
        assert_eq!(status.phase, TaskPhase::OutcomeUnknown);
        assert_eq!(status.assessment, None);
        assert_eq!(runner.dispatch_count(), 0);
        drop(app);
        let app = open(&db, host, runner, Startup::OpenTest);
        assert_eq!(app.status(&caller(), r).unwrap(), status);
    }
}

#[test]
fn one_device_service_accepts_distinct_callers_without_reopening_storage() {
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("shared-runner"), TestScenario::Wait, 16).unwrap();
    let mut app = open(&db, host.clone(), runner, Startup::CreateTest);
    let first = plan();
    app.request_execution(&caller(), &first).unwrap();
    let mut second = first.spec().clone();
    second.request.request_id = RequestId::new("second-plan").unwrap();
    second.request.request_id = request("second-request");
    second.request.actor = ActorId::new("second-user").unwrap();
    host.state.lock().unwrap().actor = second.request.actor.clone();
    let second =
        FrozenExecution::freeze(second, &execution_app::test_store_limits().input).unwrap();
    app.request_execution(
        &RequestContext {
            actor: second.spec().request.actor.clone(),
        },
        &second,
    )
    .unwrap();
    assert_eq!(db.count("executions"), 2);
    assert!(app
        .task_details(&caller(), &first.spec().request.request_id)
        .is_ok());
    assert_eq!(
        app.task_details(&caller(), &second.spec().request.request_id)
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(app.tasks(&caller(), None, 128).unwrap().items.len(), 1);
}

#[test]
fn live_capture_stays_running_and_retired_capture_survives_reopen() {
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    struct Capturing {
        inner: DeterministicTestRunner,
        facts: Arc<Mutex<Option<ProcessEvidence>>>,
    }
    impl RunnerPort for Capturing {
        fn id(&self) -> Id {
            self.inner.id()
        }
        fn mode(&self) -> execution_lifecycle::ExecutionMode {
            self.inner.mode()
        }
        fn dispatch(&self, p: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
            self.inner.dispatch(p)
        }
        fn stop(&self, p: &FrozenExecution, a: &AttemptId) -> Result<(), Error> {
            self.inner.stop(p, a)
        }
        fn observe(
            &self,
            _: &FrozenExecution,
            _: &AttemptId,
            _: ObservationStage,
            _: u64,
        ) -> Result<Option<execution_lifecycle::ObservationFacts>, Error> {
            Ok(None)
        }
        fn evidence(
            &self,
            _: &FrozenExecution,
            _: &AttemptId,
        ) -> Result<Option<ProcessEvidence>, Error> {
            Ok(self.facts.lock().unwrap().clone())
        }
        fn acknowledge_capture(
            &self,
            _: &FrozenExecution,
            facts: &ProcessEvidence,
        ) -> Result<(), Error> {
            assert!(facts.finished);
            self.facts.lock().unwrap().take();
            Ok(())
        }
    }
    let db = Database::new();
    let host = TestHost::new();
    let runner = Capturing {
        inner: DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 8).unwrap(),
        facts: Arc::new(Mutex::new(None)),
    };
    let p = plan();
    let request = &p.spec().request.request_id;
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let status = app.request_execution(&caller(), &p).unwrap();
    let attempt = status.attempt_id.unwrap();
    *runner.facts.lock().unwrap() = Some(ProcessEvidence {
        content_digest: p.digest().clone(),
        attempt_id: attempt,
        runner: id("test-runner"),
        scope: ProcessScope::Preparing {},
        finished: false,
        exit_code: None,
        end: ProcessEnd::Unknown,
        failure_kind: ProcessFailureKind::None,
        quiescent: false,
        stdout: vec![],
        stderr: vec![],
        total_output_bytes: 0,
        quality: OutputQuality::Partial,
    });
    for _ in 0..3 {
        assert_eq!(app.reconcile(request).unwrap().phase, TaskPhase::Running);
    }
    {
        let mut guard = runner.facts.lock().unwrap();
        let f = guard.as_mut().unwrap();
        f.finished = true;
        f.exit_code = Some(0);
        f.end = ProcessEnd::Exited;
        f.quality = OutputQuality::Complete;
        f.stdout = b"secret-canary".to_vec();
        f.total_output_bytes = 13;
    }
    let status = app.reconcile(request).unwrap();
    assert_eq!(status.phase, TaskPhase::OutcomeUnknown);
    assert!(runner.facts.lock().unwrap().is_none());
    drop(app);
    let app = ExecutionApp::start(
        &db.path,
        Startup::OpenTest,
        host,
        runner,
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let details = app.task_details(&caller(), request).unwrap();
    assert_eq!(details.status.process.as_ref().unwrap().exit_code, Some(0));
    assert!(!serde_json::to_string(&details)
        .unwrap()
        .contains("secret-canary"));
}

#[test]
fn software_application_persists_before_ack_and_reconciles_after_reopen_without_dispatch() {
    use execution_lifecycle::{ExecutionMode, Observation, ObservationFacts};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    #[derive(Clone)]
    struct SoftwareRunner {
        inner: DeterministicTestRunner,
        db: std::path::PathBuf,
        capture: Arc<Mutex<Option<ProcessEvidence>>>,
        ready: Arc<AtomicBool>,
        recovered: Arc<AtomicBool>,
    }
    impl RunnerPort for SoftwareRunner {
        fn id(&self) -> Id {
            self.inner.id()
        }
        fn mode(&self) -> ExecutionMode {
            ExecutionMode::Test
        }
        fn dispatch(&self, p: AuthorizedDispatch) -> Result<DispatchOutcome, Error> {
            self.inner.dispatch(p)
        }
        fn stop(&self, p: &FrozenExecution, a: &AttemptId) -> Result<(), Error> {
            self.inner.stop(p, a)
        }
        fn evidence(
            &self,
            _: &FrozenExecution,
            _: &AttemptId,
        ) -> Result<Option<ProcessEvidence>, Error> {
            Ok(self.capture.lock().unwrap().clone())
        }
        fn software_evidence(
            &self,
            p: &FrozenExecution,
            a: &AttemptId,
            _: execution_app::SoftwareObservation<'_>,
        ) -> Result<Option<SoftwareEvidence>, Error> {
            Ok(self.ready.load(Ordering::SeqCst).then(|| SoftwareEvidence {
                staging: execution_contract::SoftwareStaging::NotRequired {},
                attempt_id: a.clone(),
                content_digest: p.digest().clone(),
                runner: self.id(),
                before: if self.recovered.load(Ordering::SeqCst) {
                    None
                } else {
                    Some(SoftwareState::Absent {})
                },
                detected: SoftwareState::Present {
                    version: PackageValue::new("1.0").unwrap(),
                },
                restart_required: false,
                object_identity: None,
                boot_generation: Some(id("test-boot")),
            }))
        }
        fn acknowledge_capture(
            &self,
            _: &FrozenExecution,
            f: &ProcessEvidence,
        ) -> Result<(), Error> {
            let sql = rusqlite::Connection::open(&self.db).unwrap();
            let bytes: Vec<u8> = sql
                .query_row(
                    "SELECT body FROM software_evidence WHERE attempt_id=?1",
                    [f.attempt_id.as_str()],
                    |r| r.get(0),
                )
                .unwrap();
            let stored: SoftwareEvidence = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(stored.before, Some(SoftwareState::Absent {}));
            assert_eq!(stored.content_digest, f.content_digest);
            self.capture.lock().unwrap().take();
            Ok(())
        }
        fn observe(
            &self,
            p: &FrozenExecution,
            a: &AttemptId,
            stage: ObservationStage,
            now: u64,
        ) -> Result<Option<ObservationFacts>, Error> {
            Ok(
                (self.recovered.load(Ordering::SeqCst) && stage == ObservationStage::Termination)
                    .then(|| ObservationFacts {
                        request_id: p.spec().request.request_id.clone(),
                        content_digest: p.digest().clone(),
                        attempt_id: a.clone(),
                        observed_at_unix_ms: now,
                        evidence: EvidenceRef {
                            reference: reference("recovered-quiescence"),
                            kind: EvidenceKind::TestResult,
                            runner: self.id(),
                        },
                        observation: Observation::Quiescent {},
                    }),
            )
        }
    }
    let db = Database::new();
    let mut host = TestHost::new();
    let p = FrozenExecution::freeze(
        decode_execution(
            include_bytes!("../../execution-contract/tests/fixtures/software.json"),
            &test_store_limits().input,
        )
        .unwrap(),
        &test_store_limits().input,
    )
    .unwrap();
    host.template = p.clone();
    let runner = SoftwareRunner {
        inner: DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 8).unwrap(),
        db: db.path.clone(),
        capture: Arc::new(Mutex::new(None)),
        ready: Arc::new(AtomicBool::new(false)),
        recovered: Arc::new(AtomicBool::new(false)),
    };
    let request = &p.spec().request.request_id;
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let first = app.request_execution(&caller(), &p).unwrap();
    let attempt = first.attempt_id.unwrap();
    assert_eq!(runner.inner.dispatch_count(), 1);
    runner.ready.store(true, Ordering::SeqCst);
    *runner.capture.lock().unwrap() = Some(ProcessEvidence {
        content_digest: p.digest().clone(),
        attempt_id: attempt,
        runner: id("test-runner"),
        scope: ProcessScope::Preparing {},
        finished: true,
        exit_code: Some(0),
        end: ProcessEnd::Exited,
        failure_kind: ProcessFailureKind::None,
        quiescent: false,
        stdout: vec![],
        stderr: vec![],
        total_output_bytes: 0,
        quality: OutputQuality::Complete,
    });
    app.reconcile(request).unwrap();
    assert!(runner.capture.lock().unwrap().is_none());
    assert_eq!(db.count("software_claims"), 2);
    drop(app);
    runner.recovered.store(true, Ordering::SeqCst);
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::OpenTest,
        host,
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    assert_eq!(app.request_execution(&caller(), &p).unwrap().attempts, 1);
    let result = app.reconcile(request).unwrap();
    assert_eq!(
        result.assessment,
        Some(execution_lifecycle::EffectAssessment::Satisfied)
    );
    assert_eq!(
        result.software,
        Some(SoftwareDiagnostic::DesiredStateObserved)
    );
    assert_eq!(runner.inner.dispatch_count(), 1);
    assert_eq!(db.count("software_claims"), 0);
    assert_eq!(db.count("software_ownership"), 1);
}

#[test]
fn exact_action_confirmation_is_durable_and_does_not_change_origin() {
    for ai in [false, true] {
        let db = Database::new();
        let mut host = TestHost::new();
        let mut spec = execution_contract::decode_execution(
            include_bytes!("../../execution-contract/tests/fixtures/plan.json"),
            &test_store_limits().input,
        )
        .unwrap();
        if ai {
            let Initiator::Human { os_session } = spec.request.initiator.clone() else {
                panic!("fixture")
            };
            spec.request.initiator = Initiator::Ai {
                os_session,
                provider: id("provider"),
                config: reference("config"),
                conversation: id("conversation"),
                tool_call: id("call"),
            };
        }
        let p = FrozenExecution::freeze(spec, &test_store_limits().input).unwrap();
        host.template = p.clone();
        host.state.lock().unwrap().risk = Some(execution_admission::RiskLevel::Two);
        let runner =
            DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
        let r = &p.spec().request.request_id;
        let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
        assert_eq!(app.request_execution(&caller(), &p).unwrap().attempts, 0);
        assert_eq!(app.request_execution(&caller(), &p).unwrap().attempts, 0);
        assert_eq!(runner.dispatch_count(), 0);
        assert_eq!(db.count("interactions"), 1);
        drop(app);
        let mut app = open(&db, host, runner.clone(), Startup::OpenTest);
        let wrong = Digest::new("0".repeat(64)).unwrap();
        assert!(app.confirm_execution(&caller(), r, &wrong, true).is_err());
        assert_eq!(
            app.confirm_execution(&caller(), r, p.digest(), true)
                .unwrap()
                .attempts,
            1
        );
        assert_eq!(
            app.confirm_execution(&caller(), r, p.digest(), true)
                .unwrap()
                .attempts,
            1
        );
        assert_eq!(runner.dispatch_count(), 1);
        assert_eq!(
            app.task_details(&caller(), r).unwrap().action.initiator,
            p.spec().request.initiator
        );
    }
}

#[test]
fn ai_risk_and_confirmation_failure_matrix_never_dispatches_without_a_current_gate() {
    for risk in [
        None,
        Some(execution_admission::RiskLevel::Zero),
        Some(execution_admission::RiskLevel::One),
        Some(execution_admission::RiskLevel::Two),
        Some(execution_admission::RiskLevel::Three),
    ] {
        for failure in ["none", "decline", "expire", "revoke", "changed"] {
            let db = Database::new();
            let mut host = TestHost::new();
            let mut spec = decode_execution(
                include_bytes!("../../execution-contract/tests/fixtures/plan.json"),
                &test_store_limits().input,
            )
            .unwrap();
            let Initiator::Human { os_session } = spec.request.initiator.clone() else {
                panic!("fixture")
            };
            spec.request.initiator = Initiator::Ai {
                os_session,
                provider: id("provider"),
                config: reference("config"),
                conversation: id("conversation"),
                tool_call: id("call"),
            };
            let p = FrozenExecution::freeze(spec, &test_store_limits().input).unwrap();
            host.template = p.clone();
            host.state.lock().unwrap().risk = risk;
            let runner =
                DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
            let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
            let r = &p.spec().request.request_id;
            let result = app.request_execution(&caller(), &p).unwrap();
            match risk {
                Some(
                    execution_admission::RiskLevel::Zero | execution_admission::RiskLevel::One,
                ) => assert_eq!(result.attempts, 1),
                Some(execution_admission::RiskLevel::Two) => {
                    assert_eq!(result.phase, TaskPhase::ConfirmationRequired);
                    match failure {
                        "expire" => host.state.lock().unwrap().now = 2000,
                        "revoke" => host.state.lock().unwrap().allow = false,
                        "changed" => {
                            let mut input = p.spec().clone();
                            input.budget.total_output_bytes -= 1;
                            let changed =
                                FrozenExecution::freeze(input, &test_store_limits().input).unwrap();
                            assert_eq!(
                                app.request_execution(&caller(), &changed).unwrap_err(),
                                Error::Conflict
                            );
                        }
                        _ => {}
                    }
                    if failure != "changed" {
                        let _ =
                            app.confirm_execution(&caller(), r, p.digest(), failure != "decline");
                    }
                    assert_eq!(runner.dispatch_count(), usize::from(failure == "none"));
                }
                _ => {
                    assert_eq!(result.phase, TaskPhase::AdmissionDenied);
                    assert_eq!(runner.dispatch_count(), 0);
                }
            }
        }
    }
}

#[test]
fn confirmations_of_two_requests_in_one_journal_do_not_collide() {
    let db = Database::new();
    let original = decode_execution(
        include_bytes!("../../execution-contract/tests/fixtures/plan.json"),
        &test_store_limits().input,
    )
    .unwrap();
    for (index, startup) in [(0, Startup::CreateTest), (1, Startup::OpenTest)] {
        let mut input = original.clone();
        input.request.request_id = request(&format!("human-{index}"));
        let p = FrozenExecution::freeze(input, &test_store_limits().input).unwrap();
        let mut host = TestHost::new();
        host.template = p.clone();
        let runner =
            DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
        let mut app = open(&db, host, runner.clone(), startup);
        let r = &p.spec().request.request_id;
        assert_eq!(app.request_execution(&caller(), &p).unwrap().attempts, 0);
        assert_eq!(
            app.confirm_execution(&caller(), r, p.digest(), true)
                .unwrap()
                .attempts,
            1
        );
    }
    assert_eq!(db.count("attempts"), 2);
}

#[test]
fn service_resumes_before_first_intent_but_never_replays_an_existing_attempt() {
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
    let p = plan();
    let r = &p.spec().request.request_id;
    db.sql().execute_batch("CREATE TRIGGER fail_initial AFTER INSERT ON attempts BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(app.request_execution(&caller(), &p).is_err());
    drop(app);
    db.sql()
        .execute_batch("DROP TRIGGER fail_initial;")
        .unwrap();
    let mut app = open(&db, host, runner.clone(), Startup::OpenTest);
    assert_eq!(app.resume_initial(r).unwrap().attempts, 1);
    assert_eq!(app.resume_initial(r).unwrap().attempts, 1);
    assert_eq!(runner.dispatch_count(), 1);
}
#[test]
fn service_expires_pending_confirmation_and_exposes_corrupt_confirmation_storage() {
    for corrupt in [false, true] {
        let db = Database::new();
        let mut host = TestHost::new();
        let p = FrozenExecution::freeze(
            decode_execution(
                include_bytes!("../../execution-contract/tests/fixtures/plan.json"),
                &test_store_limits().input,
            )
            .unwrap(),
            &test_store_limits().input,
        )
        .unwrap();
        host.template = p.clone();
        let runner =
            DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
        let mut app = open(&db, host.clone(), runner.clone(), Startup::CreateTest);
        let r = &p.spec().request.request_id;
        assert_eq!(
            app.request_execution(&caller(), &p).unwrap().phase,
            TaskPhase::ConfirmationRequired
        );
        if corrupt {
            db.sql()
                .execute_batch("UPDATE interactions SET snapshot=X'00'")
                .unwrap();
            assert_eq!(app.status(&caller(), r).unwrap_err(), Error::Storage);
        } else {
            host.state.lock().unwrap().now = 2000;
            assert_eq!(app.resume_initial(r).unwrap().phase, TaskPhase::Cancelled);
            let confirmation = execution_sqlite::execution_confirmation(&p);
            assert!(matches!(
                app.interaction(&caller(), r, &confirmation.id)
                    .unwrap()
                    .snapshot()
                    .status,
                execution_interaction::Status::Expired { .. }
            ));
        }
        assert_eq!(runner.dispatch_count(), 0);
    }
}

#[test]
fn old_schema_reports_exact_versions_and_keeps_bytes() {
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 16).unwrap();
    drop(open(&db, host.clone(), runner.clone(), Startup::CreateTest));
    db.sql().execute_batch("PRAGMA user_version=4;").unwrap();
    let before = std::fs::read(&db.path).unwrap();
    let error = match ExecutionApp::start(
        &db.path,
        Startup::OpenTest,
        host,
        runner,
        AppConfig::test_defaults(1),
    ) {
        Ok(_) => panic!("old format opened"),
        Err(e) => e,
    };
    assert_eq!(error.to_string(),"execution database schema 4 is unsupported; required schema 5; preserve the existing database and explicitly select a new test data directory");
    assert_eq!(std::fs::read(&db.path).unwrap(), before);
}
