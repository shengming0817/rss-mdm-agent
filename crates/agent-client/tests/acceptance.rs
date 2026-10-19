use agent_client::{Client, Config, Error, Limits, OpenMode, Transport};
use rss_mdm_agent_wire::runtime::*;
use url::Url;
use uuid::Uuid;
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use rss_mdm_agent_wire::runtime::{TaskArchitecture, TaskPlatform};
use support::*;
#[tokio::test]
async fn refresh_inspection_reads_live_binding_without_locking_or_creating_state() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let cfg = client.configuration().clone();
    let before = std::fs::read(root.path.join("communication.sqlite")).unwrap();
    let registration = agent_client::inspect_binding(&root.path, &cfg).unwrap();
    assert_eq!(
        registration.registration_id,
        client.registration().unwrap().registration_id
    );
    let mut wrong = cfg.clone();
    wrong.tenant = Uuid::new_v4();
    assert!(matches!(
        agent_client::inspect_binding(&root.path, &wrong),
        Err(Error::Identity)
    ));
    assert_eq!(
        before,
        std::fs::read(root.path.join("communication.sqlite")).unwrap()
    );
    let empty = Root::new();
    assert!(agent_client::inspect_binding(&empty.path, &cfg).is_err());
    assert_eq!(std::fs::read_dir(&empty.path).unwrap().count(), 0);
}

#[tokio::test]
async fn expired_lost_claim_recovers_after_restart_with_new_operation() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    server.data.lock().unwrap().claim_failure = true;
    assert!(matches!(client.claim().await, Err(Error::Unavailable)));
    drop(client);
    server.time.set(72);
    let mut client = server.client(&root, OpenMode::Existing);
    let offer = client.claim().await.unwrap().offer.unwrap();
    assert_eq!(offer.payload().expires_at(), 132);
    let ops = server.data.lock().unwrap().claim_ops.clone();
    assert_eq!(ops.len(), 2);
    assert_ne!(ops[0], ops[1]);
}

#[tokio::test]
async fn lost_start_reply_replays_same_request_after_offer_expiry_without_renewing_permit() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    server.time.set(59);
    client.received(&offer).await.unwrap();
    server.data.lock().unwrap().start_failure = true;
    assert!(matches!(
        client.request_start(&offer, &materials).await,
        Err(Error::Unavailable)
    ));
    let task = offer.task_id();
    drop(materials);
    drop(offer);
    drop(client);
    server.time.set(62);
    let mut client = server.client(&root, OpenMode::Existing);
    let recovered = client.recover_start(task).await.unwrap();
    let start = recovered.start;
    let ops = server.data.lock().unwrap().start_ops.clone();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0], ops[1]);
    server.time.set(75);
    assert_eq!(client.validate_start(&start), Err(Error::Expired));
}

#[tokio::test]
async fn material_reader_starts_at_zero_and_revocation_invalidates_held_start() {
    use std::io::Read;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    let mut bytes = Vec::new();
    materials.files()[0]
        .reader()
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, server.data.lock().unwrap().bytes);
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    client
        .queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            1,
        )
        .unwrap();
    server.data.lock().unwrap().denied = true;
    assert_eq!(client.flush_reports(1).await, Err(Error::CredentialRevoked));
    assert_eq!(client.validate_start(&start), Err(Error::CredentialRevoked));
}

#[tokio::test]
async fn queue_capacity_clock_rollback_and_namespace_change_fail_closed() {
    let server = Server::new().await;
    let root = Root::new();
    let mut cfg = server.config();
    cfg.limits.pending_reports = 1;
    let mut client = Client::open(
        &root.path,
        cfg,
        OpenMode::Create,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    server.register(&mut client).await;
    server.time.set(2);
    client
        .queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            2,
        )
        .unwrap();
    assert_eq!(
        client.queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed
            },
            2
        ),
        Err(Error::Capacity)
    );
    server.time.set(1);
    assert_eq!(client.flush_reports(1).await, Err(Error::Clock));
    drop(client);
    let mut cfg = server.config();
    cfg.tenant = Uuid::new_v4();
    assert!(matches!(
        Client::open(
            &root.path,
            cfg,
            OpenMode::Existing,
            server.secrets.clone(),
            server.time.clone()
        ),
        Err(Error::Identity)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_partial_never_reads_or_overwrites_external_file() {
    let server = Server::new().await;
    let root = Root::new();
    let external = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    server.data.lock().unwrap().content_failure = true;
    assert!(client.prepare(&offer).await.is_err());
    let partial = std::fs::read_dir(root.path.join("content"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let target = external.path.join("protected");
    platform_private_storage::write_new(&target, b"keep").unwrap();
    std::fs::remove_file(&partial).unwrap();
    std::os::unix::fs::symlink(&target, &partial).unwrap();
    assert!(matches!(client.prepare(&offer).await, Err(Error::Storage)));
    assert_eq!(std::fs::read(target).unwrap(), b"keep");
}

#[tokio::test]
async fn register_report_recovery_and_exact_ack() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.data.lock().unwrap().registration_failure = true;
    let capabilities = vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1];
    assert!(matches!(
        client.request_authorization(capabilities.clone()).await,
        Err(Error::Unavailable)
    ));
    let operation = server.data.lock().unwrap().operation;
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client
        .request_authorization(capabilities.clone())
        .await
        .unwrap();
    assert_eq!(server.data.lock().unwrap().operation, operation);
    assert!(matches!(
        client.request_authorization(capabilities).await,
        Err(Error::Conflict)
    ));
    let report = client
        .queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            1,
        )
        .unwrap();
    server.data.lock().unwrap().report_failure = true;
    assert_eq!(client.flush_reports(8).await, Err(Error::Unavailable));
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    server.data.lock().unwrap().bad_ack = true;
    assert_eq!(client.flush_reports(8).await, Err(Error::Protocol));
    server.data.lock().unwrap().bad_ack = false;
    assert_eq!(client.flush_reports(8).await.unwrap(), 1);
    assert_eq!(client.flush_reports(8).await.unwrap(), 0);
    let data = server.data.lock().unwrap();
    assert_eq!(data.reports.len(), 1);
    assert_eq!(data.reports[&report.to_string()]["sequence"], 0);
    let bytes = std::fs::read(root.path.join("communication.sqlite")).unwrap();
    assert!(!bytes
        .windows(43)
        .any(|b| b == b"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"));
}

#[tokio::test]
async fn revoked_identity_blocks_retries_and_clock_rollback_preserves_queue() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    client
        .queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            1,
        )
        .unwrap();
    server.time.set(2);
    server.data.lock().unwrap().denied = true;
    assert_eq!(client.flush_reports(4).await, Err(Error::CredentialRevoked));
    server.data.lock().unwrap().denied = false;
    assert_eq!(client.flush_reports(4).await, Err(Error::CredentialRevoked));
    drop(client);
    let cfg = server.config();
    server.time.set(0);
    let mut client = Client::open(
        &root.path,
        cfg,
        OpenMode::Existing,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    assert_eq!(
        client.queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed
            },
            1
        ),
        Err(Error::CredentialRevoked)
    );
}

