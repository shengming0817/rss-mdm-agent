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
fn shared_v2_golden_and_safe_diagnostics() {
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
    assert!(decode(b"\xff", &limits()).is_err());
}
