use execution_contract::{decode_plan, PlanLimits};

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

#[test]
fn v3_requires_explicit_execution_kind_and_rejects_old_plans() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 3.into();
    value["execution"] = serde_json::json!({"kind":"process"});
    assert!(decode_plan(&serde_json::to_vec(&value).unwrap(), &limits()).is_ok());
    value["schemaVersion"] = 2.into();
    assert!(decode_plan(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
    value["schemaVersion"] = 3.into();
    value.as_object_mut().unwrap().remove("execution");
    assert!(decode_plan(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}

#[test]
fn process_cannot_impersonate_software_operation() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 3.into();
    value["execution"] = serde_json::json!({"kind":"process"});
    value["request"]["operation"]["action"] = "software.install".into();
    assert!(decode_plan(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}

#[test]
fn software_semantics_are_typed_bound_and_not_process_parameters() {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/software.json")).unwrap();
    let freeze = |v: &serde_json::Value| {
        execution_contract::FrozenPlan::freeze(
            decode_plan(&serde_json::to_vec(v).unwrap(), &limits()).unwrap(),
            &limits(),
        )
        .unwrap()
    };
    let original = freeze(&value);
    for field in ["source", "resource"] {
        let mut changed = value.clone();
        changed["execution"]["software"][field] = "different".into();
        assert_ne!(original.digest(), freeze(&changed).digest());
    }
    let mut changed = value.clone();
    changed["execution"]["software"]["manager"]["sha256"] = "34".repeat(32).into();
    assert_ne!(original.digest(), freeze(&changed).digest());
    for (field, replacement) in [
        ("execution", serde_json::json!({"kind":"process"})),
        ("schemaVersion", serde_json::json!(2)),
    ] {
        let mut changed = value.clone();
        changed[field] = replacement;
        assert!(decode_plan(&serde_json::to_vec(&changed).unwrap(), &limits()).is_err());
    }
    let mut changed = value;
    changed["request"]["parameters"] =
        serde_json::json!({"software":{"kind":"literal","value":{}}});
    assert!(decode_plan(&serde_json::to_vec(&changed).unwrap(), &limits()).is_err());
}