#[tokio::test]
async fn offer_materials_start_and_input_tampering() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    assert_eq!(materials.files().len(), 1);
    assert!(matches!(
        client.request_start(&offer, &materials).await,
        Err(Error::Conflict)
    ));
    client.received(&offer).await.unwrap();
    server.data.lock().unwrap().forged_start = true;
    assert!(matches!(
        client.request_start(&offer, &materials).await,
        Err(Error::Untrusted)
    ));
    server.data.lock().unwrap().forged_start = false;
    let start = client.request_start(&offer, &materials).await.unwrap();
    server.time.set(17);
    assert_eq!(client.validate_start(&start), Err(Error::Expired));
}

#[tokio::test]
async fn signature_namespace_and_expiry_are_checked_before_content() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    server.data.lock().unwrap().script();
    {
        let mut data = server.data.lock().unwrap();
        let mut payload = data.offer.as_ref().unwrap().payload.clone();
        if let TaskPayload::Script(v) = &mut payload {
            v.tenant_id = Uuid::new_v4();
        }
        data.offer = Some(data.signed(payload));
    }
    assert!(matches!(client.claim().await, Err(Error::Untrusted)));
    server.time.set(72);
    server.data.lock().unwrap().script();
    let offer = client.claim().await.unwrap().offer.unwrap();
    server.time.set(132);
    assert!(matches!(client.prepare(&offer).await, Err(Error::Expired)));
    assert!(server.data.lock().unwrap().content_calls.is_empty());
}

#[tokio::test]
async fn cache_budget_corruption_and_complete_file_replacement_fail_before_start() {
    let server = Server::new().await;
    let root = Root::new();
    let mut cfg = server.config();
    cfg.limits.artifact_bytes = 8;
    let mut client = Client::open(
        &root.path,
        cfg,
        OpenMode::Create,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    assert!(matches!(client.prepare(&offer).await, Err(Error::Capacity)));
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    let offer = client.offer(offer.task_id()).unwrap();
    server.data.lock().unwrap().bad_etag = true;
    assert!(matches!(
        client.prepare(&offer).await,
        Err(Error::Untrusted)
    ));
    server.data.lock().unwrap().bad_etag = false;
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    std::fs::write(materials.files()[0].path(), b"changed").unwrap();
    assert!(matches!(
        client.request_start(&offer, &materials).await,
        Err(Error::Untrusted)
    ));
    assert!(!server.data.lock().unwrap().started);
}

#[tokio::test]
async fn all_software_steps_are_consumed_and_user_start_is_explicit() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    {
        let mut data = server.data.lock().unwrap();
        data.software(3, true);
    }
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    assert_eq!(materials.files().len(), 3);
    client.received(&offer).await.unwrap();
    assert!(matches!(
        client.request_start(&offer, &materials).await,
        Err(Error::Denied)
    ));
    let start = client
        .start_user_initiated(&offer, &materials)
        .await
        .unwrap();
    assert!(matches!(start.payload(),TaskPayload::Software(v) if v.steps.len()==3));
}

#[test]
fn unsupported_database_and_changed_namespace_are_preserved() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.block_on(Server::new());
    let root = Root::new();
    drop(server.client(&root, OpenMode::Create));
    let path = root.path.join("communication.sqlite");
    for version in [1, 2, 3, 4, 5, 6, 7, 9] {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.pragma_update(None, "user_version", version).unwrap();
        drop(db);
        let before = std::fs::read(&path).unwrap();
        assert!(matches!(
            Client::open(
                &root.path,
                server.config(),
                OpenMode::Existing,
                server.secrets.clone(),
                server.time.clone()
            ),
            Err(Error::Schema)
        ));
        assert_eq!(before, std::fs::read(&path).unwrap());
    }
}

#[tokio::test]
async fn prior_communication_schema_preserves_registration_pending_delivery_and_materials() {
    use execution_contract::{ActorId, Digest, EventId, RequestId};
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    let association = client
        .reserve_association(
            &start,
            &RequestId::new("pending-original").unwrap(),
            &Digest::new("ab".repeat(32)).unwrap(),
            &ActorId::new("original-actor").unwrap(),
        )
        .unwrap();
    let event = EventId::new("original-result").unwrap();
    client
        .freeze_result(
            &association,
            &event,
            std::slice::from_ref(&event),
            TaskEvent::Cancelled,
        )
        .unwrap();
    client
        .queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            1,
        )
        .unwrap();
    let material_bytes: Vec<_> = materials
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), std::fs::read(file.path()).unwrap()))
        .collect();
    let credentials = server.secrets.0.lock().unwrap().clone();
    drop(client);
    let path = root.path.join("communication.sqlite");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 6).unwrap();
    drop(db);
    // Snapshot all existing entries; rejection must not initialize a lock or sidecar.
    let entries = || {
        let mut v: Vec<_> = std::fs::read_dir(&root.path)
            .unwrap()
            .map(|v| v.unwrap().file_name())
            .collect();
        v.sort();
        v
    };
    let before_entries = entries();
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        Client::open(
            &root.path,
            server.config(),
            OpenMode::Existing,
            server.secrets.clone(),
            server.time.clone()
        ),
        Err(Error::Schema)
    ));
    assert!(matches!(
        Client::open(
            &root.path,
            server.config(),
            OpenMode::Create,
            server.secrets.clone(),
            server.time.clone()
        ),
        Err(Error::Schema)
    ));
    assert_eq!(
        agent_client::validate_existing(&root.path, &server.config()),
        Err(Error::Schema)
    );
    assert_eq!(
        agent_client::inspect_binding(&root.path, &server.config()),
        Err(Error::Schema)
    );
    assert_eq!(
        agent_client::validate_identity(
            &root.path,
            &server.config(),
            &server.secrets,
            &server.time
        ),
        Err(Error::Schema)
    );
    assert_eq!(entries(), before_entries);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    for (path, bytes) in material_bytes {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    assert_eq!(*server.secrets.0.lock().unwrap(), credentials);
}

