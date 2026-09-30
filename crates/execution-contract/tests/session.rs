use execution_contract::{decode_execution, ExecutionLimits, FrozenExecution};
use serde_json::{json, Value};
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
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/plan.json")).unwrap()
}
fn decode(
    v: &Value,
) -> Result<execution_contract::ExecutionInput, execution_contract::ContractError> {
    decode_execution(&serde_json::to_vec(v).unwrap(), &limits())
}
#[test]
fn session_requirement_is_required_and_bound_to_the_plan() {
    let mut v = fixture();
    v.as_object_mut().unwrap().remove("sessionRequirement");
    assert!(
        decode(&v).is_err(),
        "old plans must not silently omit session requirements"
    );
    v["sessionRequirement"] = json!({"kind":"notRequired"});
    let first = FrozenExecution::freeze(decode(&v).unwrap(), &limits()).unwrap();
    v["sessionRequirement"] = json!({"kind":"activeUser","session":"10", "account":{"platform":"linux","subject":"uid:1000"}});
    let active = FrozenExecution::freeze(decode(&v).unwrap(), &limits()).unwrap();
    assert_ne!(first.digest(), active.digest());
    v["sessionRequirement"]["account"]["subject"] = json!("uid:1001");
    let changed = FrozenExecution::freeze(decode(&v).unwrap(), &limits()).unwrap();
    assert_ne!(changed.digest(), active.digest());
}
#[test]
fn invalid_session_context_is_rejected_by_decode_and_freeze() {
    let mut v = fixture();
    use execution_contract::{ErrorKind as K, Field as F, Rule as R};
    for (session, expected) in [
        (
            json!({"kind":"activeUser","session":"10","account":{"platform":"windows","subject":"sid-1"}}),
            (K::InconsistentContext, F::Session, R::Mismatch),
        ),
        (
            json!({"kind":"activeUser","session":"10","account":{"platform":"linux","subject":""}}),
            (K::InvalidValue, F::Identifier, R::Identifier),
        ),
        (
            json!({"kind":"unknown"}),
            (K::Encoding, F::Document, R::Syntax),
        ),
    ] {
        v["sessionRequirement"] = session;
        let error = decode(&v).unwrap_err();
        assert_eq!((error.kind(), error.field(), error.rule()), expected);
    }
    v["sessionRequirement"] = json!({"kind":"activeUser","session":"10","account":{"platform":"linux","subject":"uid:1000"}});
    let mut p = decode(&v).unwrap();
    p.request.target.platform = execution_contract::Platform::Windows;
    let error = FrozenExecution::freeze(p, &limits()).unwrap_err();
    assert_eq!(
        (error.kind(), error.field(), error.rule()),
        (K::InconsistentContext, F::Session, R::Mismatch)
    );
}

#[test]
fn exact_login_session_is_required_and_bound() {
    let mut value = fixture();
    value["sessionRequirement"] =
        json!({"kind":"activeUser","account":{"platform":"linux","subject":"uid:1000"}});
    assert!(decode(&value).is_err());
    value["sessionRequirement"]["session"] = json!("10");
    let original = FrozenExecution::freeze(decode(&value).unwrap(), &limits()).unwrap();
    value["sessionRequirement"]["session"] = json!("11");
    let changed = FrozenExecution::freeze(decode(&value).unwrap(), &limits()).unwrap();
    assert_ne!(original.digest(), changed.digest());
}
