//! Real journal and HTTP seams; the provider runner is explicitly a fixture, not Windows proof.
use super::{
    bridge::ExecutionBridge,
    test_support::{
        local::{create_app, reopen_app},
        *,
    },
};
use crate::protocol_test_support::*;
use agent_client::{Clock, Error, OpenMode};
use execution_app::*;
use execution_contract::*;
use rss_mdm_agent_wire::runtime as w;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn native_capture_survives_lost_ack_and_restart_without_recollection() {
    capture_recovery(0).await;
}
#[tokio::test]
async fn macos_capture_survives_lost_ack_and_restart_without_recollection() {
    capture_recovery(1).await;
}
#[tokio::test]
async fn system_scan_evidence_survives_lost_ack_without_chunks() {
    capture_recovery(2).await;
}
#[tokio::test]
async fn certificate_metadata_survives_lost_ack() {
    capture_recovery(3).await;
}
#[tokio::test]
async fn macos_local_scan_evidence_survives_lost_ack_without_recollection() {
    capture_recovery(4).await;
}
#[tokio::test]
async fn macos_certificate_metadata_survives_lost_ack_without_recollection() {
    capture_recovery(5).await;
}
async fn capture_recovery(case: u8) {
    let macos = matches!(case, 1 | 4 | 5);
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    if case == 4 {
        server
            .data
            .lock()
            .unwrap()
            .native_system_collection(w::NativeSystemQuery::MacosUpdates {
                section: w::UpdateSection::Available,
                mode: w::UpdateScanMode::Local,
            });
    } else if case == 5 {
        server.data.lock().unwrap().native_system_collection(
            w::NativeSystemQuery::MacosCertificates {
                store: w::MacosCertificateStore::SystemRoots,
            },
        );
    } else if macos {
        server.data.lock().unwrap().native_macos_collection();
    } else if case == 2 {
        server.data.lock().unwrap().native_system_collection(
            w::NativeSystemQuery::WindowsUpdates {
                section: w::UpdateSection::Available,
                mode: w::UpdateScanMode::Local,
            },
        );
    } else if case == 3 {
        server.data.lock().unwrap().native_system_collection(
            w::NativeSystemQuery::WindowsCertificates {
                store: w::WindowsCertificateStore::My,
                user: false,
            },
        );
    } else {
        server.data.lock().unwrap().native_collection();
    }
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    assert!(materials.files().is_empty());
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    let w::TaskPayload::Collection(task) = start.payload() else {
        panic!()
    };
    let time = server.time.now().unwrap();
    let q = &task.plan.spec().queries["items"];
    let mut result = w::NativeCollectionResult {
        plan_digest: task.plan_digest,
        collected_at: time,
        queries: [(
            "items".into(),
            w::NativeQueryResult {
                query_digest: q.digest(),
                collected_at: time,
                status: w::NativeQueryStatus::Truncated,
                enumeration_complete: false,
                row_count: 1,
                rows: vec![([
                    (
                        if macos { "id" } else { "Id" }.into(),
                        serde_json::json!("stable"),
                    ),
                    (
                        if macos { "name" } else { "Name" }.into(),
                        serde_json::json!("partial evidence"),
                    ),
                ])
                .into()],
                users: None,
                scan: None,
            },
        )]
        .into(),
    };
    if case >= 2 {
        let value = result.queries.get_mut("items").unwrap();
        if matches!(case, 2 | 4) {
            value.scan = Some(w::NativeScanEvidence {
                mode: w::UpdateScanMode::Local,
                source: Some(
                    if macos {
                        "apple_softwareupdate_cache"
                    } else {
                        "wua-source"
                    }
                    .into(),
                ),
                started_at: None,
                finished_at: None,
            });
            value.rows = (0..4)
                .map(|i| {
                    [
                        ("id".into(), serde_json::json!(format!("update-{i}"))),
                        (
                            "title".into(),
                            serde_json::json!("retained update observation"),
                        ),
                        ("version".into(), serde_json::json!("1")),
                        (
                            "origin".into(),
                            serde_json::json!(if macos {
                                "apple_softwareupdate_cache"
                            } else {
                                "wua-source"
                            }),
                        ),
                        ("state".into(), serde_json::json!("available")),
                    ]
                    .into()
                })
                .collect();
        } else {
            value.rows = vec![[
                ("id".into(), serde_json::json!("a".repeat(64))),
                ("subject".into(), serde_json::json!("device certificate")),
                ("issuer".into(), serde_json::json!("test issuer")),
                ("scope".into(), serde_json::json!("device")),
                ("owner".into(), serde_json::json!("")),
                (
                    "store".into(),
                    serde_json::json!(if macos { "system_roots" } else { "my" }),
                ),
                ("not_before".into(), serde_json::json!(1)),
                ("not_after".into(), serde_json::json!(time + 100)),
            ]
            .into()];
        }
        value.row_count = value.rows.len() as u32;
        result.validate_for(&task.plan, time).unwrap();
    }
    let mut input = adapted_plan(false, "device-1", &offer).spec().clone();
    input.request.target.platform = if macos {
        Platform::Macos
    } else {
        Platform::Windows
    };
    input.constraints = IsolationPolicy::OsIdentity {};
    input.launch.cwd = if macos {
        "/Library/RSS"
    } else {
        r"C:\ProgramData\RSS"
    }
    .into();
    input.run_as = RunAs::System {
        platform: input.request.target.platform,
    };
    input.launch.interpreter.profile =
        super::plan::reference(crate::collection_worker::PROFILE, "1").unwrap();
    input.request.operation.resource =
        super::plan::reference("backend-resource", super::plan::hex(&task.resource_digest))
            .unwrap();
    input.validity.expires_at_unix_ms =
        task.expires_at as u64 * 1000 + input.budget.total_timeout_ms;
    input.request.parameters.insert(
        "collection-tools".into(),
        InputValue::Literal {
            value: serde_json::json!({}),
        },
    );
    let bytes = serde_json::to_vec(&crate::collection_worker::Input {
        task: (**task).clone(),
        finish_by_unix_ms: input.validity.not_before_unix_ms + input.budget.total_timeout_ms,
        image_sha256: input.launch.interpreter.artifact.sha256.clone(),
        run_as: input.run_as.clone(),
        session: input.session_requirement.clone(),
        tools: Default::default(),
        observed: Default::default(),
    })
    .unwrap();
    use sha2::{Digest as _, Sha256};
    input.launch.artifact.sha256 = Digest::new(super::plan::hex(&Sha256::digest(bytes))).unwrap();
    let plan = FrozenExecution::freeze(input, &test_execution_limits()).unwrap();
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner = CapturingRunner {
        inner: DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Complete, 16)
            .unwrap(),
        ready: Default::default(),
        progress: Default::default(),
        capture: Arc::new(Mutex::new(Some(CaptureSpec {
            exit_code: 0,
            quiescent: true,
            stdout: serde_json::to_vec(&result).unwrap(),
            stderr: vec![],
            quality: OutputQuality::Complete,
            end: ProcessEnd::Exited,
        }))),
    };
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let mut app = create_app(
        &db.path,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    bridge
        .dispatch(&mut client, start, &materials, &mut app, prepared)
        .unwrap();
    app.reconcile(&plan.spec().request.request_id).unwrap();
    server.data.lock().unwrap().result_failure = true;
    assert_eq!(
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 1)
            .await,
        Err(Error::Unavailable)
    );
    let original = server.data.lock().unwrap().results.clone();
    drop(app);
    drop(client);
    // A replacement fixture would return different output if recovery incorrectly dispatched.
    runner.capture.lock().unwrap().as_mut().unwrap().stdout = b"{\"wrong\":true}".to_vec();
    let mut app = reopen_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
    let mut client = server.client(&root, OpenMode::Existing);
    for _ in 0..32 {
        bridge
            .flush(&mut client, offer.task_id(), &mut app, 32)
            .await
            .unwrap();
    }
    assert_eq!(runner.inner.dispatch_count(), 1);
    assert_eq!(server.data.lock().unwrap().results, original);
    let data = server.data.lock().unwrap();
    let received = data.results.values().next().unwrap();
    assert_eq!(received["event"]["kind"], "collection_result");
    assert_eq!(received["event"]["queries"]["items"]["status"], "truncated");
    assert_eq!(
        received["event"]["queries"]["items"]["enumerationComplete"],
        false
    );
    if matches!(case, 2 | 4) {
        assert_eq!(
            received["event"]["queries"]["items"]["scan"]["source"],
            if macos {
                "apple_softwareupdate_cache"
            } else {
                "wua-source"
            }
        );
    }
    drop(data);
    bridge
        .finish(&mut client, offer.task_id(), &app, &caller)
        .unwrap();
}
