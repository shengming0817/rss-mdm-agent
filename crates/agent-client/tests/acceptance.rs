use agent_client::{Config, Error, Limits, Transport};
use url::Url;
use uuid::Uuid;

mod support;

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
async fn unsupported_and_unsubmitted_started_offers_settle_without_journal_or_cache_leaks() {
    use agent_client::Error;
    use execution_app::*;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    server.data.lock().unwrap().software(3, false);
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let db = local::Database::new();
    let host = local::TestHost::new();
    let runner =
        DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Wait, 16).unwrap();
    let app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host,
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let plan = adapted_plan(true, "device-1", &offer);
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    assert!(matches!(
        bridge.prepare(&offer, &materials, &app, &caller, &plan),
        Err(Error::Unsupported)
    ));
    let start = client.request_start(&offer, &materials).await.unwrap();
    bridge
        .abandon(&mut client, offer.task_id(), &app)
        .await
        .unwrap();
    assert!(client.validate_start(&start).is_err());
    assert_eq!(runner.dispatch_count(), 0);
    assert_eq!(client.cleanup(128).unwrap(), 0);
    drop(materials);
    assert_eq!(client.cleanup(128).unwrap(), 1);
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    {
        let mut d = server.data.lock().unwrap();
        d.task = Uuid::new_v4();
        d.results.clear();
        d.script();
    }
    let next = client.claim().await.unwrap().offer.unwrap();
    assert_ne!(next.task_id(), offer.task_id());
}
#[tokio::test]
async fn denied_admission_without_attempt_delivers_and_releases() {
    use execution_app::*;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let plan = adapted_plan(false, "device-1", &offer);
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner =
        DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Complete, 16).unwrap();
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    host.state.lock().unwrap().allow = false;
    let status = bridge
        .dispatch(&mut client, start, &materials, &mut app, prepared)
        .unwrap();
    assert_eq!(status.phase, TaskPhase::AdmissionDenied);
    assert!(status.attempt_id.is_none());
    for _ in 0..32 {
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 1)
            .await
            .unwrap();
    }
    assert_eq!(
        server.data.lock().unwrap().results.values().next().unwrap()["event"]["quality"],
        "failed"
    );
    bridge
        .finish(&mut client, offer.task_id(), &app, &caller)
        .unwrap();
    assert_eq!(runner.dispatch_count(), 0);
    drop(materials);
    assert_eq!(client.cleanup(128).unwrap(), 1);
}
#[tokio::test]
async fn reserved_binding_without_journal_can_settle_after_permission_revocation_and_restart() {
    use execution_app::*;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let plan = adapted_plan(false, "device-1", &offer);
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner =
        DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Complete, 16).unwrap();
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    host.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::ReadResult]);
    assert!(bridge
        .dispatch(&mut client, start, &materials, &mut app, prepared)
        .is_err());
    assert!(!app
        .has_service_execution(
            &plan.spec().request.request_id,
            &plan.spec().request.target.device
        )
        .unwrap());
    drop(app);
    drop(materials);
    drop(client);
    server.time.set(72);
    host.state.lock().unwrap().accesses = None;
    let app = ExecutionApp::start(
        &db.path,
        Startup::OpenTest,
        host,
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let mut client = server.client(&root, OpenMode::Existing);
    bridge
        .abandon(&mut client, offer.task_id(), &app)
        .await
        .unwrap();
    assert_eq!(
        server.data.lock().unwrap().results.values().next().unwrap()["event"]["kind"],
        "cancelled"
    );
    assert_eq!(runner.dispatch_count(), 0);
    assert_eq!(client.cleanup(128).unwrap(), 1);
    assert!(client.offer(offer.task_id()).is_err());
}
async fn no_process_terminal(cancel: bool) {
    use execution_app::*;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let plan = adapted_plan(false, "device-1", &offer);
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner = DeterministicTestRunner::new(
        local::id("test-runner"),
        if cancel {
            TestScenario::Complete
        } else {
            TestScenario::RejectBeforeDispatch
        },
        16,
    )
    .unwrap();
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    if cancel {
        host.state.lock().unwrap().capability = false;
        assert!(bridge
            .dispatch(&mut client, start, &materials, &mut app, prepared)
            .is_err());
        host.state.lock().unwrap().capability = true;
        app.cancel(&caller, &plan.spec().request.request_id)
            .unwrap();
    } else {
        bridge
            .dispatch(&mut client, start, &materials, &mut app, prepared)
            .unwrap();
        app.reconcile(&plan.spec().request.request_id).unwrap();
    }
    assert_eq!(
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 1)
            .await
            .unwrap(),
        1
    );
    for _ in 0..32 {
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 1)
            .await
            .unwrap();
    }
    assert_no_process_result(&server, cancel, runner.dispatch_count());

    bridge
        .finish(&mut client, offer.task_id(), &app, &caller)
        .unwrap();
}
#[tokio::test]
async fn durable_cancel_without_attempt_is_delivered_and_released() {
    no_process_terminal(true).await;
}
#[tokio::test]
async fn proven_never_dispatched_without_capture_is_delivered_and_released() {
    no_process_terminal(false).await;
}
#[tokio::test]
async fn accepted_remote_result_with_failed_local_confirmation_never_resends() {
    use agent_client::Error;
    use execution_app::*;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let plan = adapted_plan(false, "device-1", &offer);
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner = CapturingRunner {
        inner: DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Complete, 16)
            .unwrap(),
        ready: Default::default(),
        capture: Default::default(),
    };
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    bridge
        .dispatch(&mut client, start, &materials, &mut app, prepared)
        .unwrap();
    app.reconcile(&plan.spec().request.request_id).unwrap();
    let changed = host.clone();
    server.data.lock().unwrap().result_hook = Some(std::sync::Arc::new(move || {
        changed.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::RunnerFact]);
    }));
    assert_eq!(
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 1)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(server.data.lock().unwrap().result_calls, 1);
    host.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::Deliver]);
    assert_eq!(
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 1)
            .await
            .unwrap(),
        1
    );
    assert_eq!(server.data.lock().unwrap().result_calls, 1);
    assert_eq!(runner.inner.dispatch_count(), 1);
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
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            2,
        )
        .unwrap();
    assert_eq!(
        client.queue_report(
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
    native_process::private_storage::write_new(&target, b"keep").unwrap();
    std::fs::remove_file(&partial).unwrap();
    std::os::unix::fs::symlink(&target, &partial).unwrap();
    assert!(matches!(client.prepare(&offer).await, Err(Error::Storage)));
    assert_eq!(std::fs::read(target).unwrap(), b"keep");
}

use agent_client::{wire::*, Client, ExecutionBridge, OpenMode};
use sha2::{Digest, Sha256};
use support::execution::*;
use support::*;

async fn exercise_bridge(software: bool) {
    use agent_client::Error;
    use execution_app::*;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    if software {
        server.data.lock().unwrap().software(1, false);
    }
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    let plan = adapted_plan(software, "device-1", &offer);
    host.template = plan.clone();
    let runner = CapturingRunner {
        inner: DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Complete, 16)
            .unwrap(),
        ready: Default::default(),
        capture: Default::default(),
    };
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    bridge
        .dispatch(&mut client, start, &materials, &mut app, prepared)
        .unwrap();
    assert_eq!(runner.inner.dispatch_count(), 1);
    app.reconcile(&plan.spec().request.request_id).unwrap();
    server.data.lock().unwrap().result_failure = true;
    assert_eq!(
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 1)
            .await,
        Err(Error::Unavailable)
    );
    drop(client);
    drop(app);
    let mut client = server.client(&root, OpenMode::Existing);
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::OpenTest,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    check_frozen_delivery_permissions(
        &server,
        &host,
        &bridge,
        &mut client,
        &mut app,
        offer.task_id(),
    )
    .await;
    assert_eq!(runner.inner.dispatch_count(), 1);
    drain_delivery(&bridge, &mut client, &mut app, offer.task_id()).await;
    assert!(app
        .service_delivery(
            &plan.spec().request.request_id,
            &local::id("agent-consumer"),
            1
        )
        .unwrap()
        .is_empty());
    bridge
        .finish(&mut client, offer.task_id(), &app, &caller)
        .unwrap();
    assert_bridge_result(&server, &offer, software);
}
#[tokio::test]
async fn script_bridge_recovers_without_dispatch_and_retries_only_confirmation() {
    exercise_bridge(false).await;
}
#[tokio::test]
async fn software_bridge_uses_journal_detection_and_remote_definition() {
    exercise_bridge(true).await;
}

