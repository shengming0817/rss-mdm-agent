#![allow(dead_code)]
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use agent_client::{Error, OpenMode};
use base64::Engine;
use ring::signature::{Ed25519KeyPair, KeyPair};
use rss_mdm_agent_wire::{
    runtime::Capability,
    task_trust::{KeyState, PublicKey, Trust},
};
use serde_json::Value;
use support::{Root, Server};

fn state(root: &Root) -> Value {
    let c = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    let bytes: Vec<u8> = c
        .query_row("SELECT body FROM state WHERE key='task_trust'", [], |r| {
            r.get(0)
        })
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn prepared(server: &Server) -> Trust {
    let mut trust: Trust =
        serde_json::from_value(server.data.lock().unwrap().task_trust.clone()).unwrap();
    trust.revision += 1;
    trust.keys.push(PublicKey {
        id: "z-next".into(),
        public_key: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            Ed25519KeyPair::from_seed_unchecked(&[8; 32])
                .unwrap()
                .public_key()
                .as_ref(),
        ),
        state: KeyState::Prepared,
    });
    trust
}
fn publish(server: &Server, trust: &Trust, now: i64) {
    server.data.lock().unwrap().task_trust = serde_json::to_value(trust).unwrap();
    server.time.set(now);
}
#[tokio::test]
async fn first_sync_failure_retains_identity_and_never_uses_installation_keys() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    client
        .request_authorization(vec![
            Capability::InventoryCollectionV1,
            Capability::TaskExecuteV1,
        ])
        .await
        .unwrap();
    client.drive_pki().await.unwrap();
    let identity = client.registration().unwrap();
    assert!(matches!(client.claim().await, Err(Error::Untrusted)));
    server.data.lock().unwrap().task_trust_failure = true;
    assert!(client.drive_task_trust().await.is_err());
    assert_eq!(client.registration().unwrap(), identity);
    assert!(client.runtime_readiness().is_err());
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_task_trust().await.unwrap();
    client.runtime_readiness().unwrap();
    assert_eq!(client.registration().unwrap(), identity);
    assert!(client.task_trust_acknowledged().unwrap());
}
#[tokio::test]
async fn prepared_ack_loss_reopen_activation_and_retirement_use_only_durable_trust() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut trust = prepared(&server);
    publish(&server, &trust, 70);
    server.data.lock().unwrap().task_trust_ack_failure = true;
    assert!(client.drive_task_trust().await.is_err());
    let ack = state(&root)["ack"].clone();
    assert_eq!(ack["revision"], 2);
    assert_eq!(state(&root)["document"]["revision"], 2);
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_task_trust().await.unwrap();
    let calls = server.data.lock().unwrap().task_trust_ack_calls.clone();
    assert_eq!(calls[calls.len() - 2], ack);
    assert_eq!(calls[calls.len() - 1], ack);
    // Prepared keys authenticate content; a separate Start exchange is still mandatory.
    {
        let mut d = server.data.lock().unwrap();
        d.signer = Ed25519KeyPair::from_seed_unchecked(&[8; 32]).unwrap();
        d.task_key_id = "z-next".into();
        d.script();
    }
    let prepared_offer = client.claim().await.unwrap().offer.unwrap();
    client.prepare(&prepared_offer).await.unwrap();
    trust.revision = 3;
    trust.keys[0].state = KeyState::VerifyOnly;
    trust.keys[1].state = KeyState::Active;
    publish(&server, &trust, 140);
    client.drive_task_trust().await.unwrap();
    trust.revision = 4;
    trust.keys.remove(0);
    publish(&server, &trust, 210);
    client.drive_task_trust().await.unwrap();
    // The installation still trusts the old bootstrap key, but runtime intake cannot resurrect it.
    {
        let mut d = server.data.lock().unwrap();
        d.signer = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        d.task_key_id = "test".into();
        d.script();
        d.claims.clear();
    }
    assert!(matches!(client.claim().await, Err(Error::Untrusted)));
}
#[tokio::test]
async fn invalid_updates_and_failed_sqlite_commit_do_not_ack_or_replace_installed_authority() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let original = state(&root)["document"].clone();
    for (index, field, value) in [
        (0, "tenant", serde_json::json!(uuid::Uuid::new_v4())),
        (1, "origin", serde_json::json!("https://foreign.invalid")),
        (2, "revision", serde_json::json!(0)),
    ] {
        let mut invalid = original.clone();
        invalid[field] = value;
        server.data.lock().unwrap().task_trust = invalid;
        server.time.set(70 + index * 70);
        assert!(client.drive_task_trust().await.is_err());
        assert_eq!(state(&root)["document"], original);
    }
    server.data.lock().unwrap().task_trust = original.clone();
    let mut changed = original.clone();
    changed["keys"][0]["public_key"] =
        serde_json::json!(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([9; 32]));
    server.data.lock().unwrap().task_trust = changed;
    server.time.set(280);
    assert!(client.drive_task_trust().await.is_err());
    assert_eq!(state(&root)["document"], original);
    server.data.lock().unwrap().task_trust = original.clone();
    let trust = prepared(&server);
    publish(&server, &trust, 350);
    let ack_count = server.data.lock().unwrap().task_trust_ack_calls.len();
    let c = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    c.execute_batch("CREATE TRIGGER reject_task_commit BEFORE INSERT ON state WHEN NEW.key='task_trust' AND json_extract(CAST(NEW.body AS TEXT),'$.document.revision')=2 BEGIN SELECT RAISE(FAIL,'disk fault'); END;").unwrap();
    assert!(client.drive_task_trust().await.is_err());
    assert_eq!(state(&root)["document"], original);
    assert_eq!(
        server.data.lock().unwrap().task_trust_ack_calls.len(),
        ack_count
    );
    assert_eq!(state(&root)["latest"]["state"], "failed");
    c.execute_batch("DROP TRIGGER reject_task_commit").unwrap();
    drop(c);
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_task_trust().await.unwrap();
    assert_eq!(state(&root)["document"]["revision"], 2);
    assert!(client.task_trust_acknowledged().unwrap());
}