#[test]
fn configuration_rejects_untrusted_network_and_unbounded_budgets() {
    let mut cfg = Config {
        execution_context: rss_mdm_agent_wire::runtime::SoftwareExecutionContext {
            revision: 1,
            os_version: [14, 0, 0, 0],
            system_broker: true,
            user_helper: true,
            interactive_user: None,
            source_credentials: vec![],
            msix_sideload: false,
            msix_unsigned: false,
        },
        origin: Url::parse("http://example.com").unwrap(),
        tenant: Uuid::new_v4(),
        platform: TaskPlatform::Macos,
        architecture: TaskArchitecture::Aarch64,
        registration_keys: Default::default(),
        limits: Limits::test_defaults(),
        transport: Transport::Https,
        ca_pem: None,
    };
    assert_eq!(cfg.validate(), Err(Error::Configuration));
    cfg.origin = Url::parse("http://127.0.0.1:8080").unwrap();
    cfg.transport = Transport::TestLoopback;
    cfg.registration_keys.insert("test".into(), vec![1; 32]);
    assert!(cfg.validate().is_ok());
    cfg.limits.cache_bytes = 0;
    assert_eq!(cfg.validate(), Err(Error::Configuration));
}

#[tokio::test]
async fn offer_rotation_preserves_uncertain_start_and_replays_original_request() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    server.data.lock().unwrap().start_failure = true;
    assert!(client.request_start(&offer, &materials).await.is_err());
    let original_permit = server.data.lock().unwrap().start_permit.clone();
    drop(materials);
    drop(client);
    server.time.set(61);
    let mut client = server.client(&root, OpenMode::Existing);
    assert!(matches!(client.claim().await, Err(Error::Conflict)));
    // The socket fixture answers the durable original request, even after a new Offer.
    {
        let mut data = server.data.lock().unwrap();
        data.attempt = offer.attempt_id();
        data.start_permit = original_permit;
    }
    assert!(matches!(
        client.recover_start(offer.task_id()).await,
        Err(Error::Expired)
    ));
    let ops = server.data.lock().unwrap().start_ops.clone();
    assert_eq!(ops.len(), 2);
    assert_eq!(ops[0], ops[1]);
}

#[tokio::test]
async fn controlled_factory_software_start_never_reuses_a_prior_script_permit() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let first = client.claim().await.unwrap().offer.unwrap();
    client.received(&first).await.unwrap();
    let materials = client.prepare(&first).await.unwrap();
    client.request_start(&first, &materials).await.unwrap();
    {
        let mut data = server.data.lock().unwrap();
        data.task = uuid::Uuid::new_v4();
        data.attempt = uuid::Uuid::new_v4();
        data.software(1, false);
    }
    let second = client.claim().await.unwrap().offer.unwrap();
    client.received(&second).await.unwrap();
    let materials = client.prepare(&second).await.unwrap();
    let started = client.request_start(&second, &materials).await.unwrap();
    assert!(matches!(started.payload(), TaskPayload::Software(_)));
    assert_eq!(started.payload().attempt_id(), second.attempt_id());
}

#[tokio::test]
async fn durable_claim_does_not_wait_for_an_unresolved_execution_to_release() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let first = client.claim().await.unwrap().offer.unwrap();
    client.received(&first).await.unwrap();
    let materials = client.prepare(&first).await.unwrap();
    client.request_start(&first, &materials).await.unwrap();
    // Keep this original started task and its material references pending. A completed claim
    // is independent of journal termination, including Unknown physical outcomes.
    {
        let mut data = server.data.lock().unwrap();
        data.task = uuid::Uuid::new_v4();
        data.script();
    }
    let second = client.claim().await.unwrap().offer.unwrap();
    assert_ne!(first.task_id(), second.task_id());
    let pending = client.pending_tasks(4).unwrap();
    assert!(pending.contains(&first.task_id()));
    assert!(pending.contains(&second.task_id()));
    let operations = server.data.lock().unwrap().claim_ops.clone();
    assert_ne!(operations[0], operations[1]);
}

#[tokio::test]
async fn controlled_backend_issues_only_explicit_offers_bound_to_the_actual_claim_context() {
    let server = Server::new().await;
    server.data.lock().unwrap().explicit_offers = true;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    assert!(client.claim().await.unwrap().offer.is_none());
    let mut context = client.execution_context().unwrap();
    context.os_version = [26, 4, 0, 0];
    client.set_execution_context(context).unwrap();
    server.data.lock().unwrap().software(1, true);
    let offered = client.claim().await.unwrap().offer.unwrap();
    let TaskPayload::Software(spec) = offered.payload() else {
        panic!("software")
    };
    assert_eq!(spec.execution_context, client.execution_context().unwrap());
    server.time.set(spec.expires_at);
    assert!(client.claim().await.unwrap().offer.is_none());
    assert!(server.data.lock().unwrap().start_ops.is_empty());
}

#[tokio::test]
async fn controlled_backend_keeps_an_issued_software_offer_frozen_across_context_changes() {
    let server = Server::new().await;
    server.data.lock().unwrap().explicit_offers = true;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    server.data.lock().unwrap().software(1, true);
    let original = client.claim().await.unwrap().offer.unwrap();
    client.received(&original).await.unwrap();
    let mut observed = client.execution_context().unwrap();
    observed.os_version = [26, 4, 0, 0];
    client.set_execution_context(observed).unwrap();
    let repeated = client.claim().await.unwrap().offer.unwrap();
    assert_eq!(repeated.payload(), original.payload());
    let materials = client.prepare(&original).await.unwrap();
    let started = client
        .start_user_initiated(&original, &materials)
        .await
        .unwrap();
    assert!(matches!(started.payload(), TaskPayload::Software(_)));
    assert_eq!(started.payload().attempt_id(), original.attempt_id());
}

