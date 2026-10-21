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

#[tokio::test]
async fn gap_network_turn_recovers_unknown_receipt_and_does_not_starve() {
    let server = protocol::Server::tls().await;
    let root = protocol::Root::new();
    server.time.set(timestamp().unwrap() / 1_000_000);
    let mut client = server.client(&root, agent_client::OpenMode::Create);
    let r = server.register(&mut client).await;
    let mut t = Telemetry::open(&client).unwrap();
    t.environment.force(true, true);
    t.directory_at = Instant::now() + Duration::from_secs(60);
    let at = timestamp().unwrap();
    t.spool
        .gap_for(r.registration_id, r.generation, GapReason::Lease, 1, at, at)
        .unwrap();
    t.refresh
        .push_back((uuid::Uuid::new_v4(), uuid::Uuid::new_v4()));
    assert!(matches!(
        t.action().unwrap(),
        Some(Action::Configuration(..))
    ));
    t.request_at = Instant::now();
    let action = t.action().unwrap().unwrap();
    assert!(matches!(action, Action::SendGap(_)));
    server.data.lock().unwrap().telemetry.lose_response = true;
    let result = request(&client, &action).await;
    assert!(result.is_err());
    t.apply(&action, result).unwrap();
    let p = t
        .spool
        .pending_gap(r.registration_id, r.generation, at + 1_000_000_000)
        .unwrap()
        .unwrap();
    assert!(p.attempted());
    let retry = Action::SendGap(p);
    let result = request(&client, &retry).await;
    assert!(matches!(result, Ok(Event::GapReceipt(_))));
    t.apply(&retry, result).unwrap();
    assert!(t
        .spool
        .pending_gap(r.registration_id, r.generation, at + 1_000_000_000)
        .unwrap()
        .is_none());
    assert_eq!(server.data.lock().unwrap().telemetry.gap_attempts.len(), 1);
    assert_eq!(t.spool.summary().unwrap().gaps["lease"].count, 1);
}

fn network_evidence(
    c: &w::TelemetryConfiguration,
    at: i64,
    bytes: u64,
    complete: bool,
) -> ProcessEvidence {
    use sha2::{Digest as _, Sha256};
    let w::TelemetryCollector::Native { plan, .. } = &c.collector else {
        unreachable!()
    };
    let rows = if complete {
        vec![serde_json::from_value(
            serde_json::json!({"id":"en0","received_bytes":bytes.to_string(),"sent_bytes":"900"}),
        )
        .unwrap()]
    } else {
        vec![]
    };
    let result = w::NativeCollectionResult {
        plan_digest: plan.digest(),
        collected_at: at / 1_000_000,
        queries: [(
            "memory".into(),
            w::NativeQueryResult {
                scan: None,
                query_digest: plan.spec().queries["memory"].digest(),
                collected_at: at / 1_000_000,
                status: if complete {
                    w::NativeQueryStatus::Complete
                } else {
                    w::NativeQueryStatus::Failed
                },
                enumeration_complete: complete,
                row_count: rows.len() as u32,
                rows,
                users: None,
            },
        )]
        .into(),
    };
    let stdout = serde_json::to_vec(&result).unwrap();
    let stderr = serde_json::to_vec(&CollectionCompletion {
        native_output_bytes: stdout.len() as u64,
        input_sha256: "a".repeat(64),
        output_sha256: format!("{:x}", Sha256::digest(&stdout)),
        closed: true,
    })
    .unwrap();
    ProcessEvidence {
        content_digest: Digest::new("a".repeat(64)).unwrap(),
        attempt_id: AttemptId::new("network").unwrap(),
        runner: Id::new("native").unwrap(),
        scope: ProcessScope::NotStarted {},
        finished: true,
        quiescent: true,
        exit_code: Some(0),
        end: ProcessEnd::Exited,
        failure_kind: ProcessFailureKind::None,
        total_output_bytes: (stdout.len() + stderr.len()) as u64,
        stdout,
        stderr,
        quality: OutputQuality::Complete,
    }
}

