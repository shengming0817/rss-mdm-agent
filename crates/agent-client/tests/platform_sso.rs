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
