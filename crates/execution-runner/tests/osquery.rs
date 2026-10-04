//! Real controlled osquery seam; provide an independently verified, platform-matching binary.
use execution_contract::{InputValue, Platform};
use execution_runner::osquery;
use std::collections::BTreeMap;
#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
async fn query(template: &str, parameters: BTreeMap<String, InputValue>) -> serde_json::Value {
    let binary =
        std::env::var_os("OSQUERY_TEST_BINARY").expect("verified OSQUERY_TEST_BINARY required");
    let args = osquery::arguments(template.as_bytes(), &parameters, 2, Platform::Macos).unwrap();
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(binary)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(output.status.success(), "osquery failed: {}", output.status);
    assert!(output.stdout.len() < 4096, "unexpected output budget");
    serde_json::from_slice(&output.stdout).unwrap()
}
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires verified official osquery binary"]
async fn published_query_literal_binding_and_zero_rows_use_real_osquery() {
    let value = query("SELECT version FROM osquery_info", BTreeMap::new()).await;
    assert_eq!(value.as_array().unwrap().len(), 1);
    assert!(!value[0]["version"].as_str().unwrap().is_empty());
    let injected = query(
        "SELECT version FROM osquery_info WHERE version = :version",
        [(
            "version".into(),
            InputValue::Literal {
                value: serde_json::json!("' OR 1=1 --"),
            },
        )]
        .into(),
    )
    .await;
    assert_eq!(injected, serde_json::json!([]));
}

#[test]
fn bound_untrusted_text_remains_one_ast_literal() {
    use sqlparser::{ast::*, dialect::SQLiteDialect, parser::Parser};
    let text = "'; SELECT version FROM osquery_info; -- 设备";
    let parameters = [("value".into(), InputValue::Literal { value: text.into() })].into();
    let args = osquery::arguments(
        b"SELECT version FROM osquery_info WHERE version = :value",
        &parameters,
        2,
        Platform::Macos,
    )
    .unwrap();
    let statements = Parser::parse_sql(&SQLiteDialect {}, args.last().unwrap()).unwrap();
    assert_eq!(statements.len(), 1);
    let Statement::Query(query) = &statements[0] else {
        panic!("expected SELECT");
    };
    let SetExpr::Select(select) = query.body.as_ref() else {
        panic!("expected one SELECT");
    };
    let Some(Expr::BinaryOp { op, right, .. }) = &select.selection else {
        panic!("expected equality predicate");
    };
    assert_eq!(op, &BinaryOperator::Eq);
    let Expr::Value(value) = right.as_ref() else {
        panic!("untrusted value escaped its literal");
    };
    assert_eq!(value.value, Value::SingleQuotedString(text.into()));
}

#[test]
fn missing_and_extra_parameters_are_rejected() {
    let template = b"SELECT version FROM osquery_info WHERE version = :value";
    let value = InputValue::Literal {
        value: "1.0".into(),
    };
    for parameters in [
        BTreeMap::new(),
        [("other".into(), value.clone())].into(),
        [
            ("value".into(), value.clone()),
            ("extra".into(), value.clone()),
        ]
        .into(),
    ] {
        assert!(osquery::arguments(template, &parameters, 2, Platform::Macos).is_err());
    }
    assert!(osquery::arguments(
        template,
        &[("value".into(), value)].into(),
        2,
        Platform::Macos,
    )
    .is_ok());
}

#[test]
fn platform_tables_and_prohibited_structures_are_rejected() {
    let empty = BTreeMap::new();
    for (sql, allowed, denied) in [
        ("SELECT name FROM apps", Platform::Macos, Platform::Windows),
        (
            "SELECT name FROM programs",
            Platform::Windows,
            Platform::Macos,
        ),
    ] {
        assert!(osquery::arguments(sql.as_bytes(), &empty, 2, allowed).is_ok());
        assert!(osquery::arguments(sql.as_bytes(), &empty, 2, denied).is_err());
    }
    assert!(matches!(
        osquery::arguments(
            b"SELECT version FROM osquery_info",
            &empty,
            2,
            Platform::Linux
        ),
        Err(execution_app::Error::Unsupported)
    ));
    for sql in [
        "SELECT version FROM osquery_info LIMIT 10",
        "SELECT version FROM osquery_info; SELECT version FROM osquery_info",
        "SELECT * FROM osquery_info",
        "SELECT version FROM osquery_info JOIN os_version ON 1=1",
        "SELECT version FROM osquery_info WHERE version IN (SELECT version FROM os_version)",
        "DELETE FROM osquery_info",
    ] {
        assert!(
            osquery::arguments(sql.as_bytes(), &empty, 2, Platform::Macos).is_err(),
            "{sql}"
        );
    }
}
