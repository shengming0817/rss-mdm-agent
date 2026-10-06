use super::*;
#[test]
fn bootstrap_is_current_only_and_never_accepts_inline_secrets() {
    let value = serde_json::json!({"version":1,"origin":"https://agent.example.org","tenant":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","target":"windows-arm64","installation_artifact":"a".repeat(64),"grant":{"operationId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","grant":{"wireVersion":1,"grantId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","state":"available","expiresAt":9999999999i64,"platform":"windows"}}});
    let material: Bootstrap = serde_json::from_value(value.clone()).unwrap();
    material
        .grant
        .validate(rss_mdm_agent_wire::runtime::TaskPlatform::Windows, 0)
        .unwrap();
    assert!(material
        .grant
        .validate(rss_mdm_agent_wire::runtime::TaskPlatform::Macos, 0)
        .is_err());
    assert!(material
        .grant
        .validate(
            rss_mdm_agent_wire::runtime::TaskPlatform::Windows,
            9999999999
        )
        .is_err());
    let mut extra = value.clone();
    extra["secret"] = "CANARY_SECRET".into();
    assert!(serde_json::from_value::<Bootstrap>(extra).is_err());
    let mut old = value;
    old.as_object_mut().unwrap().remove("installation_artifact");
    assert!(serde_json::from_value::<Bootstrap>(old).is_err());
}

#[test]
fn envelope_schema_stays_bound_to_decoder_and_remote_producer() {
    let published: serde_json::Value = serde_json::from_str(include_str!(
        "../../schema/install-bootstrap-v1.schema.json"
    ))
    .unwrap();
    assert_eq!(published, bootstrap_schema().unwrap());
}

#[test]
fn candidate_rejects_previous_version_and_retired_fields() {
    let root = crate::protocol_test_support::Root::new();
    let image = Artifact {
        path: root.path.join("service"),
        sha256: "a".repeat(64),
        cdhash: None,
    };
    let candidate = Candidate {
        version: 2,
        product_version: "test".into(),
        target: Target::current().unwrap(),
        root: root.path.clone(),
        service: image,
        desktop: Artifact {
            path: root.path.join("desktop"),
            sha256: "b".repeat(64),
            cdhash: None,
        },
        runtime_root: root.path.join("runtime"),
        runtime_sha256: "c".repeat(64),
        origin: "https://agent.example.org".into(),
        tenant: uuid::Uuid::new_v4(),
        signing_keys: BTreeMap::from([(
            "key".into(),
            "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE".into(),
        )]),
        ca_pem: Some(String::from_utf8(crate::protocol_test_support::pki::ca()).unwrap()),
        signing_mode: SigningMode::Development,
    };
    let current = serde_json::to_value(&candidate).unwrap();
    let decoded: Candidate = serde_json::from_value(current.clone()).unwrap();
    // Current metadata reaches native artifact checks; this pure fixture has no installed images.
    assert_eq!(decoded.validate(), Err(Error::Identity));
    let mut old = current.clone();
    old["version"] = 1.into();
    let decoded: Candidate = serde_json::from_value(old).unwrap();
    assert_eq!(decoded.validate(), Err(Error::Configuration));
    let mut old_field = current;
    old_field["device_ca_pem"] = "retired-device-ca".into();
    assert!(serde_json::from_value::<Candidate>(old_field).is_err());
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
}
