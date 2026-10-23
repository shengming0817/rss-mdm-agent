#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use agent_client::{Client, Error, OpenMode};
use rss_mdm_agent_wire::connections::SignedConfiguration;
use serde_json::Value;
use support::{Root, Server};

fn stored(root: &Root) -> Value {
    let db = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    let bytes: Vec<u8> = db
        .query_row(
            "SELECT body FROM state WHERE key='connections'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn publish(
    server: &Server,
    origins: Vec<String>,
    preferred: String,
    revision: u64,
) -> SignedConfiguration {
    let mut c = server.config().connections.configuration;
    c.origins = origins;
    c.origins.sort();
    c.preferred_origin = preferred;
    c.revision = revision;
    let signed = support::sign_connections(c);
    server.data.lock().unwrap().connection_configuration = Some(signed.clone());
    signed
}
#[tokio::test]
async fn real_tls_address_publication_preserves_original_identity_cache_pending_work_and_restart() {
    let a = Server::tls().await;
    let b = a.alias().await;
    let root = Root::new();
    let cfg = a.config();
    let mut client = a.client(&root, OpenMode::Create);
    let identity = a.register(&mut client).await;
    let spool = client
        .telemetry_spool(agent_client::telemetry::Limits::production())
        .unwrap();
    let original_spool = serde_json::to_value(spool.summary().unwrap()).unwrap();
    drop(spool);
    client.drive_connections().await.unwrap();
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    a.data.lock().unwrap().start_failure = true;
    assert!(client.request_start(&offer, &materials).await.is_err());
    let original_credentials = a.secrets.0.lock().unwrap().clone();
    let original_cache = std::fs::read_dir(root.path.join("content"))
        .unwrap()
        .map(|p| p.unwrap().file_name())
        .collect::<Vec<_>>();
    let ao = a.url.origin().ascii_serialization();
    let bo = b.url.origin().ascii_serialization();
    publish(&a, vec![ao, bo.clone()], bo.clone(), 2);
    a.time.set(40);
    client.drive_connections().await.unwrap();
    assert_eq!(client.connection_status().unwrap().origin, bo);
    assert!(client.connection_status().unwrap().acknowledged);
    assert_eq!(client.registration().unwrap(), identity);
    drop(materials);
    drop(client);
    drop(a); // Every socket from A is closed, including pooled connections.
    let mut client = Client::open(
        &root.path,
        cfg.clone(),
        OpenMode::Existing,
        b.secrets.clone(),
        b.time.clone(),
    )
    .unwrap();
    let resumed = client.recover_start(offer.task_id()).await.unwrap();
    assert_eq!(resumed.offer.task_id(), offer.task_id());
    assert_eq!(client.registration().unwrap(), identity);
    let final_doc = publish(&b, vec![bo.clone()], bo.clone(), 3);
    b.time.set(80);
    client.drive_connections().await.unwrap();
    drop(client);
    // Program refresh retains its old seed and all protected storage rather than rebinding to A.
    let client = Client::open(
        &root.path,
        cfg,
        OpenMode::Existing,
        b.secrets.clone(),
        b.time.clone(),
    )
    .unwrap();
    assert_eq!(client.connection_status().unwrap().revision, 3);
    assert_eq!(client.connection_status().unwrap().origin, bo);
    assert_eq!(client.registration().unwrap(), identity);
    assert_eq!(*b.secrets.0.lock().unwrap(), original_credentials);
    assert_eq!(
        std::fs::read_dir(root.path.join("content"))
            .unwrap()
            .map(|p| p.unwrap().file_name())
            .collect::<Vec<_>>(),
        original_cache
    );
    assert!(!client.pending_tasks(32).unwrap().is_empty());
    let mut updated_package = client.configuration().clone();
    updated_package.origin = b.url.clone();
    updated_package.connections = final_doc;
    drop(client);
    let mut client = Client::open(
        &root.path,
        updated_package,
        OpenMode::Existing,
        b.secrets.clone(),
        b.time.clone(),
    )
    .unwrap();
    let spool = client
        .telemetry_spool(agent_client::telemetry::Limits::production())
        .unwrap();
    assert_eq!(
        serde_json::to_value(spool.summary().unwrap()).unwrap(),
        original_spool
    );
    client.recover_start(offer.task_id()).await.unwrap();
    assert_eq!(client.registration().unwrap(), identity);
}
#[tokio::test]
async fn unknown_ack_replays_original_operation_after_restart_and_new_route_gets_new_operation() {
    let a = Server::tls().await;
    let b = a.alias().await;
    let root = Root::new();
    let cfg = a.config();
    let mut client = a.client(&root, OpenMode::Create);
    a.register(&mut client).await;
    a.data.lock().unwrap().connection_ack_reply_lost = true;
    assert!(client.drive_connections().await.is_err());
    let pending = stored(&root)["ack"].clone();
    assert!(!client.connection_status().unwrap().acknowledged);
    drop(client);
    let mut client = a.client(&root, OpenMode::Existing);
    client.drive_connections().await.unwrap();
    assert_eq!(stored(&root)["ack"], pending);
    {
        let data = a.data.lock().unwrap();
        assert_eq!(data.connection_ack_calls[0], data.connection_ack_calls[1]);
    }
    let ao = a.url.origin().ascii_serialization();
    let bo = b.url.origin().ascii_serialization();
    let overlap = publish(&a, vec![ao.clone(), bo], ao.clone(), 2);
    client
        .import_connections(&serde_json::to_vec(&overlap).unwrap())
        .unwrap();
    client.drive_connections().await.unwrap();
    let before = stored(&root)["ack"].clone();
    drop(client);
    drop(a);
    let mut client = Client::open(
        &root.path,
        cfg,
        OpenMode::Existing,
        b.secrets.clone(),
        b.time.clone(),
    )
    .unwrap();
    // This drive reports a network error and selects B; it does not replay a write itself.
    b.time.set(80);
    assert_eq!(client.drive_connections().await, Err(Error::Unavailable));
    assert_ne!(stored(&root)["ack"]["operationId"], before["operationId"]);
    assert_ne!(stored(&root)["ack"]["origin"], before["origin"]);
    client.drive_connections().await.unwrap();
    assert!(client.connection_status().unwrap().acknowledged);
}
#[tokio::test]
async fn offline_import_rejects_forgery_identity_rollback_and_revision_reuse_without_changes() {
    let server = Server::tls().await;
    let root = Root::new();
    let cfg = server.config();
    let mut client = server.client(&root, OpenMode::Create);
    let identity = server.register(&mut client).await;
    let mut newer = cfg.connections.configuration.clone();
    newer.revision = 2;
    newer.origins = vec!["https://offline.fixture.test:9445".into()];
    newer.preferred_origin = newer.origins[0].clone();
    let newer = support::sign_connections(newer);
    client
        .import_connections(&serde_json::to_vec(&newer).unwrap())
        .unwrap();
    let installed = stored(&root);
    let keys = server.secrets.0.lock().unwrap().clone();
    let mut invalid = vec![cfg.connections.clone()];
    let mut forged = newer.clone();
    forged.signature.replace_range(..1, "A");
    invalid.push(forged);
    for field in ["tenant", "service", "same_revision", "unknown_key"] {
        let mut c = newer.configuration.clone();
        match field {
            "tenant" => c.tenant_id = uuid::Uuid::new_v4(),
            "service" => c.service_id = uuid::Uuid::new_v4(),
            "same_revision" => {
                c.origins = vec!["https://changed.fixture.test".into()];
                c.preferred_origin = c.origins[0].clone();
            }
            _ => c.key_id = "not-installed".into(),
        }
        invalid.push(support::sign_connections(c));
    }
    for file in invalid {
        assert!(client
            .import_connections(&serde_json::to_vec(&file).unwrap())
            .is_err());
        assert_eq!(stored(&root), installed);
    }
    assert_eq!(
        client.import_connections(&vec![b' '; 65537]),
        Err(Error::Capacity)
    );
    assert_eq!(client.registration().unwrap(), identity);
    assert_eq!(*server.secrets.0.lock().unwrap(), keys);
    assert!(server.data.lock().unwrap().connection_ack_calls.is_empty());
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    assert_eq!(client.connection_status().unwrap().revision, 2);
    assert_eq!(client.registration().unwrap(), identity);
}
#[tokio::test]
async fn failed_atomic_commit_cannot_change_route_or_ack() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let before = stored(&root);
    let db = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_connection_update BEFORE UPDATE ON state WHEN NEW.key='connections' BEGIN SELECT RAISE(ABORT,'injected commit refusal'); END;").unwrap();
    let mut c = server.config().connections.configuration;
    c.revision = 2;
    let signed = support::sign_connections(c);
    assert_eq!(
        client.import_connections(&serde_json::to_vec(&signed).unwrap()),
        Err(Error::Storage)
    );
    assert_eq!(stored(&root), before);
    assert!(server.data.lock().unwrap().connection_ack_calls.is_empty());
    db.execute_batch("DROP TRIGGER fail_connection_update;")
        .unwrap();
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    assert_eq!(client.connection_status().unwrap().revision, 1);
}

