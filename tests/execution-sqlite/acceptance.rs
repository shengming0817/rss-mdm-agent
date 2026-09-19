mod support;
use execution_contract::*;
use execution_interaction as interaction;
use execution_lifecycle as lifecycle;
use execution_sqlite::*;
use std::sync::{Arc, Barrier};
use support::*;

#[test]
fn dispatch_diagnostics_keep_submitted_attempt_and_cause_even_when_stale_or_rejected() {
    for (revision, expected) in [(0, Outcome::Stale), (1, Outcome::Rejected)] {
        let db = Database::new();
        let host = TestHost::new(0);
        let mut store = db.create();
        host.prepare(&mut store);
        let attempt = AttemptId::new("submitted-attempt").unwrap();
        let command = event(
            "diagnostic",
            revision,
            lifecycle::Command::DispatchUnconfirmed {
                attempt_id: attempt.clone(),
                cause: lifecycle::DispatchCause::RunnerError,
            },
        );
        let result = store
            .apply_execution(
                &operation("diagnostic"),
                &host.scope(),
                &command,
                &[],
                &host,
            )
            .unwrap();
        assert_eq!(result.receipt().outcome, expected);
        assert_eq!(result.receipt().attempt_id, Some(attempt.clone()));
        drop(store);
        let audit = db
            .open()
            .audit(&host.scope(), &operation("diagnostic"), &host)
            .unwrap();
        assert_eq!(audit.attempt_id, Some(attempt));
        assert_eq!(
            audit.dispatch_cause,
            Some(lifecycle::DispatchCause::RunnerError)
        );
    }
}

#[test]
fn execution_by_request_restores_exact_plan_and_checks_current_reader() {
    let db = Database::new();
    let host = TestHost::new(1);
    let mut store = db.create();
    host.prepare(&mut store);
    drop(store);
    let store = db.open();
    let request = &host.plan.spec().request.request_id;
    let restored = store.execution_by_request(request, &host).unwrap();
    assert_eq!(restored.plan().digest(), host.plan.digest());
    assert_eq!(restored.snapshot().revision, 1);
    assert_eq!(store.trust_revision(&host.scope(), &host).unwrap(), Some(1));
    let mut denied = host.clone();
    denied.read = false;
    assert!(matches!(
        store.execution_by_request(request, &denied),
        Err(Error::Denied)
    ));
    denied.denied.push(Access::ManageTrust);
    assert_eq!(
        store.trust_revision(&host.scope(), &denied),
        Err(Error::Denied)
    );
    assert!(matches!(
        store.execution_by_request(&RequestId::new("missing").unwrap(), &host),
        Err(Error::NotFound)
    ));
    db.sql()
        .execute(
            "UPDATE executions SET plan=zeroblob(?1)",
            [limits().plan.max_input_bytes + 1],
        )
        .unwrap();
    assert!(matches!(
        store.execution_by_request(request, &host),
        Err(Error::Corrupt)
    ));
}