// Real signature admission, mapping and SQLite spool; provider rows are explicit fixtures.
#[tokio::test]
async fn signed_native_network_mapping_normalizes_two_observations_and_preserves_failure() {
    let server = protocol::Server::tls().await;
    let root = protocol::Root::new();
    server.time.set(timestamp().unwrap() / 1_000_000);
    let mut client = server.client(&root, agent_client::OpenMode::Create);
    let r = server.register(&mut client).await;
    let c = client
        .verify_telemetry(fixture::native_network_configuration(&server, &r))
        .unwrap();
    let mut t = Telemetry::open(&client).unwrap();
    t.install(c.clone()).unwrap();
    let first = timestamp().unwrap();
    let key = c.payload().fields[0].key;
    let evidence = network_evidence(c.payload(), first, 100, true);
    // A malformed signed projection cannot smuggle a native counter into the start.
    let mut invalid = c.payload().clone();
    let w::TelemetryCollector::Native { mappings, .. } = &mut invalid.collector else {
        unreachable!()
    };
    let native = mappings.get_mut(key.as_str()).unwrap();
    native.mapping.columns.insert(
        "start".into(),
        w::TelemetryValueMapping {
            pointer: "/sent_bytes".into(),
            conversion: w::TelemetryConversion::Integer,
            columns: Default::default(),
        },
    );
    assert!(mapping::readings(&invalid, &evidence, first).is_err());
    let w::TelemetryCollector::Native { mappings, .. } = &mut invalid.collector else {
        unreachable!()
    };
    let native = mappings.get_mut(key.as_str()).unwrap();
    native.mapping.columns.remove("start");
    native.counter_normalization = None;
    assert!(mapping::readings(&invalid, &evidence, first).is_err());
    let mut bounded = c.payload().clone();
    let w::ValueType::Array { items, .. } = &mut bounded.fields[0].value_type else {
        unreachable!()
    };
    let w::ValueType::Object { properties } = items.as_mut() else {
        unreachable!()
    };
    properties.insert(
        "id".into(),
        w::ValueType::String {
            max_length: 1,
            allow_empty: false,
        },
    );
    assert!(mapping::readings(&bounded, &evidence, first).is_err());
    let mut bounded = c.payload().clone();
    bounded.max_points = 0;
    assert!(mapping::readings(&bounded, &evidence, first).is_err());
    for (at, raw, complete) in [
        (first, 100, true),
        (first + 5_000_000, 160, true),
        (first + 10_000_000, 0, false),
    ] {
        let mapped = mapping::readings(
            c.payload(),
            &network_evidence(c.payload(), at, raw, complete),
            at,
        )
        .unwrap();
        if complete {
            let w::TelemetryReading::Valid {
                value: w::Scalar::Array(items),
                interval_start: None,
            } = &mapped[&key]
            else {
                panic!("expected projected native counter")
            };
            let w::Scalar::Object(item) = &items[0] else {
                panic!("expected item")
            };
            assert_eq!(item["start"], w::Scalar::Integer(at));
            assert_eq!(item["bytes"], w::Scalar::Integer(raw as i64));
        } else {
            assert!(matches!(mapped[&key], w::TelemetryReading::Failed { .. }));
        }
        t.spool.append(&c, at, mapped, "boot").unwrap();
    }
    t.spool.freeze_all(&c).unwrap();
    let pending = t.spool.pending(first / 1_000_000, false).unwrap().unwrap();
    let action = Action::Send(pending);
    let receipt = request(&client, &action).await.unwrap();
    t.apply(&action, Ok(receipt)).unwrap();
    let batch: w::TelemetryUpload =
        serde_json::from_value(server.data.lock().unwrap().telemetry.uploads[0].clone()).unwrap();
    batch.validate().unwrap();
    assert_eq!(batch.observations.len(), 3);
    assert!(matches!(
        batch.observations[0].values[&key],
        w::TelemetryReading::Missing { .. }
    ));
    let w::TelemetryReading::Valid {
        value: w::Scalar::Array(items),
        interval_start: None,
    } = &batch.observations[1].values[&key]
    else {
        panic!("expected normalized counter")
    };
    let w::Scalar::Object(item) = &items[0] else {
        panic!("expected item")
    };
    assert_eq!(item["bytes"], w::Scalar::Integer(60));
    assert_eq!(item["start"], w::Scalar::Integer(first));
    assert!(first < batch.observations[1].observed_at);
    c.payload().fields[0]
        .validate_scalar(&w::Scalar::Array(items.clone()))
        .unwrap();
    mapping::validate_budget(
        c.payload(),
        &batch.observations[1].values,
        batch.observations[1].observed_at,
    )
    .unwrap();
    assert!(matches!(
        batch.observations[2].values[&key],
        w::TelemetryReading::Failed { .. }
    ));
}
