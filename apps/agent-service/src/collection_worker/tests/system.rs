use super::*;
fn system(query: w::NativeSystemQuery) -> w::NativeQuery {
    w::NativeQuery {
        version: query.engine().contract_version(),
        cardinality: query.cardinality(),
        instance_keys: if query.cardinality() == w::NativeCardinality::List {
            vec!["id".into()]
        } else {
            vec![]
        },
        definition: w::NativeQueryDefinition::System { query },
        budget: w::NativeBudget {
            timeout_seconds: 10,
            output_bytes: 8192,
            max_rows: 4,
        },
    }
}
#[test]
fn every_update_failure_retains_explicit_scan_mode_without_fabricated_times() {
    for mode in [w::UpdateScanMode::Local, w::UpdateScanMode::Online] {
        let q = system(w::NativeSystemQuery::WindowsUpdates {
            section: w::UpdateSection::Available,
            mode,
        });
        for status in [
            Status::Denied,
            Status::Timeout,
            Status::Truncated,
            Status::SourceUnavailable,
            Status::Failed,
            Status::NotScanned,
        ] {
            let value = failure(&q, status, 100);
            assert!(value.evidence_matches(&q));
            let scan = value.scan.as_ref().unwrap();
            assert_eq!(scan.mode, mode);
            assert_eq!((scan.started_at, scan.finished_at), (None, None));
            assert!(!value.complete_for(&q, 100));
        }
    }
}
#[test]
fn complete_empty_update_set_needs_actual_scan_but_reboot_does_not() {
    let q = system(w::NativeSystemQuery::WindowsUpdates {
        section: w::UpdateSection::Available,
        mode: w::UpdateScanMode::Local,
    });
    let value = Rows::new(&q, 100).finish(Ok(()));
    assert_eq!(value.status, Status::Partial);
    let q = system(w::NativeSystemQuery::WindowsUpdates {
        section: w::UpdateSection::Available,
        mode: w::UpdateScanMode::Online,
    });
    let mut rows = Rows::new(&q, 100);
    rows.result.scan = Some(w::NativeScanEvidence {
        mode: w::UpdateScanMode::Online,
        source: Some("source-1".into()),
        started_at: Some(98),
        finished_at: Some(100),
    });
    assert!(rows.finish(Ok(())).complete_for(&q, 100));
    let q = system(w::NativeSystemQuery::WindowsUpdates {
        section: w::UpdateSection::Reboot,
        mode: w::UpdateScanMode::Local,
    });
    let mut rows = Rows::new(&q, 100);
    rows.push([("state".into(), json!("required"))].into())
        .unwrap();
    assert!(rows.finish(Ok(())).complete_for(&q, 100));
}
#[test]
fn certificate_store_owner_and_secret_fields_cannot_escape_closed_schema() {
    let q = system(w::NativeSystemQuery::WindowsCertificates {
        store: w::WindowsCertificateStore::My,
        user: false,
    });
    let valid: Row = [
        ("id".into(), json!("a".repeat(64))),
        ("subject".into(), json!("subject")),
        ("issuer".into(), json!("issuer")),
        ("scope".into(), json!("device")),
        ("owner".into(), json!("")),
        ("store".into(), json!("my")),
        ("not_before".into(), json!(1)),
        ("not_after".into(), json!(200)),
    ]
    .into();
    for (key, value) in [
        ("owner", json!("S-1-5-21-1-2-3-1001")),
        ("store", json!("root")),
        ("der", json!("forbidden")),
        ("private_key", json!("forbidden")),
    ] {
        let mut row = valid.clone();
        row.insert(key.into(), value);
        assert_eq!(Rows::new(&q, 100).push(row), Err(Status::Invalid));
    }
    let mut rows = Rows::new(&q, 100);
    rows.push(valid).unwrap();
    assert!(rows.finish(Ok(())).complete_for(&q, 100));
    assert!(Rows::new(&q, 100).finish(Ok(())).complete_for(&q, 100));
}
#[test]
fn missing_bitlocker_observation_is_partial_and_passive_is_not_healthy() {
    let q = system(w::NativeSystemQuery::WindowsBitlocker);
    let mut rows = Rows::new(&q, 100);
    rows.push(
        [
            ("id".into(), json!("volume")),
            ("conversion".into(), json!("encrypted")),
            ("protection".into(), json!("unprotected")),
            ("algorithm".into(), json!("fve_7")),
        ]
        .into(),
    )
    .unwrap();
    assert_eq!(rows.finish(Ok(())).status, Status::Partial);
    let q = system(w::NativeSystemQuery::WindowsDefender);
    let mut rows = Rows::new(&q, 100);
    rows.push(
        [
            ("presence".into(), json!("present")),
            ("configured".into(), json!("enabled")),
            ("running".into(), json!("disabled")),
            ("mode".into(), json!("passive")),
            ("health".into(), json!("passive")),
            ("signature_version".into(), json!("1.2")),
        ]
        .into(),
    )
    .unwrap();
    assert!(rows.finish(Ok(())).complete_for(&q, 100));
}
#[test]
fn selected_accounts_and_user_certificates_bind_exact_identity() {
    let q = system(w::NativeSystemQuery::WindowsAccounts {
        section: w::AccountSection::Administrators,
        selection: w::AccountSelection::Selected {
            identities: ["S-1-5-21-1-2-3-1001".into()].into(),
        },
    });
    let mut rows = Rows::new(&q, 100);
    assert!(rows
        .push(
            [
                ("id".into(), json!("entry")),
                ("member".into(), json!("S-1-5-21-1-2-3-1002")),
                ("member_type".into(), json!("unknown")),
                ("resolved".into(), json!("unresolved"))
            ]
            .into()
        )
        .is_err());
    let q = system(w::NativeSystemQuery::WindowsCertificates {
        store: w::WindowsCertificateStore::Root,
        user: true,
    });
    let plan = w::NativeCollectionPlan::new(w::NativePlanSpec {
        version: 1,
        scope: rss_mdm_agent_wire::CollectionScope::User {
            identity: "S-1-5-21-1-2-3-1001".into(),
        },
        budget: q.budget,
        queries: [("certs".into(), q)].into(),
    })
    .unwrap();
    let mut context = crate::protocol_test_support::execution_context();
    context.user_helper = true;
    context.interactive_user = Some(w::SoftwareInteractiveUser {
        identity: "S-1-5-21-1-2-3-1001".into(),
        session_id: uuid::Uuid::from_u128(1),
        administrator: false,
        generated_uid: None,
    });
    let binding = w::NativeExecutionBinding::bind(&plan, &context).unwrap();
    context.interactive_user.as_mut().unwrap().session_id = uuid::Uuid::from_u128(2);
    assert!(!binding.matches_context(&plan, &context));
    context.interactive_user.as_mut().unwrap().identity = "S-1-5-18".into();
    assert!(w::NativeExecutionBinding::bind(&plan, &context).is_err());
}

