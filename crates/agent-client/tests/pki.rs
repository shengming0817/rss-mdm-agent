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

fn idp(only_ca: bool) -> Vec<u8> {
    use der::Encode;
    use x509_cert::ext::pkix::{
        crl::IssuingDistributionPoint,
        name::{DistributionPointName, GeneralName},
    };
    IssuingDistributionPoint {
        distribution_point: Some(DistributionPointName::FullName(vec![
            GeneralName::UniformResourceIdentifier(
                der::asn1::Ia5String::new("https://step-ca/1.0/crl").unwrap(),
            ),
        ])),
        only_contains_user_certs: false,
        only_contains_ca_certs: only_ca,
        only_some_reasons: None,
        indirect_crl: false,
        only_contains_attribute_certs: false,
    }
    .to_der()
    .unwrap()
}

#[tokio::test]
async fn full_direct_critical_idp_allows_registration_and_refresh() {
    let server = Server::tls().await;
    let root = Root::new();
    {
        let mut data = server.data.lock().unwrap();
        let crl = support::pki::crl_with_extension(1, "2.5.29.28", true, idp(false));
        data.trust["issuers"][0]["crl"] = crl.clone().into();
        data.trust["issuers"][0]["root_crl"] = crl.into();
    }
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    {
        let mut data = server.data.lock().unwrap();
        let crl = support::pki::crl_with_extension(2, "2.5.29.28", true, idp(false));
        data.trust["issuers"][0]["crl"] = crl.clone().into();
        data.trust["issuers"][0]["root_crl"] = crl.into();
    }
    server.time.set(301);
    client.drive_pki().await.unwrap();
    client.runtime_readiness().unwrap();
    assert_eq!(client.registration().unwrap(), binding);
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
async fn ack_replays_the_original_operation_after_private_trust_readback_and_restart() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    server.data.lock().unwrap().trust["revision"] = 2.into();
    server.data.lock().unwrap().trust_ack_failure = true;
    server.time.set(301);
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Unavailable);
    let ack = state(&root, "pki")["ack"]["request"].clone();
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    server.time.set(302);
    client.drive_pki().await.unwrap(); // Original durable backoff still applies.
    assert_eq!(server.data.lock().unwrap().trust_ack_calls.len(), 2);
    server.time.set(305);
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
        "service",
        "tenant",
        "mode",
        "signature",
        "future",
        "expired",
        "partitioned-idp",
        "noncritical-idp",
        "unknown-critical",
        "delta",
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
                "service" => data.trust["service_id"] = uuid::Uuid::new_v4().to_string().into(),
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
                "partitioned-idp" | "noncritical-idp" => {
                    data.trust["issuers"][0]["crl"] = support::pki::crl_with_extension(
                        2,
                        "2.5.29.28",
                        fault != "noncritical-idp",
                        idp(fault == "partitioned-idp"),
                    )
                    .into();
                }
                "unknown-critical" | "delta" => {
                    data.trust["issuers"][0]["crl"] = support::pki::crl_with_extension(
                        2,
                        if fault == "delta" {
                            "2.5.29.27"
                        } else {
                            "1.2.3.4"
                        },
                        true,
                        vec![2, 1, 1],
                    )
                    .into();
                }
                _ => unreachable!(),
            }
            if matches!(
                fault,
                "partitioned-idp" | "noncritical-idp" | "unknown-critical" | "delta"
            ) {
                // This fixture uses one self-signed CA for both roles: keep the
                // CRL identical so rejection cannot come from watermark conflict.
                data.trust["issuers"][0]["root_crl"] = data.trust["issuers"][0]["crl"].clone();
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
                .prepare_configuration(
                    server.configuration(),
                    rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
                    binding.capabilities.clone()
                )
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
async fn retained_identity_and_private_trust_refresh_ignore_historical_machine_receipts() {
    use rss_mdm_agent_wire::pki::ObservationState;
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    let original = client.pki_observation().unwrap().unwrap();
    let identity = state(&root, "identity");
    let mut pki = state(&root, "pki");
    let watermarks = pki["watermarks"].clone();
    let history = serde_json::json!({"root/previous-installation": {
        "store":"Root", "der":[1,2,3], "ownership":"Owned"
    }});
    pki["installations"] = history.clone();
    drop(client);
    let connection = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    connection
        .execute(
            "UPDATE state SET body=?1 WHERE key='pki'",
            [serde_json::to_vec(&pki).unwrap()],
        )
        .unwrap();
    drop(connection);
    let mut client = server.client(&root, OpenMode::Existing);
    client.runtime_readiness().unwrap();
    server.time.set(301);
    client.drive_pki().await.unwrap();
    let refreshed = client.pki_observation().unwrap().unwrap();
    assert_eq!(refreshed.state, ObservationState::Applied);
    assert_eq!(refreshed.actual_revision, original.actual_revision);
    assert!(refreshed.sequence > original.sequence);
    assert!(client.pki_acknowledged().unwrap());
    assert_eq!(client.registration().unwrap(), binding);
    assert_eq!(state(&root, "identity"), identity);
    assert_eq!(state(&root, "pki")["watermarks"], watermarks);
    assert_eq!(state(&root, "pki")["installations"], history);
    assert_eq!(server.data.lock().unwrap().certificate_calls.len(), 1);
}

#[tokio::test]
async fn native_locator_change_preserves_the_agent_service_binding() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let binding = server.register(&mut client).await;
    {
        let mut data = server.data.lock().unwrap();
        data.trust["origin"] = "https://native-entry.example.test:9445".into();
        data.trust["revision"] = 2.into();
    }
    server.time.set(400);
    client.drive_pki().await.unwrap();
    assert_eq!(client.registration().unwrap(), binding);
    assert_eq!(
        state(&root, "pki")["document"]["origin"],
        "https://native-entry.example.test:9445"
    );
}
