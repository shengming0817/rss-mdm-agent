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
