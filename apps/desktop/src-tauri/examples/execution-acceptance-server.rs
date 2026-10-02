//! Explicit test-only SQLite/MCP assembly. Never linked into desktop production composition.
use execution_contract::RequestId;
use execution_mcp::{ExecutionMcp, McpLimits};
#[path = "execution-acceptance/service.rs"]
mod fixture;
use fixture::FixtureService;
use serde_json::json;
use std::{io::Write, path::PathBuf, time::Duration};
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
    platform_private_storage::directory(&user_root)?;
    let users_path = user_root.join("users.json");
    if !users_path.try_exists()? {
        platform_private_storage::create_new(&users_path)?.write_all(
            serde_json::to_vec(&json!({
            "schemaVersion": 7,
            "kind": "testUserPage",
            "users": [{
                "schemaVersion": 7,
                "kind": "testUser",
                "userId": "fixture-actor",
                "displayName": "Fixture",
                "nameKey": "fixture"
            }],
            "current": {
                "schemaVersion": 7,
                "kind": "userContext",
                "user": {
                    "schemaVersion": 7,
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
    let execution = FixtureService::open(&database)?;
    std::fs::write(&args[1], serde_json::to_vec_pretty(&execution.selection())?)?;
    let limits = McpLimits {
        frame_bytes: 262_144,
        response_bytes: 262_144,
        json_depth: 64,
        json_nodes: 16_384,
        in_flight: 16,
        session_frames: 100_000,
        request_timeout: Duration::from_secs(10),
        io_timeout: Duration::from_secs(60),
    };
    ExecutionMcp::new(execution.clone(), limits)?
        .serve(
            tokio::io::stdin(),
            tokio::io::stdout(),
            CancellationToken::new(),
        )
        .await?;
    let request = RequestId::new(args[2].to_string_lossy().into_owned())?;
    let status = execution.details(&request)?;
    std::fs::write(&args[1], serde_json::to_vec_pretty(&status)?)?;

    Ok(())
}