#[tokio::test]
async fn register_report_recovery_and_exact_ack() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.data.lock().unwrap().registration_failure = true;
    let operation = Uuid::new_v4();
    let enrollment = Uuid::new_v4();
    let capabilities = vec![Capability::InventoryBasicV4, Capability::TaskExecuteV4];
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
async fn partial_cache_reopens_and_checks_range_etag_and_reference_cleanup() {
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    server.data.lock().unwrap().content_failure = true;
    assert!(matches!(
        client.prepare(&offer).await,
        Err(Error::Unavailable)
    ));
    let directory = root.path.join("content");
    let partial = std::fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let bytes = server.data.lock().unwrap().bytes.clone();
    std::fs::write(&partial, &bytes[..100]).unwrap();
    drop(client);
    let mut client = server.client(&root, OpenMode::Existing);
    let offer = client.offer(offer.task_id()).unwrap();
    server.data.lock().unwrap().bad_range = true;
    assert!(matches!(
        client.prepare(&offer).await,
        Err(Error::Untrusted)
    ));
    server.data.lock().unwrap().bad_range = false;
    let materials = client.prepare(&offer).await.unwrap();
    let blob = materials.files()[0].path().to_owned();
    assert_eq!(
        Sha256::digest(std::fs::read(blob).unwrap()).as_slice(),
        Sha256::digest(&bytes).as_slice()
    );
    assert!(server
        .data
        .lock()
        .unwrap()
        .content_calls
        .iter()
        .any(|v| v.as_deref() == Some("bytes=100-")));
    assert_eq!(client.cleanup(128).unwrap(), 0);
    let db = local::Database::new();
    let host = local::TestHost::new();
    let runner = execution_app::DeterministicTestRunner::new(
        local::id("test-runner"),
        execution_app::TestScenario::Wait,
        16,
    )
    .unwrap();
    let app = execution_app::ExecutionApp::start(
        &db.path,
        execution_app::Startup::CreateTest,
        host,
        runner,
        execution_app::AppConfig::test_defaults(1),
    )
    .unwrap();
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    bridge
        .abandon(&mut client, offer.task_id(), &app)
        .await
        .unwrap();
    assert_eq!(client.cleanup(128).unwrap(), 0);
    drop(materials);
    assert_eq!(client.cleanup(128).unwrap(), 1);
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
    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 9).unwrap();
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
    assert_eq!(before, std::fs::read(path).unwrap());
}

