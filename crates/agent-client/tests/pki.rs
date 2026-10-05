#![allow(dead_code)]
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use agent_client::{Error, OpenMode};
use serde_json::Value;
use support::{Root, Server};

fn state(root: &Root, key: &str) -> Value {
    let connection = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    let body: Vec<u8> = connection
        .query_row("SELECT body FROM state WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn committed_renewal_is_recovered_over_anonymous_https_after_expiry_without_old_signer() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    let report = client
        .queue_report(
            "inventory",
            rss_mdm_agent_wire::runtime::ReportBody::Failed {
                code: rss_mdm_agent_wire::runtime::FailureCode::CollectionFailed,
            },
            1,
        )
        .unwrap();
    server.time.set(4_070_908_700);
    {
        let mut data = server.data.lock().unwrap();
        data.candidate_not_after = 4_071_000_000;
        data.registration_failure = true; // Receipt is committed but not returned.
    }
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Unavailable);
    let frozen = state(&root, "identity");
    assert!(frozen["pending"]["grant"].is_null());
    let old_key = frozen["active"]["key_reference"].as_str().unwrap();
    server.secrets.0.lock().unwrap().remove(old_key);
    server.time.set(4_070_908_801);
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    assert_eq!(
        client.runtime_readiness().unwrap_err(),
        Error::CertificateExpired
    );
    client.drive_pki().await.unwrap();
    assert_eq!(client.registration().unwrap(), binding);
    client.runtime_readiness().unwrap();
    {
        let data = server.data.lock().unwrap();
        assert_eq!(data.renewal_lookup_authenticated, vec![false]);
        assert_eq!(
            data.renewal_lookup_calls[0]["operationId"],
            frozen["pending"]["operation"]
        );
        assert_eq!(
            data.certificate_calls[1]["operationId"],
            frozen["pending"]["certificate_operation"]
        );
        assert_eq!(data.certificate_calls[1]["csr"], frozen["pending"]["csr"]);
        assert_eq!(
            data.activation_calls[1]["operationId"],
            frozen["pending"]["activation_operation"]
        );
    }
    assert_eq!(client.flush_reports(1).await.unwrap(), 1);
    assert!(server
        .data
        .lock()
        .unwrap()
        .reports
        .contains_key(&report.to_string()));
}

#[tokio::test]
async fn ack_requires_native_readback_and_replays_the_original_operation_after_restart() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    server.data.lock().unwrap().trust["revision"] = 2.into();
    server
        .secrets
        .0
        .lock()
        .unwrap()
        .insert("machine/read-failure".into(), "injected".into());
    server.time.set(301);
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Storage);
    assert_eq!(server.data.lock().unwrap().trust_ack_calls.len(), 1);
    server
        .secrets
        .0
        .lock()
        .unwrap()
        .remove("machine/read-failure");
    server.data.lock().unwrap().trust_ack_failure = true;
    server.time.set(303);
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Unavailable);
    let ack = state(&root, "pki")["ack"]["request"].clone();
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    server.time.set(304);
    client.drive_pki().await.unwrap(); // Durable exponential backoff prevents a premature retry.
    assert_eq!(server.data.lock().unwrap().trust_ack_calls.len(), 2);
    server.time.set(307);
    client.drive_pki().await.unwrap();
    assert_eq!(client.registration().unwrap(), binding);
    let data = server.data.lock().unwrap();
    assert_eq!(data.trust_ack_calls[1], ack);
    assert_eq!(data.trust_ack_calls[2], ack);
    assert_eq!(data.certificate_calls.len(), 1);
}

#[tokio::test]
async fn invalid_complete_trust_set_blocks_runtime_without_replacing_registration() {
    for fault in [
        "windows-purpose",
        "apple-purpose",
        "origin",
        "tenant",
        "mode",
        "signature",
        "future",
        "expired",
    ] {
        let server = Server::tls().await;
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        let binding = server.register(&mut client).await;
        let identity = state(&root, "identity");
        let accepted = state(&root, "pki")["document"].clone();
        {
            use base64::Engine;
            let mut data = server.data.lock().unwrap();
            match fault {
                "windows-purpose" => data.trust["purpose"] = "windows".into(),
                "apple-purpose" => data.trust["purpose"] = "apple".into(),
                "origin" => data.trust["origin"] = "https://another.example.test".into(),
                "tenant" => data.trust["tenant"] = uuid::Uuid::new_v4().to_string().into(),
                "mode" => data.trust["issuers"][0]["mode"] = "retired".into(),
                "signature" => {
                    let mut der = base64::engine::general_purpose::STANDARD
                        .decode(data.trust["issuers"][0]["crl"].as_str().unwrap())
                        .unwrap();
                    *der.last_mut().unwrap() ^= 1;
                    data.trust["issuers"][0]["crl"] =
                        base64::engine::general_purpose::STANDARD.encode(der).into();
                }
                "future" => {
                    data.trust["issuers"][0]["crl"] = support::pki::crl(2, 302, 600, &[]).into()
                }
                "expired" => {
                    data.trust["issuers"][0]["crl"] = support::pki::crl(2, 0, 301, &[]).into()
                }
                _ => unreachable!(),
            }
        }
        server.time.set(301);
        assert!(client.drive_pki().await.is_err(), "{fault}");
        assert_eq!(
            client.runtime_readiness().unwrap_err(),
            Error::Untrusted,
            "{fault}"
        );
        assert_eq!(client.registration().unwrap(), binding);
        assert_eq!(state(&root, "identity"), identity);
        assert_eq!(state(&root, "pki")["document"], accepted);
        assert_eq!(server.data.lock().unwrap().trust_ack_calls.len(), 1);
    }
}

