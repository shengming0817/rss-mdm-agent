use super::test_support::local::{create_app, reopen_app};
use super::{bridge::ExecutionBridge, test_support::*};
use crate::protocol_test_support::*;
use agent_client::{wire::*, Client, Error, OpenMode};
use sha2::{Digest, Sha256};
use uuid::Uuid;
#[tokio::test]
async fn mismatched_and_unsubmitted_started_offers_settle_without_journal_or_cache_leaks() {
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
    let app = create_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let plan = adapted_plan(true, "device-1", &offer);
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    assert!(matches!(
        bridge.prepare(&offer, &materials, &app, &caller, &plan),
        Err(Error::Untrusted)
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
    let mut app = create_app(
        &db.path,
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
    let mut app = create_app(
        &db.path,
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
    host.state.lock().unwrap().accesses = Some(vec![execution_app::Access::ReadResult]);
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
    let app = reopen_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
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
    let mut app = create_app(
        &db.path,
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
    let mut app = create_app(
        &db.path,
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
        changed.state.lock().unwrap().accesses = Some(vec![execution_app::Access::RunnerFact]);
    }));
    assert_eq!(
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 1)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(server.data.lock().unwrap().result_calls, 1);
    host.state.lock().unwrap().accesses = Some(vec![execution_app::Access::Deliver]);
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
    let mut app = create_app(
        &db.path,
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
    // Simulate process death after the journal committed the attempt but before the
    // transport-side association persisted its local attempt identifier.
    let conn = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    let key = format!("binding/{}", offer.task_id());
    let body: Vec<u8> = conn
        .query_row("SELECT body FROM state WHERE key=?1", [&key], |r| r.get(0))
        .unwrap();
    let mut body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    body["local_attempt"] = serde_json::Value::Null;
    conn.execute(
        "UPDATE state SET body=?1 WHERE key=?2",
        rusqlite::params![serde_json::to_vec(&body).unwrap(), key],
    )
    .unwrap();
    drop(conn);
    let mut client = server.client(&root, OpenMode::Existing);
    let mut app = reopen_app(
        &db.path,
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
    let app = create_app(
        &db.path,
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
        assert_eq!(result["event"]["steps"][0]["after"]["state"], "present");
        assert_eq!(result["event"]["steps"][0]["after"]["version"], "1.0");
        assert_eq!(result["event"]["steps"][0]["before"]["state"], "present");
        assert_eq!(result["event"]["steps"][0]["process"]["kind"], "not_run");
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
    app: &mut execution_app::ExecutionApp<
        local::TestHost,
        CapturingRunner,
        execution_sqlite::Store,
    >,
    task: Uuid,
) {
    host.state.lock().unwrap().accesses = Some(vec![execution_app::Access::RunnerFact]);
    assert_eq!(bridge.flush(client, task, app, 1).await, Err(Error::Denied));
    assert_eq!(server.data.lock().unwrap().result_calls, 1);
    host.state.lock().unwrap().accesses = Some(vec![execution_app::Access::Deliver]);
    assert_eq!(bridge.flush(client, task, app, 1).await, Err(Error::Denied));
    assert_eq!(server.data.lock().unwrap().result_calls, 1);
    host.state.lock().unwrap().accesses = None;
    assert_eq!(bridge.flush(client, task, app, 1).await.unwrap(), 1);
    assert_eq!(bridge.flush(client, task, app, 1).await.unwrap(), 0);
}

async fn drain_delivery<R: execution_app::RunnerPort>(
    bridge: &ExecutionBridge<FixtureOutput>,
    client: &mut Client<Secrets, Time>,
    app: &mut execution_app::ExecutionApp<local::TestHost, R, execution_sqlite::Store>,
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
    let app = create_app(
        &db.path,
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
            br#"[{"id":"a"},{"id":"b"}]"#.to_vec(),
            vec![],
            OutputQuality::Truncated,
            ProcessEnd::Exited,
            "truncated",
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
    let plan = execution_contract::FrozenExecution::freeze(spec, &test_execution_limits()).unwrap();
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner = CapturingRunner {
        inner: DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Complete, 16)
            .unwrap(),
        ready: Default::default(),
        capture: std::sync::Arc::new(std::sync::Mutex::new(Some(CaptureSpec {
            quiescent: true,
            stdout,
            stderr,
            quality,
            end,
        }))),
    };
    let mut app = create_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
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
    if expected == "truncated" {
        assert_eq!(result["event"]["diagnostics"]["failure"], "output_limit");
    }
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

#[tokio::test]
async fn start_window_does_not_replace_the_signed_runtime_budget() {
    use execution_app::*;
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    let mut input = adapted_plan(false, "device-1", &offer).spec().clone();
    input.validity.expires_at_unix_ms =
        start.payload().expires_at() as u64 * 1000 + input.budget.total_timeout_ms;
    let plan =
        execution_contract::FrozenExecution::freeze(input, &test_execution_limits()).unwrap();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner =
        DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Wait, 16).unwrap();
    let db = local::Database::new();
    let mut app = create_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    let status = bridge
        .dispatch(&mut client, start, &materials, &mut app, prepared)
        .unwrap();
    assert_eq!(status.attempts, 1);
    assert_eq!(runner.dispatch_count(), 1);
}

#[tokio::test]
async fn root_exit_is_delivered_while_overall_quiescence_stays_unknown() {
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
        capture: std::sync::Arc::new(std::sync::Mutex::new(Some(CaptureSpec {
            quiescent: false,
            stdout: b"{\"ok\":true}".to_vec(),
            stderr: vec![],
            quality: execution_contract::OutputQuality::Complete,
            end: execution_contract::ProcessEnd::Exited,
        }))),
    };
    let mut app = create_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
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
    assert_eq!(
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 64)
            .await
            .unwrap(),
        1
    );
    {
        let data = server.data.lock().unwrap();
        let result = data.results.values().next().unwrap();
        assert_eq!(result["event"]["exitCode"], 0);
        assert_eq!(result["event"]["quality"], "partial");
    }
    assert_eq!(
        bridge.finish(&mut client, offer.task_id(), &app, &caller),
        Err(agent_client::Error::Conflict)
    );
    assert!(client.bound_request(offer.task_id()).unwrap().is_some());
    assert_eq!(runner.inner.dispatch_count(), 1);
}

#[tokio::test]
async fn acknowledged_v5_result_is_not_replaced_by_later_local_facts() {
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
        capture: std::sync::Arc::new(std::sync::Mutex::new(Some(CaptureSpec {
            quiescent: false,
            stdout: b"{\"ok\":true}".to_vec(),
            stderr: vec![],
            quality: execution_contract::OutputQuality::Partial,
            end: execution_contract::ProcessEnd::Unknown,
        }))),
    };
    let mut app = create_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
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
    assert_eq!(
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 64)
            .await
            .unwrap(),
        1
    );
    {
        let data = server.data.lock().unwrap();
        let result = data.results.values().next().unwrap();
        assert_eq!(result["event"]["exitCode"], 0);
        assert_eq!(result["event"]["quality"], "partial");
    }
    {
        let mut capture = runner.capture.lock().unwrap();
        let facts = capture.as_mut().unwrap();
        facts.quiescent = true;
        facts.quality = execution_contract::OutputQuality::Complete;
        facts.end = execution_contract::ProcessEnd::Exited;
    }
    runner
        .ready
        .store(true, std::sync::atomic::Ordering::SeqCst);
    app.reconcile(&plan.spec().request.request_id).unwrap();
    for _ in 0..4 {
        assert_eq!(
            bridge
                .flush(&mut client, offer.task_id(), &mut app, 64)
                .await
                .unwrap(),
            0
        );
    }
    assert_eq!(server.data.lock().unwrap().result_calls, 1);
    assert_eq!(runner.inner.dispatch_count(), 1);
    let status = app
        .status(&caller, &plan.spec().request.request_id)
        .unwrap();
    assert!(status.process.unwrap().quiescent);
}