use rss_mdm_agent_wire::{TaskArchitecture, TaskPlatform};
#[test]
fn configuration_rejects_untrusted_network_and_unbounded_budgets() {
    let mut cfg = Config {
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

fn assert_no_process_result(server: &Server, cancel: bool, dispatch_count: usize) {
    let data = server.data.lock().unwrap();
    let result = data.results.values().next().unwrap();
    if cancel {
        assert_eq!(result["event"]["kind"], "cancelled");
        assert_eq!(dispatch_count, 0);
    } else {
        assert_eq!(result["event"]["kind"], "result");
        assert_eq!(result["event"]["quality"], "failed");
        assert_eq!(result["event"]["diagnostics"]["failure"], "launch_failed");
    }
    drop(data);
}

fn assert_bridge_result(server: &Server, offer: &agent_client::Offer, software: bool) {
    let data = server.data.lock().unwrap();
    assert_eq!(data.results.len(), 1);
    let result = data.results.values().next().unwrap();
    assert!(!result.to_string().contains("secret-canary"));
    if software {
        assert_eq!(result["event"]["kind"], "software_result");
        assert_eq!(result["event"]["detection"], "present");
        assert_eq!(result["event"]["observedVersion"], "1.0");
        let TaskPayload::Software(spec) = offer.payload() else {
            panic!()
        };
        assert_eq!(
            result["event"]["definitionDigest"],
            serde_json::to_value(spec.definition_digest).unwrap()
        );
    } else {
        assert_eq!(result["event"]["kind"], "result");
        assert_eq!(result["event"]["output"]["ok"], true);
    }
}

async fn check_frozen_delivery_permissions(
    server: &Server,
    host: &local::TestHost,
    bridge: &ExecutionBridge<FixtureOutput>,
    client: &mut Client<Secrets, Time>,
    app: &mut execution_app::ExecutionApp<local::TestHost, CapturingRunner>,
    task: Uuid,
) {
    host.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::RunnerFact]);
    assert_eq!(bridge.flush(client, task, app, 1).await, Err(Error::Denied));
    assert_eq!(server.data.lock().unwrap().result_calls, 1);
    host.state.lock().unwrap().accesses = Some(vec![execution_sqlite::Access::Deliver]);
    assert_eq!(bridge.flush(client, task, app, 1).await, Err(Error::Denied));
    assert_eq!(server.data.lock().unwrap().result_calls, 1);
    host.state.lock().unwrap().accesses = None;
    assert_eq!(bridge.flush(client, task, app, 1).await.unwrap(), 1);
    assert_eq!(bridge.flush(client, task, app, 1).await.unwrap(), 0);
}

async fn drain_delivery<R: execution_app::RunnerPort>(
    bridge: &ExecutionBridge<FixtureOutput>,
    client: &mut Client<Secrets, Time>,
    app: &mut execution_app::ExecutionApp<local::TestHost, R>,
    task: Uuid,
) {
    for _ in 0..32 {
        bridge.flush(client, task, app, 1).await.unwrap();
    }
}