#[tokio::test]
async fn claim_retry_freezes_executor_profiles_and_context_across_restart() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    client
        .set_profiles(vec![ExecutorProfile::PosixSh, ExecutorProfile::Osquery])
        .unwrap();
    let environment = rss_mdm_agent_wire::CollectionEnvironment {
        native_engines: Default::default(),
        edition: Some("Professional".into()),
        tools: [("powershell".into(), "7.4.13".into())].into(),
    };
    client
        .set_collection_environment(environment.clone())
        .unwrap();
    server.data.lock().unwrap().claim_failure = true;
    assert!(matches!(client.claim().await, Err(Error::Unavailable)));
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.set_profiles(vec![]).unwrap();
    client
        .set_collection_environment(Default::default())
        .unwrap();
    let mut context = client.execution_context().unwrap();
    context.os_version = [15, 1, 0, 0];
    client.set_execution_context(context).unwrap();
    assert_eq!(client.execution_context().unwrap().revision, 2);
    client.claim().await.unwrap();
    let inputs = server.data.lock().unwrap().claim_inputs.clone();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0], inputs[1]);
    assert_eq!(
        inputs[0]["environment"],
        serde_json::to_value(environment).unwrap()
    );
    assert_eq!(
        inputs[0]["profiles"],
        serde_json::json!(["posix_sh", "osquery"])
    );
    client.claim().await.unwrap();
    let inputs = server.data.lock().unwrap().claim_inputs.clone();
    assert_ne!(inputs[1]["operationId"], inputs[2]["operationId"]);
    assert_eq!(
        inputs[2]["environment"],
        serde_json::json!({"edition":null,"tools":{},"nativeEngines":{}})
    );
}

#[tokio::test]
async fn obsolete_claim_is_rejected_without_resetting_identity_or_sending_a_new_request() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    drop(client);
    let old = serde_json::to_vec(&serde_json::json!({
        "operation":Uuid::new_v4(), "profiles":[], "recover_until":1000,
        "execution_context":server.config().execution_context,
    }))
    .unwrap();
    let db = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    db.execute("INSERT INTO state(key,body) VALUES('claim',?1)", [&old])
        .unwrap();
    drop(db);
    let mut client = server.client(&root, OpenMode::Existing);
    assert!(client.claim().await.is_err());
    assert!(server.data.lock().unwrap().claim_inputs.is_empty());
    assert!(client.registration().is_ok());
    drop(client);
    let db = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    let retained: Vec<u8> = db
        .query_row("SELECT body FROM state WHERE key='claim'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(retained, old);
}

#[tokio::test]
async fn review_regression_registration_context_survives_restart_before_first_poll() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    drop(client);
    let mut config = server.config();
    config.execution_context.os_version = [15, 1, 0, 0];
    let observed = config.execution_context.clone();
    let mut client = Client::open(
        &root.path,
        config,
        OpenMode::Existing,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    assert_eq!(
        client.execution_context().unwrap().os_version,
        [14, 0, 0, 0]
    );
    client.set_execution_context(observed).unwrap();
    client.claim().await.unwrap();
    let inputs = server.data.lock().unwrap().claim_inputs.clone();
    assert_eq!(inputs[0]["executionContext"]["revision"], 2);
    assert_eq!(
        inputs[0]["executionContext"]["osVersion"],
        serde_json::json!([15, 1, 0, 0])
    );
}

#[tokio::test]
async fn activation_response_loss_recovers_original_candidate_over_real_tls() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.data.lock().unwrap().activation_failure = true;
    assert!(matches!(
        client
            .request_authorization(vec![
                Capability::InventoryCollectionV1,
                Capability::TaskExecuteV1
            ])
            .await,
        Err(Error::ActivationUnknown)
    ));
    let secrets = server.secrets.0.lock().unwrap().clone();
    let receipt = server.data.lock().unwrap().activation.clone().unwrap();
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_enrollment().await.unwrap();
    assert_eq!(client.registration().unwrap(), receipt.runtime);
    assert_eq!(server.secrets.0.lock().unwrap().clone(), secrets);
    let data = server.data.lock().unwrap();
    assert_eq!(data.certificate_calls.len(), 1);
    assert_eq!(data.activation_calls.len(), 1);
}
#[tokio::test]
async fn certificate_response_loss_replays_exact_frozen_request_and_key() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.data.lock().unwrap().certificate_failure = true;
    assert!(matches!(
        client
            .request_authorization(vec![
                Capability::InventoryCollectionV1,
                Capability::TaskExecuteV1
            ])
            .await,
        Err(Error::Unavailable)
    ));
    let secrets = server.secrets.0.lock().unwrap().clone();
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.drive_enrollment().await.unwrap();
    assert_eq!(server.secrets.0.lock().unwrap().clone(), secrets);
    let data = server.data.lock().unwrap();
    assert_eq!(data.certificate_calls.len(), 2);
    assert_eq!(data.certificate_calls[0], data.certificate_calls[1]);
}
#[tokio::test]
async fn unknown_issuance_never_submits_another_certificate_operation() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.data.lock().unwrap().issuance_unknown = true;
    let caps = vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1];
    assert!(matches!(
        client.request_authorization(caps.clone()).await,
        Err(Error::IssuanceUnknown)
    ));
    let secrets = server.secrets.0.lock().unwrap().clone();
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    assert_eq!(client.drive_enrollment().await, Err(Error::IssuanceUnknown));
    assert!(matches!(
        client.request_authorization(caps).await,
        Err(Error::IssuanceUnknown)
    ));
    assert_eq!(server.data.lock().unwrap().certificate_calls.len(), 1);
    assert_eq!(server.secrets.0.lock().unwrap().clone(), secrets);
}

