//! Read-only browser catalogue from the actual S1 SQLite composition, without fake executions.
use rss_mdm_desktop::{composition::execution::ExecutionHandle, self_service::SnapshotQuery};
#[tokio::main]
async fn main() {
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("rss-browser-catalog-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let handle = ExecutionHandle::start(&root.join("execution.db")).unwrap();
    let caller = handle.for_caller("browser-catalog").unwrap();
    let snapshot = caller.snapshot(SnapshotQuery::default()).await.unwrap();
    println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
    handle.close().await;
    std::fs::remove_dir_all(root).unwrap();
}