#[test]
fn initialize_and_reopen_preserve_authority_and_reject_implicit_creation() {
    let db = Database::new();
    let authority = plan().spec().request.authority.clone();
    assert!(Store::open(&db.path, &authority, limits()).is_err());
    assert!(!db.path.exists());
    drop(db.create());
    assert!(matches!(
        Store::open(&db.path, &authority, limits()).unwrap(),
        OpenOutcome::Ready(_)
    ));
    let other = Authority::Test {
        id: id("different"),
    };
    assert!(Store::open(&db.path, &other, limits()).is_err());
    assert!(Store::initialize_test(&db.path, authority, limits()).is_err());
}
#[test]
fn first_commit_consumes_all_approvals_once_and_replay_never_reauthorizes_or_dispatches() {
    let db = Database::new();
    let mut store = db.create();
    let mut host = TestHost::new(2);
    host.prepare(&mut store);
    let first = store
        .apply_execution(
            &operation("start"),
            &host.scope(),
            &host.begin(),
            &host.bindings(),
            &host,
        )
        .unwrap();
    let receipt = first.receipt().clone();
    match first {
        CommitOutcome::Applied {
            first_dispatch: Some(action),
            ..
        } => action.dispatch(|a| assert_eq!(a.attempt_id().as_str(), "attempt-1")),
        _ => panic!("first action"),
    }
    assert_eq!(host.calls.get(), 1);
    assert_eq!(db.count("attempts"), 1);
    assert_eq!(db.count("approval_consumptions"), 2);
    let audit = store
        .audit(&host.scope(), &operation("start"), &host)
        .unwrap();
    assert_eq!(audit.admission.as_ref().unwrap().rule_ids.len(), 2);
    assert_eq!(audit.consumptions.len(), 2);
    assert!(audit
        .consumptions
        .iter()
        .all(|c| c.used_before == 0 && c.used_after == 1));
    assert_audit_event(&host, &audit, &receipt);
    assert_admission_evidence(&host, &audit);
    assert_approval_evidence(&host, &audit);
    assert_audit_wire(&audit);
    host.now.set(5000);
    host.write = false;
    let replay = store
        .apply_execution(
            &operation("start"),
            &host.scope(),
            &host.begin(),
            &host.bindings(),
            &host,
        )
        .unwrap();
    assert!(matches!(replay, CommitOutcome::AlreadyCommitted(_)));
    assert_eq!(replay.receipt(), &receipt);
    assert_eq!(host.calls.get(), 1);
    assert_eq!(
        store.execution(&host.scope(), &host).unwrap().phase(),
        lifecycle::Phase::Starting
    );
    host.audit = false;
    assert_eq!(
        store
            .audit(&host.scope(), &operation("start"), &host)
            .unwrap_err(),
        Error::Denied
    );
    host.read = false;
    assert_eq!(
        store
            .receipt(&host.scope(), &operation("start"), &host)
            .unwrap_err(),
        Error::Denied
    );
}
#[test]
fn second_approval_write_failure_rolls_back_first_consumption_intent_state_and_audit() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(2);
    host.prepare(&mut store);
    let receipts = db.count("receipts");
    db.sql().execute_batch("CREATE TRIGGER fail_second BEFORE UPDATE ON approval_usage WHEN NEW.record_id='approval-1' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert_eq!(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &host.bindings(),
                &host
            )
            .unwrap_err(),
        Error::Conflict
    );
    assert_eq!(
        db.sql()
            .query_row("SELECT sum(used) FROM approval_usage", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(db.count("attempts"), 0);
    assert_eq!(db.count("approval_consumptions"), 0);
    assert_eq!(db.count("receipts"), receipts);
    assert_eq!(db.count("audits"), receipts);
    assert_eq!(
        store
            .execution(&host.scope(), &host)
            .unwrap()
            .snapshot()
            .revision,
        1
    );
    db.sql().execute_batch("DROP TRIGGER fail_second").unwrap();
    assert!(matches!(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &host.bindings(),
                &host
            )
            .unwrap(),
        CommitOutcome::Applied {
            first_dispatch: Some(_),
            ..
        }
    ));
}
#[test]
fn concurrent_duplicate_has_one_action_one_attempt_and_one_consumption() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(1);
    host.prepare(&mut store);
    drop(store);
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let mut store = db.open();
            let host = host.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .apply_execution(
                        &operation("start"),
                        &host.scope(),
                        &host.begin(),
                        &host.bindings(),
                        &host,
                    )
                    .unwrap()
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(
                r,
                CommitOutcome::Applied {
                    first_dispatch: Some(_),
                    ..
                }
            ))
            .count(),
        1
    );
    assert_eq!(results[0].receipt(), results[1].receipt());
    assert_eq!(db.count("attempts"), 1);
    assert_eq!(db.count("approval_consumptions"), 1);
}
#[test]
fn content_and_subject_conflicts_do_not_mutate_and_historical_event_ids_cannot_be_reused() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(0);
    host.prepare(&mut store);
    let changed = event("prepare", 0, lifecycle::Command::Cancel);
    assert_eq!(
        store
            .apply_execution(&operation("prepare"), &host.scope(), &changed, &[], &host)
            .unwrap_err(),
        Error::Conflict
    );
    assert_eq!(
        store
            .apply_execution(
                &operation("another-operation"),
                &host.scope(),
                &event("prepare", 1, lifecycle::Command::Wait),
                &[],
                &host
            )
            .unwrap_err(),
        Error::Conflict
    );
    let mut other = host.clone();
    let mut spec = other.plan.spec().clone();
    spec.request.actor = ActorId::new("another-actor").unwrap();
    other.plan = FrozenPlan::freeze(spec, &limits().plan).unwrap();
    assert_eq!(
        store
            .open_execution(&operation("open"), &other.plan, &other)
            .unwrap_err(),
        Error::Conflict
    );
    assert_eq!(db.count("executions"), 1);
    assert_eq!(
        store
            .execution(&host.scope(), &host)
            .unwrap()
            .snapshot()
            .revision,
        1
    );
}
#[test]
fn same_record_covering_multiple_profiles_is_consumed_once() {
    let db = Database::new();
    let mut store = db.create();
    let mut host = TestHost::new(1);
    host.entries[0]
        .definition
        .profiles
        .push(reference("extra-profile"));
    host.prepare(&mut store);
    store
        .apply_execution(
            &operation("start"),
            &host.scope(),
            &host.begin(),
            &host.bindings(),
            &host,
        )
        .unwrap();
    assert_eq!(db.count("approval_consumptions"), 1);
    assert_eq!(
        store
            .audit(&host.scope(), &operation("start"), &host)
            .unwrap()
            .admission
            .unwrap()
            .rule_ids
            .len(),
        2
    );
}
#[test]
fn expired_or_revoked_decision_is_durable_rejection_without_attempt_or_consumption() {
    for expiry in [false, true] {
        let db = Database::new();
        let mut store = db.create();
        let mut host = TestHost::new(1);
        if !expiry {
            host.entries[0].state = ApprovalState::Revoked;
        }
        host.prepare(&mut store);
        host.expire_during_admission = expiry;
        let result = store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &host.bindings(),
                &host,
            )
            .unwrap();
        assert_eq!(result.receipt().outcome, Outcome::Rejected);
        assert_eq!(db.count("attempts"), 0);
        assert_eq!(db.count("approval_consumptions"), 0);
        assert!(matches!(
            store
                .apply_execution(
                    &operation("start"),
                    &host.scope(),
                    &host.begin(),
                    &host.bindings(),
                    &host
                )
                .unwrap(),
            CommitOutcome::AlreadyCommitted(_)
        ));
    }
}
#[test]
fn refresh_cannot_rewrite_definition_refund_usage_or_resurrect_old_trust_revision() {
    let db = Database::new();
    let mut store = db.create();
    let mut host = TestHost::new(1);
    host.prepare(&mut store);
    store
        .apply_execution(
            &operation("start"),
            &host.scope(),
            &host.begin(),
            &host.bindings(),
            &host,
        )
        .unwrap();
    host.approval.revision = id("2");
    store
        .refresh_trust(&operation("refresh"), &host.scope(), Some(1), &host)
        .unwrap();
    assert_eq!(
        db.sql()
            .query_row("SELECT used FROM approval_usage", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
    host.entries[0].definition.max_uses = 9;
    assert_eq!(
        store
            .refresh_trust(&operation("rewrite"), &host.scope(), Some(2), &host)
            .unwrap_err(),
        Error::Conflict
    );
    host.entries[0].definition.max_uses = 1;
    host.approval.revision = id("1");
    assert_eq!(
        store
            .refresh_trust(&operation("old-revision"), &host.scope(), Some(2), &host)
            .unwrap_err(),
        Error::Conflict
    );
}
#[test]
fn interaction_answers_race_with_one_winner_and_unchanged_outcomes_replay() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(0);
    host.prepare(&mut store);
    let spec = spec(&host);
    store
        .open_interaction(&operation("interaction"), &host.scope(), &spec, &host)
        .unwrap();
    let early = store
        .apply_interaction(
            &operation("early"),
            &host.scope(),
            &spec.id,
            &interaction::Command::CheckExpiry {},
            &host,
        )
        .unwrap();
    assert_eq!(early.receipt().outcome, Outcome::NotDue);
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|n| {
            let mut store = db.open();
            let host = host.clone();
            let barrier = barrier.clone();
            let spec = spec.clone();
            std::thread::spawn(move || {
                let answer = interaction::Command::Answer {
                    id: interaction::Reference::new(format!("answer-{n}")).unwrap(),
                    response: interaction::Response::Confirmation { accepted: n == 0 },
                };
                barrier.wait();
                store
                    .apply_interaction(
                        &operation(&format!("answer-{n}")),
                        &host.scope(),
                        &spec.id,
                        &answer,
                        &host,
                    )
                    .unwrap()
                    .receipt()
                    .outcome
            })
        })
        .collect();
    let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        outcomes.iter().filter(|&&o| o == Outcome::Answered).count(),
        1
    );
    assert_eq!(outcomes.iter().filter(|&&o| o == Outcome::Late).count(), 1);
    host.now.set(1900);
    let replay = store
        .apply_interaction(
            &operation("early"),
            &host.scope(),
            &spec.id,
            &interaction::Command::CheckExpiry {},
            &host,
        )
        .unwrap();
    assert_eq!(replay.receipt(), early.receipt());
    assert_eq!(
        store.execution(&host.scope(), &host).unwrap().phase(),
        lifecycle::Phase::Prepared
    );
}
#[test]
fn lost_result_and_confirmation_responses_recover_without_new_events() {
    let db = Database::new();
    let host = TestHost::new(0);
    let mut store = db.create();
    host.prepare(&mut store);
    drop(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &[],
                &host,
            )
            .unwrap(),
    );
    drop(store);
    let mut store = db.open();
    let receipt = store
        .receipt(&host.scope(), &operation("start"), &host)
        .unwrap()
        .unwrap();
    let consumer = id("execution-app");
    let first = store
        .pull_results(&host.scope(), &consumer, 64, &host)
        .unwrap();
    assert!(first.iter().any(|r| r == &receipt));
    assert_eq!(
        first,
        store
            .pull_results(&host.scope(), &consumer, 64, &host)
            .unwrap()
    );
    store
        .confirm(&host.scope(), &consumer, &receipt.event_id, &host)
        .unwrap();
    drop(store);
    let mut store = db.open();
    let count = db.count("receipts");
    store
        .confirm(&host.scope(), &consumer, &receipt.event_id, &host)
        .unwrap();
    assert!(!store
        .pull_results(&host.scope(), &consumer, 64, &host)
        .unwrap()
        .iter()
        .any(|r| r == &receipt));
    assert_eq!(db.count("receipts"), count);
    assert_eq!(
        store
            .receipt(&host.scope(), &operation("start"), &host)
            .unwrap(),
        Some(receipt)
    );
}
#[test]
fn busy_and_clock_rollback_fail_without_partial_writes() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(0);
    host.prepare(&mut store);
    drop(store);
    let mut bounds = limits();
    bounds.busy_timeout_ms = 0;
    let mut store = db.open_with(bounds);
    let blocker = db.sql();
    blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &[],
                &host
            )
            .unwrap_err(),
        Error::Busy
    );
    blocker.execute_batch("ROLLBACK").unwrap();
    host.now.set(999);
    assert_eq!(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &[],
                &host
            )
            .unwrap_err(),
        Error::Clock
    );
    assert_eq!(db.count("attempts"), 0);
    host.now.set(1000);
    store
        .apply_execution(
            &operation("start"),
            &host.scope(),
            &host.begin(),
            &[],
            &host,
        )
        .unwrap();
}
#[test]
fn actual_sqlite_full_rolls_back_and_preserves_existing_authority() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(1);
    host.prepare(&mut store);
    drop(store);
    let conn = db.sql();
    conn.execute_batch("CREATE TABLE ballast(bytes BLOB); CREATE TRIGGER consume_pages AFTER INSERT ON receipts BEGIN INSERT INTO ballast VALUES(zeroblob(1048576)); END; PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
    let mut bounds = limits();
    bounds.max_database_pages = conn
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap();
    let mut store = db.open_with(bounds);
    let before = db.count("receipts");
    assert_eq!(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &host.bindings(),
                &host
            )
            .unwrap_err(),
        Error::Capacity
    );
    assert_eq!(db.count("receipts"), before);
    assert_eq!(db.count("attempts"), 0);
    assert_eq!(
        db.sql()
            .query_row("SELECT used FROM approval_usage", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        store.execution(&host.scope(), &host).unwrap().phase(),
        lifecycle::Phase::Prepared
    );
}
#[test]
fn newer_schema_is_diagnostics_only_and_corrupt_database_is_never_reinitialized() {
    let db = Database::new();
    drop(db.create());
    let conn = db.sql();
    conn.pragma_update(None, "user_version", 99).unwrap();
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(conn);
    let before = std::fs::read(&db.path).unwrap();
    assert!(matches!(
        Store::open(&db.path, &plan().spec().request.authority, limits()).unwrap(),
        OpenOutcome::NewerSchema {
            found: 99,
            supported: 1
        }
    ));
    assert_eq!(std::fs::read(&db.path).unwrap(), before);
    std::fs::write(&db.path, b"corrupt fixture").unwrap();
    assert!(Store::open(&db.path, &plan().spec().request.authority, limits()).is_err());
    assert_eq!(std::fs::read(&db.path).unwrap(), b"corrupt fixture");
}

