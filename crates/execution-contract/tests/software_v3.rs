use execution_contract::{decode_execution, ExecutionLimits};

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
fn v3_requires_explicit_execution_kind_and_rejects_old_plans() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 4.into();
    value["execution"] = serde_json::json!({"kind":"process"});
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_ok());
    value["schemaVersion"] = 2.into();
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
    value["schemaVersion"] = 4.into();
    value.as_object_mut().unwrap().remove("execution");
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}

#[test]
fn process_cannot_impersonate_software_operation() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 4.into();
    value["execution"] = serde_json::json!({"kind":"process"});
    value["request"]["operation"]["action"] = "software.install".into();
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}

#[test]
fn software_semantics_are_typed_bound_and_not_process_parameters() {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/software.json")).unwrap();
    let freeze = |v: &serde_json::Value| {
        execution_contract::FrozenExecution::freeze(
            decode_execution(&serde_json::to_vec(v).unwrap(), &limits()).unwrap(),
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
    changed["execution"]["software"]["installer"]["artifact"]["sha256"] = "34".repeat(32).into();
    assert_ne!(original.digest(), freeze(&changed).digest());
    for (field, replacement) in [
        ("execution", serde_json::json!({"kind":"process"})),
        ("schemaVersion", serde_json::json!(2)),
    ] {
        let mut changed = value.clone();
        changed[field] = replacement;
        assert!(decode_execution(&serde_json::to_vec(&changed).unwrap(), &limits()).is_err());
    }
    let mut changed = value;
    changed["request"]["parameters"] =
        serde_json::json!({"software":{"kind":"literal","value":{}}});
    assert!(decode_execution(&serde_json::to_vec(&changed).unwrap(), &limits()).is_err());
}

#[test]
fn installer_capabilities_are_bound_and_unsupported_semantics_fail_closed() {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/software.json")).unwrap();
    let decode =
        |v: &serde_json::Value| decode_execution(&serde_json::to_vec(v).unwrap(), &limits());
    let original =
        execution_contract::FrozenExecution::freeze(decode(&v).unwrap(), &limits()).unwrap();
    let mut changed = v.clone();
    changed["execution"]["software"]["installer"]["restart"] = "never".into();
    assert_ne!(
        original.digest(),
        execution_contract::FrozenExecution::freeze(decode(&changed).unwrap(), &limits())
            .unwrap()
            .digest()
    );
    for (field, value) in [
        ("restart", serde_json::json!("automatic")),
        ("operations", serde_json::json!(["uninstall"])),
        ("upgradeStrategy", serde_json::json!("uninstallThenInstall")),
    ] {
        let mut changed = v.clone();
        changed["execution"]["software"]["installer"][field] = value;
        assert!(decode(&changed).is_err(), "{field}");
    }
}

#[test]
fn software_wire_is_camel_case_and_physical_binding_is_covered_by_digest() {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/software.json")).unwrap();
    let freeze = |v: &serde_json::Value| {
        execution_contract::FrozenExecution::freeze(
            decode_execution(&serde_json::to_vec(v).unwrap(), &limits()).unwrap(),
            &limits(),
        )
        .unwrap()
    };
    let original = freeze(&value);
    for field in ["parent", "object"] {
        let mut changed = value.clone();
        changed["execution"]["software"]["resourceBinding"][field] = "other-object".into();
        assert_ne!(original.digest(), freeze(&changed).digest());
    }
    let mut old = value.clone();
    old["execution"]["software"]["mutation"] = "Install".into();
    assert!(decode_execution(&serde_json::to_vec(&old).unwrap(), &limits()).is_err());
    let mut old = value;
    let installer = old["execution"]["software"]["installer"]
        .as_object_mut()
        .unwrap();
    let detect = installer.remove("canDetect").unwrap();
    installer.insert("can_detect".into(), detect);
    assert!(decode_execution(&serde_json::to_vec(&old).unwrap(), &limits()).is_err());
}
