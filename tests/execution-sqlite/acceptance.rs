mod support;
use execution_contract::*;
use execution_interaction as interaction;
use execution_lifecycle as lifecycle;
use execution_sqlite::*;
use std::sync::{Arc, Barrier};
use support::*;

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
    assert_eq!(audit.rule_ids.len(), 2);
    assert_eq!(audit.consumptions.len(), 2);
    assert!(audit
        .consumptions
        .iter()
        .all(|c| c.used_before == 0 && c.used_after == 1));
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
        .pull_results(&host.scope(), &consumer, 0, 64, &host)
        .unwrap();
    assert!(first.iter().any(|r| r == &receipt));
    assert_eq!(
        first,
        store
            .pull_results(&host.scope(), &consumer, 0, 64, &host)
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
        .pull_results(&host.scope(), &consumer, 0, 64, &host)
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
    bounds.max_receipts = 8;
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
    // 4 receipts + 4 terminal reservations exhaust ordinary allocation.
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
            .pull_results(&host.scope(), &id("ui"), 0, 10, &host)
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
