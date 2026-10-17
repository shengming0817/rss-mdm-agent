#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use agent_client::{Error, OpenMode};
use rss_mdm_agent_wire::runtime::Capability;
use support::{Root, Server};
fn state(root: &Root) -> serde_json::Value {
    let c = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    let b: Vec<u8> = c
        .query_row("SELECT body FROM state WHERE key='identity'", [], |r| {
            r.get(0)
        })
        .unwrap();
    serde_json::from_slice(&b).unwrap()
}
#[tokio::test]
async fn lost_configuration_exchange_recovers_original_key_and_grant_after_file_expiry() {
    let server = Server::tls().await;
    let root = Root::new();
    let file = server.configuration([4; 32]);
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file.clone(),
            [4; 32],
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    server.data.lock().unwrap().registration_failure = true;
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Unavailable);
    let original = state(&root);
    assert!(original["pending"]["configuration"]["submitted"]
        .as_bool()
        .unwrap());
    assert!(!serde_json::to_string(&original)
        .unwrap()
        .contains(file.secret.expose()));
    drop(client);
    server.time.set(101);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_pki().await.unwrap();
    assert!(client.enrollment_status().unwrap().registered);
    let data = server.data.lock().unwrap();
    assert_eq!(data.configuration_calls.len(), 1);
    assert_eq!(data.configuration_status_calls.len(), 1);
    assert_eq!(
        data.configuration_status_calls[0]["operationId"],
        original["pending"]["operation"]
    );
    assert_eq!(data.certificate_calls[0]["csr"], original["pending"]["csr"]);
}
#[tokio::test]
async fn tampering_and_foreign_configuration_cannot_create_identity_or_replace_pending() {
    let server = Server::tls().await;
    let root = Root::new();
    let file = server.configuration([4; 32]);
    let mut client = server.client(&root, OpenMode::Create);
    let mut altered = file.clone();
    altered.configuration.expires_at += 1;
    assert_eq!(
        client
            .prepare_configuration(
                altered,
                [4; 32],
                vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1]
            )
            .unwrap_err(),
        Error::Untrusted
    );
    assert_eq!(
        client.enrollment_status().unwrap().phase,
        agent_client::EnrollmentPhase::Unregistered
    );
    client
        .prepare_configuration(
            file.clone(),
            [4; 32],
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    let original = state(&root);
    client
        .prepare_configuration(
            file.clone(),
            [4; 32],
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    assert_eq!(state(&root), original);
    assert_eq!(
        client
            .prepare_configuration(
                file,
                [5; 32],
                vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1]
            )
            .unwrap_err(),
        Error::Conflict
    );
    assert_eq!(state(&root), original);
}

#[tokio::test]
async fn login_can_reuse_a_never_submitted_configuration_but_cannot_replace_unknown_allocation() {
    let server = Server::tls().await;
    let root = Root::new();
    let file = server.configuration([4; 32]);
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file,
            [4; 32],
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    let original = state(&root);
    client
        .prepare_authorization(vec![
            Capability::InventoryCollectionV1,
            Capability::TaskExecuteV1,
        ])
        .unwrap();
    let switched = state(&root);
    assert_eq!(
        switched["pending"]["operation"],
        original["pending"]["operation"]
    );
    assert_eq!(switched["pending"]["csr"], original["pending"]["csr"]);
    assert!(switched["pending"]["configuration"].is_null());
    let server = Server::tls().await;
    let root = Root::new();
    let file = server.configuration([4; 32]);
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file,
            [4; 32],
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    server.data.lock().unwrap().registration_failure = true;
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Unavailable);
    let original = state(&root);
    assert_eq!(
        client
            .prepare_authorization(vec![
                Capability::InventoryCollectionV1,
                Capability::TaskExecuteV1
            ])
            .unwrap_err(),
        Error::Conflict
    );
    assert_eq!(state(&root), original);
}

#[tokio::test]
async fn signed_foreign_expired_and_wrong_target_files_are_rejected_before_machine_state() {
    use base64::Engine;
    let server = Server::tls().await;
    let file = server.configuration([4; 32]);
    let cfg = server.config();
    for kind in ["tenant", "origin", "target", "expiry"] {
        let mut changed = file.clone();
        match kind {
            "tenant" => changed.configuration.tenant_id = uuid::Uuid::new_v4(),
            "origin" => changed.configuration.origin = "https://other.example".into(),
            "target" => {
                changed.configuration.targets[0].platform =
                    rss_mdm_agent_wire::runtime::TaskPlatform::Windows
            }
            _ => changed.configuration.expires_at = 1,
        }
        changed.signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            server
                .data
                .lock()
                .unwrap()
                .signer
                .sign(
                    &changed
                        .configuration
                        .signing_bytes(&changed.secret)
                        .unwrap(),
                )
                .as_ref(),
        );
        assert!(
            agent_client::verify_configuration(&changed, &cfg, 1).is_err(),
            "{kind}"
        );
    }
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file,
            [5; 32],
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Denied);
    assert!(server.data.lock().unwrap().configuration_calls.is_empty());
    let original = state(&root);
    client
        .prepare_authorization(vec![
            Capability::InventoryCollectionV1,
            Capability::TaskExecuteV1,
        ])
        .unwrap();
    assert_eq!(state(&root)["pending"]["csr"], original["pending"]["csr"]);
}