#[test]
fn existing_writer_is_fenced_when_schema_changes_after_open() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(0);
    host.prepare(&mut store);
    db.sql().pragma_update(None, "user_version", 99).unwrap();
    assert_eq!(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &[],
                &host
            )
            .unwrap_err(),
        Error::Schema
    );
    assert_eq!(db.count("attempts"), 0);
}

#[test]
fn reserved_terminal_receipts_survive_exhausted_normal_capacity() {
    let db = Database::new();
    let mut bounds = limits();
    bounds.max_receipts = 9;
    let host = TestHost::new(0);
    let mut store = Store::initialize_test(&db.path, host.scope().authority, bounds).unwrap();
    host.prepare(&mut store);
    store
        .apply_execution(
            &operation("start"),
            &host.scope(),
            &host.begin(),
            &[],
            &host,
        )
        .unwrap();
    // 4 receipts + 5 terminal reservations exhaust ordinary allocation.
    assert_eq!(
        store
            .apply_execution(
                &operation("output"),
                &host.scope(),
                &event(
                    "output",
                    2,
                    lifecycle::Command::Output {
                        attempt_id: AttemptId::new("attempt-1").unwrap(),
                        total_bytes: 1
                    }
                ),
                &[],
                &host
            )
            .unwrap_err(),
        Error::Capacity
    );
    let cancel = event("cancel", 2, lifecycle::Command::Cancel);
    store
        .apply_execution(&operation("cancel"), &host.scope(), &cancel, &[], &host)
        .unwrap();
    store
        .apply_execution(
            &operation("recover"),
            &host.scope(),
            &event("recover", 3, lifecycle::Command::Recover),
            &[],
            &host,
        )
        .unwrap();
    let mut observer = host.clone();
    observer.observation = lifecycle::Observation::Exited {
        exit_code: 0,
        total_output_bytes: 0,
    };
    let evidence = |name: &str| EvidenceRef {
        reference: reference(name),
        kind: EvidenceKind::TestResult,
        runner: id("test-runner"),
    };
    store
        .apply_execution(
            &operation("exit"),
            &host.scope(),
            &event(
                "exit",
                4,
                lifecycle::Command::Observe {
                    attempt_id: AttemptId::new("attempt-1").unwrap(),
                    evidence: evidence("exit"),
                },
            ),
            &[],
            &observer,
        )
        .unwrap();
    observer.observation = lifecycle::Observation::Effect {
        assessment: lifecycle::EffectAssessment::Satisfied,
    };
    store
        .apply_execution(
            &operation("verify"),
            &host.scope(),
            &event(
                "verify",
                5,
                lifecycle::Command::Observe {
                    attempt_id: AttemptId::new("attempt-1").unwrap(),
                    evidence: evidence("verify"),
                },
            ),
            &[],
            &observer,
        )
        .unwrap();
    assert_eq!(
        store
            .execution(&host.scope(), &host)
            .unwrap()
            .directive(1000)
            .unwrap(),
        lifecycle::Directive::Done
    );
    assert_eq!(db.count("receipts"), 8);
}

