use super::*;
use std::time::{Duration, Instant};

fn connection(now: Instant, challenge: &str) -> Connection {
    Connection::new(
        challenge.to_owned(),
        now,
        Status {
            version: 1,
            installation: "test-installation".into(),
            platform: "macos-arm64".into(),
            build: "test-build".into(),
            capability: Capability::StatusOnly,
        },
    )
}
fn request(challenge: &str) -> Vec<u8> {
    serde_json::to_vec(&Request {
        version: 1,
        method: Method::GetServiceStatus,
        challenge: challenge.into(),
    })
    .unwrap()
}
#[test]
fn challenge_is_consumed_before_processing_and_cannot_be_replayed() {
    let now = Instant::now();
    let mut c = connection(now, "a".repeat(64).as_str());
    assert!(c.accept(&request(&"a".repeat(64)), now).is_ok());
    assert!(c.accept(&request(&"a".repeat(64)), now).is_err());
}
#[test]
fn cross_connection_restart_and_expired_challenges_never_authorize() {
    let now = Instant::now();
    let old = request(&"a".repeat(64));
    let mut c = connection(now, &"b".repeat(64));
    assert!(c.accept(&old, now).is_err());
    assert!(c.accept(&request(&"b".repeat(64)), now).is_err());
    for elapsed in [Duration::from_secs(5), Duration::from_secs(6)] {
        let mut c = connection(now, &"a".repeat(64));
        assert!(c.accept(&old, now + elapsed).is_err());
    }
    let mut c = connection(now, &"a".repeat(64));
    assert!(c.accept(&old, now + Duration::from_millis(4999)).is_ok());
}
#[test]
fn legacy_unknown_duplicate_and_secret_fields_fail_closed() {
    let now = Instant::now();
    for bytes in [
        br#"{"version":0,"method":"getServiceStatus","challenge":"x"}"#.as_slice(),
        br#"{"version":1,"version":1,"method":"getServiceStatus","challenge":"x"}"#,
        br#"{"version":1,"method":"getServiceStatus","challenge":"x","secret":"canary"}"#,
        br#"{"version":1,"method":"exec","challenge":"x"}"#,
        br#"{"schemaVersion":5,"kind":"nativeCall","method":"masterKey"}"#,
        &[0; 65537],
    ] {
        let mut c = connection(now, "x");
        assert!(c.accept(bytes, now).is_err());
        assert!(c.accept(&request("x"), now).is_err());
    }
}

#[test]
fn client_rejects_wrong_challenge_version_and_extra_response_fields() {
    let now = Instant::now();
    let mut server = connection(now, &"a".repeat(64));
    let (challenge, request) = client_request(&server.greeting().unwrap()).unwrap();
    let reply = server.accept(&request, now).unwrap();
    assert!(client_reply(&reply, &challenge).is_ok());
    assert!(client_reply(&reply, &"b".repeat(64)).is_err());
    for (key, value) in [
        ("version", serde_json::json!(0)),
        ("secret", serde_json::json!("canary")),
    ] {
        let mut edited: serde_json::Value = serde_json::from_slice(&reply).unwrap();
        edited[key] = value;
        assert!(client_reply(&serde_json::to_vec(&edited).unwrap(), &challenge).is_err());
    }
}

#[test]
fn concurrent_replays_admit_exactly_once() {
    let now = Instant::now();
    let connection = std::sync::Arc::new(std::sync::Mutex::new(connection(now, &"a".repeat(64))));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let connection = connection.clone();
            std::thread::spawn(move || {
                connection
                    .lock()
                    .unwrap()
                    .accept(&request(&"a".repeat(64)), now)
                    .is_ok()
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .map(|t| usize::from(t.join().unwrap()))
            .sum::<usize>(),
        1
    );
}

#[cfg(test)]
#[test]
fn wire_values_roundtrip_and_reject_unknown_authority() {
    use crate::*;

    let status = Status {
        version: 1,
        installation: "fixture".into(),
        platform: "fixture".into(),
        build: "fixture".into(),
        capability: Capability::StatusOnly,
    };
    let bytes = serde_json::to_vec(&status).unwrap();
    assert_eq!(serde_json::from_slice::<Status>(&bytes).unwrap(), status);
    assert!(serde_json::from_str::<Capability>(r#""execute""#).is_err());
    let mut edited = serde_json::to_value(&status).unwrap();
    edited["credential"] = serde_json::json!("untrusted");
    assert!(serde_json::from_value::<Status>(edited).is_err());
    assert_eq!(
        serde_json::to_value(ServiceView::Rejected).unwrap()["phase"],
        "rejected"
    );
}

#[test]
fn installed_policy_has_one_strict_format() {
    let artifact =
        serde_json::json!({"path": "/fixture/program", "sha256": "a".repeat(64), "cdhash": null});
    let value = serde_json::json!({
        "version": 1, "installation": "fixture", "build": "test", "platform": "macos-arm64",
        "service_subject": "service", "allowed_users": ["501"],
        "client": artifact, "probe": artifact, "service": artifact
    });
    let policy: crate::policy::Policy = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(policy).unwrap(), value);
    let mut unknown = value.clone();
    unknown["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<crate::policy::Policy>(unknown).is_err());
    let mut missing = value;
    missing.as_object_mut().unwrap().remove("allowed_users");
    assert!(serde_json::from_value::<crate::policy::Policy>(missing).is_err());
}
