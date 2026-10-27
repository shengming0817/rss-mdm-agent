use agent_client::OpenMode;
use rss_mdm_agent_wire::platform_sso as w;
use serde_json::json;
use uuid::Uuid;
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use support::*;
#[tokio::test]
async fn psso_uses_device_mtls_without_employee_session_and_sends_revocation_fence() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let source = Uuid::new_v4();
    let tenant = server.data.lock().unwrap().tenant;
    let challenge = w::BootstrapResponse {
        id: Uuid::new_v4(),
        nonce: "test-challenge".into(),
        expires_at: 120,
        source_id: source,
        source_version: 2,
        endpoints: w::Endpoints::new("https://mdm.example", tenant).unwrap(),
        client_id: Uuid::new_v4(),
        login_user_name: source.to_string(),
    };
    server.data.lock().unwrap().response_override = Some(("/bootstrap".into(), json!(challenge)));
    let bootstrap = w::Bootstrap {
        operation_id: Uuid::new_v4(),
        kind: w::Kind::Device,
        binding_id: None,
        binding_generation: None,
    };
    assert_eq!(
        client
            .psso_bootstrap(&bootstrap)
            .await
            .unwrap()
            .source_version,
        2
    );
    server.data.lock().unwrap().response_override = Some((
        "/revoke".into(),
        json!(w::RevocationReceipt {
            source_id: source,
            source_version: 3,
            revoked: true
        }),
    ));
    assert_eq!(
        client.psso_revoke(source, 2).await.unwrap().source_version,
        3
    );
    let data = server.data.lock().unwrap();
    assert_eq!(data.psso_calls.len(), 2);
    assert_eq!(data.psso_calls[0].1, json!(bootstrap));
    assert!(data.psso_calls[1]
        .0
        .ends_with(&format!("/platform-sso/sources/{source}/revoke")));
    assert_eq!(data.psso_calls[1].1, json!({"sourceVersion": 2}));
}

#[tokio::test]
async fn registration_distinguishes_rejection_from_unknown_and_malformed_success() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let key = w::PublicKey {
        kty: "EC".into(),
        crv: "P-256".into(),
        x: String::new(),
        y: String::new(),
    };
    let input = w::Registration {
        operation_id: Uuid::new_v4(),
        challenge_id: Uuid::new_v4(),
        signing_key: Some(key.clone()),
        encryption_key: key,
        proof: "test-proof".into(),
    };
    for status in [400, 401, 403, 404, 409, 422] {
        server.data.lock().unwrap().psso_status_override =
            Some(axum::http::StatusCode::from_u16(status).unwrap());
        assert!(client.psso_register(&input).await.unwrap().is_none());
    }
    for status in [408, 500, 503] {
        server.data.lock().unwrap().psso_status_override =
            Some(axum::http::StatusCode::from_u16(status).unwrap());
        assert!(client.psso_register(&input).await.is_err());
    }
    server.data.lock().unwrap().psso_status_override = None;
    server.data.lock().unwrap().response_override =
        Some(("/register".into(), json!({"invalid":"success"})));
    assert!(client.psso_register(&input).await.is_err());
    let receipt = w::RegistrationReceipt {
        source_id: Uuid::new_v4(),
        source_version: 1,
        kind: w::Kind::Device,
        registered: true,
        hardware_attested: false,
        registration_trust: w::RegistrationTrust::MdmBootstrapPop,
    };
    server.data.lock().unwrap().response_override = Some(("/register".into(), json!(receipt)));
    assert_eq!(
        client
            .psso_register(&input)
            .await
            .unwrap()
            .unwrap()
            .source_id,
        receipt.source_id
    );
    let mut oversized = input;
    oversized.proof = "x".repeat(131073);
    let calls = server.data.lock().unwrap().psso_calls.len();
    assert!(client.psso_register(&oversized).await.unwrap().is_none());
    assert_eq!(server.data.lock().unwrap().psso_calls.len(), calls);
}

#[tokio::test]
#[should_panic(expected = "fixture HTTP contract violations")]
async fn invalid_request_cannot_pass_as_an_expected_http_error() {
    let server = Server::tls().await;
    let http = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let response = http
        .post(
            server
                .url
                .join("api/v1/agent/runtime/filevault/configuration")
                .unwrap(),
        )
        .send()
        .await
        .unwrap();
    assert!(response.status().is_client_error());
    // The fixture's owning test thread must fail even though the caller expected an error.
    drop(server);
}

#[tokio::test]
#[should_panic(expected = "fixture HTTP contract violations")]
async fn invalid_response_cannot_hide_in_an_unobserved_connection_task() {
    let server = Server::tls().await;
    server.data.lock().unwrap().trust = json!({});
    let http = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let response = http
        .get(server.url.join("api/v1/agent/enroll/trust").unwrap())
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    drop(server);
}
