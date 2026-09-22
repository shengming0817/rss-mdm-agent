use native_process::{absent, Ready, Scope};
fn main() {
    let reference = Scope::ProcessGroup { root: 0 };
    assert!(!absent(&reference));
    let ready = Ready {
        version: 1,
        launch_id: "consumer".into(),
        launcher_pid: 42,
        worker_pid: 43,
        scope: Scope::JobObject {
            name: "not-a-worker-job".into(),
            session: 0,
        },
        artifact: "f".repeat(64),
    };
    let encoded = serde_json::to_vec(&ready).unwrap();
    let decoded: Ready = serde_json::from_slice(&encoded).unwrap();
    assert!(!absent(&decoded.scope));
    assert!(
        serde_json::from_str::<Scope>(r#"{"kind":"jobObject","name":"old-without-session"}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<Scope>(r#"{"kind":"processGroup","root":1,"authority":true}"#)
            .is_err()
    );
}
