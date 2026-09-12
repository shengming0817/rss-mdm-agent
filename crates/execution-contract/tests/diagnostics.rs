use execution_contract::{
    decode_audit, decode_plan, ContractError, ErrorKind as K, Field as F, FrozenPlan, PlanLimits,
    Rule as R,
};
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
fn check(error: ContractError, kind: K, field: F, rule: R) {
    assert_eq!(
        (error.kind(), error.field(), error.rule()),
        (kind, field, rule)
    );
    let json = serde_json::to_string(&error).unwrap();
    assert_eq!(serde_json::from_str::<ContractError>(&json).unwrap(), error);
    for text in [json, error.to_string(), format!("{error:?}")] {
        assert!(!text.contains("private"));
    }
    assert!(std::error::Error::source(&error).is_none());
}
#[test]
fn configuration_errors_identify_each_host_bound_before_input_parsing() {
    let l = limits();
    for (bad, field) in [
        (
            PlanLimits {
                max_input_bytes: 0,
                ..l
            },
            F::InputBytes,
        ),
        (PlanLimits { max_depth: 0, ..l }, F::Depth),
        (PlanLimits { max_nodes: 0, ..l }, F::Nodes),
        (
            PlanLimits {
                max_string_bytes: 0,
                ..l
            },
            F::StringBytes,
        ),
        (
            PlanLimits {
                max_collection_items: 0,
                ..l
            },
            F::CollectionItems,
        ),
        (
            PlanLimits {
                max_timeout_ms: 0,
                ..l
            },
            F::Timeout,
        ),
        (
            PlanLimits {
                max_output_bytes: 0,
                ..l
            },
            F::OutputBytes,
        ),
        (
            PlanLimits {
                max_attempts: 0,
                ..l
            },
            F::Attempts,
        ),
    ] {
        check(
            decode_plan(b"private malformed", &bad).unwrap_err(),
            K::InvalidConfiguration,
            field,
            R::NonZero,
        );
        check(
            decode_audit(b"private malformed", &bad).unwrap_err(),
            K::InvalidConfiguration,
            field,
            R::NonZero,
        );
    }
    check(
        decode_plan(b"{", &PlanLimits { max_depth: 65, ..l }).unwrap_err(),
        K::InvalidConfiguration,
        F::Depth,
        R::NumericRange,
    );
}
#[test]
fn input_limits_and_budget_limits_are_distinct_from_configuration() {
    let source = include_bytes!("fixtures/plan.json");
    let l = limits();
    for (bound, kind, field, rule) in [
        (
            PlanLimits {
                max_input_bytes: 1,
                ..l
            },
            K::LimitExceeded,
            F::InputBytes,
            R::ByteLimit,
        ),
        (
            PlanLimits { max_depth: 1, ..l },
            K::LimitExceeded,
            F::Depth,
            R::DepthLimit,
        ),
        (
            PlanLimits { max_nodes: 1, ..l },
            K::LimitExceeded,
            F::Nodes,
            R::NodeLimit,
        ),
        (
            PlanLimits {
                max_string_bytes: 63,
                ..l
            },
            K::LimitExceeded,
            F::StringBytes,
            R::ByteLimit,
        ),
        (
            PlanLimits {
                max_collection_items: 1,
                ..l
            },
            K::LimitExceeded,
            F::CollectionItems,
            R::CollectionLimit,
        ),
        (
            PlanLimits {
                max_timeout_ms: 999,
                ..l
            },
            K::InvalidBudget,
            F::Timeout,
            R::BudgetLimit,
        ),
        (
            PlanLimits {
                max_output_bytes: 4095,
                ..l
            },
            K::InvalidBudget,
            F::OutputBytes,
            R::BudgetLimit,
        ),
    ] {
        check(decode_plan(source, &bound).unwrap_err(), kind, field, rule);
    }
    for (pointer, value, field, rule) in [
        ("/budget/maxAttempts", json!(0), F::Attempts, R::NonZero),
        ("/budget/maxAttempts", json!(4), F::Attempts, R::BudgetLimit),
        (
            "/validity/expiresAtUnixMs",
            json!(0),
            F::Validity,
            R::TimeOrder,
        ),
    ] {
        let mut v = fixture();
        *v.pointer_mut(pointer).unwrap() = value;
        let plan = serde_json::from_value(v.clone()).unwrap();
        check(
            FrozenPlan::freeze(plan, &l).unwrap_err(),
            K::InvalidBudget,
            field,
            rule,
        );
        check(
            decode_plan(&serde_json::to_vec(&v).unwrap(), &l).unwrap_err(),
            K::InvalidBudget,
            field,
            rule,
        );
    }
}
#[test]
fn semantic_errors_survive_serde_without_disclosing_rejected_values() {
    for (path, value, kind, field, rule) in [
        (
            "/schemaVersion",
            json!(999),
            K::UnsupportedVersion,
            F::Version,
            R::Version,
        ),
        (
            "/request/actor",
            json!("private invalid actor"),
            K::InvalidValue,
            F::Actor,
            R::Identifier,
        ),
        (
            "/launch/cwd",
            json!("private/relative"),
            K::InvalidValue,
            F::WorkingDirectory,
            R::AbsolutePath,
        ),
        (
            "/launch/argv",
            json!(["private\u{0}argument"]),
            K::InvalidValue,
            F::Arguments,
            R::Nul,
        ),
        (
            "/runAs/account/platform",
            json!("windows"),
            K::InconsistentContext,
            F::RunAs,
            R::Mismatch,
        ),
        (
            "/request/target/scope/account/platform",
            json!("windows"),
            K::InconsistentContext,
            F::Target,
            R::Mismatch,
        ),
        (
            "/launch/artifact/sha256",
            json!("private-digest"),
            K::InvalidValue,
            F::Digest,
            R::Digest,
        ),
    ] {
        let mut v = fixture();
        *v.pointer_mut(path).unwrap() = value;
        check(
            decode_plan(&serde_json::to_vec(&v).unwrap(), &limits()).unwrap_err(),
            kind,
            field,
            rule,
        );
    }
    for raw in [
        b"{\"private\":1,\"private\":2}".as_slice(),
        b"{\"private\":{\"private\":1,\"private\":2}}",
    ] {
        check(
            decode_plan(raw, &limits()).unwrap_err(),
            K::Encoding,
            F::Document,
            R::DuplicateKey,
        );
    }
    let mut v = fixture();
    v["private-unknown"] = json!("private payload");
    check(
        decode_plan(&serde_json::to_vec(&v).unwrap(), &limits()).unwrap_err(),
        K::Encoding,
        F::Document,
        R::Syntax,
    );
    check(
        decode_plan(b"{private", &limits()).unwrap_err(),
        K::Encoding,
        F::Document,
        R::Syntax,
    );
}
#[test]
fn audit_diagnostics_preserve_evidence_rules_and_version() {
    let original: Value = serde_json::from_str(include_str!("fixtures/audit.json")).unwrap();
    for (path, value, kind, field, rule) in [
        (
            "/schemaVersion",
            json!(999),
            K::UnsupportedVersion,
            F::Version,
            R::Version,
        ),
        (
            "/decision/evidence",
            json!([]),
            K::InvalidValue,
            F::AuditEvidence,
            R::Empty,
        ),
        (
            "/decision/evidence/0/kind",
            json!("processExited"),
            K::InconsistentContext,
            F::AuditEvidence,
            R::TestEvidence,
        ),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        check(
            decode_audit(&serde_json::to_vec(&v).unwrap(), &limits()).unwrap_err(),
            kind,
            field,
            rule,
        );
    }
}
