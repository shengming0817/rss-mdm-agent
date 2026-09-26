use crate::host::*;
use crate::runner::support::*;
use execution_app::*;
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
        uid: Some(1),
        session: 1,
        native: 0,
    };
    let request = serde_json::to_vec(
        &serde_json::json!({"version":4,"request":{"method":"execute","input":plan().spec()}}),
    )
    .unwrap();
    for _ in 0..2 {
        let reply: serde_json::Value =
            serde_json::from_slice(&dispatch(&mut endpoint, &peer, &request)).unwrap();
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

#[test]
#[cfg(windows)]
fn windows_installer_stops_uncertain_start_before_removing_registration() {
    let pwsh = std::env::var_os("RSS_TEST_PWSH7")
        .unwrap_or_else(|| r"C:\Program Files\PowerShell\7\pwsh.exe".into());
    assert!(std::process::Command::new(pwsh)
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/service/execution-windows.test.ps1"
        ))
        .status()
        .unwrap()
        .success());
}

#[test]
fn only_v4_envelope_calls_the_handler() {
    struct Spy(usize);
    impl Handler for Spy {
        fn handle(&mut self, _: &Peer, _: Request) -> Reply {
            self.0 += 1;
            Reply::Unavailable
        }
        fn tick(&mut self) -> Result<(), Error> {
            Ok(())
        }
        fn stop(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }
    let mut spy = Spy(0);
    let peer = Peer {
        pid: 1,
        uid: Some(1),
        session: 1,
        native: 0,
    };
    for version in [1, 2, 3, 5] {
        let bytes = serde_json::to_vec(
            &serde_json::json!({"version":version,"request":{"method":"status","request":"r"}}),
        )
        .unwrap();
        assert_eq!(dispatch(&mut spy, &peer, &bytes), br#"{"kind":"rejected"}"#);
        assert_eq!(spy.0, 0);
    }
    assert_eq!(
        dispatch(
            &mut spy,
            &peer,
            br#"{"version":4,"request":{"method":"status","request":"r"}}"#
        ),
        br#"{"kind":"unavailable"}"#
    );
    assert_eq!(spy.0, 1);
}