#[tokio::test]
async fn manual_renewal_keeps_binding_tasks_reports_and_original_recovery_operations() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let old_leaf = server.data.lock().unwrap().candidate_leaf.clone().unwrap();
    let binding = client.registration().unwrap();
    let offer = client.claim().await.unwrap().offer.unwrap();
    let task = offer.task_id();
    client
        .queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            1,
        )
        .unwrap();
    server.data.lock().unwrap().activation_failure = true;
    assert_eq!(
        client.renew_certificate().await.unwrap_err(),
        Error::ActivationUnknown
    );
    server.data.lock().unwrap().denied = true;
    assert_eq!(
        client.flush_reports(1).await.unwrap_err(),
        Error::CredentialRevoked
    );
    let status = client.enrollment_status().unwrap();
    assert_eq!(
        status.failure,
        Some(rss_mdm_agent_wire::enroll::ErrorCode::CredentialRevoked)
    );
    assert_eq!(
        status.phase,
        agent_client::EnrollmentPhase::ActivationUnknown
    );
    server.data.lock().unwrap().denied = false;
    let secrets = server.secrets.0.lock().unwrap().clone();
    let (candidate, operation) = {
        let data = server.data.lock().unwrap();
        assert_ne!(
            data.certificate_calls[0]["operationId"],
            data.certificate_calls[1]["operationId"]
        );
        assert_ne!(
            data.certificate_calls[0]["csr"],
            data.certificate_calls[1]["csr"]
        );
        (data.issuance, data.activation.clone().unwrap().operation_id)
    };
    // Use a real TLS connection authenticated by the former active certificate.
    // A same-CA peer must still match the exact candidate being activated.
    use der::{DecodePem, Encode};
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(
            x509_cert::Certificate::from_pem(support::pki::ca())
                .unwrap()
                .to_der()
                .unwrap(),
        ))
        .unwrap();
    let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_client_auth_cert(
        vec![rustls::pki_types::CertificateDer::from(old_leaf)],
        rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
            include_bytes!("../../../tests/agent-protocol/pki/device-test-key.pk8").to_vec(),
        )),
    )
    .unwrap();
    let old_client = reqwest::Client::builder()
        .no_proxy()
        .tls_backend_preconfigured(tls)
        .build()
        .unwrap();
    let rejected = old_client
        .post(server.url.join("api/v1/agent/enroll/activate").unwrap())
        .json(&rss_mdm_agent_wire::enroll::ActivationRequest {
            wire_version: 1,
            issuance_id: candidate,
            operation_id: operation,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        rejected
            .json::<rss_mdm_agent_wire::enroll::ErrorBody>()
            .await
            .unwrap()
            .code,
        rss_mdm_agent_wire::enroll::ErrorCode::InvalidIdentity
    );
    assert_eq!(server.data.lock().unwrap().activation_calls.len(), 2);
    assert_eq!(server.data.lock().unwrap().binding.as_ref(), Some(&binding));
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.renew_certificate().await.unwrap();
    assert_eq!(client.registration().unwrap(), binding);
    assert_eq!(client.pending_tasks(8).unwrap(), vec![task]);
    assert_eq!(client.flush_reports(1).await.unwrap(), 1);
    assert_eq!(server.secrets.0.lock().unwrap().clone(), secrets);
    let data = server.data.lock().unwrap();
    assert_eq!(data.certificate_calls.len(), 2);
    assert_eq!(data.activation_calls.len(), 2);
    assert_eq!(data.activation.as_ref().unwrap().issuance_id, candidate);
    assert_eq!(data.activation.as_ref().unwrap().operation_id, operation);
}
#[tokio::test]
async fn readonly_identity_preflight_shares_runtime_rules_and_preserves_state() {
    use rss_mdm_agent_wire::enroll::ErrorCode;
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let cfg = client.configuration().clone();
    let binding = client.registration().unwrap();
    let validate =
        || agent_client::validate_identity(&root.path, &cfg, &server.secrets, &server.time);
    assert_eq!(validate().unwrap(), binding); // Validates while the original owner holds its lease.
    let secrets = server.secrets.0.lock().unwrap().clone();
    let stored = || agent_client::validate_stored_identity(&root.path, &cfg, &server.secrets);
    let database = std::fs::read(root.path.join("communication.sqlite")).unwrap();
    assert_eq!(stored().unwrap(), Some(binding.clone()));
    let reader = rusqlite::Connection::open_with_flags(
        root.path.join("communication.sqlite"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let body: Vec<u8> = reader
        .query_row("SELECT body FROM state WHERE key='identity'", [], |r| {
            r.get(0)
        })
        .unwrap();
    drop(reader);
    let identity: serde_json::Value = serde_json::from_slice(&body).unwrap();
    for field in ["key_reference", "secret_reference"] {
        let reference = identity["active"][field].as_str().unwrap();
        server.secrets.0.lock().unwrap().remove(reference);
        assert!(
            stored().is_err(),
            "retained credential must remain readable"
        );
        *server.secrets.0.lock().unwrap() = secrets.clone();
    }
    assert_eq!(
        std::fs::read(root.path.join("communication.sqlite")).unwrap(),
        database
    );
    for value in server.secrets.0.lock().unwrap().values_mut() {
        if value == "fixture-key" {
            *value = "fixture-renewal-key".into();
        }
    }
    assert!(matches!(validate(), Err(Error::Identity)));
    *server.secrets.0.lock().unwrap() = secrets.clone();
    server
        .secrets
        .0
        .lock()
        .unwrap()
        .retain(|_, value| value != "fixture-key");
    assert_eq!(validate().unwrap_err(), Error::Identity);
    *server.secrets.0.lock().unwrap() = secrets.clone();
    server.time.set(4_070_908_800);
    assert_eq!(validate().unwrap_err(), Error::CertificateExpired);
    assert_eq!(stored().unwrap(), Some(binding.clone()));
    server.time.set(1_700_000_000);
    drop(client);
    let path = root.path.join("communication.sqlite");
    for (code, expected) in [
        (ErrorCode::CredentialRevoked, Error::CredentialRevoked),
        (ErrorCode::CredentialRetired, Error::CredentialRetired),
        (ErrorCode::CertificateExpired, Error::CertificateExpired),
    ] {
        let db = rusqlite::Connection::open(&path).unwrap();
        let bytes: Vec<u8> = db
            .query_row("SELECT body FROM state WHERE key='identity'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let mut state: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        state["failure"] = serde_json::to_value(code).unwrap();
        db.execute(
            "UPDATE state SET body=?1 WHERE key='identity'",
            [serde_json::to_vec(&state).unwrap()],
        )
        .unwrap();
        drop(db);
        let before = std::fs::read(&path).unwrap();
        assert_eq!(validate().unwrap_err(), expected);
        assert_eq!(
            agent_client::inspect_binding(&root.path, &cfg).unwrap(),
            binding
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(*server.secrets.0.lock().unwrap(), secrets);
    }
}

#[tokio::test]
async fn obsolete_wire_and_signing_domain_never_accept_a_task() {
    use base64::Engine;
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    server.data.lock().unwrap().script();
    let signed = server.data.lock().unwrap().offer.clone().unwrap();
    let response =
        serde_json::to_value(TaskClaimResponse::new(Some(signed.clone()), vec![]).unwrap())
            .unwrap();
    let mut obsolete_version = response.clone();
    obsolete_version["wireVersion"] = 6.into();
    let mut obsolete_task = response.clone();
    obsolete_task["task"]["payload"]["wireVersion"] = 6.into();
    let mut old_domain = signed.payload.signing_bytes("test").unwrap();
    // Only mutate the historical domain separator: the payload stays valid V1.
    let version = old_domain.windows(3).position(|v| v == b"-v1").unwrap() + 2;
    old_domain[version] = b'6';
    let mut obsolete_signature = signed.clone();
    obsolete_signature.signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        server
            .data
            .lock()
            .unwrap()
            .signer
            .sign(&old_domain)
            .as_ref(),
    );
    let obsolete_signature =
        serde_json::to_value(TaskClaimResponse::new(Some(obsolete_signature), vec![]).unwrap())
            .unwrap();
    for (body, expected) in [
        (obsolete_version, Error::Protocol),
        (obsolete_task, Error::Protocol),
        (obsolete_signature, Error::Untrusted),
    ] {
        server.data.lock().unwrap().response_override = Some(("/claim".into(), body));
        assert!(matches!(client.claim().await, Err(error) if error == expected));
        assert!(client.pending_tasks(8).unwrap().is_empty());
        let data = server.data.lock().unwrap();
        assert!(data.content_calls.is_empty());
        assert!(data.start_ops.is_empty());
    }
    server.data.lock().unwrap().response_override = None;
    assert!(client.claim().await.unwrap().offer.is_some());
}

#[tokio::test]
async fn obsolete_ack_preserves_the_original_report_for_retry() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let report = client
        .queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            1,
        )
        .unwrap();
    server.data.lock().unwrap().response_override = Some((
        "/reports".into(),
        serde_json::json!({
            "wireVersion":6,"reportId":report,"receivedAt":1,"intake":"durable"
        }),
    ));
    assert_eq!(client.flush_reports(1).await, Err(Error::Protocol));
    drop(client);
    server.data.lock().unwrap().response_override = None;
    let mut client = server.client(&root, OpenMode::Existing);
    assert_eq!(client.flush_reports(1).await.unwrap(), 1);
    assert_eq!(server.data.lock().unwrap().reports.len(), 1);
    assert!(server
        .data
        .lock()
        .unwrap()
        .reports
        .contains_key(&report.to_string()));
}

#[tokio::test]
async fn obsolete_activation_capability_preserves_active_binding_and_candidate() {
    use rss_mdm_agent_wire::enroll;
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let binding = client.registration().unwrap();
    server.data.lock().unwrap().activation_failure = true;
    assert_eq!(
        client.renew_certificate().await.unwrap_err(),
        Error::ActivationUnknown
    );
    let (mut response, calls) = {
        let data = server.data.lock().unwrap();
        (
            serde_json::to_value(enroll::Progress {
                grant: data.grant.clone().unwrap(),
                issuance_id: Some(data.issuance),
                issuance_unknown: false,
                activation: data.activation.clone(),
            })
            .unwrap(),
            data.activation_calls.len(),
        )
    };
    response["activation"]["runtime"]["capabilities"][0] = "inventory.collection.v6".into();
    server.data.lock().unwrap().response_override = Some(("/status".into(), response));
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    let identity = || {
        rusqlite::Connection::open(root.path.join("communication.sqlite"))
            .unwrap()
            .query_row("SELECT body FROM state WHERE key='identity'", [], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .unwrap()
    };
    let before = identity();
    assert_eq!(client.drive_enrollment().await, Err(Error::Protocol));
    assert_eq!(identity(), before);
    assert_eq!(client.registration().unwrap(), binding);
    assert!(client.has_pending_enrollment().unwrap());
    server.data.lock().unwrap().response_override = None;
    client.drive_enrollment().await.unwrap();
    assert_eq!(client.registration().unwrap(), binding);
    assert_eq!(server.data.lock().unwrap().activation_calls.len(), calls);
}

#[tokio::test]
async fn imports_signed_configuration_over_real_tls_without_replacing_the_operation() {
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
    client.drive_enrollment().await.unwrap();
    assert!(client.enrollment_status().unwrap().registered);
    let data = server.data.lock().unwrap();
    assert_eq!(data.configuration_calls.len(), 1);
    assert_eq!(data.certificate_calls.len(), 1);
    assert_eq!(data.activation_calls.len(), 1);
}

#[tokio::test]
async fn device_self_service_uses_the_registered_channel_without_a_user_header() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let page = client
        .self_service_directory(
            None,
            &SelfServiceDirectory {
                source: SelfServiceCaller::Ai,
                after: None,
            },
        )
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(server.data.lock().unwrap().employee_calls.len(), 1);
    // A rejected user request never produces a second, device-only request.
    assert!(client
        .self_service_directory(
            Some(&"d".repeat(64)),
            &SelfServiceDirectory {
                source: SelfServiceCaller::Ai,
                after: None,
            }
        )
        .await
        .is_err());
    assert_eq!(server.data.lock().unwrap().employee_calls.len(), 1);
}

#[tokio::test]
async fn employee_self_service_uses_device_mtls_and_recovers_the_original_uuid() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let directory = policy_directory_fixture(server.data.lock().unwrap().tenant);
    server.data.lock().unwrap().self_service_directory = Some(SelfServiceDirectoryResponse {
        items: vec![directory.clone()],
        next_cursor: None,
        as_of: 1,
    });
    let secret = "c".repeat(64);
    let query = SelfServiceDirectory {
        source: SelfServiceCaller::Ai,
        after: None,
    };
    assert!(matches!(
        client
            .self_service_directory(Some("not-an-identity-cookie"), &query)
            .await,
        Err(Error::Identity)
    ));
    assert!(client
        .self_service_directory(Some(&"d".repeat(64)), &query)
        .await
        .is_err());
    assert_eq!(
        client
            .self_service_directory(Some(&secret), &query)
            .await
            .unwrap()
            .items[0]
            .policy_revision,
        2
    );
    let input = SelfServiceInvoke {
        operation_id: Uuid::new_v4(),
        policy_id: directory.policy_id,
        policy_revision: directory.policy_revision,
        policy_version: directory.policy_version,
        source: SelfServiceCaller::Ai,
        confirmed: true,
        parameters: std::collections::BTreeMap::from([("name".into(), serde_json::json!("Acme"))]),
    };
    server.data.lock().unwrap().invocation_reply_lost = true;
    assert!(matches!(
        client.invoke_self_service(Some(&secret), &input).await,
        Err(Error::Unavailable)
    ));
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    let status = client
        .self_service_status(Some(&secret), input.operation_id)
        .await
        .unwrap();
    let retry = client
        .invoke_self_service(Some(&secret), &input)
        .await
        .unwrap();
    assert_eq!(status.run_id, retry.run_id);
    assert_eq!(server.data.lock().unwrap().invocations.len(), 1);
    assert!(matches!(
        client
            .self_service_status(Some(&secret), Uuid::new_v4())
            .await,
        Err(Error::NotFound)
    ));
    server.data.lock().unwrap().response_override = Some((
        "invocations".into(),
        serde_json::json!({"requestId":Uuid::new_v4(),"runId":retry.run_id,"policyId":input.policy_id,"policyRevision":input.policy_revision,"policyVersion":input.policy_version}),
    ));
    assert!(matches!(
        client.invoke_self_service(Some(&secret), &input).await,
        Err(Error::Untrusted)
    ));
    let data = server.data.lock().unwrap();
    assert!(data
        .employee_calls
        .iter()
        .any(|(uri, _)| uri.ends_with("self-service?source=ai")));
    assert!(data
        .employee_calls
        .iter()
        .filter(|(uri, _)| uri.ends_with("invocations"))
        .all(|(_, body)| body["operationId"] == input.operation_id.to_string()));
    assert!(!serde_json::to_string(&data.employee_calls)
        .unwrap()
        .contains(&secret));
}