#[test]
fn subprocess_committed_intent_cannot_regain_first_dispatch_after_restart() {
    const MARKER: &str = "EXECUTION_SQLITE_CRASH_DB";
    if let Some(path) = std::env::var_os(MARKER) {
        let host = TestHost::new(1);
        let mut store = match Store::open(
            std::path::Path::new(&path),
            &host.scope().authority,
            limits(),
        )
        .unwrap()
        {
            OpenOutcome::Ready(store) => store,
            _ => panic!("current"),
        };
        let result = store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &host.bindings(),
                &host,
            )
            .unwrap();
        assert!(matches!(
            result,
            CommitOutcome::Applied {
                first_dispatch: Some(_),
                ..
            }
        ));
        // No Rust destructors and no action dispatch: emulate death after COMMIT before replying.
        std::process::exit(0);
    }
    let db = Database::new();
    let host = TestHost::new(1);
    let mut store = db.create();
    host.prepare(&mut store);
    drop(store);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "subprocess_committed_intent_cannot_regain_first_dispatch_after_restart",
        ])
        .env(MARKER, &db.path)
        .status()
        .unwrap();
    assert!(status.success());
    let mut store = db.open();
    assert!(matches!(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &host.bindings(),
                &host
            )
            .unwrap(),
        CommitOutcome::AlreadyCommitted(_)
    ));
    let state = store.execution(&host.scope(), &host).unwrap();
    assert_eq!(state.phase(), lifecycle::Phase::Starting);
    assert_eq!(
        state.directive(1000).unwrap(),
        lifecycle::Directive::Reconcile
    );
    store
        .apply_execution(
            &operation("recover"),
            &host.scope(),
            &event("recover", 2, lifecycle::Command::Recover),
            &[],
            &host,
        )
        .unwrap();
    assert_eq!(
        store.execution(&host.scope(), &host).unwrap().phase(),
        lifecycle::Phase::OutcomeUnknown
    );
    assert_eq!(db.count("attempts"), 1);
}

#[cfg(unix)]
#[test]
fn private_path_and_sidecar_permissions_are_required() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let db = Database::new();
    let store = db.create();
    for path in [
        &db.path,
        &db.root.join("execution.db-wal"),
        &db.root.join("execution.db-shm"),
    ] {
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o077,
            0
        );
    }
    let link = db.root.join("link.db");
    symlink(&db.path, &link).unwrap();
    assert!(Store::open(&link, &plan().spec().request.authority, limits()).is_err());
    std::fs::set_permissions(&db.path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Store::open(&db.path, &plan().spec().request.authority, limits()).is_err());
    drop(store);
}

#[cfg(target_os = "macos")]
#[test]
fn restricted_test_process_cannot_read_or_write_database_or_sidecars() {
    const MARKER: &str = "EXECUTION_SQLITE_DENIED_ROOT";
    if let Some(root) = std::env::var_os(MARKER) {
        let root = std::path::Path::new(&root);
        for name in ["execution.db", "execution.db-wal", "execution.db-shm"] {
            assert!(std::fs::read(root.join(name)).is_err());
            assert!(std::fs::OpenOptions::new()
                .write(true)
                .open(root.join(name))
                .is_err());
        }
        assert!(std::fs::write(root.join("replacement.db"), b"not allowed").is_err());
        return;
    }
    let db = Database::new();
    let store = db.create();
    let profile = format!(
        "(version 1) (allow default) (deny file-read* file-write* (subpath \"{}\"))",
        db.root.display()
    );
    let output = std::process::Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "restricted_test_process_cannot_read_or_write_database_or_sidecars",
        ])
        .env(MARKER, &db.root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::read(&db.path).is_ok());
    drop(store);
}

#[test]
fn distinct_attempts_race_without_spending_approval_twice() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(1);
    host.prepare(&mut store);
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|n| {
            let mut store = db.open();
            let host = host.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .apply_execution(
                        &operation(&format!("start-{n}")),
                        &host.scope(),
                        &event(
                            &format!("begin-{n}"),
                            1,
                            lifecycle::Command::BeginAttempt {
                                attempt_id: AttemptId::new(format!("attempt-{n}")).unwrap(),
                                runner: id("test-runner"),
                                mode: lifecycle::ExecutionMode::Test,
                            },
                        ),
                        &host.bindings(),
                        &host,
                    )
                    .unwrap()
            })
        })
        .collect();
    let outcomes: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(
                o,
                CommitOutcome::Applied {
                    first_dispatch: Some(_),
                    ..
                }
            ))
            .count(),
        1
    );
    assert_eq!(db.count("attempts"), 1);
    assert_eq!(db.count("approval_consumptions"), 1);
    assert_eq!(db.count("receipts"), 5);
}

#[test]
fn approval_free_attempt_still_requires_current_authorization_revision() {
    let db = Database::new();
    let mut store = db.create();
    let mut host = TestHost::new(0);
    host.prepare(&mut store);
    host.authorization = reference("new-authorization");
    let rejected = store
        .apply_execution(
            &operation("stale-epoch"),
            &host.scope(),
            &host.begin(),
            &[],
            &host,
        )
        .unwrap();
    assert_eq!(rejected.receipt().outcome, Outcome::Rejected);
    assert_eq!(db.count("attempts"), 0);
    store
        .refresh_trust(&operation("refresh"), &host.scope(), Some(1), &host)
        .unwrap();
    let mut next = host.begin();
    next.id = EventId::new("current-epoch").unwrap();
    assert!(matches!(
        store
            .apply_execution(
                &operation("current-epoch"),
                &host.scope(),
                &next,
                &[],
                &host
            )
            .unwrap(),
        CommitOutcome::Applied {
            first_dispatch: Some(_),
            ..
        }
    ));
}

#[test]
fn old_schema_handle_cannot_read_or_ack_after_upgrade() {
    let db = Database::new();
    let mut store = db.create();
    let host = TestHost::new(0);
    host.prepare(&mut store);
    let receipt = store
        .receipt(&host.scope(), &operation("open"), &host)
        .unwrap()
        .unwrap();
    db.sql().execute_batch("PRAGMA user_version=2;").unwrap();
    assert_eq!(
        store
            .receipt(&host.scope(), &operation("open"), &host)
            .unwrap_err(),
        Error::Schema
    );
    assert_eq!(
        store
            .audit(&host.scope(), &operation("open"), &host)
            .unwrap_err(),
        Error::Schema
    );
    assert!(matches!(
        store.execution(&host.scope(), &host),
        Err(Error::Schema)
    ));
    assert_eq!(
        store
            .pull_results(&host.scope(), &id("ui"), 10, &host)
            .unwrap_err(),
        Error::Schema
    );
    assert_eq!(
        store
            .confirm(&host.scope(), &id("ui"), &receipt.event_id, &host)
            .unwrap_err(),
        Error::Schema
    );
}

