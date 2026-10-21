//! Actual SQLite and mutually authenticated TLS recovery; independent of product T3.
use super::*;
use crate::{
    test_support::{telemetry as fixture, Root, Server},
    OpenMode,
};
use std::collections::BTreeMap;
fn values(n: i64) -> BTreeMap<w::FieldKey, w::TelemetryReading> {
    [(
        w::FieldKey::parse("custom.load").unwrap(),
        w::TelemetryReading::Valid {
            value: w::Scalar::Integer(n),
            interval_start: None,
        },
    )]
    .into()
}
#[tokio::test]
async fn lost_response_reopens_same_bytes_and_resolves_durable_pending_receipt() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let signed = fixture::configuration(&server, &r);
    server.data.lock().unwrap().telemetry.configuration = Some(signed.clone());
    let directory = client.telemetry_directory().await.unwrap();
    assert_eq!(directory.assignments.len(), 1);
    let c = client
        .telemetry_configuration(
            signed.configuration.policy,
            signed.configuration.policy_version,
        )
        .await
        .unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.install(&c).unwrap();
    let at = server.time.now().unwrap() * 1_000_000;
    let sample = spool.append(&c, at, values(17), "boot-a").unwrap();
    spool.freeze(&c).unwrap();
    let pending = spool.pending(at / 1_000_000, false).unwrap().unwrap();
    let bytes = pending.body().to_vec();
    let id = pending.id();
    spool.attempting(&pending, at / 1_000_000).unwrap();
    server.data.lock().unwrap().telemetry.lose_response = true;
    assert!(client.telemetry_submit(&pending).await.is_err());
    drop(spool);
    drop(client);
    server.time.set(at / 1_000_000 + 3);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_pki().await.unwrap();
    client.drive_task_trust().await.unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    let pending = spool
        .pending(server.time.now().unwrap(), false)
        .unwrap()
        .unwrap();
    assert!(pending.attempted());
    assert_eq!(pending.id(), id);
    assert_eq!(pending.body(), bytes);
    let upload: w::TelemetryUpload = serde_json::from_slice(pending.body()).unwrap();
    assert_eq!(upload.observations[0].id, sample);
    let receipt = client.telemetry_status(&pending).await.unwrap().unwrap();
    assert!(matches!(receipt.delivery, w::TelemetryDelivery::Pending));
    let mut wrong = receipt.clone();
    wrong.batch_id = Uuid::new_v4();
    assert_eq!(spool.acknowledge(&pending, &wrong), Err(Error::Protocol));
    assert_eq!(spool.summary().unwrap().pending, 1);
    spool.acknowledge(&pending, &receipt).unwrap();
    assert_eq!(spool.summary().unwrap().pending, 0);
    assert_eq!(server.data.lock().unwrap().telemetry.uploads.len(), 1);
}
#[tokio::test]
async fn signature_identity_expiry_and_retry_after_are_enforced() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let signed = fixture::configuration(&server, &r);
    assert!(client.verify_telemetry(signed.clone()).is_ok());
    let mut changed = signed.clone();
    changed.configuration.sample_millis += 1;
    assert!(client.verify_telemetry(changed).is_err());
    for kind in 0..4 {
        let mut c = signed.configuration.clone();
        match kind {
            0 => c.tenant_id = Uuid::new_v4(),
            1 => c.registration_id = Uuid::new_v4(),
            2 => c.generation += 1,
            _ => c.device_id = "different".into(),
        };
        let s = fixture::sign(&server.data.lock().unwrap(), c);
        assert!(client.verify_telemetry(s).is_err());
    }
    server.data.lock().unwrap().telemetry.throttled = true;
    let error = client.telemetry_directory().await.err().unwrap();
    assert_eq!(error.retry_after, Some(90));
    assert_eq!(error.error, Error::Unavailable);
    server.time.set(signed.configuration.expires_at);
    assert!(client.verify_telemetry(signed).is_err());
}
#[tokio::test]
async fn quota_eviction_and_registration_fence_preserve_loss_evidence() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let c = client
        .verify_telemetry(fixture::configuration(&server, &r))
        .unwrap();
    let mut spool = client
        .telemetry_spool(Limits {
            bytes: 65536,
            observations: 2,
            pages: 256,
        })
        .unwrap();
    assert!(client.telemetry_spool(Limits::production()).is_err());
    let at = server.time.now().unwrap() * 1_000_000;
    for n in 0..4 {
        spool.append(&c, at + n, values(n), "boot").unwrap();
    }
    assert_eq!(spool.summary().unwrap().pending, 2);
    assert_eq!(spool.summary().unwrap().gaps["capacity"].count, 2);
    spool.freeze(&c).unwrap();
    spool
        .prune(at / 1_000_000, Uuid::new_v4(), r.generation)
        .unwrap();
    let summary = spool.summary().unwrap();
    assert_eq!(summary.pending, 0);
    assert_eq!(summary.gaps["registration"].count, 2);
    assert!(client.registration().is_ok());
}
#[tokio::test]
async fn fixed_batch_replay_and_backoff_survive_reopen() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let c = client
        .verify_telemetry(fixture::configuration(&server, &r))
        .unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    let now = server.time.now().unwrap();
    spool
        .append(&c, now * 1_000_000, values(4), "boot")
        .unwrap();
    spool.freeze(&c).unwrap();
    let p = spool.pending(now, false).unwrap().unwrap();
    spool.attempting(&p, now).unwrap();
    let first = client.telemetry_submit(&p).await.unwrap();
    let second = client.telemetry_submit(&p).await.unwrap();
    assert_eq!(first.batch_id, second.batch_id);
    let uploads = &server.data.lock().unwrap().telemetry.uploads;
    assert_eq!(uploads[0], uploads[1]);
    spool.retry(&p, now, Some(90)).unwrap();
    drop(spool);
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    assert!(spool.pending(now + 89, false).unwrap().is_none());
    let pending = spool.pending(now + 90, false).unwrap().unwrap();
    spool.reject(&pending, GapReason::Rejected).unwrap();
    assert_eq!(spool.summary().unwrap().gaps["rejected"].count, 1);
}
#[tokio::test]
async fn counter_origin_and_sequence_survive_restart_without_reusing_an_old_boot() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let mut signed = fixture::configuration(&server, &r);
    let binding = signed.configuration.fields[0].time_series.as_mut().unwrap();
    binding.kind = w::TelemetryKind::CumulativeCounter;
    binding.aggregations = [w::TelemetryAggregation::Increase].into();
    let signed = fixture::sign(&server.data.lock().unwrap(), signed.configuration);
    let c = client.verify_telemetry(signed).unwrap();
    let at = server.time.now().unwrap() * 1_000_000;
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.append(&c, at, values(100), "boot-a").unwrap();
    drop(spool);
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.append(&c, at + 1, values(125), "boot-a").unwrap();
    spool.append(&c, at + 2, values(200), "boot-b").unwrap();
    spool.freeze(&c).unwrap();
    let p = spool.pending(at / 1_000_000, false).unwrap().unwrap();
    let b: w::TelemetryUpload = serde_json::from_slice(p.body()).unwrap();
    assert_eq!(
        b.observations
            .iter()
            .map(|v| v.sequence)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let key = w::FieldKey::parse("custom.load").unwrap();
    assert!(matches!(
        b.observations[0].values[&key],
        w::TelemetryReading::Missing { .. }
    ));
    assert_eq!(
        b.observations[1].values[&key],
        w::TelemetryReading::Valid {
            value: w::Scalar::Integer(25),
            interval_start: Some(at)
        }
    );
    assert!(matches!(
        b.observations[2].values[&key],
        w::TelemetryReading::Missing { .. }
    ));
}

