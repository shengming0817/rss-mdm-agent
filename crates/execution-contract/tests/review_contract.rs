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
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/plan.json")).unwrap()
}
fn decode(v: &Value) -> Result<execution_contract::PlanSpec, execution_contract::ContractError> {
    decode_plan(&serde_json::to_vec(v).unwrap(), &limits())
}
fn freeze(v: &Value) -> FrozenPlan {
    FrozenPlan::freeze(decode(v).unwrap(), &limits()).unwrap()
}
fn windows() -> Value {
    let mut v = fixture();
    v["request"]["target"]["platform"] = json!("windows");
    v["request"]["target"]["scope"]["account"]["platform"] = json!("windows");
    v["runAs"]["account"]["platform"] = json!("windows");
    v["launch"]["cwd"] = json!("C:\\work");
    v["constraints"]["readPaths"] = json!(["C:\\work"]);
    v
}
#[test]
fn windows_environment_collisions_are_rejected_before_freezing() {
    let mut v = windows();
    v["launch"]["env"] =
        json!({"PATH":{"kind":"literal","value":"a"},"Path":{"kind":"literal","value":"b"}});
    assert!(decode(&v).is_err());
    let mut unix = fixture();
    unix["launch"]["env"] = v["launch"]["env"].clone();
    assert!(decode(&unix).is_ok());
}
#[test]
fn unsafe_string_network_destinations_are_not_accepted() {
    for host in [
        "https://allowed.example@169.254.169.254/",
        " example.com",
        "example.com/path",
        "example.com#fragment",
        "example.com?query",
    ] {
        let mut v = fixture();
        v["constraints"]["network"] = json!({"kind":"allowlist","destinations":[host]});
        assert!(decode(&v).is_err(), "{host}");
    }
}
#[test]
fn human_origin_cannot_omit_os_login_provenance() {
    let mut v = fixture();
    v["request"]["initiator"] = json!({"kind":"human"});
    assert!(decode(&v).is_err());
}
#[test]
fn destination_normalization_and_explicit_port_are_part_of_the_digest() {
    let mut v = fixture();
    v["constraints"]["network"] = json!({"kind":"allowlist","destinations":[{"scheme":"https","host":"BÜCHER.Example.","port":443}]});
    let first = freeze(&v);
    v["constraints"]["network"]["destinations"][0]["host"] = json!("xn--bcher-kva.example");
    assert_eq!(freeze(&v).digest(), first.digest());
    v["constraints"]["network"]["destinations"][0]["port"] = json!(8443);
    assert_ne!(freeze(&v).digest(), first.digest());
    v["constraints"]["network"]["destinations"][0]
        .as_object_mut()
        .unwrap()
        .remove("port");
    assert!(decode(&v).is_err());
}

#[test]
fn frozen_environment_uses_target_case_semantics_and_string_values() {
    let mut v = windows();
    v["launch"]["env"] = json!({"Path":{"kind":"literal","value":"C:\\bin"}});
    let mixed = freeze(&v);
    assert_eq!(
        mixed.spec().launch.env.keys().next().unwrap().as_str(),
        "PATH"
    );
    v["launch"]["env"] = json!({"PATH":{"kind":"literal","value":"C:\\bin"}});
    assert_eq!(mixed.digest(), freeze(&v).digest());
    for value in [
        json!(3),
        json!(null),
        json!(true),
        json!(["x"]),
        json!("bad\u{0}value"),
    ] {
        v["launch"]["env"]["PATH"]["value"] = value;
        let error = decode(&v).unwrap_err();
        assert_eq!(error.field(), execution_contract::Field::Environment);
        assert_eq!(error.rule(), execution_contract::Rule::EnvironmentValue);
    }
    for name in ["", "=hidden", "9BAD", "PÄTH", "BAD NAME", "A\u{0}B"] {
        assert!(execution_contract::EnvironmentKey::new(name).is_err());
    }
}