#[tokio::test]
async fn paused_activation_preserves_original_key_candidate_and_expiry_across_restart() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.data.lock().unwrap().enrollment_paused = true;
    assert_eq!(
        client
            .request_authorization(vec![
                Capability::InventoryCollectionV1,
                Capability::TaskExecuteV1
            ])
            .await
            .unwrap_err(),
        Error::EnrollmentPaused
    );
    let status = client.enrollment_status().unwrap();
    assert_eq!(status.phase, agent_client::EnrollmentPhase::Paused);
    assert!(!status.registered);
    let secrets = server.secrets.0.lock().unwrap().clone();
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    assert_eq!(
        client.enrollment_status().unwrap().expires_at,
        status.expires_at
    );
    assert_eq!(
        client.drive_enrollment().await,
        Err(Error::EnrollmentPaused)
    );
    server.data.lock().unwrap().enrollment_paused = false;
    client.drive_enrollment().await.unwrap();
    assert_eq!(
        client.enrollment_status().unwrap().phase,
        agent_client::EnrollmentPhase::Active
    );
    assert_eq!(server.secrets.0.lock().unwrap().clone(), secrets);
    let data = server.data.lock().unwrap();
    assert_eq!(data.certificate_calls.len(), 1);
    assert_eq!(data.activation_calls.len(), 3);
    assert!(data
        .activation_calls
        .iter()
        .all(|call| call == &data.activation_calls[0]));
}

