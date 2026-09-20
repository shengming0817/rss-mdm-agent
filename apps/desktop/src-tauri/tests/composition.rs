use execution_contract::{Initiator, RequestId};
use execution_mcp::{
    CatalogCandidate, ExecutionServicePort, OperationRequest, PreviewRequest, SubmitRequest,
};
use rss_mdm_desktop::{
    composition::{
        execution::{ExecutionHandle, BINDING},
        origin::AiBinding,
    },
    self_service as ui,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;
fn binding() -> AiBinding {
    AiBinding::for_user("fixture-actor").unwrap()
}
fn directory() -> PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let p = std::env::temp_dir().join(format!(
        "rss-composition-{}-{}-{}",
        std::process::id(),
        rss_mdm_desktop::composition::execution::now().unwrap(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
    p.canonicalize().unwrap()
}
fn bound(handle: &ExecutionHandle, session: &str, operation: &str) -> Arc<ExecutionHandle> {
    Arc::new(handle.clone()).bind_call(json!({"com.rss-mdm/ai-origin":{"version":1,"namespace":{"tenantId":"test-users","principalId":"fixture-actor","authorityId":"desktop-fixture","sessionId":session},"operationId":operation,"provider":"codex","config":{"id":"local","revision":"r1"}}}).as_object().unwrap()).unwrap()
}
async fn draft(handle: &ExecutionHandle, request: &str, item: &str) -> ui::PlanView {
    let snapshot = handle.snapshot(Default::default()).await.unwrap();
    let item = snapshot
        .catalog
        .into_iter()
        .find(|i| i.item_id.as_str() == item)
        .unwrap();
    handle
        .preview_ui(ui::Draft {
            instance_id: BINDING.into(),
            request_id: RequestId::new(request).unwrap(),
            revision: 1,
            catalog: item.catalog,
            item_id: item.item_id,
            variant_id: item.variant_id,
            fields: Default::default(),
        })
        .await
        .unwrap()
}
fn submission(plan: &ui::PlanView) -> ui::Submission {
    ui::Submission {
        instance_id: BINDING.into(),
        request_id: plan.request_id.clone(),
        plan_id: plan.plan_id.clone(),
        digest: plan.digest.clone(),
    }
}
#[tokio::test]
async fn ai_cannot_preview_submit_read_or_cancel_a_human_request() {
    let root = directory();
    let handle = ExecutionHandle::start(&root.join("execution.sqlite"))
        .unwrap()
        .for_caller(binding().caller.principal_id.as_str())
        .unwrap();
    let plan = draft(&handle, "human-private", "office").await;
    let details = handle.details(plan.request_id.clone()).await.unwrap();
    let ai = bound(&handle, "conversation-a", "foreign-access");
    let request = || OperationRequest {
        operation_request_id: plan.request_id.clone(),
    };
    let denied = vec![
        ai.preview(
            PreviewRequest::Candidate {
                operation_request_id: plan.request_id.clone(),
                candidate: details.plan.artifact,
            },
            CancellationToken::new(),
        )
        .await
        .err(),
        ai.submit(
            SubmitRequest {
                operation_request_id: plan.request_id.clone(),
                plan: execution_mcp::PlanRef {
                    plan_id: plan.plan_id.clone(),
                    digest: plan.digest.clone(),
                },
            },
            CancellationToken::new(),
        )
        .await
        .err(),
        ai.status(request(), CancellationToken::new()).await.err(),
        ai.cancel(request(), CancellationToken::new()).await.err(),
    ];
    handle.close().await;
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(denied, vec![Some(execution_mcp::ServiceError::Denied); 4]);
}
#[tokio::test]
async fn shared_durable_service_distinguishes_preview_submission_approval_and_replay() {
    let root = directory();
    let path = root.join("execution.sqlite");
    let handle = ExecutionHandle::start(&path)
        .unwrap()
        .for_caller(binding().caller.principal_id.as_str())
        .unwrap();
    let plan = draft(&handle, "human-office", "office").await;
    let before = handle.details(plan.request_id.clone()).await.unwrap();
    assert!(!before.status.submitted);
    assert_eq!(before.status.attempts, 0);
    assert!(matches!(before.plan.initiator, Initiator::Human { .. }));
    assert!(handle
        .snapshot(Default::default())
        .await
        .unwrap()
        .requests
        .is_empty());
    assert_eq!(
        handle.submit_ui(submission(&plan)).await.unwrap().status,
        ui::RequestStatus::Approval
    );
    assert!(
        handle
            .details(plan.request_id.clone())
            .await
            .unwrap()
            .status
            .submitted
    );
    let mut stale = submission(&plan);
    stale.digest = execution_contract::Digest::new("0".repeat(64)).unwrap();
    assert!(handle.approve_ui(stale).await.is_err());
    handle.approve_ui(submission(&plan)).await.unwrap();
    handle.approve_ui(submission(&plan)).await.unwrap();
    assert_eq!(
        handle
            .details(plan.request_id.clone())
            .await
            .unwrap()
            .status
            .attempts,
        1
    );
    tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
    assert_eq!(
        handle
            .details(plan.request_id.clone())
            .await
            .unwrap()
            .status
            .phase,
        execution_app::TaskPhase::TestCompleted
    );
    handle.close().await;
    let restored = ExecutionHandle::start(&path)
        .unwrap()
        .for_caller(binding().caller.principal_id.as_str())
        .unwrap();
    restored.submit_ui(submission(&plan)).await.unwrap();
    assert_eq!(
        restored
            .details(plan.request_id)
            .await
            .unwrap()
            .status
            .attempts,
        1
    );
    restored.close().await;
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn ai_origin_is_host_bound_and_recovery_never_redispatches_unknown_attempts() {
    let root = directory();
    let path = root.join("execution.sqlite");
    let handle = ExecutionHandle::start(&path)
        .unwrap()
        .for_caller(binding().caller.principal_id.as_str())
        .unwrap();
    assert!(Arc::new(handle.clone())
        .bind_call(
            json!({"actor":"admin","approved":true})
                .as_object()
                .unwrap()
        )
        .is_err());
    let valid = json!({"version":1,"namespace":{"tenantId":"test-users","principalId":"fixture-actor","authorityId":"desktop-fixture","sessionId":"conversation-a"},"operationId":"preview-delivery","provider":"codex","config":{"id":"local","revision":"r1"}});
    for pointer in [
        "/namespace/tenantId",
        "/namespace/principalId",
        "/namespace/authorityId",
        "/provider",
    ] {
        let mut changed = valid.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("foreign");
        assert!(
            Arc::new(handle.clone())
                .bind_call(
                    json!({"com.rss-mdm/ai-origin":changed})
                        .as_object()
                        .unwrap()
                )
                .is_err(),
            "{pointer}"
        );
    }
    let mut forged = valid.clone();
    forged["approved"] = json!(true);
    assert!(Arc::new(handle.clone())
        .bind_call(json!({"com.rss-mdm/ai-origin":forged}).as_object().unwrap())
        .is_err());
    let ai = bound(&handle, "conversation-a", "preview-delivery");
    let catalog = ai.catalog(None, CancellationToken::new()).await.unwrap();
    let selected=catalog.select(&serde_json::to_vec(&json!({"catalog":catalog.reference(),"itemId":"unknown","variantId":"test","arguments":{}})).unwrap(), &service_catalog::CatalogLimits { max_bytes:262144,max_depth:32,max_nodes:16384,max_string_bytes:16384,max_collection_items:128 }, &service_catalog::ParameterLimits { max_bytes:16384,max_string_bytes:4096,max_parameters:32 }).unwrap();
    let request = RequestId::new("ai-unknown").unwrap();
    let preview = ai
        .preview(
            PreviewRequest::Catalog(Box::new(CatalogCandidate {
                operation_request_id: request.clone(),
                selection: selected,
            })),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    // The trusted Host can name another connection, but it cannot use that origin to read an old task.
    for pointer in ["/config/id", "/config/revision"] {
        let mut changed = valid.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("foreign");
        let other = Arc::new(handle.clone())
            .bind_call(
                json!({"com.rss-mdm/ai-origin":changed})
                    .as_object()
                    .unwrap(),
            )
            .unwrap();
        assert!(other
            .status(
                OperationRequest {
                    operation_request_id: request.clone()
                },
                CancellationToken::new()
            )
            .await
            .is_err());
    }
    let detail = handle.details(request.clone()).await.unwrap();
    assert!(
        matches!(detail.plan.initiator,Initiator::Ai {conversation,tool_call,..} if conversation.as_str()=="conversation-a" && tool_call.as_str()=="preview-delivery")
    );
    let submit = bound(&handle, "conversation-a", "submit-delivery");
    submit
        .submit(
            SubmitRequest {
                operation_request_id: request.clone(),
                plan: preview.plan.clone(),
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(bound(&handle, "conversation-b", "status-delivery")
        .status(
            OperationRequest {
                operation_request_id: request.clone()
            },
            CancellationToken::new()
        )
        .await
        .is_err());
    handle.close().await;
    let restored = ExecutionHandle::start(&path)
        .unwrap()
        .for_caller(binding().caller.principal_id.as_str())
        .unwrap();
    let ai = bound(&restored, "conversation-a", "submit-delivery");
    let status = ai
        .submit(
            SubmitRequest {
                operation_request_id: request.clone(),
                plan: preview.plan,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(status.phase, execution_mcp::OperationPhase::OutcomeUnknown);
    assert_eq!(restored.details(request).await.unwrap().status.attempts, 1);
    restored.close().await;
    std::fs::remove_dir_all(root).unwrap();
}
