//! Real controlled osquery seam; provide an independently verified, platform-matching binary.
#![cfg(target_os = "macos")]
use execution_contract::{InputValue, Platform};
use execution_runner::osquery;
use std::{collections::BTreeMap, time::Duration};
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
