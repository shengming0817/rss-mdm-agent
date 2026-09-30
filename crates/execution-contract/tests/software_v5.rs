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
fn v5_requires_explicit_execution_kind_and_rejects_old_plans() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 5.into();
    value["execution"] = serde_json::json!({"kind":"process"});
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_ok());
    value["schemaVersion"] = 2.into();
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
    value["schemaVersion"] = 5.into();
    value.as_object_mut().unwrap().remove("execution");
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}

#[test]
fn process_cannot_impersonate_software_operation() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/plan.json")).unwrap();
    value["schemaVersion"] = 5.into();
    value["execution"] = serde_json::json!({"kind":"process"});
    value["request"]["operation"]["action"] = "software.install".into();
    assert!(decode_execution(&serde_json::to_vec(&value).unwrap(), &limits()).is_err());
}

#[test]
fn software_steps_bind_order_materials_and_context_without_legacy_parser() {
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
    for field in ["package", "architecture"] {
        let mut changed = value.clone();
        changed["execution"]["program"]["steps"][0][field] = if field == "architecture" {
            "x86_64"
        } else {
            "another-package"
        }
        .into();
        assert_ne!(original.digest(), freeze(&changed).digest());
    }
    let mut changed = value.clone();
    changed["execution"]["program"]["steps"][0]["install"]["launch"]["artifact"]["sha256"] =
        "ab".repeat(32).into();
    assert_ne!(original.digest(), freeze(&changed).digest());
    let steps = changed["execution"]["program"]["steps"]
        .as_array_mut()
        .unwrap();
    let mut other = steps[0].clone();
    other["package"] = "second-package".into();
    steps.push(other);
    let ordered = freeze(&changed);
    changed["execution"]["program"]["steps"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_ne!(ordered.digest(), freeze(&changed).digest());
    assert!(
        decode_execution(include_bytes!("fixtures/software-obsolete.json"), &limits()).is_err()
    );
    for replacement in [serde_json::json!([]), serde_json::json!([{}])] {
        let mut changed = value.clone();
        changed["execution"]["program"]["steps"] = replacement;
        assert!(decode_execution(&serde_json::to_vec(&changed).unwrap(), &limits()).is_err());
    }
}
#[test]
fn journal_history_cannot_skip_unknown_steps_or_rewrite_exits() {
    use execution_contract::*;
    let plan = FrozenExecution::freeze(
        decode_execution(include_bytes!("fixtures/software.json"), &limits()).unwrap(),
        &limits(),
    )
    .unwrap();
    let mut progress = SoftwareProgress {
        attempt_id: AttemptId::new("attempt").unwrap(),
        content_digest: plan.digest().clone(),
        runner: Id::new("runner").unwrap(),
        checkpoints: vec![],
        elapsed_ms: 1,
        output_bytes: 0,
    };
    progress.checkpoints.push(SoftwareCheckpoint::Begin {
        step: 0,
        phase: SoftwarePhase::Before,
    });
    assert!(progress.valid_for(&plan));
    let begun = progress.clone();
    progress.checkpoints.push(SoftwareCheckpoint::End {
        step: 0,
        phase: SoftwarePhase::Before,
        process: None,
        detected: Some(SoftwareState::Unknown {
            reason: SoftwareDetectionFailure::Unavailable,
        }),
        quiescent: false,
    });
    assert!(progress.valid_for(&plan));
    assert!(progress.extends(&begun));
    assert!(!progress.complete(&plan));
    progress
        .checkpoints
        .push(SoftwareCheckpoint::Complete { step: 0 });
    assert!(!progress.valid_for(&plan));
    progress.checkpoints.clear();
    assert!(!progress.extends(&begun));
}