#[tokio::test]
async fn tls_and_http_errors_never_authorize_candidate_switching() {
    let a = Server::tls().await;
    let b = a.alias().await;
    let root = Root::new();
    let mut cfg = a.config();
    let mut client = a.client(&root, OpenMode::Create);
    a.register(&mut client).await;
    let ao = a.url.origin().ascii_serialization();
    let doc = publish(
        &a,
        vec![ao.clone(), b.url.origin().ascii_serialization()],
        ao.clone(),
        2,
    );
    client
        .import_connections(&serde_json::to_vec(&doc).unwrap())
        .unwrap();
    for status in [
        axum::http::StatusCode::UNAUTHORIZED,
        axum::http::StatusCode::TOO_MANY_REQUESTS,
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
    ] {
        a.data.lock().unwrap().connection_get_fault = Some(status);
        assert!(client.drive_connections().await.is_err());
        assert_eq!(client.connection_status().unwrap().origin, ao);
    }
    let before = stored(&root);
    drop(client);
    cfg.ca_pem = Some(support::pki::unrelated_ca());
    let mut client = Client::open(
        &root.path,
        cfg,
        OpenMode::Existing,
        a.secrets.clone(),
        a.time.clone(),
    )
    .unwrap();
    assert!(client.drive_connections().await.is_err());
    assert_eq!(stored(&root), before);
    assert_eq!(client.connection_status().unwrap().origin, ao);
    assert!(a.data.lock().unwrap().connection_ack_calls.is_empty());
}

#[tokio::test]
async fn missing_retained_configuration_never_reinitializes_from_the_package_seed() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let keys = server.secrets.0.lock().unwrap().clone();
    drop(client);
    let db = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    db.execute("DELETE FROM state WHERE key='connections'", [])
        .unwrap();
    drop(db);
    assert!(matches!(
        Client::open(
            &root.path,
            server.config(),
            OpenMode::Existing,
            server.secrets.clone(),
            server.time.clone()
        ),
        Err(Error::Storage)
    ));
    assert_eq!(*server.secrets.0.lock().unwrap(), keys);
    let db = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM state WHERE key='connections'",
            [],
            |row| row.get::<_, u64>(0)
        )
        .unwrap(),
        0
    );
}
