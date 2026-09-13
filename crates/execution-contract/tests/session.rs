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
        max_attempts: 3,
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/plan.json")).unwrap()
}
fn decode(v: &Value) -> Result<execution_contract::PlanSpec, execution_contract::ContractError> {
    decode_plan(&serde_json::to_vec(v).unwrap(), &limits())
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
    let first = FrozenPlan::freeze(decode(&v).unwrap(), &limits()).unwrap();
    v["sessionRequirement"] =
        json!({"kind":"activeUser", "account":{"platform":"linux","subject":"uid:1000"}});
    let active = FrozenPlan::freeze(decode(&v).unwrap(), &limits()).unwrap();
    assert_ne!(first.digest(), active.digest());
    v["sessionRequirement"]["account"]["subject"] = json!("uid:1001");
    let changed = FrozenPlan::freeze(decode(&v).unwrap(), &limits()).unwrap();
    assert_ne!(changed.digest(), active.digest());
}
#[test]
fn invalid_session_context_is_rejected_by_decode_and_freeze() {
    let mut v = fixture();
    for session in [
        json!({"kind":"activeUser","account":{"platform":"windows","subject":"sid-1"}}),
        json!({"kind":"activeUser","account":{"platform":"linux","subject":""}}),
        json!({"kind":"unknown"}),
    ] {
        v["sessionRequirement"] = session;
        assert!(decode(&v).is_err());
    }
    v["sessionRequirement"] =
        json!({"kind":"activeUser","account":{"platform":"linux","subject":"uid:1000"}});
    let mut p = decode(&v).unwrap();
    p.request.target.platform = execution_contract::Platform::Windows;
    assert!(FrozenPlan::freeze(p, &limits()).is_err());
}
