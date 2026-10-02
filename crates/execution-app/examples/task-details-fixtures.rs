//! Regenerable UI evidence through the real S1 application, deterministic runner and SQLite.
#[path = "../tests/support/mod.rs"]
mod support;
use execution_app::*;
use support::*;
fn main() {
    let mut fixtures = serde_json::Map::new();
    for (name, scenario, approval, reconcile, cancel) in [
        ("software", TestScenario::Wait, false, false, false),
        ("running", TestScenario::Wait, false, false, false),
        ("approvalRequired", TestScenario::Wait, true, false, false),
        ("outcomeUnknown", TestScenario::Unknown, false, true, false),
        ("verified", TestScenario::Complete, false, true, false),
        ("cancelled", TestScenario::Wait, false, true, true),
        ("cancelling", TestScenario::Wait, false, false, true),
    ] {
        let db = Database::new();
        let mut host = TestHost::new();
        let p = if name == "software" {
            execution_contract::FrozenExecution::freeze(
                execution_contract::decode_execution(
                    include_bytes!("../../execution-contract/tests/fixtures/software.json"),
                    &test_store_limits().input,
                )
                .unwrap(),
                &test_store_limits().input,
            )
            .unwrap()
        } else {
            plan()
        };
        host.template = p.clone();
        host.state.lock().unwrap().approval = approval;
        let runner = DeterministicTestRunner::new(id("test-runner"), scenario, 16).unwrap();
        let mut app = ExecutionApp::start(
            &db.path,
            Startup::CreateTest,
            host,
            runner,
            AppConfig::test_defaults(1),
        )
        .unwrap();
        let request = &p.spec().request.request_id;
        app.request_execution(&caller(), &p).unwrap();
        if cancel {
            app.cancel(&caller(), request).unwrap();
        }
        if reconcile {
            app.reconcile(request).unwrap();
        }
        fixtures.insert(
            name.into(),
            serde_json::to_value(app.task_details(&caller(), request).unwrap()).unwrap(),
        );
    }
    use execution_contract::*;
    let offer = BackendTask {
        request: RequestId::new(
            fixtures["running"]["status"]["operationRequestId"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
        task: id("backend-task"),
        attempt: id("backend-attempt"),
        revision: Digest::new("a".repeat(64)).unwrap(),
        title: "办公套件".into(),
        summary: BackendTaskSummary::Software {
            intent: SoftwareOperation::Install,
            steps: vec![BackendStepSummary {
                package: "办公套件".into(),
                version: "1.0".into(),
                identity: BackendIdentity::System,
            }],
        },
        expires_at: 9999999999,
        user_initiated: true,
    };
    fixtures.insert("offer".into(), serde_json::to_value(&offer).unwrap());
    for (name, state, failure) in [
        ("proposed", BackendRequestState::Proposed, None),
        (
            "denied",
            BackendRequestState::Failed,
            Some(BackendRequestFailure::Revoked),
        ),
    ] {
        let request = BackendRequest {
            offer: offer.clone(),
            revision: 1,
            state,
            failure,
            trigger: BackendTrigger::Ai {
                os_session: OsSessionRef {
                    device: DeviceId::new("fixture-device").unwrap(),
                    account: OsAccountRef {
                        platform: Platform::Macos,
                        subject: id("fixture-user"),
                    },
                    session: id("fixture-login"),
                },
                config: VersionedRef {
                    id: id("fixture-config"),
                    revision: id("1"),
                },
                conversation: id("fixture-conversation"),
                tool_call: id("fixture-tool"),
            },
        };
        fixtures.insert(name.into(), serde_json::to_value(request).unwrap());
    }
    println!("{}", serde_json::to_string_pretty(&fixtures).unwrap());
}