#[tokio::test]
async fn rotated_uninstall_evidence_only_recovers_original_installation_and_task() {
    let server = Server::tls().await;
    server.data.lock().unwrap().unmanage();
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let cfg = client.configuration().clone();
    let mut trust = prepared(&server);
    publish(&server, &trust, 70);
    client.drive_task_trust().await.unwrap();
    trust.revision = 3;
    trust.keys[0].state = KeyState::VerifyOnly;
    trust.keys[1].state = KeyState::Active;
    publish(&server, &trust, 140);
    client.drive_task_trust().await.unwrap();
    {
        let mut d = server.data.lock().unwrap();
        d.signer = Ed25519KeyPair::from_seed_unchecked(&[8; 32]).unwrap();
        d.task_key_id = "z-next".into();
        d.unmanage();
    }
    let offer = client.claim().await.unwrap().offer.unwrap();
    let installation = uuid::Uuid::new_v4();
    let permit = client
        .authorize_unmanage(&offer, installation)
        .await
        .unwrap();
    let permit = client.unmanage_drained(permit).unwrap();
    assert!(!cfg.registration_keys.contains_key("z-next"));
    drop(client);
    let c = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    c.execute("DELETE FROM state WHERE key='task_trust'", [])
        .unwrap();
    drop(c);
    let mut client = agent_client::Client::open(
        &root.path,
        cfg.clone(),
        OpenMode::Existing,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    assert!(client.accepted_unmanage(installation).unwrap().is_some());
    permit.verify_handoff(&cfg, installation).unwrap();
    assert!(permit.verify_handoff(&cfg, uuid::Uuid::new_v4()).is_err());
    assert!(client.claim().await.is_err());
    let mut tampered = serde_json::to_value(&permit).unwrap();
    tampered["start"]["payload"]["task"]["taskId"] = serde_json::json!(uuid::Uuid::new_v4());
    let tampered: agent_client::UnmanagePermit = serde_json::from_value(tampered).unwrap();
    assert!(tampered.verify_handoff(&cfg, installation).is_err());
}
#[tokio::test]
async fn old_database_is_rejected_before_writing_original_materials() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    drop(client);
    let db = root.path.join("communication.sqlite");
    let c = rusqlite::Connection::open(&db).unwrap();
    c.pragma_update(None, "user_version", 7).unwrap();
    drop(c);
    let before = std::fs::read(&db).unwrap();
    assert!(matches!(
        agent_client::Client::open(
            &root.path,
            server.config(),
            OpenMode::Existing,
            server.secrets.clone(),
            server.time.clone()
        ),
        Err(Error::Schema)
    ));
    assert_eq!(std::fs::read(db).unwrap(), before);
}