#[test]
fn time_is_rechecked_after_trust_and_answer_verification() {
    let db = Database::new();
    let mut store = db.create();
    let mut host = TestHost::new(0);
    host.snapshot_clock = Some(2000);
    assert_eq!(
        store
            .refresh_trust(&operation("expired-snapshot"), &host.scope(), None, &host)
            .unwrap_err(),
        Error::Trust
    );
    assert_eq!(db.count("trust_heads"), 0);
    assert_eq!(db.count("receipts"), 0);
    host.snapshot_clock = None;
    host.now.set(1000);
    host.prepare(&mut store);
    let spec = spec(&host);
    store
        .open_interaction(&operation("interaction"), &host.scope(), &spec, &host)
        .unwrap();
    host.answer_clock = Some(spec.expires_at_unix_ms);
    let command = interaction::Command::Answer {
        id: interaction::Reference::new("answer").unwrap(),
        response: interaction::Response::Confirmation { accepted: true },
    };
    let result = store
        .apply_interaction(
            &operation("answer"),
            &host.scope(),
            &spec.id,
            &command,
            &host,
        )
        .unwrap();
    assert_eq!(result.receipt().outcome, Outcome::Expired);
    assert_eq!(
        result.receipt().occurred_at_unix_ms,
        spec.expires_at_unix_ms
    );
}

#[test]
fn rejected_approval_audit_preserves_exact_trusted_and_submitted_evidence() {
    let db = Database::new();
    let mut store = db.create();
    let mut host = TestHost::new(1);
    host.entries[0].state = ApprovalState::Revoked;
    host.prepare(&mut store);
    let result = store
        .apply_execution(
            &operation("denied"),
            &host.scope(),
            &host.begin(),
            &host.bindings(),
            &host,
        )
        .unwrap();
    assert_eq!(result.receipt().outcome, Outcome::Rejected);
    let audit = store
        .audit(&host.scope(), &operation("denied"), &host)
        .unwrap();
    let trust = audit.trust.as_ref().unwrap();
    assert_eq!(trust.approval_revision, host.approval);
    assert_eq!(trust.authorization_revision, host.authorization);
    assert_eq!(trust.fresh_until_unix_ms, 2000);
    assert_eq!(
        audit.submitted_approvals[0].record,
        host.entries[0].definition.reference
    );
    assert_eq!(
        audit.protected_approvals[0].definition,
        host.entries[0].definition
    );
    assert_eq!(audit.protected_approvals[0].state, ApprovalState::Revoked);
    assert_eq!(audit.protected_approvals[0].used, 0);
    assert_eq!(audit.protected_approvals[0].consumption_revision, 0);
    assert!(audit.consumptions.is_empty());
    assert_eq!(
        audit.approval.as_ref().unwrap().outcome,
        execution_approval::ApprovalOutcome::Rejected(execution_approval::Reason::Revoked)
    );
    assert_eq!(
        audit.admission.as_ref().unwrap().reason,
        execution_admission::Reason::NeedsApproval
    );
    host.approval = reference("new-head");
    host.entries.clear();
    store
        .refresh_trust(&operation("refresh"), &host.scope(), Some(1), &host)
        .unwrap();
    drop(store);
    assert_eq!(
        db.open()
            .audit(&host.scope(), &operation("denied"), &host)
            .unwrap(),
        audit
    );
    assert_eq!(db.count("attempts"), 0);
}

