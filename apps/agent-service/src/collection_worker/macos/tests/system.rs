use super::*;
use w::{MacosSecuritySection as S, NativeSystemQuery as Q};
fn system(recipe: Q) -> w::NativeQuery {
    query(w::NativeQueryDefinition::System { query: recipe })
}
#[test]
fn observed_security_formats_do_not_conflate_unknown_with_disabled() {
    // Unedited non-identifying command output from macOS 26.4 / 25E246 arm64.
    for (section, sample, expected) in [
        (S::Gatekeeper, "assessments enabled\n", "enabled"),
        (
            S::Sip,
            "System Integrity Protection status: enabled.\n",
            "enabled",
        ),
        (S::Filevault, "FileVault is On.\n", "enabled"),
        (
            S::Firewall,
            "Firewall is disabled. (State = 0)\n",
            "disabled",
        ),
    ] {
        assert_eq!(security::switch(section, sample).unwrap(), expected);
    }
    // Synthetic negative/transition samples.
    assert_eq!(
        security::switch(
            S::Filevault,
            "Encryption in progress: Percent completed = 50"
        )
        .unwrap(),
        "unknown"
    );
    assert_eq!(
        security::switch(
            S::Sip,
            "System Integrity Protection status: unknown (Custom Configuration).\n"
        )
        .unwrap(),
        "unknown"
    );
    for sample in ["", "permission denied", "unexpected enabled status"] {
        assert!(security::switch(S::Gatekeeper, sample).is_err());
    }
}
#[test]
fn extension_configuration_never_proves_running_and_count_must_close() {
    let q = system(Q::MacosSecurity {
        section: S::SystemExtensions,
    });
    let mut rows = Rows::new(&q, 100);
    security::extensions("0 extension(s)\n", &mut rows).unwrap();
    assert!(rows.finish(Ok(())).complete_for(&q, 100));
    let sample = "1 extension(s)\n--- com.apple.system_extension.endpoint_security\nenabled active teamID bundleID (version) name [state]\n* * ABCDE12345 org.example.security (1.0/2) Example [activated enabled]\n";
    let mut rows = Rows::new(&q, 100);
    security::extensions(sample, &mut rows).unwrap();
    let result = rows.finish(Ok(()));
    assert_eq!(result.rows[0]["configured"], "enabled");
    assert_eq!(result.rows[0]["running"], "unknown");
    assert!(security::extensions("1 extension(s)\n", &mut Rows::new(&q, 100)).is_err());
    assert!(security::extensions("", &mut Rows::new(&q, 100)).is_err());
}
#[test]
fn tool_authorization_is_bound_to_recipe_and_never_enables_online_update() {
    let q = system(Q::MacosSecurity {
        section: S::Gatekeeper,
    });
    let input = input(q.clone());
    for (name, args, allowed) in [
        ("spctl", vec!["--status".into()], true),
        ("spctl", vec!["--master-disable".into()], false),
        ("fdesetup", vec!["status".into()], false),
        ("softwareupdate", vec!["--list".into()], false),
        ("sh", vec!["-c".into(), "true".into()], false),
    ] {
        assert_eq!(
            tool::allowed(&q.definition, name, &args, &input.task.execution),
            allowed
        );
    }
    for section in [w::UpdateSection::Scan, w::UpdateSection::Available] {
        let q = system(Q::MacosUpdates {
            section,
            mode: w::UpdateScanMode::Online,
        });
        let input = super::input(q.clone());
        let result = execute(&input, &q).unwrap();
        assert_eq!(result.status, Status::Unsupported);
        assert!(result.evidence_matches(&q));
        assert_eq!(result.scan.unwrap().source, None);
        assert_eq!(input.observed.load(std::sync::atomic::Ordering::Relaxed), 0);
    }
}
#[test]
fn cached_updates_need_original_interval_and_never_invent_empty_success() {
    // Shape sampled from the local SoftwareUpdate plist; values anonymized/synthetic.
    let mut d = plist::Dictionary::from_iter::<[(String, plist::Value); 3]>([
        ("LastResultCode".into(), 0i64.into()),
        (
            "LastSuccessfulDate".into(),
            plist::Value::Date(
                std::time::UNIX_EPOCH
                    .checked_add(Duration::from_secs(90))
                    .unwrap()
                    .into(),
            ),
        ),
        ("RecommendedUpdates".into(), plist::Value::Array(vec![])),
    ]);
    for section in [w::UpdateSection::Scan, w::UpdateSection::Available] {
        let q = system(Q::MacosUpdates {
            section,
            mode: w::UpdateScanMode::Local,
        });
        let mut rows = Rows::new(&q, 100);
        assert!(matches!(
            updates::cached(&d, section, &mut rows),
            Err(Fault::Query(Status::Partial))
        ));
        let result = rows.finish(Err(Status::Partial));
        assert!(result.evidence_matches(&q));
        assert!(!result.complete_for(&q, 100));
        let scan = result.scan.unwrap();
        assert_eq!((scan.started_at, scan.finished_at), (None, None));
    }
    d.insert("LastResultCode".into(), 1i64.into());
    let q = system(Q::MacosUpdates {
        section: w::UpdateSection::Scan,
        mode: w::UpdateScanMode::Local,
    });
    let mut rows = Rows::new(&q, 100);
    assert!(matches!(
        updates::cached(&d, w::UpdateSection::Scan, &mut rows),
        Err(Fault::Query(Status::Failed))
    ));
    assert_eq!(rows.result.rows[0]["availability"], "unknown");
    d.remove("RecommendedUpdates");
    assert!(matches!(
        updates::cached(&d, w::UpdateSection::Scan, &mut Rows::new(&q, 100)),
        Err(Fault::Query(Status::Invalid))
    ));
}
#[test]
fn system_user_results_bind_generated_uid_without_rewriting_software_identity() {
    let q = system(Q::MacosSecurity {
        section: S::SecureToken,
    });
    let input = input(q.clone());
    let guid = input.task.execution.generated_uid.as_deref().unwrap();
    let mut result = failure(&q, Status::Partial, 100);
    result
        .rows
        .push(row(&[("owner", guid.into()), ("state", "unknown".into())]));
    assert!(owner_matches(&input, &q, &result));
    result.rows[0].insert("owner".into(), "501".into());
    assert!(!owner_matches(&input, &q, &result));
    let q = system(Q::MacosApplication {
        target: w::MacosApplicationTarget::Agent,
        permissions: Default::default(),
    });
    assert!(owner_matches(
        &input,
        &q,
        &failure(&q, Status::Partial, 100)
    ));
}
#[test]
fn selected_account_guid_and_certificate_store_are_closed_by_wire() {
    let guid = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let q = system(Q::MacosAccounts {
        section: w::AccountSection::Accounts,
        selection: w::AccountSelection::Selected {
            identities: [guid.into()].into(),
        },
    });
    let mut r = row(&[
        ("id", guid.into()),
        ("uid", 501.into()),
        ("name", "fixture".into()),
        ("account_type", "local".into()),
        ("enabled", "unknown".into()),
    ]);
    assert!(Rows::new(&q, 100).push(r.clone()).is_ok());
    r.insert("id".into(), "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into());
    assert_eq!(Rows::new(&q, 100).push(r), Err(Status::Invalid));
    let q = system(Q::MacosCertificates {
        store: w::MacosCertificateStore::Login,
    });
    let mut r = row(&[
        ("id", "a".repeat(64).into()),
        ("subject", "CN=fixture".into()),
        ("issuer", "CN=CA".into()),
        ("not_before", 1.into()),
        ("not_after", 200.into()),
        ("store", "login".into()),
        ("scope", "user".into()),
        ("owner", guid.into()),
    ]);
    assert!(Rows::new(&q, 100).push(r.clone()).is_ok());
    r.insert("private_key".into(), "forbidden".into());
    assert_eq!(Rows::new(&q, 100).push(r), Err(Status::Invalid));
}

#[test]
fn public_certificate_is_projected_without_encoded_material_or_private_fields() {
    use base64::Engine;
    let pem = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/agent-protocol/pki/root-test-ca.pem"
    ));
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("---"))
        .collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(body)
        .unwrap();
    let row = certificates::decode(&bytes, "system_roots", "").unwrap();
    let q = system(Q::MacosCertificates {
        store: w::MacosCertificateStore::SystemRoots,
    });
    assert!(q.definition.accepts_row(&row));
    assert_eq!(row.len(), 8);
    assert_eq!(row["id"], hex(&bytes));
    assert!(row["not_after"].as_i64().unwrap() > row["not_before"].as_i64().unwrap());
    assert!(certificates::decode(b"invalid certificate", "system_roots", "").is_err());
}
