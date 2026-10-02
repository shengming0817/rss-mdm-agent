use execution_contract::{decode_execution, ExecutionLimits, FrozenExecution};

fn limits() -> ExecutionLimits {
    ExecutionLimits {
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

#[test]
fn explicit_os_identity_requires_v6_and_changes_the_frozen_digest() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 6.into();
    value["constraints"]["kind"] = "restricted".into();
    value["launch"]["output"]["format"] = serde_json::json!({"kind":"text"});
    let restricted = FrozenExecution::freeze(
        decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap(),
        &limits(),
    )
    .unwrap();
    value["constraints"] = serde_json::json!({"kind":"osIdentity"});
    let native = FrozenExecution::freeze(
        decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).unwrap(),
        &limits(),
    )
    .unwrap();
    assert_ne!(restricted.digest(), native.digest());
    value["schemaVersion"] = 1.into();
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
    value["schemaVersion"] = 6.into();
    value["constraints"] = serde_json::json!({"kind":"automatic"});
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}