#[test]
fn structured_network_rejects_url_syntax_and_implicit_or_duplicate_endpoints() {
    use execution_contract::{Field, NetworkHost, Rule};
    for host in [
        "",
        "example.com/path",
        "example.com?query",
        "example.com#fragment",
        "allowed.example@169.254.169.254",
        "https://example.com",
        " example.com",
        "example.com ",
        "*.example.com",
        "exa_mple.com",
        "example..com",
        "example.com:443",
        "[fe80::1%25en0]",
        "-bad.example",
        "example.com..",
        "a\\b",
        "example%2ecom",
        "ex\u{0}ample.com",
    ] {
        assert!(NetworkHost::new(host).is_err(), "{host}");
        let mut v = fixture();
        v["constraints"]["network"] =
            json!({"kind":"allowlist","destinations":[{"scheme":"https","host":host,"port":443}]});
        let error = decode(&v).unwrap_err();
        assert_eq!(
            (error.field(), error.rule()),
            (Field::NetworkHost, Rule::HostSyntax)
        );
    }
    let mut v = fixture();
    for endpoints in [
        json!([]),
        json!([{"scheme":"https","host":"a.example","port":0}]),
        json!([{"scheme":"https","host":"a.example","port":65536}]),
        json!([{"scheme":"ftp","host":"a.example","port":21}]),
        json!([{"scheme":"https","host":"A.Example.","port":443},{"scheme":"https","host":"a.example","port":443}]),
    ] {
        v["constraints"]["network"] = json!({"kind":"allowlist","destinations":endpoints});
        assert!(decode(&v).is_err());
    }
    let a = NetworkHost::new("[2001:0DB8:0000:0000:0000:0000:0000:0001]").unwrap();
    assert_eq!(a.as_str(), "[2001:db8::1]");
    assert_eq!(NetworkHost::new(a.as_str()).unwrap(), a);
    assert_eq!(NetworkHost::new("127.1").unwrap().as_str(), "127.0.0.1");
    v["constraints"]["network"] = json!({"kind":"allowlist","destinations":[{"scheme":"https","host":"example.com","port":443}]});
    let original = freeze(&v);
    v["constraints"]["network"]["destinations"][0]["scheme"] = json!("tcp");
    assert_ne!(freeze(&v).digest(), original.digest());
    v["constraints"]["network"]["destinations"][0]["host"] = json!("other.example");
    assert_ne!(freeze(&v).digest(), original.digest());
}

#[test]
fn origin_accounts_are_required_independent_and_bound_in_plan_and_audit() {
    let mut v = fixture();
    let origin = json!({"kind":"ai","provider":"provider-1","conversation":"conversation-1","toolCall":"call-1",
        "osSession":{"device":"origin-device","account":{"platform":"macos","subject":"uid:501"},"session":"login-1"},
        "config":{"id":"profile-1","revision":"1"}});
    v["request"]["initiator"] = origin.clone();
    let original = freeze(&v);
    for (path, replacement) in [
        ("/osSession/device", "different-device"),
        ("/osSession/account/platform", "windows"),
        ("/osSession/account/subject", "different-user"),
        ("/osSession/session", "different-login"),
        ("/config/id", "different-config"),
        ("/config/revision", "2"),
        ("/provider", "different-provider"),
        ("/conversation", "different-conversation"),
        ("/toolCall", "different-call"),
    ] {
        let mut changed = v.clone();
        *changed["request"]["initiator"].pointer_mut(path).unwrap() = json!(replacement);
        assert_ne!(freeze(&changed).digest(), original.digest(), "{path}");
        assert_eq!(changed["request"]["actor"], v["request"]["actor"]);
        assert_eq!(changed["runAs"], v["runAs"]);
    }
    for key in ["osSession", "config"] {
        let mut missing = v.clone();
        missing["request"]["initiator"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(decode(&missing).is_err());
    }
    for key in ["id", "revision"] {
        let mut missing = v.clone();
        missing["request"]["initiator"]["config"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(decode(&missing).is_err());
    }
    let mut audit: Value = serde_json::from_str(include_str!("fixtures/audit.json")).unwrap();
    audit["initiator"] = origin.clone();
    let decoded =
        execution_contract::decode_audit(&serde_json::to_vec(&audit).unwrap(), &limits()).unwrap();
    assert_eq!(serde_json::to_value(decoded.initiator).unwrap(), origin);
}

#[test]
fn new_origin_and_endpoint_shapes_are_required_by_derived_schema() {
    let validator = jsonschema::validator_for(
        &serde_json::to_value(execution_contract::plan_schema()).unwrap(),
    )
    .unwrap();
    let mut v = fixture();
    v["constraints"]["network"] = json!({"kind":"allowlist","destinations":[{"scheme":"https","host":"example.com","port":443}]});
    assert!(validator.is_valid(&v));
    v["constraints"]["network"]["destinations"][0]
        .as_object_mut()
        .unwrap()
        .remove("port");
    assert!(!validator.is_valid(&v));
    v = fixture();
    v["request"]["initiator"] = json!({"kind":"human"});
    assert!(!validator.is_valid(&v));
}
