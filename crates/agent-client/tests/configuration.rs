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
    let file = server.configuration();
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file.clone(),
            rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
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
    let file = server.configuration();
    let mut client = server.client(&root, OpenMode::Create);
    let mut altered = file.clone();
    altered.configuration.expires_at += 1;
    assert_eq!(
        client
            .prepare_configuration(
                altered,
                rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
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
            rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    let original = state(&root);
    client
        .prepare_configuration(
            file.clone(),
            rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    assert_eq!(state(&root), original);
    assert_eq!(
        client
            .prepare_configuration(
                file,
                rss_mdm_agent_wire::enroll::configuration::InstallerFormat::WindowsExe,
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
    let file = server.configuration();
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file,
            rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
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
    let file = server.configuration();
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file,
            rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
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
    let file = server.configuration();
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
            rss_mdm_agent_wire::enroll::configuration::InstallerFormat::WindowsExe,
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

#[tokio::test]
async fn explicit_first_exchange_refusal_allows_login_with_the_same_machine_key() {
    for code in [
        rss_mdm_agent_wire::enroll::ErrorCode::PermissionDenied,
        rss_mdm_agent_wire::enroll::ErrorCode::RegistrationLimit,
        rss_mdm_agent_wire::enroll::ErrorCode::GrantExpired,
        rss_mdm_agent_wire::enroll::ErrorCode::GrantRevoked,
    ] {
        let server = Server::tls().await;
        let root = Root::new();
        let file = server.configuration();
        let mut client = server.client(&root, OpenMode::Create);
        let caps = vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1];
        client
            .prepare_configuration(
                file,
                rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg,
                caps.clone(),
            )
            .unwrap();
        server.data.lock().unwrap().configuration_rejection = Some(code);
        assert!(client.drive_pki().await.is_err());
        let rejected = state(&root);
        assert!(!rejected["pending"]["configuration"]["rejection"].is_null());
        drop(client);
        let mut client = server.client(&root, OpenMode::Existing);
        client.prepare_authorization(caps).unwrap();
        assert_eq!(state(&root)["pending"]["csr"], rejected["pending"]["csr"]);
        assert_eq!(
            state(&root)["pending"]["operation"],
            rejected["pending"]["operation"]
        );
        assert!(server.data.lock().unwrap().grant.is_none());
    }
}

#[tokio::test]
async fn highest_applicable_version_is_frozen_before_unknown_exchange() {
    use rss_mdm_agent_wire::enroll::configuration::InstallerFormat;
    let server = Server::tls().await;
    let file = server.configuration();
    let root = Root::new();
    let chosen = uuid::Uuid::from_u128(10);
    {
        let mut data = server.data.lock().unwrap();
        let base = data.packages[0].clone();
        data.packages.clear();
        for (version, id) in [
            ("1.9.0", 9),
            ("1.10.0.1", 10),
            ("1.10.0.1", 8),
            ("1.10.0", 11),
        ] {
            let mut package = base.clone();
            package.version = version.into();
            package.release_id = uuid::Uuid::from_u128(id);
            data.packages.push(package);
        }
        for kind in ["tenant", "platform", "architecture", "format"] {
            let mut package = base.clone();
            package.version = "99.0.0".into();
            match kind {
                "tenant" => package.tenant_id = uuid::Uuid::new_v4(),
                "platform" => {
                    package.target.platform = rss_mdm_agent_wire::runtime::TaskPlatform::Windows
                }
                "architecture" => {
                    package.target.architecture =
                        rss_mdm_agent_wire::runtime::TaskArchitecture::X86_64
                }
                _ => package.format = InstallerFormat::WindowsExe,
            }
            data.packages.push(package);
        }
        data.registration_failure = true;
    }
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file,
            InstallerFormat::MacosPkg,
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Unavailable);
    let original = state(&root);
    assert_eq!(
        original["pending"]["configuration"]["installation"]["releaseId"],
        chosen.to_string()
    );
    drop(client);
    // A new release cannot change the already submitted operation or its recovery binding.
    server.data.lock().unwrap().packages[0].version = "100.0.0".into();
    server.time.set(101);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_pki().await.unwrap();
    assert!(client.enrollment_status().unwrap().registered);
    let data = server.data.lock().unwrap();
    assert_eq!(data.configuration_calls.len(), 1);
    assert_eq!(
        data.configuration_calls[0]["installation"]["releaseId"],
        chosen.to_string()
    );
}

#[tokio::test]
async fn invalid_applicable_version_does_not_silently_choose_another_release() {
    use rss_mdm_agent_wire::enroll::configuration::InstallerFormat;
    let server = Server::tls().await;
    let file = server.configuration();
    server.data.lock().unwrap().packages[0].version = "unversioned".into();
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    client
        .prepare_configuration(
            file,
            InstallerFormat::MacosPkg,
            vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1],
        )
        .unwrap();
    assert_eq!(client.drive_pki().await.unwrap_err(), Error::Protocol);
    assert!(server.data.lock().unwrap().configuration_calls.is_empty());
}

#[tokio::test]
async fn current_version_with_a_different_layout_is_rejected_without_writing() {
    let server = Server::tls().await;
    let root = Root::new();
    drop(server.client(&root, OpenMode::Create));
    let path = root.path.join("communication.sqlite");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("ALTER TABLE reports ADD COLUMN unexpected TEXT")
        .unwrap();
    drop(db);
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        agent_client::validate_existing(&root.path, &server.config()).unwrap_err(),
        Error::Schema
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