#[tokio::test]
async fn crl_watermarks_reject_rollback_and_conflict_and_revocation_survives_restart() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    let publish = |number, time, revoked: &[u8]| {
        let crl = support::pki::crl(number, time, 4_102_444_800, revoked);
        let mut data = server.data.lock().unwrap();
        data.trust["issuers"][0]["crl"] = crl.clone().into();
        data.trust["issuers"][0]["root_crl"] = crl.into();
    };
    publish(10, 1, &[]);
    server.time.set(301);
    client.drive_pki().await.unwrap();
    let accepted = state(&root, "pki")["watermarks"].clone();
    for (now, number, issued) in [(602, 9, 1), (903, 10, 2)] {
        publish(number, issued, &[]);
        server.time.set(now);
        assert_eq!(client.drive_pki().await.unwrap_err(), Error::Conflict);
        assert_eq!(state(&root, "pki")["watermarks"], accepted);
    }
    publish(11, 3, &[1]);
    server.time.set(1204);
    assert_eq!(
        client.drive_pki().await.unwrap_err(),
        Error::CredentialRevoked
    );
    assert_eq!(
        client.runtime_readiness().unwrap_err(),
        Error::CredentialRevoked
    );
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    assert_eq!(
        client.runtime_readiness().unwrap_err(),
        Error::CredentialRevoked
    );
    assert_eq!(client.registration().unwrap(), binding);
}

#[tokio::test]
async fn an_interrupted_native_add_never_becomes_durable_ownership_from_presence() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server
        .secrets
        .0
        .lock()
        .unwrap()
        .insert("machine/add-response-lost".into(), "injected".into());
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Storage);
    assert!(state(&root, "pki")["installations"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| v["ownership"] == "Intent"));
    drop(client);
    server.time.set(3);
    let mut client = server.client(&root, OpenMode::Existing);
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Untrusted);
    // Physical presence after an interrupted add is ambiguous. Administrator completes the exact setting.
    use agent_client::MachineTrust;
    let der: Vec<u8> = serde_json::from_value(
        state(&root, "pki")["installations"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()["der"]
            .clone(),
    )
    .unwrap();
    server
        .secrets
        .configure_certificate(agent_client::MachineStore::Root, &der)
        .unwrap();
    server.time.set(7);
    client.drive_pki().await.unwrap();
    assert!(state(&root, "pki")["installations"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| v["ownership"] == "Borrowed"));
    assert!(server.data.lock().unwrap().trust_ack_calls.is_empty());
}

#[tokio::test]
async fn overlap_renews_under_new_issuer_and_root_crl_revocation_is_enforced() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    {
        let mut data = server.data.lock().unwrap();
        data.trust["revision"] = 2.into();
        data.trust["issuers"]
            .as_array_mut()
            .unwrap()
            .push(support::pki::rotation_issuer(false));
    }
    server.time.set(301);
    client.drive_pki().await.unwrap();
    let installed = state(&root, "pki")["installations"].clone();
    assert_eq!(installed.as_object().unwrap().len(), 3);
    assert_eq!(
        installed
            .as_object()
            .unwrap()
            .values()
            .filter(|v| v["store"] == "Root")
            .count(),
        2
    );
    {
        let mut data = server.data.lock().unwrap();
        data.trust["revision"] = 3.into();
        data.trust["issuers"][0]["mode"] = "verify_only".into();
        data.trust["issuers"][1]["mode"] = "active".into();
        data.candidate_rotated = true;
        data.candidate_not_after = 4_071_000_000;
    }
    server.time.set(4_070_908_700);
    client.drive_pki().await.unwrap();
    assert_eq!(client.registration().unwrap(), binding);
    client.runtime_readiness().unwrap();
    {
        let mut data = server.data.lock().unwrap();
        data.trust["revision"] = 4.into();
        let mut issuer = support::pki::rotation_issuer(true);
        issuer["mode"] = "active".into();
        data.trust["issuers"] = serde_json::json!([issuer]);
    }
    server.time.set(4_070_909_001);
    assert_eq!(
        client.drive_pki().await.unwrap_err(),
        Error::CredentialRevoked
    );
    assert_eq!(
        client.runtime_readiness().unwrap_err(),
        Error::CredentialRevoked
    );
    assert_eq!(client.registration().unwrap(), binding);
    assert_eq!(
        state(&root, "pki")["installations"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        server
            .secrets
            .0
            .lock()
            .unwrap()
            .keys()
            .filter(|key| key.starts_with("machine/"))
            .count(),
        2
    );
}

