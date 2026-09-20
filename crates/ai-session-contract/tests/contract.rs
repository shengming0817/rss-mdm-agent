use ai_session_contract::{decode, encode, fingerprint, Limits};
use serde_json::Value;
const FIXTURES: &str = include_str!("../../../packages/ai-contract/src/testing/fixtures.json");
fn limits() -> Limits {
    Limits {
        max_bytes: 262144,
        max_text_bytes: 131072,
        max_depth: 32,
        max_nodes: 16384,
    }
}

#[test]
fn native_thread_identity_roundtrips_and_rejects_invalid_ids() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    for kind in ["session", "commandRecord"] {
        let mut value = fixtures["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["kind"] == kind && (kind == "session" || row.get("dispatch").is_some()))
            .unwrap()
            .clone();
        let field = if kind == "session" {
            "/stages/0/binding"
        } else {
            "/dispatch"
        };
        value.pointer_mut(field).unwrap()["nativeThreadId"] = Value::String("native-thread".into());
        let record = decode(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap();
        let encoded: Value = serde_json::from_slice(&encode(&record, &limits()).unwrap()).unwrap();
        assert_eq!(
            encoded.pointer(field).unwrap()["nativeThreadId"],
            "native-thread"
        );
        value.pointer_mut(field).unwrap()["nativeThreadId"] =
            Value::String("invalid thread".into());
        assert!(decode(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
    }
}
#[test]
fn shared_v5_golden_and_safe_diagnostics() {
    let f: Value = serde_json::from_str(FIXTURES).unwrap();
    for v in f["valid"].as_array().unwrap() {
        let record = decode(&serde_json::to_vec(v).unwrap(), &limits()).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&encode(&record, &limits()).unwrap()).unwrap(),
            *v
        );
        assert!(!format!("{record:?}").contains("fixture text"));
    }
    for c in f["invalid"].as_array().unwrap() {
        let mut l = limits();
        let o = &c["limits"];
        if let Some(n) = o["maxBytes"].as_u64() {
            l.max_bytes = n as usize;
        }
        if let Some(n) = o["maxTextBytes"].as_u64() {
            l.max_text_bytes = n as usize;
        }
        if let Some(n) = o["maxDepth"].as_u64() {
            l.max_depth = n as usize;
        }
        if let Some(n) = o["maxNodes"].as_u64() {
            l.max_nodes = n as usize;
        }
        let e = decode(c["raw"].as_str().unwrap().as_bytes(), &l).unwrap_err();
        assert_eq!(
            serde_json::to_value(e.code).unwrap(),
            c["code"],
            "{}",
            c["name"]
        );
        assert!(!format!("{e:?}").contains("do-not-report"));
    }
    let c: ai_session_contract::Command = serde_json::from_value(f["valid"][0].clone()).unwrap();
    assert_eq!(fingerprint(&c, &limits()).unwrap(), f["commandHash"]);
}
#[test]
fn constructed_data_gets_the_same_limits_and_no_old_version_fallback() {
    let f: Value = serde_json::from_str(FIXTURES).unwrap();
    let bytes = serde_json::to_vec(&f["valid"][0]).unwrap();
    let record = decode(&bytes, &limits()).unwrap();
    assert!(encode(
        &record,
        &Limits {
            max_bytes: 8,
            ..limits()
        }
    )
    .is_err());
    assert!(decode(
        &bytes,
        &Limits {
            max_depth: 0,
            ..limits()
        }
    )
    .is_err());
    assert_eq!(
        encode(
            &record,
            &Limits {
                max_bytes: 0,
                ..limits()
            }
        )
        .unwrap_err()
        .code,
        ai_session_contract::Diagnostic::Configuration
    );
    assert!(decode(b"\xff", &limits()).is_err());
}

#[test]
fn event_discriminators_select_the_actual_rust_variant() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    for value in fixtures["valid"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["kind"] == "event")
    {
        let event: ai_session_contract::Event = serde_json::from_value(value.clone()).unwrap();
        match (
            value["body"]["type"].as_str(),
            value["body"]["operation"].as_str(),
        ) {
            (Some("error"), _) => {
                assert!(matches!(event, ai_session_contract::Event::Error { .. }))
            }
            (Some("invalidated"), _) => assert!(matches!(
                event,
                ai_session_contract::Event::Invalidated { .. }
            )),
            (Some("surface"), _) => {
                assert!(matches!(event, ai_session_contract::Event::Surface { .. }))
            }
            _ => (),
        }
    }
}

#[test]
fn local_and_native_acknowledgements_have_distinct_schema_derived_variants() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    let mut event = fixtures["valid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["kind"] == "event" && v.get("attemptId").is_some())
        .unwrap()
        .clone();
    event["body"] = serde_json::json!({"type":"acknowledged","acknowledgement":{"type":"cancel","confirmation":"request_only"}});
    decode(&serde_json::to_vec(&event).unwrap(), &limits()).unwrap();
    assert!(matches!(
        serde_json::from_value::<ai_session_contract::Event>(event.clone()).unwrap(),
        ai_session_contract::Event::Acknowledged { .. }
    ));
    event.as_object_mut().unwrap().remove("attemptId");
    event["body"] = serde_json::json!({"type":"acknowledged","acknowledgement":{"type":"queued_cancelled","targetCommandId":"queued"}});
    decode(&serde_json::to_vec(&event).unwrap(), &limits()).unwrap();
    assert!(matches!(
        serde_json::from_value::<ai_session_contract::Event>(event).unwrap(),
        ai_session_contract::Event::AcknowledgedQueuedCancelled { .. }
    ));
}

#[test]
fn preference_patch_retains_set_clear_and_omission_through_rust_roundtrip() {
    for patch in [
        serde_json::json!({}),
        serde_json::json!({"selectedSessionId":{"set":"session"}}),
        serde_json::json!({"selectedSessionId":{"clear":true}}),
    ] {
        let value =
            serde_json::json!({"schemaVersion":5,"kind":"preferencesRequest","patch":patch});
        let record = decode(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&encode(&record, &limits()).unwrap()).unwrap(),
            value
        );
    }
    for patch in [
        serde_json::json!({"selectedSessionId":null}),
        serde_json::json!({"selectedSessionId":{"set":"session","clear":true}}),
    ] {
        let value =
            serde_json::json!({"schemaVersion":5,"kind":"preferencesRequest","patch":patch});
        assert!(decode(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
    }
}

#[test]
fn native_control_and_execution_origin_are_generated_closed_rust_types() {
    for value in [
        serde_json::json!({"schemaVersion":5,"kind":"nativeCall","id":1,"method":"masterKey","data":{"create":false}}),
        serde_json::json!({"schemaVersion":5,"kind":"executionOrigin","namespace":{"tenantId":"test-users","principalId":"alice","authorityId":"desktop-fixture","sessionId":"session-1"},"userGeneration":"generation-a","operationId":"operation-1","provider":"codex","config":{"id":"connection-1","revision":"1"}}),
    ] {
        let record = decode(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&encode(&record, &limits()).unwrap()).unwrap(),
            value
        );
    }
    let invalid = serde_json::json!({"schemaVersion":5,"kind":"nativeCall","id":1,"method":"masterKey","data":{"create":false,"secret":"forged"}});
    assert!(decode(&serde_json::to_vec(&invalid).unwrap(), &limits()).is_err());
}
