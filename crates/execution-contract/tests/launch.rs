use execution_contract::{decode_plan, FrozenPlan, PlanLimits};
use serde_json::{json, Value};

fn limits() -> PlanLimits {
    PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 65536,
        max_stdin_bytes: 65536,
        max_attempts: 3,
    }
}
fn complete() -> Value {
    let mut v: Value = serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    let artifact = v["launch"]["artifact"].clone();
    v["launch"]["interpreter"] =
        json!({"artifact": artifact, "profile":{"id":"test-file","revision":"1"}});
    v["launch"]["argv"] = json!([{"kind":"artifactPath"}, {"kind":"literal","value":"--host"}, {"kind":"literal","value":"example.invalid"}]);
    v["launch"]["artifactEncoding"] = json!("utf8");
    v["launch"]["stdin"] = json!({"kind":"controlled","reference":{"id":"input-1","revision":"1"},"encoding":"utf8","maxBytes":64});
    v["launch"]["output"] = json!({"stdout":"utf8","stderr":"utf8"});
    v
}
fn freeze(v: &Value) -> FrozenPlan {
    FrozenPlan::freeze(
        decode_plan(&serde_json::to_vec(v).unwrap(), &limits()).unwrap(),
        &limits(),
    )
    .unwrap()
}
#[test]
fn complete_launch_keeps_v1_and_binds_every_new_requirement() {
    let v = complete();
    assert_eq!(v["schemaVersion"], 1);
    let original = freeze(&v);
    for (pointer, value) in [
        ("/launch/interpreter/profile/revision", json!("2")),
        ("/launch/artifactEncoding", json!("utf8Bom")),
        ("/launch/stdin/reference/revision", json!("2")),
        ("/launch/stdin/maxBytes", json!(65)),
        ("/launch/stdin/encoding", json!("utf16Le")),
        ("/launch/output/stdout", json!("utf16Le")),
        ("/launch/output/stderr", json!("utf16Le")),
    ] {
        let mut changed = v.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert_ne!(original.digest(), freeze(&changed).digest(), "{pointer}");
    }
}
#[test]
fn launch_rejects_missing_requirements_legacy_argv_and_bad_slot_counts() {
    for field in ["stdin", "output", "artifactEncoding"] {
        let mut v = complete();
        v["launch"].as_object_mut().unwrap().remove(field);
        assert!(decode_plan(&serde_json::to_vec(&v).unwrap(), &limits()).is_err());
    }
    for argv in [
        json!(["legacy"]),
        json!([]),
        json!([{"kind":"literal","value":"no-slot"}]),
        json!([{"kind":"artifactPath"},{"kind":"artifactPath"}]),
    ] {
        let mut v = complete();
        v["launch"]["argv"] = argv;
        assert!(decode_plan(&serde_json::to_vec(&v).unwrap(), &limits()).is_err());
    }
    let mut v = complete();
    v["launch"]["stdin"]["maxBytes"] = json!(0);
    assert!(decode_plan(&serde_json::to_vec(&v).unwrap(), &limits()).is_err());
    v["launch"]["stdin"]["maxBytes"] = json!(65537);
    assert!(decode_plan(&serde_json::to_vec(&v).unwrap(), &limits()).is_err());
}
