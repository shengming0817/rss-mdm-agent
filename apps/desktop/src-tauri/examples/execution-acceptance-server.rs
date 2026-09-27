//! Process-bound acceptance fixture for the real Desktop execution composition and MCP adapter.
use execution_contract::RequestId;
use execution_mcp::{ExecutionMcp, McpLimits};
use rss_mdm_desktop::composition::execution::ExecutionHandle;
use serde_json::json;
use std::{
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "usage: execution-acceptance-server DATABASE AUDIT OPERATION_REQUEST_ID".into(),
        );
    }
    let database = PathBuf::from(&args[0]);
    let user_root = database
        .parent()
        .ok_or("execution database must have a parent")?
        .join("execution-users");
    native_process::private_storage::directory(&user_root)?;
    let users_path = user_root.join("users.json");
    if !users_path.try_exists()? {
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&users_path)?
            .write_all(
                serde_json::to_vec(&json!({
                "schemaVersion": 6,
                "kind": "testUserPage",
                "users": [{
                    "schemaVersion": 6,
                    "kind": "testUser",
                    "userId": "fixture-actor",
                    "displayName": "Fixture",
                    "nameKey": "fixture"
                }],
                "current": {
                    "schemaVersion": 6,
                    "kind": "userContext",
                    "user": {
                        "schemaVersion": 6,
                        "kind": "testUser",
                        "userId": "fixture-actor",
                        "displayName": "Fixture",
                        "nameKey": "fixture"
                    },
                    "generation": "fixture-generation"
                }
                }))?
                .as_slice(),
            )?;
    }
    let users = rss_mdm_desktop::composition::users::Users::open(&user_root)?;
    let generation = users.current()?.generation.to_string();
    std::fs::write(
        database
            .parent()
            .ok_or("execution database must have a parent")?
            .join("execution-user.json"),
        serde_json::to_vec(&json!({ "generation": generation }))?,
    )?;
    let execution = ExecutionHandle::start(&database)?
        .with_trusted_users(Arc::new(Mutex::new(users)))
        .for_caller("fixture-actor")?;
    let catalog = rss_mdm_desktop::composition::execution::catalog()?;
    let input = json!({"catalog":{"selection":{
        "operationRequestId":"ai-unknown", "catalog":catalog.reference(),
        "itemId":"unknown", "variantId":"test", "arguments":{}
    }}});
    std::fs::write(&args[1], serde_json::to_vec_pretty(&input)?)?;
    let limits = McpLimits {
        frame_bytes: 262_144,
        response_bytes: 262_144,
        json_depth: 64,
        json_nodes: 16_384,
        in_flight: 16,
        session_frames: 100_000,
        request_timeout: Duration::from_secs(10),
        io_timeout: Duration::from_secs(60),
        catalog: service_catalog::CatalogLimits {
            max_bytes: 65_536,
            max_depth: 16,
            max_nodes: 4_096,
            max_string_bytes: 4_096,
            max_collection_items: 128,
        },
        parameters: service_catalog::ParameterLimits {
            max_bytes: 4_096,
            max_string_bytes: 1_024,
            max_parameters: 16,
        },
    };
    ExecutionMcp::new(Arc::new(execution.clone()), limits)?
        .serve(
            tokio::io::stdin(),
            tokio::io::stdout(),
            CancellationToken::new(),
        )
        .await?;
    let request = RequestId::new(args[2].to_string_lossy().into_owned())?;
    let status = execution.details(request).await?.status;
    std::fs::write(&args[1], serde_json::to_vec_pretty(&status)?)?;
    execution.close().await;
    Ok(())
}
