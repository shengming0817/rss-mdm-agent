use agent_client::{wire::*, Client, Config, Error, Limits, OpenMode, Transport};
use url::Url;
use uuid::Uuid;
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use agent_client::wire::{TaskArchitecture, TaskPlatform};
use support::*;
#[tokio::test]
async fn refresh_inspection_reads_live_binding_without_locking_or_creating_state() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let cfg = client.configuration().clone();
    let before = std::fs::read(root.path.join("communication.sqlite")).unwrap();
    let registration = agent_client::inspect_registration(&root.path, &cfg).unwrap();
    assert_eq!(
        registration.registration_id,
        client.registration().unwrap().registration_id
    );
    let mut wrong = cfg.clone();
    wrong.tenant = Uuid::new_v4();
    assert!(matches!(
        agent_client::inspect_registration(&root.path, &wrong),
        Err(Error::Identity)
    ));
    assert_eq!(
        before,
        std::fs::read(root.path.join("communication.sqlite")).unwrap()
    );
    let empty = Root::new();
    assert!(agent_client::inspect_registration(&empty.path, &cfg).is_err());
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
    assert_eq!(client.flush_reports(1).await, Err(Error::Identity));
    assert_eq!(client.validate_start(&start), Err(Error::Identity));
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
    let operation = Uuid::new_v4();
    let enrollment = Uuid::new_v4();
    let capabilities = vec![Capability::InventoryCollectionV5, Capability::TaskExecuteV5];
    assert_eq!(
        client
            .register(
                operation,
                enrollment,
                "password",
                "credential",
                capabilities.clone()
            )
            .await
            .unwrap_err(),
        Error::Unavailable
    );
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client
        .register(
            operation,
            enrollment,
            "password",
            "credential",
            capabilities.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        client
            .register(
                Uuid::new_v4(),
                enrollment,
                "password",
                "credential",
                capabilities
            )
            .await
            .unwrap_err(),
        Error::Conflict
    );
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
    assert_eq!(client.flush_reports(4).await, Err(Error::Identity));
    server.data.lock().unwrap().denied = false;
    assert_eq!(client.flush_reports(4).await, Err(Error::Identity));
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
        Err(Error::Identity)
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
    for version in [1, 2] {
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

#[test]
fn configuration_rejects_untrusted_network_and_unbounded_budgets() {
    let mut cfg = Config {
        execution_context: agent_client::wire::SoftwareExecutionContext {
            revision: 1,
            os_version: [14, 0, 0, 0],
            system_broker: true,
            interactive_user: None,
            source_credentials: vec![],
            msix_sideload: false,
            msix_unsigned: false,
        },
        origin: Url::parse("http://example.com").unwrap(),
        tenant: Uuid::new_v4(),
        platform: TaskPlatform::Macos,
        architecture: TaskArchitecture::Aarch64,
        keys: Default::default(),
        limits: Limits::test_defaults(),
        transport: Transport::Https,
        ca_pem: None,
    };
    assert_eq!(cfg.validate(), Err(Error::Configuration));
    cfg.origin = Url::parse("http://127.0.0.1:8080").unwrap();
    cfg.transport = Transport::TestLoopback;
    cfg.keys.insert("test".into(), vec![1; 32]);
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
    server.data.lock().unwrap().claim_failure = true;
    assert!(matches!(client.claim().await, Err(Error::Unavailable)));
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    client.set_profiles(vec![]).unwrap();
    let mut context = client.execution_context().unwrap();
    context.os_version = [15, 1, 0, 0];
    client.set_execution_context(context).unwrap();
    assert_eq!(client.execution_context().unwrap().revision, 2);
    client.claim().await.unwrap();
    let inputs = server.data.lock().unwrap().claim_inputs.clone();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0], inputs[1]);
    assert_eq!(
        inputs[0]["profiles"],
        serde_json::json!(["posix_sh", "osquery"])
    );
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