#[tokio::test]
async fn native_start_rejects_a_changed_execution_context() {
    let server = Server::new().await;
    server.data.lock().unwrap().native_collection();
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    client.received(&offer).await.unwrap();
    let material = client.prepare(&offer).await.unwrap();
    let mut context = client.execution_context().unwrap();
    context.system_broker = false;
    client.set_execution_context(context).unwrap();
    assert!(matches!(
        client.request_start(&offer, &material).await,
        Err(Error::Untrusted)
    ));
    assert!(server.data.lock().unwrap().start_ops.is_empty());
}

#[tokio::test]
async fn native_handoff_lost_response_reopens_same_operation_and_secret() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    server.data.lock().unwrap().native_enrollment();
    let offer = client.claim().await.unwrap().offer.unwrap();
    client.received(&offer).await.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    server.data.lock().unwrap().native_failure = true;
    assert!(matches!(
        client.native_enrollment(&start).await,
        Err(Error::Unavailable)
    ));
    drop(materials);
    drop(offer);
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    let material = client.native_enrollment(&start).await.unwrap();
    let data = server.data.lock().unwrap();
    assert_eq!(data.native_inputs.len(), 2);
    assert_eq!(data.native_inputs[0], data.native_inputs[1]);
    assert_eq!(
        material.receipt.enrollment_id,
        data.native_receipt.as_ref().unwrap().enrollment_id
    );
    let storage = std::fs::read(root.path.join("communication.sqlite")).unwrap();
    assert!(!storage
        .windows(material.password.expose().len())
        .any(|w| w == material.password.expose().as_bytes()));
}

#[tokio::test]
async fn managed_identity_arrives_later_and_grant_response_loss_preserves_daily_key() {
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let capabilities = vec![
        Capability::InventoryCollectionV1,
        Capability::TaskExecuteV1,
        Capability::MdmEnrollmentV1,
    ];
    assert!(!client
        .prepare_managed_enrollment(capabilities.clone())
        .unwrap());
    assert!(!client.has_pending_enrollment().unwrap());
    let installation = Uuid::new_v4();
    let bootstrap = Uuid::new_v4();
    {
        let mut keys = server.secrets.0.lock().unwrap();
        keys.insert("managed-installation".into(), installation.to_string());
        keys.insert("managed-attempt".into(), bootstrap.to_string());
    }
    {
        let mut data = server.data.lock().unwrap();
        data.managed_installation = Some(installation);
        data.registration_failure = true;
    }
    assert!(client
        .prepare_managed_enrollment(capabilities.clone())
        .unwrap());
    let fingerprint = client.enrollment_status().unwrap().public_key_fingerprint;
    assert!(matches!(
        client.drive_enrollment().await,
        Err(Error::Unavailable)
    ));
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    assert!(client
        .prepare_managed_enrollment(capabilities.clone())
        .unwrap());
    assert_eq!(
        client.enrollment_status().unwrap().public_key_fingerprint,
        fingerprint
    );
    client.drive_enrollment().await.unwrap();
    assert!(client.enrollment_status().unwrap().registered);
    assert!(!client.prepare_managed_enrollment(capabilities).unwrap());
    let data = server.data.lock().unwrap();
    assert_eq!(data.native_inputs.len(), 2);
    assert_eq!(data.native_inputs[0], data.native_inputs[1]);
    assert_eq!(data.certificate_calls.len(), 1);
}

