use super::*;
use crate::protocol_test_support::{self as protocol, telemetry as fixture};
use execution_contract::*;
fn values() -> BTreeMap<w::FieldKey, w::TelemetryReading> {
    [(
        w::FieldKey::parse("custom.load").unwrap(),
        w::TelemetryReading::Valid {
            value: w::Scalar::Integer(1),
            interval_start: None,
        },
    )]
    .into()
}
#[tokio::test]
async fn withdrawal_expiry_and_clock_jump_preserve_gaps_without_restarting_work() {
    let _budget = crate::scenario_budget::enter();
    let mut case = crate::service::product_gate_tests::Case::new(None).await;
    case.service.client.drive_pki().await.unwrap();
    case.service.client.drive_task_trust().await.unwrap();
    assert!(case.service.client.runtime_readiness().is_ok());
    case.service
        .core
        .host
        .revoked
        .store(false, std::sync::atomic::Ordering::Release);
    let c = case
        .service
        .client
        .verify_telemetry(fixture::configuration(
            &case._server,
            &case.service.client.registration().unwrap(),
        ))
        .unwrap();
    let mut t = Telemetry::open(&case.service.client).unwrap();
    t.install(c.clone()).unwrap();
    t.environment.force(true, false);
    t.slots.get_mut(&c.payload().policy).unwrap().due = Instant::now();
    t.step(&mut case.service.core).unwrap();
    assert_eq!(
        t.spool.summary().unwrap().gaps["collection"].count,
        1,
        "missing content must report a missed sampling instant"
    );
    t.apply(
        &Action::Directory,
        Ok(Event::Directory(w::TelemetryDirectory {
            assignments: vec![],
        })),
    )
    .unwrap();
    let at = timestamp().unwrap();
    let e = ProcessEvidence {
        content_digest: Digest::new("a".repeat(64)).unwrap(),
        attempt_id: AttemptId::new("late").unwrap(),
        runner: Id::new("periodic").unwrap(),
        scope: ProcessScope::NotStarted {},
        finished: true,
        quiescent: true,
        exit_code: Some(0),
        end: ProcessEnd::Exited,
        failure_kind: ProcessFailureKind::None,
        stdout: br#"{"load":1}"#.to_vec(),
        stderr: vec![],
        total_output_bytes: 10,
        quality: OutputQuality::Complete,
    };
    t.complete(&c, at, &e, true, at).unwrap();
    assert_eq!(t.spool.summary().unwrap().pending, 0);
    assert_eq!(t.spool.summary().unwrap().gaps["configuration"].count, 1);
    t.install(c.clone()).unwrap();
    t.complete(&c, at, &e, true, c.payload().expires_at * 1_000_000)
        .unwrap();
    assert_eq!(t.spool.summary().unwrap().pending, 0);
    t.last_time = Some((Instant::now(), at + 10_000_000));
    t.step(&mut case.service.core).unwrap();
    assert!(!t.collector.active());
    assert_eq!(t.spool.summary().unwrap().gaps["clock"].count, 1);
    assert!(t.slots.values().all(|s| s.due > Instant::now()));
}
#[tokio::test]
async fn network_classes_rotate_and_lost_response_resolves_original_batch() {
    let server = protocol::Server::tls().await;
    let root = protocol::Root::new();
    server.time.set(timestamp().unwrap() / 1_000_000);
    let mut client = server.client(&root, agent_client::OpenMode::Create);
    let r = server.register(&mut client).await;
    let c = client
        .verify_telemetry(fixture::configuration(&server, &r))
        .unwrap();
    let mut t = Telemetry::open(&client).unwrap();
    t.install(c.clone()).unwrap();
    t.environment.force(true, true);
    let at = timestamp().unwrap();
    t.spool.append(&c, at, values(), "boot").unwrap();
    t.spool.freeze_all(&c).unwrap();
    t.directory_at = Instant::now() + Duration::from_secs(60);
    for _ in 0..256 {
        t.refresh
            .push_back((uuid::Uuid::new_v4(), uuid::Uuid::new_v4()));
    }
    assert!(matches!(
        t.action().unwrap(),
        Some(Action::Configuration(..))
    ));
    t.request_at = Instant::now();
    assert!(matches!(t.action().unwrap(), Some(Action::Content(..))));
    t.request_at = Instant::now();
    let action = t.action().unwrap().unwrap();
    let Action::Send(ref p) = action else {
        panic!("upload starved");
    };
    let id = p.id();
    server.data.lock().unwrap().telemetry.lose_response = true;
    let failed = request(&client, &action).await;
    assert!(failed.is_err());
    t.apply(&action, failed).unwrap();
    // Select a due persisted batch without sleeping through transport backoff.
    let p = t
        .spool
        .pending(at / 1_000_000 + 120, false)
        .unwrap()
        .unwrap();
    assert_eq!(p.id(), id);
    let action = Action::Send(p);
    let receipt = request(&client, &action).await.unwrap();
    t.apply(&action, Ok(receipt)).unwrap();
    assert_eq!(t.spool.summary().unwrap().pending, 0);
    assert_eq!(server.data.lock().unwrap().telemetry.uploads.len(), 1);
}

#[tokio::test]
async fn authenticated_configuration_cannot_authorize_a_different_policy_plan() {
    let server = protocol::Server::new().await;
    let root = protocol::Root::new();
    let mut client = server.client(&root, agent_client::OpenMode::Create);
    let r = server.register(&mut client).await;
    let c = client
        .verify_telemetry(fixture::configuration(&server, &r))
        .unwrap();
    let mut input: ExecutionInput = serde_json::from_str(include_str!(
        "../../../../crates/execution-contract/tests/fixtures/plan.json"
    ))
    .unwrap();
    input.execution = ExecutionSpec::Process {};
    input.request.delegation = None;
    input.request.initiator = Initiator::Policy {
        policy: input.policy.clone(),
    };
    input.request.operation.action = Id::new("telemetry-sample").unwrap();
    input.run_as = RunAs::System {
        platform: input.request.target.platform,
    };
    let now = c.payload().issued_at as u64 * 1000;
    input.validity = ValidityWindow {
        not_before_unix_ms: now,
        expires_at_unix_ms: now + 1000,
    };
    input.budget.total_timeout_ms = 1000;
    let plan =
        FrozenExecution::freeze(input, &crate::backend::plan::storage_limits().input).unwrap();
    let artifacts = execution_runner::Artifacts {
        program: vec![],
        delegate: None,
        interpreter: root.path.join("absent"),
        content: root.path.join("absent"),
        work_root: root.path.clone(),
        controlled_input: None,
    };
    let mut runner = execution_runner::PeriodicRunner::new().unwrap();
    assert!(runner
        .start(&c, plan.clone(), artifacts.clone(), now)
        .is_err());
    assert!(!runner.active());
    assert!(runner
        .start(&c, plan, artifacts, c.payload().expires_at as u64 * 1000)
        .is_err());
    assert!(!runner.active());
}