#[tokio::test]
async fn administrator_recovery_rejects_foreign_registration_and_retains_original_identity() {
    for foreign in [false, true] {
        let server = Server::tls().await;
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        let binding = server.register(&mut client).await;
        let original = state(&root, "identity")["active"].clone();
        server
            .secrets
            .0
            .lock()
            .unwrap()
            .remove(original["key_reference"].as_str().unwrap());
        server.time.set(4_070_908_801);
        let operation = uuid::Uuid::new_v4();
        let secret = rss_mdm_agent_wire::runtime::Secret::parse(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        )
        .unwrap();
        let grant = rss_mdm_agent_wire::enroll::GrantReceipt {
            wire_version: 1,
            grant_id: uuid::Uuid::new_v4(),
            state: rss_mdm_agent_wire::enroll::GrantState::Available,
            platform: server.config().platform,
            expires_at: 4_070_909_101,
        };
        {
            let mut data = server.data.lock().unwrap();
            data.operation = Some(operation);
            data.bootstrap = Some(secret.expose().into());
            data.grant = Some(grant.clone());
            data.activation = None;
            data.profile = None;
            data.issuance = uuid::Uuid::new_v4();
            data.candidate_not_after = 4_071_000_000;
            if foreign {
                data.binding.as_mut().unwrap().registration_id = uuid::Uuid::new_v4();
            }
        }
        let material = agent_client::GrantImport {
            operation_id: operation,
            grant,
        };
        assert_eq!(
            client
                .validate_grant_import(&material, "recovery", &binding.capabilities)
                .unwrap_err(),
            Error::Conflict
        );
        client
            .validate_recovery_import(&material, "recovery")
            .unwrap();
        client.import_bootstrap_secret("recovery", &secret).unwrap();
        client.import_recovery_grant(material, "recovery").unwrap();
        if foreign {
            assert_eq!(client.drive_pki().await.unwrap_err(), Error::Identity);
            assert_eq!(state(&root, "identity")["active"], original);
            assert!(!state(&root, "identity")["pending"].is_null());
        } else {
            client.drive_pki().await.unwrap();
            client.runtime_readiness().unwrap();
        }
        assert_eq!(client.registration().unwrap(), binding);
    }
}

#[tokio::test]
async fn expired_trust_blocks_during_durable_backoff_without_new_http() {
    let server = Server::tls().await;
    let crl = support::pki::crl(1, 0, 4, &[]);
    {
        let mut data = server.data.lock().unwrap();
        data.trust["issuers"][0]["crl"] = crl.clone().into();
        data.trust["issuers"][0]["root_crl"] = crl.into();
    }
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    server.data.lock().unwrap().trust_ack_failure = true;
    // Refresh before expiry, then transport failure starts a durable retry interval.
    server.time.set(3);
    server.data.lock().unwrap().registration_failure = true;
    server.data.lock().unwrap().trust["revision"] = 2.into();
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Unavailable);
    server.time.set(4);
    client.drive_pki().await.unwrap();
    assert_eq!(client.runtime_readiness().unwrap_err(), Error::Untrusted);
    assert_eq!(client.registration().unwrap(), binding);
}