#[tokio::test]
async fn same_process_ack_recovery_restores_and_reports_applied_without_refresh() {
    use rss_mdm_agent_wire::task_trust::ObservationState;
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let trust = prepared(&server);
    publish(&server, &trust, 70);
    server.data.lock().unwrap().task_trust_ack_failure = true;
    assert_eq!(
        client.drive_task_trust().await.unwrap_err(),
        Error::Unavailable
    );
    let failed = client.task_trust_observation().unwrap().unwrap();
    assert_eq!(failed.state, ObservationState::Failed);
    assert!(!client.task_trust_acknowledged().unwrap());
    let frozen = state(&root);
    // No reopen and no refresh: recovery must happen in the ACK-only branch.
    server.data.lock().unwrap().task_trust_failure = true;
    server.time.set(72);
    client.drive_task_trust().await.unwrap();
    assert!(client.task_trust_acknowledged().unwrap());
    let applied = client.task_trust_observation().unwrap().unwrap();
    assert_eq!(applied.state, ObservationState::Applied);
    assert_eq!(applied.actual_revision, Some(trust.revision));
    assert_eq!(
        applied.target_digest,
        Some(trust.installation_digest().unwrap())
    );
    assert!(applied.sequence > failed.sequence);
    assert_eq!(state(&root)["refresh_at"], frozen["refresh_at"]);
    let data = server.data.lock().unwrap();
    assert!(data.task_trust_failure); // GET fault was never consumed.
    assert_eq!(
        data.task_trust_ack_calls[data.task_trust_ack_calls.len() - 1],
        frozen["ack"]
    );
    assert_eq!(
        data.task_trust_ack_calls[data.task_trust_ack_calls.len() - 2],
        frozen["ack"]
    );
    assert!(data
        .observations
        .iter()
        .any(|v| v["operation"] == applied.operation.to_string() && v["state"] == "applied"));
}

#[tokio::test]
async fn protected_exact_origin_allowlist_preserves_identity_and_installed_authority() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut cfg = server.config();
    cfg.task_trust_origins = vec!["https://task-authority.example.org".into()];
    let open = |mode, config| {
        agent_client::Client::open(
            &root.path,
            config,
            mode,
            server.secrets.clone(),
            server.time.clone(),
        )
        .unwrap()
    };
    let mut client = open(OpenMode::Create, cfg.clone());
    client
        .request_authorization(vec![
            Capability::InventoryCollectionV1,
            Capability::TaskExecuteV1,
        ])
        .await
        .unwrap();
    client.drive_pki().await.unwrap();
    let identity = client.registration().unwrap();
    let mut trust = {
        let data = server.data.lock().unwrap();
        Trust {
            version: 1,
            tenant: data.tenant,
            origin: String::new(),
            revision: 1,
            keys: vec![PublicKey {
                id: data.task_key_id.clone(),
                public_key: base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(data.signer.public_key().as_ref()),
                state: KeyState::Active,
            }],
        }
    };
    trust.origin = "https://task-authority.example.org:9445".into();
    publish(&server, &trust, 70);
    assert!(matches!(
        client.drive_task_trust().await,
        Err(Error::Untrusted)
    ));
    assert!(!client.task_trust_acknowledged().unwrap());
    trust.origin = cfg.task_trust_origins[0].clone();
    publish(&server, &trust, 80);
    client.drive_task_trust().await.unwrap();
    client.runtime_readiness().unwrap();
    assert!(client.task_trust_acknowledged().unwrap());
    assert_eq!(client.registration().unwrap(), identity);
    let installed = state(&root)["document"].clone();
    drop(client);
    // Removing local approval closes readiness without discarding identity or trust history.
    let mut removed = cfg.clone();
    removed.task_trust_origins.clear();
    let client = open(OpenMode::Existing, removed);
    assert!(matches!(client.runtime_readiness(), Err(Error::Untrusted)));
    assert_eq!(client.registration().unwrap(), identity);
    drop(client);
    // Even another explicitly approved origin cannot replace the installed authority.
    cfg.task_trust_origins
        .push("https://other-authority.example.org".into());
    let mut client = open(OpenMode::Existing, cfg);
    trust.origin = "https://other-authority.example.org".into();
    trust.revision += 1;
    publish(&server, &trust, 150);
    assert!(client.drive_task_trust().await.is_err());
    assert_eq!(state(&root)["document"], installed);
    assert_eq!(client.registration().unwrap(), identity);
}
#[test]
fn task_trust_allowlist_rejects_ambiguous_or_unbounded_origins() {
    for value in [
        "http://example.org",
        "https://example.org/",
        "https://example.org/path",
        "https://example.org?x=1",
        "https://example.org#x",
        "https://u:p@example.org",
        "https://*.example.org",
    ] {
        assert!(
            agent_client::validate_task_trust_origins(&[value.into()]).is_err(),
            "{value}"
        );
    }
    assert!(
        agent_client::validate_task_trust_origins(&vec!["https://example.org".into(); 2]).is_err()
    );
    assert!(agent_client::validate_task_trust_origins(
        &(0..9)
            .map(|i| format!("https://host{i}.example.org"))
            .collect::<Vec<_>>()
    )
    .is_err());
    agent_client::validate_task_trust_origins(&[
        "https://example.org".into(),
        "https://example.org:9445".into(),
    ])
    .unwrap();
}