#[test]
fn provider_failure_stays_query_local_and_retains_a_single_control_receipt() {
    let mut spec = plan().spec().clone();
    spec.queries.get_mut("b").unwrap().budget.timeout_seconds = 9;
    let p = w::NativeCollectionPlan::new(spec).unwrap();
    let until = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 10_000;
    let result = collect(
        &p,
        now().unwrap() + 10,
        until,
        &Default::default(),
        |id, q, _, _| {
            Ok(failure(
                q,
                if id == "a" {
                    Status::Denied
                } else {
                    Status::Unsupported
                },
                now().unwrap(),
            ))
        },
    )
    .unwrap();
    assert_eq!(result.queries["a"].status, Status::Denied);
    assert_eq!(result.queries["b"].status, Status::Unsupported);
    let bytes = serde_json::to_vec(&result).unwrap();
    let completion = CollectionCompletion {
        input_sha256: hex(b"input"),
        output_sha256: hex(&bytes),
        closed: true,
        native_output_bytes: 0,
    };
    let control = serde_json::to_vec(&completion).unwrap();
    let decoded: CollectionCompletion = serde_json::from_slice(&control).unwrap();
    assert!(decoded.closed);
    assert_eq!(decoded.output_sha256, hex(&bytes));
}

#[test]
fn truncation_preserves_observed_scan_and_failure_capacity_is_reserved() {
    let mut q = system(w::NativeSystemQuery::WindowsUpdates {
        section: w::UpdateSection::Available,
        mode: w::UpdateScanMode::Online,
    });
    q.budget.output_bytes = 1024;
    let mut rows = Rows::new(&q, 100);
    rows.result.scan = Some(w::NativeScanEvidence {
        mode: w::UpdateScanMode::Online,
        source: Some("s".repeat(255)),
        started_at: Some(99),
        finished_at: Some(100),
    });
    let scan = rows.result.scan.clone();
    let value = rows.finish(Err(Status::Truncated));
    assert_eq!(value.scan, scan);
    let reduced = truncated(&q, &value);
    assert_eq!(reduced.scan, scan);
    assert!(serde_json::to_vec(&reduced).unwrap().len() <= q.budget.output_bytes as usize);
    assert!(
        serde_json::to_vec(&failure_reserve(&q, i64::MAX))
            .unwrap()
            .len()
            >= serde_json::to_vec(&reduced).unwrap().len()
    );
}
