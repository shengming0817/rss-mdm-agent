use super::*;
use crate::protocol_test_support::{Root, Server};
use agent_client::OpenMode;
fn id(s: &str) -> Id {
    Id::new(s).unwrap()
}
fn read_plan() -> ReadPlan {
    let definition = w::NativeQueryDefinition::MacosHardware {
        section: w::MacosHardwareSection::Memory,
    };
    let budget = w::NativeBudget {
        timeout_seconds: 30,
        output_bytes: 65536,
        max_rows: 16,
    };
    let query = w::NativeQuery {
        version: 1,
        definition,
        cardinality: w::NativeCardinality::ExactlyOne,
        instance_keys: vec![],
        budget: budget.clone(),
    };
    let plan = w::NativeCollectionPlan::new(w::NativePlanSpec {
        version: 1,
        scope: rss_mdm_agent_wire::CollectionScope::Device,
        queries: [("arbitrarySourceName".into(), query)].into(),
        budget,
    })
    .unwrap();
    ReadPlan {
        plan,
        platform: w::TaskPlatform::Macos,
        deadline: 200,
        execution: w::NativeExecutionBinding {
            identity: w::ExecutionIdentity::System,
            context_revision: 1,
            target: w::SoftwareExecutionTarget::Device,
            generated_uid: None,
        },
    }
}
fn evidence(read: &ReadPlan, complete: bool) -> ProcessEvidence {
    let query = &read.plan.spec().queries["arbitrarySourceName"];
    let result = w::NativeCollectionResult {
        plan_digest: read.plan.digest(),
        collected_at: 100,
        queries: [(
            "arbitrarySourceName".into(),
            w::NativeQueryResult {
                scan: None,
                query_digest: query.digest(),
                collected_at: 100,
                status: if complete {
                    w::NativeQueryStatus::Complete
                } else {
                    w::NativeQueryStatus::Partial
                },
                enumeration_complete: complete,
                row_count: 1,
                rows: vec![[("total_bytes".into(), json!("4294967296"))].into()],
                users: None,
            },
        )]
        .into(),
    };
    let stdout = serde_json::to_vec(&result).unwrap();
    let stderr = serde_json::to_vec(&CollectionCompletion {
        input_sha256: "b".repeat(64),
        native_output_bytes: stdout.len() as u64,
        closed: true,
        output_sha256: format!("{:x}", Sha256::digest(&stdout)),
    })
    .unwrap();
    ProcessEvidence {
        content_digest: Digest::new("a".repeat(64)).unwrap(),
        attempt_id: AttemptId::new("attempt").unwrap(),
        runner: id("native"),
        scope: ProcessScope::NotStarted {},
        finished: true,
        exit_code: Some(0),
        end: ProcessEnd::Exited,
        failure_kind: ProcessFailureKind::None,
        quiescent: true,
        total_output_bytes: (stdout.len() + stderr.len()) as u64,
        stdout,
        stderr,
        quality: OutputQuality::Complete,
    }
}
#[tokio::test]
async fn native_projection_survives_reopen_is_local_and_preserves_quality_and_request_ownership() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = Data::new(
        client.collected_store().unwrap(),
        client.output_policy().unwrap(),
    );
    let plan = read_plan();
    let query = data.register(plan.clone()).unwrap().descriptor.reference;
    let request = RequestId::new("11111111-1111-4111-a111-111111111111").unwrap();
    let session = id("login/1");
    let association = Association {
        query: query.clone(),
        subject: "501".into(),
        session: session.clone(),
        parameters: digest(&json!({})).unwrap(),
        kind: QueryKind::Native,
        created_at: 100,
        origin: digest(&json!({"kind":"desktop"})).unwrap(),
    };
    data.associate(&request, &association).unwrap();
    let read = DataRead {
        selection: DataSelection::Request {
            request: request.clone(),
        },
        after: None,
        limit: 16,
        max_age_seconds: Some(10),
    };
    assert_eq!(
        data.read(&read, "501", &session, 100).unwrap().quality,
        DataQuality::NotCollected
    );
    data.native(
        plan.clone(),
        Some(request.clone()),
        &evidence(&plan, true),
        100_000,
    )
    .unwrap();
    drop(data); // No backend result read or upload receipt is involved.
    let mut data = Data::new(
        client.collected_store().unwrap(),
        client.output_policy().unwrap(),
    );
    let result = data.read(&read, "501", &session, 111).unwrap();
    assert_eq!(result.quality, DataQuality::Complete);
    assert!(result.stale);
    assert!(result.enumeration_complete);
    assert_eq!(
        result.fields["hardware.memory.total_bytes"],
        json!("4294967296")
    );
    assert!(data.read(&read, "502", &session, 100).is_err());
    assert!(data.read(&read, "501", &id("login/2"), 100).is_err());
    let mut conflicting = association.clone();
    conflicting.parameters = digest(&json!({"changed":true})).unwrap();
    assert_eq!(
        data.associate(&request, &conflicting).unwrap_err(),
        Error::Conflict
    );
    let mut partial = evidence(&plan, false);
    partial.quality = OutputQuality::Truncated;
    data.native(plan, Some(request), &partial, 112_000).unwrap();
    let result = data.read(&read, "501", &session, 112).unwrap();
    assert_eq!(result.quality, DataQuality::Truncated);
    assert!(!result.enumeration_complete);
}
#[tokio::test]
async fn pagination_is_bound_to_snapshot_and_budget_without_turning_reads_into_collection() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = Data::new(
        client.collected_store().unwrap(),
        client.output_policy().unwrap(),
    );
    let query = data.register(read_plan()).unwrap().descriptor.reference;
    let mut value = empty(query.clone(), None, DataQuality::Complete);
    value.collected_at = Some(100);
    value.enumeration_complete = true;
    value.rows = (0..12)
        .map(|n| json!({"id":n,"text":"x".repeat(10000)}))
        .collect();
    data.snapshot(&value).unwrap();
    let mut read = DataRead {
        selection: DataSelection::Query { query },
        after: None,
        limit: 128,
        max_age_seconds: None,
    };
    let page = data.read(&read, "501", &id("login/1"), 100).unwrap();
    assert!(page.rows.len() < 12);
    assert!(serde_json::to_vec(&page).unwrap().len() < 50 * 1024);
    read.after = page.next;
    let next = data.read(&read, "501", &id("login/1"), 100).unwrap();
    assert_ne!(page.rows[0], next.rows[0]);
    value.collected_at = Some(101);
    data.snapshot(&value).unwrap();
    assert_eq!(
        data.read(&read, "501", &id("login/1"), 101).unwrap_err(),
        Error::Conflict
    );
}
#[test]
fn mapping_uses_exact_closed_recipes_and_never_promotes_receipts_or_arbitrary_wmi() {
    assert_eq!(
        domain(&w::NativeQueryDefinition::MacosPkgReceipts {}),
        Some("software.receipts")
    );
    assert_eq!(
        domain(&w::NativeQueryDefinition::MacosApplications {
            root: w::MacosApplicationRoot::LocalApplications,
            max_depth: 4
        }),
        Some("software.installations")
    );
    let q = w::NativeQueryDefinition::WindowsWmi {
        namespace: r"root\cimv2".into(),
        class: "Arbitrary_Class".into(),
        properties: BTreeMap::new(),
        filter: None,
    };
    assert_eq!(domain(&q), None);
}
