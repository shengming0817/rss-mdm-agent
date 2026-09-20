//! Process-bound acceptance fixture for the real Desktop execution composition and MCP adapter.
use execution_contract::RequestId;
use execution_mcp::{
    CatalogCandidate, ExecutionMcp, ExecutionServicePort, McpLimits, PreviewRequest,
};
use rss_mdm_desktop::composition::execution::ExecutionHandle;
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "usage: execution-acceptance-server DATABASE AUDIT OPERATION_REQUEST_ID".into(),
        );
    }
    let execution =
        ExecutionHandle::start(&PathBuf::from(&args[0]))?.for_caller("fixture-actor")?;
    let catalog = rss_mdm_desktop::composition::execution::catalog()?;
    let selected = catalog.select(
        &serde_json::to_vec(&json!({
            "catalog": catalog.reference(),
            "itemId": "unknown",
            "variantId": "test",
            "arguments": {}
        }))?,
        &service_catalog::CatalogLimits {
            max_bytes: 65_536,
            max_depth: 16,
            max_nodes: 4_096,
            max_string_bytes: 4_096,
            max_collection_items: 128,
        },
        &service_catalog::ParameterLimits {
            max_bytes: 4_096,
            max_string_bytes: 1_024,
            max_parameters: 16,
        },
    )?;
    let metadata = json!({
        "com.rss-mdm/ai-origin": {
            "version": 1,
            "namespace": {
                "tenantId": "test-users",
                "principalId": "fixture-actor",
                "authorityId": "desktop-fixture",
                "sessionId": "conversation-a"
            },
            "operationId": "preflight-preview",
            "provider": "codex",
                        "config": { "id": "local", "revision": "r1" }
        }
    });
    let preview = Arc::new(execution.clone())
        .bind_call(metadata.as_object().expect("static metadata object"))?
        .preview(
            PreviewRequest::Catalog(Box::new(CatalogCandidate {
                operation_request_id: RequestId::new("ai-unknown")?,
                selection: selected,
            })),
            CancellationToken::new(),
        )
        .await?;
    std::fs::write(&args[1], serde_json::to_vec_pretty(&preview.plan)?)?;
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