#[tokio::test]
async fn expired_offers_retire_received_requests_with_one_task_budget_after_restart() {
    let server = Server::new().await;
    let root = Root::new();
    let mut config = server.config();
    config.limits.pending_tasks = 1;
    let mut client = Client::open(
        &root.path,
        config.clone(),
        OpenMode::Create,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    server.register(&mut client).await;
    let db = local::Database::new();
    let host = local::TestHost::new();
    let runner = execution_app::DeterministicTestRunner::new(
        local::id("test-runner"),
        execution_app::TestScenario::Wait,
        16,
    )
    .unwrap();
    let app = execution_app::ExecutionApp::start(
        &db.path,
        execution_app::Startup::CreateTest,
        host,
        runner,
        execution_app::AppConfig::test_defaults(1),
    )
    .unwrap();
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    for i in 0..8 {
        let offer = client.claim().await.unwrap().offer.unwrap();
        let materials = client.prepare(&offer).await.unwrap();
        client.received(&offer).await.unwrap();
        drop(materials);
        drop(client);
        client = Client::open(
            &root.path,
            config.clone(),
            OpenMode::Existing,
            server.secrets.clone(),
            server.time.clone(),
        )
        .unwrap();
        if i == 7 {
            bridge
                .abandon(&mut client, offer.task_id(), &app)
                .await
                .unwrap();
        } else {
            server.time.set(offer.payload().expires_at());
        }
    }
    assert_eq!(client.cleanup(128).unwrap(), 1);
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
async fn frozen_output_encodings_and_character_truncation_are_delivered() {
    use execution_contract::{OutputQuality, ProcessEnd, TextEncoding};
    for (encoding, stdout, stderr, quality, end, expected) in [
        (
            TextEncoding::Utf16Le,
            "{\"ok\":true}"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
            "secret-canary"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
            OutputQuality::Complete,
            ProcessEnd::Exited,
            "complete",
        ),
        (
            TextEncoding::Utf8,
            vec![0xff],
            b"secret-canary".to_vec(),
            OutputQuality::Failed,
            ProcessEnd::Exited,
            "failed",
        ),
        (
            TextEncoding::Utf8,
            vec![0xe4, 0xb8],
            vec![],
            OutputQuality::Truncated,
            ProcessEnd::OutputLimit,
            "truncated",
        ),
        (
            TextEncoding::Utf16Le,
            vec![b'a', 0, 0x3d, 0xd8],
            vec![],
            OutputQuality::Truncated,
            ProcessEnd::OutputLimit,
            "truncated",
        ),
        (
            TextEncoding::Utf8,
            b"{\"ok\":true}".to_vec(),
            vec![0xff],
            OutputQuality::Failed,
            ProcessEnd::Exited,
            "failed",
        ),
        (
            TextEncoding::Utf16Le,
            "{\"ok\":true}"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
            vec![0, 0xd8],
            OutputQuality::Failed,
            ProcessEnd::Exited,
            "failed",
        ),
        (
            TextEncoding::Utf16Le,
            vec![b'a'],
            vec![],
            OutputQuality::Failed,
            ProcessEnd::Exited,
            "failed",
        ),
    ] {
        check_encoded_result(encoding, stdout, stderr, quality, end, expected).await;
    }
}
async fn check_encoded_result(
    encoding: execution_contract::TextEncoding,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    quality: execution_contract::OutputQuality,
    end: execution_contract::ProcessEnd,
    expected: &str,
) {
    use execution_app::*;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let original = adapted_plan(false, "device-1", &offer);
    let mut spec = original.spec().clone();
    spec.launch.output.stdout = encoding;
    spec.launch.output.stderr = encoding;
    let plan =
        execution_contract::FrozenExecution::freeze(spec, &test_store_limits().input).unwrap();
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner = CapturingRunner {
        inner: DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Complete, 16)
            .unwrap(),
        ready: Default::default(),
        capture: std::sync::Arc::new(std::sync::Mutex::new(Some(CaptureSpec {
            stdout,
            stderr,
            quality,
            end,
        }))),
    };
    let mut app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host,
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    bridge
        .dispatch(&mut client, start, &materials, &mut app, prepared)
        .unwrap();
    app.reconcile(&plan.spec().request.request_id).unwrap();
    drain_delivery(&bridge, &mut client, &mut app, offer.task_id()).await;
    let data = server.data.lock().unwrap();
    let result = data.results.values().next().unwrap();
    assert_eq!(result["event"]["quality"], expected);
    assert!(!result.to_string().contains("secret-canary"));
    if expected == "complete" {
        assert_eq!(result["event"]["output"]["ok"], true);
    }
    drop(data);
    bridge
        .finish(&mut client, offer.task_id(), &app, &caller)
        .unwrap();
    assert_eq!(runner.inner.dispatch_count(), 1);
}
