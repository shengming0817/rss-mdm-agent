//! SQLite T2: persisted samples and native baselines must agree through discontinuities.
use super::*;
use crate::{
    test_support::{telemetry as fixture, Root, Server},
    Clock, OpenMode,
};
fn values(items: &[(&str, i64)]) -> BTreeMap<w::FieldKey, w::TelemetryReading> {
    [(
        w::FieldKey::parse("custom.load").unwrap(),
        w::TelemetryReading::Valid {
            value: w::Scalar::Array(
                items
                    .iter()
                    .map(|(id, n)| {
                        w::Scalar::Object(
                            [
                                ("id".into(), w::Scalar::String((*id).into())),
                                ("bytes".into(), w::Scalar::Integer(*n)),
                                // Candidate timestamps are replaced by the durable local interval start.
                                ("start".into(), w::Scalar::Integer(0)),
                            ]
                            .into(),
                        )
                    })
                    .collect(),
            ),
            interval_start: None,
        },
    )]
    .into()
}
fn append(
    spool: &mut Spool,
    c: &Configuration,
    at: i64,
    items: &[(&str, i64)],
    boot: &str,
) -> w::TelemetryReading {
    let id = spool.append(c, at, values(items), boot).unwrap();
    let bytes: Vec<u8> = spool
        .conn
        .query_row(
            "SELECT body FROM samples WHERE id=?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    decode::<w::TelemetryObservation>(&bytes)
        .unwrap()
        .values
        .remove(&w::FieldKey::parse("custom.load").unwrap())
        .unwrap()
}
fn assert_items(reading: w::TelemetryReading, missing: bool, expected: &[(&str, i64, i64)]) {
    let value = match reading {
        w::TelemetryReading::Missing {
            context: Some(v), ..
        } if missing => v,
        w::TelemetryReading::Valid { value, .. } if !missing => value,
        other => panic!("unexpected reading: {other:?}"),
    };
    let w::Scalar::Array(items) = value else {
        panic!("array required")
    };
    assert_eq!(items.len(), expected.len());
    for (item, (id, bytes, start)) in items.iter().zip(expected) {
        let w::Scalar::Object(p) = item else {
            panic!("object required")
        };
        assert_eq!(p["id"], w::Scalar::String((*id).into()));
        assert_eq!(p["bytes"], w::Scalar::Integer(*bytes));
        assert_eq!(p["start"], w::Scalar::Integer(*start));
    }
}
#[tokio::test]
async fn durable_membership_baselines_reset_only_discontinuous_instances() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let c = client
        .verify_telemetry(fixture::native_network_configuration(&server, &r))
        .unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.install(&c).unwrap();
    let t = server.time.now().unwrap() * 1_000_000;
    assert_items(
        append(&mut spool, &c, t, &[("a", 100), ("b", 200)], "boot"),
        true,
        &[("a", 0, t), ("b", 0, t)],
    );
    assert_items(
        append(&mut spool, &c, t + 1, &[("b", 220), ("a", 110)], "boot"),
        false,
        &[("b", 20, t), ("a", 10, t)],
    );
    // Installing the same lease and reopening cannot erase committed baselines.
    spool.install(&c).unwrap();
    drop(spool);
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    assert_items(
        append(&mut spool, &c, t + 2, &[("a", 120), ("c", 300)], "boot"),
        true,
        &[("a", 20, t), ("c", 0, t + 2)],
    );
    assert_items(
        append(&mut spool, &c, t + 3, &[("a", 130), ("b", 240)], "boot"),
        true,
        &[("a", 30, t), ("b", 0, t + 3)],
    );
    assert_items(
        append(&mut spool, &c, t + 4, &[("a", 5), ("b", 250)], "boot"),
        true,
        &[("a", 0, t + 4), ("b", 10, t + 3)],
    );
    assert_items(
        append(&mut spool, &c, t + 5, &[("a", 8), ("b", 260)], "new-boot"),
        true,
        &[("a", 0, t + 5), ("b", 0, t + 5)],
    );
    assert_items(
        append(&mut spool, &c, t + 4, &[("a", 9), ("b", 270)], "new-boot"),
        true,
        &[("a", 0, t + 4), ("b", 0, t + 4)],
    );
    let missing = [(
        w::FieldKey::parse("custom.load").unwrap(),
        w::TelemetryReading::Missing {
            interval_start: None,
            context: None,
        },
    )]
    .into();
    spool.append(&c, t + 6, missing, "new-boot").unwrap();
    assert_items(
        append(&mut spool, &c, t + 7, &[("a", 12)], "new-boot"),
        true,
        &[("a", 0, t + 7)],
    );
    assert_items(append(&mut spool, &c, t + 8, &[], "new-boot"), false, &[]);
    assert_items(
        append(&mut spool, &c, t + 9, &[("a", 15)], "new-boot"),
        true,
        &[("a", 0, t + 9)],
    );
}
#[tokio::test]
async fn failed_append_rolls_back_baselines_and_recipe_changes_reclaim_state() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let c = client
        .verify_telemetry(fixture::native_network_configuration(&server, &r))
        .unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.install(&c).unwrap();
    let t = server.time.now().unwrap() * 1_000_000;
    append(&mut spool, &c, t, &[("a", 100)], "boot");
    // A full metadata budget permits replacement, but adding a member fails atomically.
    {
        let tx = spool.conn.transaction().unwrap();
        for n in 0..4095 {
            tx.execute(
                "INSERT INTO native_counters VALUES('retired',?1,?2)",
                params![n.to_string(), b"{}".as_slice()],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    assert_eq!(
        spool.append(&c, t + 1, values(&[("a", 110), ("b", 200)]), "boot"),
        Err(Error::Capacity)
    );
    spool.install(&c).unwrap();
    // Force a sample persistence failure after normalization writes its new state.
    spool.conn.execute_batch("CREATE TRIGGER fail_sample BEFORE INSERT ON samples BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
    assert!(spool
        .append(&c, t + 1, values(&[("a", 1)]), "boot")
        .is_err());
    spool
        .conn
        .execute_batch("DROP TRIGGER fail_sample;")
        .unwrap();
    assert_items(
        append(&mut spool, &c, t + 2, &[("a", 120)], "boot"),
        false,
        &[("a", 20, t)],
    );
    let mut changed = c.payload().clone();
    let w::TelemetryCollector::Native { mappings, .. } = &mut changed.collector else {
        unreachable!()
    };
    mappings
        .get_mut("custom.load")
        .unwrap()
        .mapping
        .columns
        .get_mut("bytes")
        .unwrap()
        .pointer = "/sent_bytes".into();
    let changed = client
        .verify_telemetry(fixture::sign(&server.data.lock().unwrap(), changed))
        .unwrap();
    spool.install(&changed).unwrap();
    assert_eq!(
        spool
            .conn
            .query_row("SELECT count(*) FROM native_counters", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_items(
        append(&mut spool, &changed, t + 3, &[("a", 1000)], "boot"),
        true,
        &[("a", 0, t + 3)],
    );
    spool
        .retain_configurations(&w::TelemetryDirectory {
            assignments: vec![],
        })
        .unwrap();
    assert_eq!(
        spool
            .conn
            .query_row("SELECT count(*) FROM native_counters", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn full_metadata_budget_uses_final_membership_independent_of_field_order() {
    for reverse in [false, true] {
        let server = Server::new().await;
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        let r = server.register(&mut client).await;
        let mut config = fixture::native_network_configuration(&server, &r).configuration;
        let load = w::FieldKey::parse("custom.load").unwrap();
        let other = w::FieldKey::parse("custom.other").unwrap();
        let mut second = config.fields[0].clone();
        second.key = other;
        config.fields.push(second);
        let w::TelemetryCollector::Native { mappings, .. } = &mut config.collector else {
            unreachable!()
        };
        mappings.insert(other.as_str().into(), mappings[load.as_str()].clone());
        if reverse {
            config.fields.reverse();
        }
        let c = client
            .verify_telemetry(fixture::sign(&server.data.lock().unwrap(), config))
            .unwrap();
        let mut spool = client.telemetry_spool(Limits::production()).unwrap();
        spool.install(&c).unwrap();
        let t = server.time.now().unwrap() * 1_000_000;
        let pair = |left: &[(&str, i64)], right: &[(&str, i64)]| {
            let mut readings = values(left);
            readings.insert(other, values(right).remove(&load).unwrap());
            readings
        };
        spool
            .append(
                &c,
                t,
                pair(&[("a", 100)], &[("b", 200), ("c", 300)]),
                "boot",
            )
            .unwrap();
        // Fill the unrelated retained-state budget: the two fields may exchange slots,
        // but their final combined membership must remain at most three.
        {
            let tx = spool.conn.transaction().unwrap();
            for n in 0..4093 {
                tx.execute(
                    "INSERT INTO native_counters VALUES('unrelated',?1,?2)",
                    params![n.to_string(), b"{}".as_slice()],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        }
        let id = spool
            .append(
                &c,
                t + 1,
                pair(&[("a", 110), ("d", 400)], &[("b", 220)]),
                "boot",
            )
            .unwrap();
        let body: Vec<u8> = spool
            .conn
            .query_row(
                "SELECT body FROM samples WHERE id=?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        let mut observation: w::TelemetryObservation = decode(&body).unwrap();
        assert_items(
            observation.values.remove(&load).unwrap(),
            true,
            &[("a", 10, t), ("d", 0, t + 1)],
        );
        assert_items(
            observation.values.remove(&other).unwrap(),
            false,
            &[("b", 20, t)],
        );
        let snapshot = || {
            spool
                .conn
                .prepare("SELECT field,item,body FROM native_counters ORDER BY field,item")
                .unwrap()
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        let before = snapshot();
        assert_eq!(before.len(), 4096);
        // The final membership genuinely exceeds capacity. Deletions and all earlier
        // inserts must roll back, including the original cumulative baselines.
        assert_eq!(
            spool.append(
                &c,
                t + 2,
                pair(&[("a", 120), ("d", 410)], &[("b", 230), ("e", 500)]),
                "boot"
            ),
            Err(Error::Capacity)
        );
        let after = spool
            .conn
            .prepare("SELECT field,item,body FROM native_counters ORDER BY field,item")
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(after, before);
        assert_eq!(
            spool
                .conn
                .query_row("SELECT count(*) FROM samples", [], |r| r.get::<_, u64>(0))
                .unwrap(),
            2
        );
    }
}

#[tokio::test]
async fn native_integer_baselines_samples_and_configuration_remain_exact_after_reopen() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let mut signed = fixture::native_network_configuration(&server, &r);
    // Ordinary identities retain their previous bytes when no precision correction is needed.
    let w::TelemetryCollector::Native { plan, mappings, .. } = &signed.configuration.collector
    else {
        unreachable!()
    };
    assert_eq!(
        encode(&signed.configuration.fields[0]).unwrap(),
        crate::store::encode(&signed.configuration.fields[0]).unwrap()
    );
    assert_eq!(
        encode(&(
            signed.configuration.registration_id,
            signed.configuration.generation,
            signed.configuration.source,
            signed.configuration.source_epoch
        ))
        .unwrap(),
        crate::store::encode(&(
            signed.configuration.registration_id,
            signed.configuration.generation,
            signed.configuration.source,
            signed.configuration.source_epoch
        ))
        .unwrap()
    );
    assert_eq!(
        encode(&(
            &signed.configuration.fields[0],
            &mappings["custom.load"],
            plan
        ))
        .unwrap(),
        crate::store::encode(&(
            &signed.configuration.fields[0],
            &mappings["custom.load"],
            plan
        ))
        .unwrap()
    );
    signed.configuration.fields[0].version = i64::MAX as u64;
    let c = client
        .verify_telemetry(fixture::sign(
            &server.data.lock().unwrap(),
            signed.configuration,
        ))
        .unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.install(&c).unwrap();
    let t = server.time.now().unwrap() * 1_000_000;
    let base = (1_i64 << 53) + 1;
    append(
        &mut spool,
        &c,
        t,
        &[("a", base), ("b", i64::MAX - 1), ("c", 0)],
        "boot",
    );
    assert_items(
        append(
            &mut spool,
            &c,
            t + 1,
            &[("a", base + 2), ("b", i64::MAX), ("c", i64::MAX)],
            "boot",
        ),
        false,
        &[("a", 2, t), ("b", 1, t), ("c", i64::MAX, t)],
    );
    drop(spool);
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    let restored = spool.configurations().unwrap().pop().unwrap();
    assert_eq!(restored.configuration.fields[0].version, i64::MAX as u64);
    client.verify_telemetry(restored).unwrap();
    let pending = spool.pending(t / 1_000_000, false).unwrap().unwrap();
    let mut upload: w::TelemetryUpload = decode(pending.body()).unwrap();
    assert_items(
        upload
            .observations
            .last_mut()
            .unwrap()
            .values
            .remove(&w::FieldKey::parse("custom.load").unwrap())
            .unwrap(),
        false,
        &[("a", 2, t), ("b", 1, t), ("c", i64::MAX, t)],
    );
    assert_items(
        append(
            &mut spool,
            &c,
            t + 2,
            &[("a", base + 5), ("b", i64::MAX), ("c", i64::MAX)],
            "boot",
        ),
        false,
        &[("a", 5, t), ("b", 1, t), ("c", i64::MAX, t)],
    );
}

#[tokio::test]
async fn scalar_counter_baseline_above_double_precision_survives_reopen() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let mut config = fixture::configuration(&server, &r).configuration;
    let binding = config.fields[0].time_series.as_mut().unwrap();
    binding.kind = w::TelemetryKind::CumulativeCounter;
    binding.aggregations = [w::TelemetryAggregation::Increase].into();
    let c = client
        .verify_telemetry(fixture::sign(&server.data.lock().unwrap(), config))
        .unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.install(&c).unwrap();
    let t = server.time.now().unwrap() * 1_000_000;
    let base = i64::MAX - 2;
    let scalar = |n| {
        [(
            w::FieldKey::parse("custom.load").unwrap(),
            w::TelemetryReading::Valid {
                value: w::Scalar::Integer(n),
                interval_start: None,
            },
        )]
        .into()
    };
    spool.append(&c, t, scalar(base), "boot").unwrap();
    drop(spool);
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    let id = spool.append(&c, t + 1, scalar(i64::MAX), "boot").unwrap();
    let bytes: Vec<u8> = spool
        .conn
        .query_row(
            "SELECT body FROM samples WHERE id=?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let sample: w::TelemetryObservation = decode(&bytes).unwrap();
    assert!(
        matches!(sample.values[&w::FieldKey::parse("custom.load").unwrap()], w::TelemetryReading::Valid { value: w::Scalar::Integer(2), interval_start: Some(start) } if start == t)
    );
}
