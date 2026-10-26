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
        assert_eq!(batch.configuration().binding, c.payload().binding);
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

#[tokio::test]
async fn interrupted_initialization_recovers_only_a_blank_database() {
    for rollback in [false, true] {
        let server = Server::new().await;
        let root = Root::new();
        let client = server.client(&root, OpenMode::Create);
        let path = root.path.join("telemetry.sqlite");
        drop(platform_private_storage::create_new(&path).unwrap());
        if rollback {
            let mut conn = rusqlite::Connection::open(&path).unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute_batch("CREATE TABLE identity(binding BLOB); PRAGMA application_id=1381192781; PRAGMA user_version=1;").unwrap();
            tx.rollback().unwrap();
        }
        let spool = client.telemetry_spool(Limits::production()).unwrap();
        assert_eq!(spool.summary().unwrap().pending, 0);
        drop(spool);
        assert!(client.telemetry_spool(Limits::production()).is_ok());
    }
    for sql in [
        "CREATE TABLE foreign_data(value TEXT)",
        "PRAGMA application_id=42",
        "PRAGMA user_version=42",
    ] {
        let server = Server::new().await;
        let root = Root::new();
        let client = server.client(&root, OpenMode::Create);
        let path = root.path.join("telemetry.sqlite");
        drop(platform_private_storage::create_new(&path).unwrap());
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(sql).unwrap();
        drop(conn);
        assert!(matches!(
            client.telemetry_spool(Limits::production()),
            Err(Error::Schema)
        ));
    }
}

#[tokio::test]
async fn gap_snapshot_survives_reopen_and_lost_receipt_without_consuming_new_loss() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let at = server.time.now().unwrap() * 1_000_000;
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    // Historical unowned loss remains local and is never inferred to belong to r.
    spool.gap(GapReason::Clock, 99, at, at).unwrap();
    spool
        .gap_for(
            r.registration_id,
            r.generation,
            GapReason::Capacity,
            2,
            at,
            at,
        )
        .unwrap();
    spool
        .freeze_gaps(r.registration_id, r.generation, at)
        .unwrap();
    let p = spool
        .pending_gap(r.registration_id, r.generation, at)
        .unwrap()
        .unwrap();
    let bytes = p.body().to_vec();
    let id = p.id();
    let report: w::TelemetryGapReport = decode(&bytes).unwrap();
    assert_eq!(report.gaps.len(), 1);
    assert_eq!(report.gaps[0].count, 2);
    spool
        .gap_for(
            r.registration_id,
            r.generation,
            GapReason::Capacity,
            3,
            at,
            at + 1,
        )
        .unwrap();
    spool.attempting_gap(&p, at / 1_000_000).unwrap();
    server.data.lock().unwrap().telemetry.lose_response = true;
    assert!(client.telemetry_gap_submit(&p).await.is_err());
    drop(spool);
    server.time.set(at / 1_000_000 + 3);
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    let p = spool
        .pending_gap(r.registration_id, r.generation, at + 3_000_000)
        .unwrap()
        .unwrap();
    assert_eq!(p.id(), id);
    assert_eq!(p.body(), bytes);
    assert!(p.attempted());
    let receipt = client.telemetry_gap_status(&p).await.unwrap().unwrap();
    let mut wrong = receipt.clone();
    wrong.report_id = Uuid::new_v4();
    assert_eq!(spool.acknowledge_gap(&p, &wrong), Err(Error::Protocol));
    assert!(spool
        .pending_gap(r.registration_id, r.generation, at + 3_000_000)
        .unwrap()
        .is_some());
    spool.acknowledge_gap(&p, &receipt).unwrap();
    spool
        .freeze_gaps(r.registration_id, r.generation, at + 3_000_000)
        .unwrap();
    let next = spool
        .pending_gap(r.registration_id, r.generation, at + 3_000_000)
        .unwrap()
        .unwrap();
    assert_ne!(next.id(), id);
    let report: w::TelemetryGapReport = decode(next.body()).unwrap();
    assert_eq!(report.gaps[0].count, 3);
    assert_eq!(spool.summary().unwrap().gaps["capacity"].count, 5);
    assert_eq!(server.data.lock().unwrap().telemetry.gap_attempts.len(), 1);
}