#[tokio::test]
async fn replacement_freezes_every_old_batch_with_original_binding() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let mut signed = fixture::configuration(&server, &r);
    signed.configuration.max_points = 1;
    let signed = fixture::sign(&server.data.lock().unwrap(), signed.configuration);
    let c = client.verify_telemetry(signed.clone()).unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.install(&c).unwrap();
    let at = server.time.now().unwrap() * 1_000_000;
    for n in 0..3 {
        spool.append(&c, at + n, values(n), "boot").unwrap();
    }
    let mut changed = signed.configuration;
    changed.policy_version = Uuid::new_v4();
    changed.binding = "b".repeat(64);
    let replacement = client
        .verify_telemetry(fixture::sign(&server.data.lock().unwrap(), changed))
        .unwrap();
    spool.install(&replacement).unwrap();
    for _ in 0..3 {
        let p = spool.pending(at / 1_000_000, false).unwrap().unwrap();
        let batch: w::TelemetryUpload = serde_json::from_slice(p.body()).unwrap();
        assert_eq!(batch.binding, c.payload().binding);
        assert_eq!(batch.observations.len(), 1);
        spool.reject(&p, GapReason::Rejected).unwrap();
    }
    assert_eq!(spool.summary().unwrap().pending, 0);
}

#[tokio::test]
async fn expired_collection_lease_still_recovers_unfrozen_retained_observations() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let signed = fixture::configuration(&server, &r);
    let c = client.verify_telemetry(signed.clone()).unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.install(&c).unwrap();
    spool
        .append(
            &c,
            server.time.now().unwrap() * 1_000_000,
            values(3),
            "boot",
        )
        .unwrap();
    drop(spool);
    server.time.set(c.payload().expires_at);
    assert!(client.verify_telemetry(signed).is_err());
    let spool = client.telemetry_spool(Limits::production()).unwrap();
    let pending = spool
        .pending(server.time.now().unwrap(), false)
        .unwrap()
        .unwrap();
    let receipt = client.telemetry_submit(&pending).await.unwrap();
    assert_eq!(receipt.batch_id, pending.id());
}
