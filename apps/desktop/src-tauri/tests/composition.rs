use execution_mcp::ExecutionServicePort;
use rss_mdm_desktop::composition::{execution::ExecutionHandle, users::Users};
use std::{path::PathBuf, sync::Arc};
fn directory() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let p = std::env::temp_dir().join(format!(
        "rss-composition-{}-{}-{}",
        std::process::id(),
        rss_mdm_desktop::composition::execution::now().unwrap(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    platform_private_storage::directory(&p).unwrap();
    p.canonicalize().unwrap()
}
#[test]
fn production_execution_consumer_does_not_initialize_a_private_journal() {
    let root = directory();
    let users = Users::open(&root).unwrap();
    let handle = ExecutionHandle::new().with_trusted_users(Arc::new(std::sync::Mutex::new(users)));
    assert!(handle.check_binding().is_ok());
    assert!(!root.join("execution.sqlite").exists());
    assert!(ExecutionHandle::new().check_binding().is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn old_desktop_script_and_catalog_inputs_are_not_backend_task_references() {
    for value in [
        serde_json::json!({"sourceUtf8":"echo old","operationRequestId":"old"}),
        serde_json::json!({"catalog":{},"itemId":"old","variantId":"old","fields":{}}),
    ] {
        assert!(serde_json::from_value::<execution_contract::BackendSelection>(value).is_err());
    }
}
