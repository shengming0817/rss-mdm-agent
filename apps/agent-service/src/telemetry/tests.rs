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
async fn request_timeout_preserves_batch_through_upload_and_status_retry() {
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
    let at = timestamp().unwrap();
    t.spool.append(&c, at, values(), "boot").unwrap();
    t.spool.freeze_all(&c).unwrap();
    let p = t.spool.pending(at / 1_000_000, false).unwrap().unwrap();
    let id = p.id();
    let bytes = p.bytes();
    t.spool.attempting(&p, at / 1_000_000).unwrap();
    server.data.lock().unwrap().telemetry.timed_out = true;
    let action = Action::Send(p); // first POST
    let failed = request(&client, &action).await;
    assert!(matches!(
        &failed,
        Err(Failure {
            error: Error::Unavailable,
            ..
        })
    ));
    t.apply(&action, failed).unwrap();
    assert!(t.spool.pending(at / 1_000_000, false).unwrap().is_none());
    let p = t
        .spool
        .pending(at / 1_000_000 + 120, false)
        .unwrap()
        .unwrap();
    assert!(p.attempted());
    assert_eq!(p.id(), id);
    assert_eq!(p.bytes(), bytes);
    let action = Action::Send(p); // status GET now times out
    let failed = request(&client, &action).await;
    assert!(matches!(
        &failed,
        Err(Failure {
            error: Error::Unavailable,
            ..
        })
    ));
    t.apply(&action, failed).unwrap();
    assert_eq!(t.spool.summary().unwrap().pending, 1);
    server.data.lock().unwrap().telemetry.timed_out = false;
    let p = t
        .spool
        .pending(at / 1_000_000 + 240, false)
        .unwrap()
        .unwrap();
    assert_eq!(p.id(), id);
    assert_eq!(p.bytes(), bytes);
    let action = Action::Send(p);
    let receipt = request(&client, &action).await.unwrap();
    t.apply(&action, Ok(receipt)).unwrap();
    assert_eq!(t.spool.summary().unwrap().pending, 0);
    assert_eq!(server.data.lock().unwrap().telemetry.uploads.len(), 1);
    let d = server.data.lock().unwrap();
    assert_eq!(d.telemetry.upload_attempts.len(), 2);
    assert_eq!(
        d.telemetry.upload_attempts[0],
        d.telemetry.upload_attempts[1]
    );
}