#[tokio::test]
async fn gap_retry_is_fixed_bounded_fenced_and_expires_without_recreating_loss() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let at = server.time.now().unwrap() * 1_000_000;
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    for reason in [
        GapReason::Expired,
        GapReason::Capacity,
        GapReason::Lease,
        GapReason::Power,
        GapReason::Suspended,
        GapReason::Clock,
        GapReason::Collection,
        GapReason::Rejected,
        GapReason::Registration,
        GapReason::Configuration,
    ] {
        spool
            .gap_for(
                r.registration_id,
                r.generation,
                reason,
                i64::MAX as u64,
                0,
                i64::MAX - 1,
            )
            .unwrap();
    }
    spool
        .freeze_gaps(r.registration_id, r.generation, at)
        .unwrap();
    let p = spool
        .pending_gap(r.registration_id, r.generation, at)
        .unwrap()
        .unwrap();
    assert!(p.bytes() <= w::TELEMETRY_GAP_MAX_BYTES);
    let report: w::TelemetryGapReport = decode(p.body()).unwrap();
    assert_eq!(report.gaps.len(), 10);
    // Durable gap payloads must retain exact integers across storage and recovery.
    assert!(report
        .gaps
        .iter()
        .all(|gap| gap.count == i64::MAX as u64 && gap.first == 0 && gap.last == i64::MAX - 1));
    server.data.lock().unwrap().telemetry.throttled = true;
    let f = client.telemetry_gap_submit(&p).await.unwrap_err();
    assert_eq!(f.retry_after, Some(90));
    spool.attempting_gap(&p, at / 1_000_000).unwrap();
    spool.retry_gap(&p, at / 1_000_000, f.retry_after).unwrap();
    assert!(spool
        .pending_gap(r.registration_id, r.generation, at + 89_000_000)
        .unwrap()
        .is_none());
    server.data.lock().unwrap().telemetry.throttled = false;
    server.time.set(at / 1_000_000 + 90);
    let p = spool
        .pending_gap(r.registration_id, r.generation, at + 90_000_000)
        .unwrap()
        .unwrap();
    assert!(client.telemetry_gap_status(&p).await.unwrap().is_none());
    let first = client.telemetry_gap_submit(&p).await.unwrap();
    let replay = client.telemetry_gap_submit(&p).await.unwrap();
    assert!(!first.replayed && replay.replayed);
    {
        let attempts = &server.data.lock().unwrap().telemetry.gap_attempts;
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0], attempts[1]);
    }
    let other = Uuid::new_v4();
    assert!(spool
        .pending_gap(other, r.generation, at + 90_000_000)
        .unwrap()
        .is_none());
    let mut wrong = r.clone();
    wrong.generation += 1;
    assert!(!p.belongs_to(&wrong));
    let end = at + w::TELEMETRY_GAP_WINDOW_MICROS;
    server.time.set(end / 1_000_000);
    assert_eq!(
        client.telemetry_gap_submit(&p).await.unwrap_err().error,
        Error::Expired
    );
    spool
        .freeze_gaps(r.registration_id, r.generation, end)
        .unwrap();
    assert!(spool
        .pending_gap(r.registration_id, r.generation, end)
        .unwrap()
        .is_none());
    assert_eq!(
        spool.summary().unwrap().gaps["capacity"].count,
        i64::MAX as u64
    );
    // Permanent rejection does not mint another report or increment local loss.
    spool
        .gap_for(
            r.registration_id,
            r.generation,
            GapReason::Power,
            1,
            end,
            end,
        )
        .unwrap();
    spool
        .freeze_gaps(r.registration_id, r.generation, end)
        .unwrap();
    let p = spool
        .pending_gap(r.registration_id, r.generation, end)
        .unwrap()
        .unwrap();
    spool.reject_gap(&p).unwrap();
    spool
        .freeze_gaps(r.registration_id, r.generation, end + 1)
        .unwrap();
    assert!(spool
        .pending_gap(r.registration_id, r.generation, end + 1)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn signed_native_performance_configuration_is_consumed_from_the_runtime_route() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let signed = fixture::native_performance_configuration(&server, &r);
    server.data.lock().unwrap().telemetry.configuration = Some(signed.clone());
    let c = client
        .telemetry_configuration(
            signed.configuration.policy,
            signed.configuration.policy_version,
        )
        .await
        .unwrap();
    assert_eq!(c.payload().source, w::Source::AgentNative);
    let w::TelemetryCollector::Native { plan, mappings, .. } = &c.payload().collector else {
        panic!("native configuration was not retained");
    };
    assert!(matches!(
        plan.spec().queries["memory"].definition,
        w::NativeQueryDefinition::WindowsPerformance {
            section: w::PerformanceSection::Memory
        } | w::NativeQueryDefinition::MacosPerformance {
            section: w::PerformanceSection::Memory
        }
    ));
    assert_eq!(mappings["custom.load"].query, "memory");
    let mut changed = signed;
    changed.configuration.execution_millis += 1;
    assert!(client.verify_telemetry(changed).is_err());
}