#[tokio::test]
async fn retiring_approved_material_preserves_a_preexisting_machine_root() {
    use agent_client::{MachineStore, MachineTrust};
    use der::{DecodePem, Encode};
    let server = Server::tls().await;
    let old = x509_cert::Certificate::from_pem(support::pki::ca())
        .unwrap()
        .to_der()
        .unwrap();
    server
        .secrets
        .add_certificate(MachineStore::Root, &old)
        .unwrap();
    server
        .secrets
        .configure_certificate(MachineStore::Root, &old)
        .unwrap();
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    client.drive_pki().await.unwrap();
    assert_eq!(
        state(&root, "pki")["installations"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()["ownership"],
        "Borrowed"
    );
    let mut issuer = support::pki::rotation_issuer(false);
    issuer["mode"] = "active".into();
    {
        let mut data = server.data.lock().unwrap();
        data.trust["revision"] = 2.into();
        data.trust["issuers"] = serde_json::json!([issuer]);
    }
    server.time.set(301);
    client.drive_pki().await.unwrap();
    assert_eq!(
        state(&root, "pki")["installations"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert!(server
        .secrets
        .read_certificate(MachineStore::Root, &old)
        .unwrap());
}

#[tokio::test]
async fn machine_root_configuration_failure_retries_only_after_durable_ownership() {
    let server = Server::tls().await;
    let root = Root::new();
    server
        .secrets
        .0
        .lock()
        .unwrap()
        .insert("machine/configure-failure".into(), "injected".into());
    let mut client = server.client(&root, OpenMode::Create);
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Storage);
    assert!(state(&root, "pki")["installations"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| v["ownership"] == "Owned"));
    drop(client);
    server
        .secrets
        .0
        .lock()
        .unwrap()
        .remove("machine/configure-failure");
    server.time.set(3);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_pki().await.unwrap();
    assert!(state(&root, "pki")["installations"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| v["ownership"] == "Owned"));
}

#[tokio::test]
async fn rebuilding_preserves_external_and_ambiguous_material_through_restart_and_retirement() {
    use agent_client::{MachineStore, MachineTrust};
    use der::{DecodePem, Encode};
    for (fault, recorded, error) in [
        ("machine/add-present-race", "Borrowed", Error::Untrusted),
        ("machine/add-response-lost", "Intent", Error::Storage),
    ] {
        let server = Server::tls().await;
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        let binding = server.register(&mut client).await;
        let old = x509_cert::Certificate::from_pem(support::pki::ca())
            .unwrap()
            .to_der()
            .unwrap();
        // External removal invalidates the previous physical creation fact.
        server
            .secrets
            .delete_certificate(MachineStore::Root, &old)
            .unwrap();
        server
            .secrets
            .0
            .lock()
            .unwrap()
            .insert(fault.into(), "injected".into());
        server.time.set(301);
        assert_eq!(client.drive_pki().await.unwrap_err(), error);
        assert!(state(&root, "pki")["installations"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v["ownership"] == recorded));
        assert!(server
            .secrets
            .certificate_present(MachineStore::Root, &old)
            .unwrap());
        assert!(!server
            .secrets
            .read_certificate(MachineStore::Root, &old)
            .unwrap());
        drop(client);
        server.time.set(303);
        let mut client = server.client(&root, OpenMode::Existing);
        assert_eq!(client.drive_pki().await.unwrap_err(), Error::Untrusted);
        assert!(state(&root, "pki")["installations"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v["ownership"] == "Borrowed"));
        // Only an administrator repairs externally owned/uncertain root settings.
        server
            .secrets
            .configure_certificate(MachineStore::Root, &old)
            .unwrap();
        server.time.set(307);
        client.drive_pki().await.unwrap();
        let mut next = support::pki::rotation_issuer(false);
        next["mode"] = "active".into();
        {
            let mut data = server.data.lock().unwrap();
            data.trust["revision"] = 2.into();
            data.trust["issuers"] = serde_json::json!([next]);
        }
        server.time.set(901);
        assert_eq!(client.drive_pki().await.unwrap_err(), Error::Untrusted);
        assert!(server
            .secrets
            .certificate_present(MachineStore::Root, &old)
            .unwrap());
        assert_eq!(client.registration().unwrap(), binding);
    }
}

#[tokio::test]
async fn rebuilding_created_material_gets_fresh_ownership_and_can_be_retired() {
    use agent_client::{MachineStore, MachineTrust};
    use der::{DecodePem, Encode};
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let old = x509_cert::Certificate::from_pem(support::pki::ca())
        .unwrap()
        .to_der()
        .unwrap();
    server
        .secrets
        .delete_certificate(MachineStore::Root, &old)
        .unwrap();
    server.time.set(301);
    client.drive_pki().await.unwrap();
    assert!(state(&root, "pki")["installations"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| v["ownership"] == "Owned"));
    assert!(server
        .secrets
        .read_certificate(MachineStore::Root, &old)
        .unwrap());
    let mut next = support::pki::rotation_issuer(false);
    next["mode"] = "active".into();
    {
        let mut data = server.data.lock().unwrap();
        data.trust["revision"] = 2.into();
        data.trust["issuers"] = serde_json::json!([next]);
    }
    server.time.set(601);
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Untrusted);
    assert!(!server
        .secrets
        .certificate_present(MachineStore::Root, &old)
        .unwrap());
}