#[test]
fn each_endpoint_requires_its_exact_access_and_delivery_consumer() {
    let db = Database::new();
    let mut store = db.create();
    let mut host = TestHost::new(0);
    host.prepare(&mut store);
    let spec = spec(&host);
    store
        .open_interaction(&operation("interaction"), &host.scope(), &spec, &host)
        .unwrap();
    let initial = db.count("receipts");
    host.denied = vec![Access::ManageTrust];
    assert_eq!(
        store
            .refresh_trust(&operation("blocked-trust"), &host.scope(), Some(1), &host)
            .unwrap_err(),
        Error::Denied
    );
    host.denied = vec![Access::Create];
    assert_eq!(
        store
            .open_execution(&operation("blocked-open"), &host.plan, &host)
            .unwrap_err(),
        Error::Denied
    );
    host.denied = vec![Access::Execute];
    assert_eq!(
        store
            .apply_execution(
                &operation("blocked-execute"),
                &host.scope(),
                &host.begin(),
                &[],
                &host
            )
            .unwrap_err(),
        Error::Denied
    );
    host.denied = vec![Access::RunnerFact];
    assert_eq!(
        store
            .apply_execution(
                &operation("blocked-runner"),
                &host.scope(),
                &event("recover", 1, lifecycle::Command::Recover),
                &[],
                &host
            )
            .unwrap_err(),
        Error::Denied
    );
    host.denied = vec![Access::Interact];
    assert_eq!(
        store
            .open_interaction(
                &operation("blocked-interaction"),
                &host.scope(),
                &spec,
                &host
            )
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(
        store
            .apply_interaction(
                &operation("blocked-answer"),
                &host.scope(),
                &spec.id,
                &interaction::Command::CheckExpiry {},
                &host
            )
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(db.count("receipts"), initial);
    host.denied = vec![Access::Deliver];
    let receipt = store
        .receipt(&host.scope(), &operation("open"), &host)
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .pull_results(&host.scope(), &id("ui"), 1, &host)
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(
        store
            .confirm(&host.scope(), &id("ui"), &receipt.event_id, &host)
            .unwrap_err(),
        Error::Denied
    );
    host.denied.clear();
    host.consumer = Some(id("authorized"));
    assert_eq!(
        store
            .pull_results(&host.scope(), &id("other"), 1, &host)
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(
        store
            .confirm(&host.scope(), &id("other"), &receipt.event_id, &host)
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(
        store
            .pull_results(&host.scope(), &id("authorized"), 1, &host)
            .unwrap()
            .len(),
        1
    );
    store
        .confirm(&host.scope(), &id("authorized"), &receipt.event_id, &host)
        .unwrap();
    store
        .apply_interaction(
            &operation("check"),
            &host.scope(),
            &spec.id,
            &interaction::Command::CheckExpiry {},
            &host,
        )
        .unwrap();
    assert!(host
        .access_calls
        .borrow()
        .iter()
        .any(|(access, consumer, context)| *access == Access::Interact
            && consumer.is_none()
            && *context));
}

#[test]
fn new_write_needs_only_write_permission_while_replay_needs_only_result_permission() {
    let db = Database::new();
    let mut store = db.create();
    let mut host = TestHost::new(0);
    host.read = false;
    let initial = store
        .refresh_trust(&operation("trust"), &host.scope(), None, &host)
        .unwrap();
    assert_eq!(initial.receipt().outcome, Outcome::Changed);
    assert_eq!(
        store
            .refresh_trust(&operation("trust"), &host.scope(), None, &host)
            .unwrap_err(),
        Error::Denied
    );
    host.read = true;
    host.write = false;
    assert!(matches!(
        store
            .refresh_trust(&operation("trust"), &host.scope(), None, &host)
            .unwrap(),
        CommitOutcome::AlreadyCommitted(_)
    ));
    assert_eq!(
        store
            .refresh_trust(&operation("new-write"), &host.scope(), Some(1), &host)
            .unwrap_err(),
        Error::Denied
    );
    assert_eq!(db.count("receipts"), 1);
}

#[test]
fn quota_accepts_uncertain_and_manual_review_before_final_evidence() {
    for assessment in [
        lifecycle::EffectAssessment::Unknown,
        lifecycle::EffectAssessment::NotSatisfied,
    ] {
        let db = Database::new();
        let mut bounds = limits();
        bounds.max_receipts = 9;
        let mut host = TestHost::new(0);
        let mut store = Store::initialize_test(&db.path, host.scope().authority, bounds).unwrap();
        host.prepare(&mut store);
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &[],
                &host,
            )
            .unwrap();
        let attempt = AttemptId::new("attempt-1").unwrap();
        let evidence = |name: &str| EvidenceRef {
            reference: reference(name),
            kind: EvidenceKind::TestResult,
            runner: id("test-runner"),
        };
        for (name, revision, observation) in [
            ("uncertain", 2, lifecycle::Observation::Uncertain),
            (
                "exit",
                4,
                lifecycle::Observation::Exited {
                    exit_code: 1,
                    total_output_bytes: 0,
                },
            ),
            ("manual", 5, lifecycle::Observation::Effect { assessment }),
            (
                "final",
                6,
                lifecycle::Observation::Effect {
                    assessment: lifecycle::EffectAssessment::NoEffect,
                },
            ),
        ] {
            host.observation = observation;
            store
                .apply_execution(
                    &operation(name),
                    &host.scope(),
                    &event(
                        name,
                        revision,
                        lifecycle::Command::Observe {
                            attempt_id: attempt.clone(),
                            evidence: evidence(name),
                        },
                    ),
                    &[],
                    &host,
                )
                .unwrap();
            if name == "uncertain" {
                store
                    .apply_execution(
                        &operation("cancel"),
                        &host.scope(),
                        &event("cancel", 3, lifecycle::Command::Cancel),
                        &[],
                        &host,
                    )
                    .unwrap();
            }
            if name == "manual" {
                assert_eq!(
                    store
                        .execution(&host.scope(), &host)
                        .unwrap()
                        .directive(1000)
                        .unwrap(),
                    lifecycle::Directive::ManualReview
                );
            }
        }
        assert_eq!(db.count("receipts"), 9);
        assert_eq!(
            store
                .execution(&host.scope(), &host)
                .unwrap()
                .directive(1000)
                .unwrap(),
            lifecycle::Directive::Done
        );
    }
}

fn assert_audit_event(host: &TestHost, audit: &AuditRecord, receipt: &Receipt) {
    let p = host.plan.spec();
    let event = audit.event.as_ref().unwrap();
    assert_eq!(event.event_id, receipt.event_id);
    assert_eq!(event.authority, p.request.authority);
    assert_eq!(event.request_id, p.request.request_id);
    assert_eq!(event.plan_id, p.plan_id);
    assert_eq!(&event.plan_digest, host.plan.digest());
    assert_eq!(
        (&event.actor, &event.initiator),
        (&p.request.actor, &p.request.initiator)
    );
    assert_eq!(event.operation, p.request.operation);
    assert_eq!(event.target, p.request.target);
    assert_eq!(event.policy, p.policy);
    assert_eq!(event.delegation, p.request.delegation);
    assert_eq!(event.decision, Decision::Admitted {});
    assert_eq!(event.occurred_at_unix_ms, 1000);
    assert_eq!(audit.attempt_id, Some(AttemptId::new("attempt-1").unwrap()));
    assert_eq!(audit.reason, AuditReason::AdmissionEvaluated);
}

fn assert_admission_evidence(host: &TestHost, audit: &AuditRecord) {
    let p = host.plan.spec();
    let admission = audit.admission.as_ref().unwrap();
    assert_eq!(admission.rule_ids, vec![id("profile-0"), id("profile-1")]);
    assert_eq!(
        admission.validity,
        Some(DecisionValidity {
            revision: host.authorization.clone(),
            verified_at_unix_ms: 1000,
            valid_until_unix_ms: 2000
        })
    );
    assert_eq!(admission.plan_id, p.plan_id);
    assert_eq!(&admission.plan_digest, host.plan.digest());
    assert_eq!(admission.attempt_id, audit.attempt_id.clone().unwrap());
    assert_eq!(admission.policy, p.policy);
    assert_eq!(admission.delegation, p.request.delegation);
    assert_eq!(admission.reason, execution_admission::Reason::NeedsApproval);
    assert_eq!(
        admission.outcome,
        execution_admission::DecisionOutcome::ApprovalRequired {
            profiles: host.bindings().iter().map(|b| b.profile.clone()).collect()
        }
    );
}

fn assert_approval_evidence(host: &TestHost, audit: &AuditRecord) {
    let approval = audit.approval.as_ref().unwrap();
    assert_eq!(
        approval.outcome,
        execution_approval::ApprovalOutcome::Satisfied
    );
    assert_eq!(
        approval.bindings,
        host.bindings()
            .iter()
            .map(ApprovalBindingAudit::from)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        approval.admission_validity,
        audit.admission.as_ref().unwrap().validity
    );
    assert_eq!(approval.pending_consumptions.len(), 2);
    assert_eq!(audit.submitted_approvals, approval.bindings);
    for (index, consumption) in audit.consumptions.iter().enumerate() {
        let d = &host.entries[index].definition;
        assert_eq!(
            *consumption,
            ConsumptionAudit {
                approval: d.reference.clone(),
                approver: d.approver.clone(),
                used_before: 0,
                used_after: 1,
                revision_before: 0,
                revision_after: 1,
                verification_revision: host.approval.clone()
            }
        );
        assert_eq!(
            approval.pending_consumptions[index],
            PendingConsumptionAudit {
                approval: d.reference.clone(),
                expected_uses: 0,
                expected_consumption_revision: 0,
                verification_revision: host.approval.clone(),
                valid_until_unix_ms: 2000
            }
        );
    }
}

fn assert_audit_wire(audit: &AuditRecord) {
    let json = serde_json::to_value(audit).unwrap();
    assert_eq!(json["reason"], "admissionEvaluated");
    assert_eq!(json["admission"]["reason"], "needsApproval");
    assert_eq!(json["approval"]["outcome"], "satisfied");
    assert_eq!(serde_json::from_value::<AuditRecord>(json).unwrap(), *audit);
}

#[test]
fn denied_audit_does_not_promote_missing_expired_or_unknown_records_to_approval() {
    for mode in ["missing", "expired", "unknown"] {
        let db = Database::new();
        let mut store = db.create();
        let mut host = TestHost::new(1);
        if mode == "expired" {
            host.entries[0].definition.validity.expires_at_unix_ms = 1100;
        }
        if mode == "unknown" {
            host.entries[0].state = ApprovalState::Unknown;
        }
        host.prepare(&mut store);
        host.now.set(1200);
        let mut bindings = host.bindings();
        if mode == "missing" {
            bindings[0].record = reference("not-in-protected-store");
        }
        store
            .apply_execution(
                &operation("denied"),
                &host.scope(),
                &host.begin(),
                &bindings,
                &host,
            )
            .unwrap();
        drop(store);
        let audit = db
            .open()
            .audit(&host.scope(), &operation("denied"), &host)
            .unwrap();
        assert_eq!(audit.submitted_approvals[0].record, bindings[0].record);
        assert!(audit.approval.as_ref().unwrap().bindings.is_empty());
        assert!(audit.consumptions.is_empty());
        assert_eq!(
            audit.protected_approvals.len(),
            usize::from(mode != "missing")
        );
        let expected = match mode {
            "missing" => execution_approval::Reason::Verification(
                execution_approval::VerificationError::Unavailable,
            ),
            "expired" => execution_approval::Reason::ApprovalExpired,
            _ => execution_approval::Reason::StatusUnknown,
        };
        assert_eq!(
            audit.approval.unwrap().outcome,
            execution_approval::ApprovalOutcome::Rejected(expected)
        );
        assert_eq!(db.count("attempts"), 0);
    }
}

#[test]
fn out_of_order_confirmation_cannot_skip_pending_results_after_reopen() {
    let db = Database::new();
    let host = TestHost::new(0);
    let mut store = db.create();
    host.prepare(&mut store);
    let consumer = id("out-of-order");
    let batch = store
        .pull_results(&host.scope(), &consumer, 2, &host)
        .unwrap();
    assert_eq!(batch.len(), 2);
    store
        .confirm(&host.scope(), &consumer, &batch[1].event_id, &host)
        .unwrap();
    drop(store);
    let mut store = db.open();
    let pending = store
        .pull_results(&host.scope(), &consumer, 2, &host)
        .unwrap();
    assert_eq!(pending, vec![batch[0].clone()]);
    store
        .confirm(&host.scope(), &consumer, &batch[0].event_id, &host)
        .unwrap();
    assert!(store
        .pull_results(&host.scope(), &consumer, 2, &host)
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .pull_results(&host.scope(), &id("independent"), 2, &host)
            .unwrap(),
        batch
    );
}

#[test]
fn rejected_and_stale_runner_events_retain_submitted_attempt_on_replay() {
    for (revision, outcome, reason, decision) in [
        (
            2,
            Outcome::Rejected,
            AuditReason::LifecycleError(lifecycle::LifecycleError::Attempt),
            Decision::Denied {},
        ),
        (
            1,
            Outcome::Stale,
            AuditReason::Lifecycle(lifecycle::Directive::Reconcile),
            Decision::Proposed {},
        ),
    ] {
        for kind in ["dispatched", "observe", "output"] {
            let db = Database::new();
            let host = TestHost::new(0);
            let mut store = db.create();
            host.prepare(&mut store);
            store
                .apply_execution(
                    &operation("start"),
                    &host.scope(),
                    &host.begin(),
                    &[],
                    &host,
                )
                .unwrap();
            let attempt_id = AttemptId::new("submitted-wrong-attempt").unwrap();
            let command = match kind {
                "dispatched" => lifecycle::Command::Dispatched {
                    attempt_id: attempt_id.clone(),
                },
                "observe" => lifecycle::Command::Observe {
                    attempt_id: attempt_id.clone(),
                    evidence: EvidenceRef {
                        reference: reference("evidence"),
                        runner: id("test-runner"),
                        kind: EvidenceKind::TestResult,
                    },
                },
                _ => lifecycle::Command::Output {
                    attempt_id: attempt_id.clone(),
                    total_bytes: 10,
                },
            };
            let command = event("runner-event", revision, command);
            let receipt = store
                .apply_execution(&operation("runner-op"), &host.scope(), &command, &[], &host)
                .unwrap()
                .receipt()
                .clone();
            assert_eq!(receipt.outcome, outcome);
            assert_eq!(
                receipt.attempt_id,
                Some(attempt_id.clone()),
                "{kind}, revision={revision}"
            );
            drop(store);
            let mut store = db.open();
            let audit = store
                .audit(&host.scope(), &operation("runner-op"), &host)
                .unwrap();
            assert_eq!(audit.attempt_id, Some(attempt_id));
            assert_eq!(audit.reason, reason);
            assert_eq!(audit.event.as_ref().unwrap().decision, decision);
            assert_eq!(
                store
                    .receipt(&host.scope(), &operation("runner-op"), &host)
                    .unwrap(),
                Some(receipt.clone())
            );
            let replay = store
                .apply_execution(&operation("runner-op"), &host.scope(), &command, &[], &host)
                .unwrap();
            assert!(matches!(replay, CommitOutcome::AlreadyCommitted(_)));
            assert_eq!(replay.receipt(), &receipt);
            assert_eq!(
                store
                    .audit(&host.scope(), &operation("runner-op"), &host)
                    .unwrap(),
                audit
            );
        }
    }
}

#[test]
fn stale_event_persists_core_directive_without_a_transition() {
    let db = Database::new();
    let host = TestHost::new(0);
    let mut store = db.create();
    host.prepare(&mut store);
    let command = event("stale", 0, lifecycle::Command::Wait);
    let result = store
        .apply_execution(&operation("stale"), &host.scope(), &command, &[], &host)
        .unwrap();
    assert_eq!(result.receipt().outcome, Outcome::Stale);
    drop(store);
    let mut store = db.open();
    let audit = store
        .audit(&host.scope(), &operation("stale"), &host)
        .unwrap();
    assert_eq!(
        audit.reason,
        AuditReason::Lifecycle(lifecycle::Directive::Ready)
    );
    let replay = store
        .apply_execution(&operation("stale"), &host.scope(), &command, &[], &host)
        .unwrap();
    assert_eq!(replay.receipt(), result.receipt());
    assert_eq!(
        store
            .audit(&host.scope(), &operation("stale"), &host)
            .unwrap(),
        audit
    );
}

#[test]
fn oversized_approval_definition_is_corrupt_before_refresh_comparison() {
    let db = Database::new();
    let host = TestHost::new(1);
    let mut store = db.create();
    host.prepare(&mut store);
    db.sql()
        .execute(
            "UPDATE approvals SET definition=zeroblob(?1)",
            [limits().max_record_bytes + 1],
        )
        .unwrap();
    assert_eq!(
        store
            .refresh_trust(&operation("refresh"), &host.scope(), Some(1), &host)
            .unwrap_err(),
        Error::Corrupt
    );
    assert_eq!(db.count("receipts"), 3);
}

#[test]
fn invalid_call_input_is_not_storage_configuration() {
    assert_eq!(
        OperationRequestId::new("").unwrap_err(),
        Error::InvalidInput
    );
    let db = Database::new();
    let host = TestHost::new(0);
    let mut store = db.create();
    host.prepare(&mut store);
    for limit in [0, limits().max_batch + 1] {
        assert_eq!(
            store
                .pull_results(&host.scope(), &id("input"), limit, &host)
                .unwrap_err(),
            Error::InvalidInput
        );
    }
    let mut invalid = spec(&host);
    invalid.expires_at_unix_ms = host.now.get();
    assert_eq!(
        store
            .open_interaction(&operation("invalid"), &host.scope(), &invalid, &host)
            .unwrap_err(),
        Error::InvalidInput
    );
    assert_eq!(db.count("interactions"), 0);
    assert_eq!(db.count("receipts"), 3);
}

#[test]
fn confirmation_commit_failure_has_its_own_recovery_and_is_retryable() {
    let db = Database::new();
    let host = TestHost::new(0);
    let mut store = db.create();
    host.prepare(&mut store);
    let consumer = id("confirm-failure");
    let receipt = store
        .pull_results(&host.scope(), &consumer, 1, &host)
        .unwrap()
        .remove(0);
    // A deferred FK fails at COMMIT, after the confirmation INSERT succeeded.
    db.sql().execute_batch("CREATE TABLE commit_fault (id INTEGER REFERENCES receipts(sequence) DEFERRABLE INITIALLY DEFERRED);
        CREATE TRIGGER fail_confirm AFTER INSERT ON confirmations BEGIN INSERT INTO commit_fault VALUES(-1); END;").unwrap();
    let error = store
        .confirm(&host.scope(), &consumer, &receipt.event_id, &host)
        .unwrap_err();
    assert_eq!(error, Error::ConfirmationCommitUnknown);
    assert_eq!(db.count("confirmations"), 0);
    db.sql()
        .execute_batch("DROP TRIGGER fail_confirm;")
        .unwrap();
    store
        .confirm(&host.scope(), &consumer, &receipt.event_id, &host)
        .unwrap();
    drop(store);
    let mut store = db.open();
    store
        .confirm(&host.scope(), &consumer, &receipt.event_id, &host)
        .unwrap();
    assert_eq!(db.count("confirmations"), 1);
    assert!(!store
        .pull_results(&host.scope(), &consumer, 64, &host)
        .unwrap()
        .contains(&receipt));
}

#[test]
fn oversized_protected_records_fail_closed_at_every_read_entry() {
    for (table, column, maximum) in [
        ("metadata", "authority", limits().max_record_bytes),
        ("receipts", "body", limits().max_record_bytes),
        ("audits", "body", limits().max_record_bytes),
        ("executions", "plan", limits().plan.max_input_bytes),
        (
            "executions",
            "snapshot",
            limits().lifecycle.max_snapshot_bytes,
        ),
        (
            "interactions",
            "snapshot",
            limits().interaction.max_snapshot_bytes,
        ),
        (
            "trust_heads",
            "authorization_revision",
            limits().max_record_bytes,
        ),
        (
            "trust_heads",
            "approval_revision",
            limits().max_record_bytes,
        ),
        ("approvals", "definition", limits().max_record_bytes),
    ] {
        let db = Database::new();
        let host = TestHost::new(1);
        let mut store = db.create();
        host.prepare(&mut store);
        let interaction = spec(&host);
        store
            .open_interaction(
                &operation("interaction"),
                &host.scope(),
                &interaction,
                &host,
            )
            .unwrap();
        db.sql()
            .execute(
                &format!("UPDATE {table} SET {column}=zeroblob(?1)"),
                [maximum + 1],
            )
            .unwrap();
        let check = |error| assert_eq!(error, Error::Corrupt, "{table}.{column}");
        match table {
            "metadata" => {
                check(store.execution(&host.scope(), &host).unwrap_err());
                match Store::open(&db.path, &host.scope().authority, limits()) {
                    Err(error) => check(error),
                    Ok(_) => panic!("oversized authority opened"),
                }
            }
            "receipts" => {
                check(
                    store
                        .receipt(&host.scope(), &operation("open"), &host)
                        .unwrap_err(),
                );
                check(
                    store
                        .pull_results(&host.scope(), &id("bounded"), 64, &host)
                        .unwrap_err(),
                );
                check(
                    store
                        .open_execution(&operation("open"), &host.plan, &host)
                        .unwrap_err(),
                );
            }
            "audits" => check(
                store
                    .audit(&host.scope(), &operation("open"), &host)
                    .unwrap_err(),
            ),
            "executions" => check(store.execution(&host.scope(), &host).unwrap_err()),
            "interactions" => check(
                store
                    .interaction(&host.scope(), &interaction.id, &host)
                    .unwrap_err(),
            ),
            _ => check(
                store
                    .apply_execution(
                        &operation("start"),
                        &host.scope(),
                        &host.begin(),
                        &host.bindings(),
                        &host,
                    )
                    .unwrap_err(),
            ),
        }
        assert_eq!(db.count("receipts"), 4);
        assert_eq!(db.count("attempts"), 0);
        assert_eq!(db.count("approval_consumptions"), 0);
    }
}

#[test]
fn operation_commit_failure_retains_operation_recovery_without_dispatch() {
    let db = Database::new();
    let host = TestHost::new(1);
    let mut store = db.create();
    host.prepare(&mut store);
    db.sql().execute_batch("CREATE TABLE commit_fault (id INTEGER REFERENCES receipts(sequence) DEFERRABLE INITIALLY DEFERRED);
        CREATE TRIGGER fail_operation AFTER INSERT ON receipts BEGIN INSERT INTO commit_fault VALUES(-1); END;").unwrap();
    assert_eq!(
        store
            .apply_execution(
                &operation("start"),
                &host.scope(),
                &host.begin(),
                &host.bindings(),
                &host
            )
            .unwrap_err(),
        Error::OperationCommitUnknown
    );
    assert!(store
        .receipt(&host.scope(), &operation("start"), &host)
        .unwrap()
        .is_none());
    assert_eq!(db.count("attempts"), 0);
    assert_eq!(db.count("approval_consumptions"), 0);
    db.sql()
        .execute_batch("DROP TRIGGER fail_operation")
        .unwrap();
    let result = store
        .apply_execution(
            &operation("start"),
            &host.scope(),
            &host.begin(),
            &host.bindings(),
            &host,
        )
        .unwrap();
    assert!(matches!(
        result,
        CommitOutcome::Applied {
            first_dispatch: Some(_),
            ..
        }
    ));
    let replay = store
        .apply_execution(
            &operation("start"),
            &host.scope(),
            &host.begin(),
            &host.bindings(),
            &host,
        )
        .unwrap();
    assert!(matches!(replay, CommitOutcome::AlreadyCommitted(_)));
    assert_eq!(replay.receipt(), result.receipt());
}

#[test]
fn invalid_store_limits_remain_configuration_errors() {
    let db = Database::new();
    let mut invalid = limits();
    invalid.max_batch = 0;
    assert!(matches!(
        Store::initialize_test(&db.path, plan().spec().request.authority.clone(), invalid),
        Err(Error::Configuration)
    ));
    assert!(!db.path.exists());
}
