use crate::host::*;
use execution_app::*;
#[path = "../../execution-app/tests/support/mod.rs"]
mod support;
use support::*;
struct Caller;
impl Ingress for Caller {
    fn authenticate(&self, _: &Peer) -> Result<RequestContext, Error> {
        Ok(caller())
    }
}
#[test]
fn ipc_submit_uses_durable_submission_and_duplicate_delivery_never_dispatches() {
    let db = Database::new();
    let host = TestHost::new();
    let runner = DeterministicTestRunner::new(id("test-runner"), TestScenario::Wait, 8).unwrap();
    let app = ExecutionApp::start(
        &db.path,
        Startup::CreateTest,
        host,
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let mut endpoint = Endpoint::new(app, Caller, test_store_limits().plan);
    let peer = Peer {
        pid: 1,
        uid: 1,
        session: 1,
        native: 0,
    };
    let request = serde_json::to_vec(
        &serde_json::json!({"version":2,"request":{"method":"submit","plan":plan().spec()}}),
    )
    .unwrap();
    for _ in 0..2 {
        let reply: serde_json::Value =
            serde_json::from_slice(&dispatch(&mut endpoint, &peer, &request)).unwrap();
        assert_eq!(reply["value"]["submitted"], true);
        assert_eq!(reply["value"]["attempts"], 1);
    }
    assert_eq!(runner.dispatch_count(), 1);
}

#[test]
#[cfg(target_os = "macos")]
fn launchd_installation_permissions_and_partial_uninstall() {
    assert!(std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/service/execution-macos.test.py"
        ))
        .status()
        .unwrap()
        .success());
}