#[tokio::test]
async fn signed_retirement_requires_start_drain_and_exact_installation_and_replays_lost_result() {
    let server = Server::tls().await;
    server.data.lock().unwrap().unmanage();
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let install = Uuid::new_v4();
    let offer = client.claim().await.unwrap().offer.unwrap();
    let cfg = client.configuration().clone();
    assert!(agent_client::inspect_unmanage(&root.path, &cfg, install, offer.task_id()).is_err());
    server.data.lock().unwrap().start_failure = true;
    assert!(matches!(
        client.authorize_unmanage(&offer, install).await,
        Err(Error::Unavailable)
    ));
    assert!(client.accepted_unmanage(install).unwrap().is_none());
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    let permit = client.authorize_unmanage(&offer, install).await.unwrap();
    assert!(!permit.drained());
    assert!(client.unmanage_transport(&permit).is_err());
    assert!(client.accepted_unmanage(Uuid::new_v4()).is_err());
    assert!(agent_client::inspect_unmanage(&root.path, &cfg, install, offer.task_id()).is_err());
    assert!(matches!(
        client.claim().await,
        Err(Error::CredentialRetired)
    ));
    let permit = client.unmanage_drained(permit).unwrap();
    assert!(
        agent_client::inspect_unmanage(&root.path, &cfg, install, offer.task_id())
            .unwrap()
            .drained()
    );
    let transport = client.unmanage_transport(&permit).unwrap();
    server.data.lock().unwrap().result_failure = true;
    assert_eq!(
        transport.report(UnmanageOutcome::Unknown).await,
        Err(Error::Unavailable)
    );
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    let permit = client.accepted_unmanage(install).unwrap().unwrap();
    permit.verify_handoff(&cfg, install).unwrap();
    assert!(permit.verify_handoff(&cfg, Uuid::new_v4()).is_err());
    let permit = client.accepted_unmanage(install).unwrap().unwrap();
    let transport = client.unmanage_transport(&permit).unwrap();
    drop(client);
    std::fs::remove_dir_all(&root.path).unwrap();
    transport.report(UnmanageOutcome::Unknown).await.unwrap();
    let data = server.data.lock().unwrap();
    assert_eq!(data.start_ops.len(), 2);
    assert_eq!(data.start_ops[0], data.start_ops[1]);
    assert_eq!(data.results.len(), 1);
    assert_eq!(data.result_calls, 2);
}

#[tokio::test]
async fn retirement_signature_binds_every_target_and_cannot_use_an_offer_as_start() {
    use agent_client::protocol::{verify_task, TaskVerification};
    let server = Server::new().await;
    let cfg = server.config();
    let mut data = server.data.lock().unwrap();
    data.unmanage();
    let signed = data.offer.clone().unwrap();
    let context = TaskVerification {
        key_id: "test",
        public_key: cfg.registration_keys.get("test").unwrap(),
        tenant_id: data.tenant,
        device_id: "device-1",
        platform: TaskPlatform::Windows,
        architecture: TaskArchitecture::Aarch64,
        registration_id: data.registration,
        generation: 1,
        task_id: data.task,
        attempt_id: data.attempt,
        permit: TaskPermit::Offer,
        now: 1,
    };
    verify_task(&signed, &context).unwrap();
    let wrong = TaskVerification {
        permit: TaskPermit::Start,
        ..context
    };
    assert!(verify_task(&signed, &wrong).is_err());
    for field in [
        "retirementId",
        "registrationId",
        "taskId",
        "attemptId",
        "generation",
        "deadline",
        "tenantId",
    ] {
        let mut payload = serde_json::to_value(&signed.payload).unwrap();
        let changed = if field == "generation" || field == "deadline" {
            serde_json::json!(999)
        } else {
            serde_json::json!(Uuid::new_v4())
        };
        // Use the unique wire variant's serialized envelope; no parallel protocol DTO.
        payload["task"][field] = changed;
        if let Ok(payload) = serde_json::from_value(payload) {
            let tampered = SignedTask {
                payload,
                ..signed.clone()
            };
            let offer_context = TaskVerification {
                permit: TaskPermit::Offer,
                ..wrong
            };
            assert!(verify_task(&tampered, &offer_context).is_err(), "{field}");
        } else {
            panic!("fixture must remain structurally valid for {field}");
        }
    }
}

#[tokio::test]
async fn credential_escrow_lost_ack_replays_ciphertext_after_sqlite_reopen() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use execution_contract::{ActorId, Digest, RequestId};
    let server = Server::tls().await;
    {
        let mut data = server.data.lock().unwrap();
        let mut task = CredentialTaskSpec {
            wire_version: 1,
            tenant_id: data.tenant,
            device_id: "device-1".into(),
            platform: TaskPlatform::Windows,
            architecture: TaskArchitecture::Aarch64,
            registration_id: data.registration,
            generation: 1,
            task_id: data.task,
            attempt_id: data.attempt,
            permit: TaskPermit::Offer,
            expires_at: 60,
            deadline: 300,
            authorization: TaskAuthorization::Policy {
                policy_id: Uuid::new_v4(),
                policy_version: Uuid::new_v4(),
            },
            operation: CredentialOperation::RotateLocalAdministrator {
                sid: "S-1-5-21-1-2-3-1001".into(),
            },
            recipient: CredentialRecipient {
                key_id: "a".repeat(64),
                public_key: STANDARD.encode([1; 32]),
            },
            expected: vec![],
            recovery: None,
            definition_digest: [0; 32],
        };
        task.definition_digest = CredentialTaskSpec::digest(
            &task.operation,
            &task.recipient,
            &task.expected,
            &task.recovery,
        )
        .unwrap();
        data.offer = Some(data.signed(TaskPayload::Credential(Box::new(task))));
    }
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    client.received(&offer).await.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    assert!(materials.files().is_empty());
    let start = client.request_start(&offer, &materials).await.unwrap();
    client
        .reserve_association(
            &start,
            &RequestId::new("credential-request").unwrap(),
            &Digest::new("a".repeat(64)).unwrap(),
            &ActorId::new("system-service").unwrap(),
        )
        .unwrap();
    let TaskPayload::Credential(spec) = start.payload() else {
        unreachable!()
    };
    let escrow = CredentialEscrow {
        definition_digest: spec.definition_digest,
        materials: vec![CredentialEnvelope {
            subject: CredentialSubject::LocalAdministrator {
                sid: "S-1-5-21-1-2-3-1001".into(),
            },
            expected_revision: 0,
            wrapped_key: STANDARD.encode([7; 256]),
            nonce: [9; 12],
            ciphertext: STANDARD.encode([3; 32]),
        }],
    };
    let task = offer.task_id();
    server.data.lock().unwrap().escrow_reply_lost = true;
    assert!(client
        .escrow_credentials(task, escrow.clone())
        .await
        .is_err());
    drop(materials);
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client
        .escrow_credentials(task, escrow.clone())
        .await
        .unwrap();
    assert!(client.association(task).unwrap().is_some());
    client
        .escrow_credentials(task, escrow.clone())
        .await
        .unwrap();
    {
        let data = server.data.lock().unwrap();
        assert_eq!(data.credential_escrows.len(), 2);
        assert_eq!(data.credential_escrows[0], data.credential_escrows[1]);
    }
    let mut tampered = escrow;
    tampered.materials[0].ciphertext = STANDARD.encode([4; 32]);
    assert_eq!(
        client.escrow_credentials(task, tampered).await,
        Err(Error::Conflict)
    );
}