#[tokio::test]
async fn dropped_sample_ownership_and_invalid_gap_input_cannot_poison_pending_reports() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let c = client
        .verify_telemetry(fixture::configuration(&server, &r))
        .unwrap();
    let at = server.time.now().unwrap() * 1_000_000;
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.append(&c, at, values(1), "boot").unwrap();
    let other = Uuid::new_v4();
    spool
        .prune(at / 1_000_000, other, r.generation + 1)
        .unwrap();
    // The rejected old sample is still attributed to its original registration.
    spool
        .freeze_gaps(r.registration_id, r.generation, at)
        .unwrap();
    let old = spool
        .pending_gap(r.registration_id, r.generation, at)
        .unwrap()
        .unwrap();
    let report: w::TelemetryGapReport = decode(old.body()).unwrap();
    assert_eq!(report.gaps[0].reason, GapReason::Registration);
    assert_eq!(report.gaps[0].count, 1);
    assert!(spool
        .pending_gap(other, r.generation + 1, at)
        .unwrap()
        .is_none());
    for (count, first, last) in [(u64::MAX, 0, 1), (1, -1, 1), (1, 2, 1), (1, 0, i64::MAX)] {
        assert!(spool
            .gap_for(
                r.registration_id,
                r.generation,
                GapReason::Capacity,
                count,
                first,
                last
            )
            .is_err());
    }
    spool.reject_gap(&old).unwrap();
    spool
        .freeze_gaps(r.registration_id, r.generation, at)
        .unwrap();
    assert!(spool
        .pending_gap(r.registration_id, r.generation, at)
        .unwrap()
        .is_none());
    assert!(!spool.summary().unwrap().gaps.contains_key("capacity"));
    spool
        .gap_for(other, r.generation + 1, GapReason::Lease, 1, at, at)
        .unwrap();
    spool.freeze_gaps(other, r.generation + 1, at).unwrap();
    let foreign = spool
        .pending_gap(other, r.generation + 1, at)
        .unwrap()
        .unwrap();
    assert_eq!(
        client
            .telemetry_gap_submit(&foreign)
            .await
            .unwrap_err()
            .error,
        Error::Untrusted
    );
    assert_eq!(
        client
            .telemetry_gap_status(&foreign)
            .await
            .unwrap_err()
            .error,
        Error::Untrusted
    );
    assert!(server
        .data
        .lock()
        .unwrap()
        .telemetry
        .gap_attempts
        .is_empty());
}

#[tokio::test]
async fn gap_storage_bound_keeps_new_loss_pending_until_a_snapshot_slot_is_released() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let at = server.time.now().unwrap() * 1_000_000;
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    for offset in 0..65 {
        spool
            .gap_for(
                r.registration_id,
                r.generation,
                GapReason::Capacity,
                1,
                at,
                at,
            )
            .unwrap();
        spool
            .freeze_gaps(r.registration_id, r.generation, at + offset)
            .unwrap();
    }
    // SQLite evidence counts the actual frozen bodies, rather than an in-memory approximation.
    let db = rusqlite::Connection::open(root.path.join("telemetry.sqlite")).unwrap();
    let (reports, bytes): (u64, u64) = db
        .query_row(
            "SELECT count(*),sum(length(body)) FROM gap_reports",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(reports, 64);
    assert!(bytes <= 64 * w::TELEMETRY_GAP_MAX_BYTES as u64);
    let pending: u64 = db
        .query_row("SELECT sum(count) FROM gap_pending", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pending, 1);
    let oldest = spool
        .pending_gap(r.registration_id, r.generation, at + 100)
        .unwrap()
        .unwrap();
    spool
        .acknowledge_gap(
            &oldest,
            &w::TelemetryGapReceipt {
                report_id: oldest.id(),
                received_at: at,
                replayed: false,
            },
        )
        .unwrap();
    spool
        .freeze_gaps(r.registration_id, r.generation, at + 101)
        .unwrap();
    let pending: u64 = db
        .query_row("SELECT count(*) FROM gap_pending", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pending, 0);
    assert_eq!(spool.summary().unwrap().gaps["capacity"].count, 65);
    let other = Uuid::new_v4();
    spool
        .freeze_gaps(other, r.generation + 1, at + 102)
        .unwrap();
    assert!(spool
        .pending_gap(other, r.generation + 1, at + 102)
        .unwrap()
        .is_none());
    assert!(spool
        .pending_gap(r.registration_id, r.generation, at + 102)
        .unwrap()
        .is_none());
    assert_eq!(spool.summary().unwrap().gaps["capacity"].count, 65);
}
