use super::*;
use crate::protocol_test_support::{Root, Server};
use agent_client::OpenMode;
fn projection<S: agent_client::IdentityProvider, C: agent_client::Clock>(
    client: &agent_client::Client<S, C>,
) -> Data {
    let mut data = Data::new(
        client.collected_store().unwrap(),
        client.output_policy().unwrap(),
        client.configuration().service_id,
    );
    let (binding, _) = crate::backend::plan::context(
        client.configuration().service_id,
        client.configuration().tenant,
        &client.registration().unwrap(),
    )
    .unwrap();
    data.bind_enterprise(Some(&binding)).unwrap();
    data
}
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
        budget,
    };
    let plan = w::NativeCollectionPlan::new(w::NativePlanSpec {
        version: 1,
        scope: rss_mdm_agent_wire::CollectionScope::Device,
        queries: [("arbitrarySourceName".into(), query)].into(),
        budget,
    })
    .unwrap();
    ReadPlan {
        source: crate::collection_worker::ReadSource::local(&plan).unwrap(),
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
    let mut data = projection(&client);
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
    let mut data = projection(&client);
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
    let mut data = projection(&client);
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

fn published_item() -> w::SelfServiceDirectoryItem {
    serde_json::from_value(json!({
        "policyId":"00000000-0000-4000-a000-000000000001","policyRevision":1,"policyVersion":"00000000-0000-4000-a000-000000000002",
        "subject":{"kind":"device"},"selfService":{"published":true,"allowAi":true,"riskLevel":1,"presentation":{"displayName":"Published fixture","description":"Safe inputs"}},
        "task":{"kind":"script","runAs":"system","resource":{"id":"resource","version":"1","variant":"default"},
        "parameterSources":{"name":{"kind":"input"},"fixed":{"kind":"fixed","value":"hidden"}},
        "parameterSchema":{"type":"object","properties":{"name":{"type":"string"},"fixed":{"type":"string"}},"required":["name","fixed"],"additionalProperties":false}}
    })).unwrap()
}
#[tokio::test]
async fn metadata_is_bounded_titles_survive_capture_and_published_inputs_remain_usable() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = projection(&client);
    let read = read_plan();
    let definition = data.register(read.clone()).unwrap();
    data.pin(&definition.descriptor.reference, &definition.read);
    data.native(read.clone(), None, &evidence(&read, true), 100_000)
        .unwrap();
    assert_eq!(
        data.definition(&definition.descriptor.reference, "501")
            .unwrap()
            .descriptor
            .title,
        definition.descriptor.title
    );
    let mut changed = read.clone();
    let mut spec: w::NativePlanSpec = changed.plan.into();
    let mut query = spec.queries.remove("arbitrarySourceName").unwrap();
    query.budget.timeout_seconds = 15;
    spec.queries.insert("serverLabelChanged".into(), query);
    changed.plan = w::NativeCollectionPlan::new(spec).unwrap();
    changed.source = crate::collection_worker::ReadSource::local(&changed.plan).unwrap();
    assert_eq!(
        data.register(changed).unwrap().descriptor.reference,
        definition.descriptor.reference
    );
    for n in 0..70 {
        let mut read = read_plan();
        let mut spec: w::NativePlanSpec = read.plan.into();
        spec.queries.values_mut().next().unwrap().definition =
            w::NativeQueryDefinition::WindowsWmi {
                namespace: r"root\cimv2".into(),
                class: format!("Unknown{n}"),
                properties: [(
                    "name".into(),
                    w::WmiProperty {
                        cim_type: w::CimType::String,
                        array: false,
                    },
                )]
                .into(),
                filter: None,
            };
        read.plan = w::NativeCollectionPlan::new(spec).unwrap();
        read.source = crate::collection_worker::ReadSource::local(&read.plan).unwrap();
        read.platform = w::TaskPlatform::Windows;
        data.register(read).unwrap();
    }
    data.maintain(&[], &Default::default(), &Default::default())
        .unwrap();
    assert!(data.scan("definition/").unwrap().len() <= 65);
    assert!(data
        .definition(&definition.descriptor.reference, "501")
        .is_ok());
    let session = id("login/1");
    for n in 0..40 {
        let context = NativeContextRef {
            id: id("context"),
            generation: id(&format!("generation-{n}")),
        };
        let descriptor = data
            .publish(published_item(), "501", &session, &context)
            .unwrap();
        assert_eq!(descriptor.title, "Published fixture");
        assert!(descriptor.input_schema["properties"].get("name").is_some());
        assert!(descriptor.input_schema["properties"].get("fixed").is_none());
        assert_eq!(descriptor.input_schema["required"], json!(["name"]));
        assert!(descriptor.policy.is_some());
        data.maintain(
            &[("501".into(), session.clone(), context)],
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        assert_eq!(data.scan("published/").unwrap().len(), 1);
    }
}
#[cfg(target_os = "macos")]
#[tokio::test]
async fn reused_numeric_uid_cannot_discover_or_read_an_old_accounts_collection() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = projection(&client);
    let caller = execution_ipc::host::current_subject().unwrap();
    let mut read = read_plan();
    let mut spec: w::NativePlanSpec = read.plan.clone().into();
    spec.queries.values_mut().next().unwrap().definition =
        w::NativeQueryDefinition::MacosApplications {
            root: w::MacosApplicationRoot::UserApplications,
            max_depth: 4,
        };
    let query = spec.queries.values_mut().next().unwrap();
    query.cardinality = w::NativeCardinality::List;
    query.instance_keys = vec!["id".into()];
    spec.scope = rss_mdm_agent_wire::CollectionScope::User {
        identity: caller.clone(),
    };
    read.plan = w::NativeCollectionPlan::new(spec).unwrap();
    read.source = crate::collection_worker::ReadSource::local(&read.plan).unwrap();
    read.execution = w::NativeExecutionBinding {
        identity: w::ExecutionIdentity::LoggedInUser,
        context_revision: 1,
        target: w::SoftwareExecutionTarget::User {
            identity: caller.clone(),
            session_id: uuid::Uuid::new_v4(),
        },
        generated_uid: Some(uuid::Uuid::new_v4().to_string()),
    };
    let query = data.register(read).unwrap().descriptor.reference;
    assert!(!data
        .directory(&caller)
        .unwrap()
        .iter()
        .any(|d| d.reference == query));
    assert!(data.definition(&query, &caller).is_err());
    let read = DataRead {
        selection: DataSelection::Query { query },
        after: None,
        limit: 1,
        max_age_seconds: None,
    };
    assert!(data.read(&read, &caller, &id("login/1"), 100).is_err());
}
#[tokio::test]
async fn telemetry_ack_preserves_normalized_projection_without_granting_unknown_fields() {
    let _budget = crate::scenario_budget::enter();
    use crate::protocol_test_support::telemetry as fixture;
    use agent_client::{telemetry::Limits, Clock};
    let server = Server::tls().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let signed = fixture::configuration(&server, &r);
    server.data.lock().unwrap().telemetry.configuration = Some(signed.clone());
    let c = client.verify_telemetry(signed).unwrap();
    let mut spool = client.telemetry_spool(Limits::production()).unwrap();
    spool.install(&c).unwrap();
    let at = server.time.now().unwrap() * 1_000_000;
    let values = [(
        w::FieldKey::parse("custom.load").unwrap(),
        w::TelemetryReading::Valid {
            value: w::Scalar::Integer(17),
            interval_start: None,
        },
    )]
    .into();
    let observation = spool.append(&c, at, values, "boot-fixture").unwrap();
    let normalized = spool.observation(&c, observation).unwrap().values;
    let mut data = projection(&client);
    data.periodic(
        c.payload(),
        &evidence(&read_plan(), true),
        at,
        Some(normalized),
    )
    .unwrap();
    assert!(!data
        .directory("501")
        .unwrap()
        .iter()
        .any(|d| d.kind == QueryKind::Telemetry));
    let key = format!(
        "latest/projected:telemetry:{}",
        digest(c.payload()).unwrap().as_str()
    );
    let retained = data.store.read(&key).unwrap().unwrap();
    spool.freeze_all(&c).unwrap();
    let pending = spool.pending(at / 1_000_000, false).unwrap().unwrap();
    spool.attempting(&pending, at / 1_000_000).unwrap();
    let receipt = client.telemetry_submit(&pending).await.unwrap();
    spool.acknowledge(&pending, &receipt).unwrap();
    assert_eq!(spool.summary().unwrap().pending, 0);
    drop(data);
    let data = projection(&client);
    assert_eq!(data.store.read(&key).unwrap(), Some(retained));
    let query = QueryRef {
        id: id(&format!(
            "telemetry:{}",
            digest(c.payload()).unwrap().as_str()
        )),
        revision: digest(c.payload()).unwrap(),
    };
    assert!(data
        .authorize_read(
            &DataRead {
                selection: DataSelection::Query { query },
                after: None,
                limit: 16,
                max_age_seconds: None
            },
            "501",
            &id("login/1")
        )
        .is_err());
}

#[tokio::test]
async fn declared_domains_and_windows_fields_match_the_same_local_display_contract() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = projection(&client);
    // Pure wire/mapper validation on this host; this is not Windows execution evidence.
    data.install_platform(Platform::Windows).unwrap();
    let entries = data.directory("501").unwrap();
    assert!(entries
        .iter()
        .any(|q| q.domains.iter().any(|d| d == "hardware.os")));
    let memory = entries
        .iter()
        .find(|q| q.domains.iter().any(|d| d == "hardware.memory"))
        .unwrap();
    let read = data.definition(&memory.reference, "501").unwrap().read;
    let q = &read.plan.spec().queries.values().next().unwrap().definition;
    assert_eq!(
        mapped_field(q, "TotalPhysicalMemory").as_deref(),
        Some("hardware.memory.total_bytes")
    );
    let disk = entries
        .iter()
        .find(|q| q.domains.iter().any(|d| d == "hardware.disks"))
        .unwrap();
    let read = data.definition(&disk.reference, "501").unwrap().read;
    let crate::collection_worker::ReadSource::Local { projection } = &read.source else {
        panic!("local recipe");
    };
    assert_eq!(
        projection.queries["read"]
            .row(
                &[
                    ("DeviceID".into(), json!("disk0")),
                    ("Model".into(), json!("native")),
                    ("Size".into(), json!("1024")),
                ]
                .into()
            )
            .unwrap(),
        json!({"id":"disk0","model":"native","size_bytes":"1024"})
    );
}

#[tokio::test]
async fn minimum_record_quota_reclaims_metadata_and_reserves_new_query_capacity() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut config = server.config();
    config.limits.pending_tasks = 1;
    let mut client = agent_client::Client::open(
        &root.path,
        config,
        OpenMode::Create,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    server.register(&mut client).await;
    let mut data = projection(&client);
    data.install_platform(Platform::Macos).unwrap();
    assert_eq!(data.store.limits().1, 32);
    let context = NativeContextRef {
        id: id("context"),
        generation: id("generation"),
    };
    let session = id("login/1");
    let contexts = [("501".into(), session.clone(), context.clone())];
    for n in 0..80 {
        data.maintain(&contexts, &Default::default(), &Default::default())
            .unwrap();
        let mut item = published_item();
        item.policy_id = uuid::Uuid::from_u128(n + 1);
        data.publish(item, "501", &session, &context).unwrap();
    }
    data.maintain(&contexts, &Default::default(), &Default::default())
        .unwrap();
    assert!(data.store.records().unwrap() <= 24);
    assert!(data.history_limit() <= 4);
    let query = data.directory("501").unwrap()[0].reference.clone();
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let association = Association {
        query,
        subject: "501".into(),
        session,
        parameters: digest(&json!({})).unwrap(),
        kind: QueryKind::Native,
        created_at: 100,
        origin: digest(&json!("desktop")).unwrap(),
    };
    data.store
        .write(
            &format!("request/{}", request.as_str()),
            &encode(&association).unwrap(),
        )
        .unwrap();
    data.remember(&request, read_plan()).unwrap();
    data.maintain(&contexts, &[request.clone()].into(), &Default::default())
        .unwrap();
    assert!(data.remembered(&request).unwrap().is_some());
    assert!(data
        .association(&request, "501", &association.session)
        .is_ok());
}

#[tokio::test]
async fn a_later_native_projection_failure_rolls_back_the_batch_and_recovers_after_reopen() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = projection(&client);
    let mut read = read_plan();
    let mut spec: w::NativePlanSpec = read.plan.clone().into();
    let mut cpu = spec.queries["arbitrarySourceName"].clone();
    cpu.definition = w::NativeQueryDefinition::MacosHardware {
        section: w::MacosHardwareSection::Cpu,
    };
    spec.queries.insert("cpu".into(), cpu);
    read.plan = w::NativeCollectionPlan::new(spec).unwrap();
    read.source = crate::collection_worker::ReadSource::local(&read.plan).unwrap();
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    data.remember(&request, read.clone()).unwrap();
    let mut refs = read
        .plan
        .spec()
        .queries
        .values()
        .map(|q| {
            data.register(single(&read, q).unwrap())
                .unwrap()
                .descriptor
                .reference
        })
        .collect::<Vec<_>>();
    refs.sort_by(|a, b| a.id.cmp(&b.id));
    let mut process = evidence(&read_plan(), true);
    let mut result: w::NativeCollectionResult = serde_json::from_slice(&process.stdout).unwrap();
    result.plan_digest = read.plan.digest();
    let mut cpu = result.queries["arbitrarySourceName"].clone();
    cpu.query_digest = read.plan.spec().queries["cpu"].digest();
    cpu.rows = vec![[
        ("model".into(), json!("Fixture CPU")),
        ("physical_cores".into(), json!(4)),
        ("logical_cores".into(), json!(8)),
    ]
    .into()];
    result.queries.insert("cpu".into(), cpu);
    process.stdout = serde_json::to_vec(&result).unwrap();
    process.stderr = serde_json::to_vec(&CollectionCompletion {
        input_sha256: "b".repeat(64),
        native_output_bytes: process.stdout.len() as u64,
        closed: true,
        output_sha256: format!("{:x}", Sha256::digest(&process.stdout)),
    })
    .unwrap();
    process.total_output_bytes = (process.stdout.len() + process.stderr.len()) as u64;
    let conn = rusqlite::Connection::open(root.path.join("communication.sqlite")).unwrap();
    conn.execute_batch(&format!("CREATE TRIGGER fixture_second_projection BEFORE INSERT ON state WHEN NEW.key GLOB 'local-data/*latest/{}' AND EXISTS(SELECT 1 FROM state WHERE key GLOB 'local-data/*latest/{}') BEGIN SELECT RAISE(ABORT,'fixture second write'); END;", refs[1].id.as_str(), refs[0].id.as_str())).unwrap();
    assert!(data
        .native(read.clone(), Some(request.clone()), &process, 100_000)
        .is_err());
    assert!(!data.captured(&request).unwrap());
    assert!(data.remembered(&request).unwrap().is_some());
    for query in &refs {
        assert!(data
            .store
            .read(&format!("latest/{}", query.id.as_str()))
            .unwrap()
            .is_none());
    }
    drop(data);
    drop(client);
    conn.execute_batch("DROP TRIGGER fixture_second_projection;")
        .unwrap();
    drop(conn);
    let client = server.client(&root, OpenMode::Existing);
    let mut data = projection(&client);
    let recovered = data.remembered(&request).unwrap().unwrap();
    data.native(recovered, Some(request.clone()), &process, 100_000)
        .unwrap();
    assert!(data.captured(&request).unwrap());
    assert!(data.remembered(&request).unwrap().is_none());
    for query in refs {
        let value = data
            .read(
                &DataRead {
                    selection: DataSelection::Query { query },
                    after: None,
                    limit: 16,
                    max_age_seconds: None,
                },
                "501",
                &id("login/new"),
                100,
            )
            .unwrap();
        assert_eq!(value.quality, DataQuality::Complete);
    }
}

#[tokio::test]
async fn authorized_policy_data_survives_context_retirement_and_reopen_without_execution_authority()
{
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = projection(&client);
    let subject = execution_ipc::host::current_subject().unwrap();
    let old_session = id("login/old");
    let old_context = NativeContextRef {
        id: id("context"),
        generation: id("old"),
    };
    let query = data
        .publish(published_item(), &subject, &old_session, &old_context)
        .unwrap()
        .reference;
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let association = Association {
        query: query.clone(),
        subject: subject.clone(),
        session: old_session.clone(),
        parameters: digest(&json!({})).unwrap(),
        kind: QueryKind::Policy,
        created_at: 100,
        origin: digest(&json!("desktop")).unwrap(),
    };
    data.associate(&request, &association).unwrap();
    let status = w::SelfServiceStatus {
        request_id: uuid::Uuid::parse_str(request.as_str()).unwrap(),
        run_id: uuid::Uuid::new_v4(),
        attempt_id: None,
        state: w::TaskRunState {
            delivery: w::TaskDelivery::Queued,
            execution: w::TaskExecution::NotStarted,
            cancellation: w::RunCancellation::None,
            deadline: 200,
            started_at: None,
        },
        available_at: 100,
        deadline: 200,
        result: Some(w::SelfServiceResult::Script {
            evidence: w::ScriptRunResult {
                collected_at: 100,
                received_at: 101,
                exit_code: Some(0),
                quality: w::OutputQuality::Complete,
                schema_valid: true,
                budget_valid: true,
                output: json!({"fixture":17}),
                output_reference: None,
                diagnostics: w::TaskDiagnostics::new(String::new(), String::new(), 1, 100, None)
                    .unwrap(),
                trusted: true,
            },
        }),
    };
    data.policy_result(&association, &status).unwrap();
    drop(data);
    drop(client);
    let client = server.client(&root, OpenMode::Existing);
    let mut data = projection(&client);
    data.maintain(&[], &Default::default(), &Default::default())
        .unwrap();
    let session = id("login/new");
    let context = NativeContextRef {
        id: id("context"),
        generation: id("new"),
    };
    assert!(data
        .published(&query, &subject, &session, &context)
        .unwrap()
        .is_none());
    assert!(data.association(&request, &subject, &session).is_err()); // no status/cancel grant
    let saved = data
        .directory(&subject)
        .unwrap()
        .into_iter()
        .find(|d| d.reference == query)
        .unwrap();
    assert!(!saved.available);
    for selection in [
        DataSelection::Query {
            query: query.clone(),
        },
        DataSelection::Request { request },
    ] {
        let read = DataRead {
            selection,
            after: None,
            limit: 16,
            max_age_seconds: None,
        };
        data.authorize_read(&read, &subject, &session).unwrap();
        let value = data.read(&read, &subject, &session, 100).unwrap();
        assert_eq!(value.rows, vec![json!({"fixture":17})]);
        assert!(data
            .authorize_read(&read, "foreign-account", &session)
            .is_err());
    }
    let republished = data
        .publish(published_item(), &subject, &session, &context)
        .unwrap();
    assert_eq!(republished.reference, query);
    // A stale or reused account cannot consume a persisted read projection.
    let key = format!("policy-read/{}", query.id.as_str());
    let mut metadata: PolicyRead = decode(data.store.read(&key).unwrap().unwrap()).unwrap();
    metadata.generated_uid = Some(uuid::Uuid::new_v4().to_string());
    data.store.write(&key, &encode(&metadata).unwrap()).unwrap();
    assert!(data.policy_read(&query, &subject).is_err());
}

#[tokio::test]
async fn observed_backend_definitions_never_create_or_replace_compiled_local_rules() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = projection(&client);
    let first = data.register(read_plan()).unwrap();
    assert!(data.definition(&first.descriptor.reference, "501").is_ok());
    assert!(matches!(
        data.local_definition(&first.descriptor.reference, "501"),
        Err(Error::Denied)
    ));
    assert!(
        !data
            .directory("501")
            .unwrap()
            .into_iter()
            .find(|d| d.reference == first.descriptor.reference)
            .unwrap()
            .available
    );
    data.pin(&first.descriptor.reference, &first.read);
    let mut changed = first.read.clone();
    changed.deadline = 999_999;
    changed.execution.context_revision = 99;
    let observed = data.register(changed).unwrap();
    assert_eq!(observed.descriptor.reference, first.descriptor.reference);
    let local = data
        .local_definition(&first.descriptor.reference, "501")
        .unwrap();
    assert_eq!(
        local.read.execution.context_revision,
        first.read.execution.context_revision
    );
    assert_eq!(local.read.deadline, first.read.deadline);
}

#[tokio::test]
async fn old_settings_tasks_cannot_replace_a_newer_observation() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = projection(&client);
    let session = id("settings-session");
    let mut value = SettingsSnapshot {
        section: SettingsSection::Network,
        revision: Digest::new("a".repeat(64)).unwrap(),
        collected_at_unix_ms: 200,
        source: "fixture native".into(),
        targets: vec![],
    };
    data.settings_write("0", &session, &value).unwrap();
    value.collected_at_unix_ms = 100;
    value.revision = Digest::new("b".repeat(64)).unwrap();
    data.settings_write("0", &session, &value).unwrap();
    assert_eq!(
        data.settings_read("0", &session, SettingsSection::Network)
            .unwrap()
            .unwrap()
            .collected_at_unix_ms,
        200
    );
}

fn enterprise_task<S: agent_client::IdentityProvider, C: agent_client::Clock>(
    client: &agent_client::Client<S, C>,
    read: &ReadPlan,
) -> w::CollectionTaskSpec {
    let r = client.registration().unwrap();
    let (fields, mappings) = crate::protocol_test_support::projection::native(&read.plan);
    let task = w::CollectionTaskSpec {
        fields,
        mappings,
        wire_version: w::COLLECTION_TASK_VERSION,
        tenant_id: client.configuration().tenant,
        registration_id: r.registration_id,
        device_id: r.device_id,
        generation: r.generation,
        platform: read.platform,
        architecture: w::TaskArchitecture::Aarch64,
        task_id: uuid::Uuid::new_v4(),
        attempt_id: uuid::Uuid::new_v4(),
        permit: w::TaskPermit::Start,
        expires_at: 100,
        deadline: read.deadline,
        resource_digest: [3; 32],
        resource: w::TaskResource {
            id: "enterprise-memory".into(),
            version: "v1".into(),
            variant: "macos".into(),
        },
        authorization: w::TaskAuthorization::Policy {
            policy_id: uuid::Uuid::new_v4(),
            policy_version: uuid::Uuid::new_v4(),
        },
        execution: read.execution.clone(),
        plan: read.plan.clone(),
        plan_digest: read.plan.digest(),
    };
    task.validate().unwrap();
    task
}

#[tokio::test]
async fn frozen_enterprise_projection_is_generic_private_and_isolated_from_local_views() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut data = projection(&client);
    let local = read_plan();
    let local_ref = data
        .register(single(&local, local.plan.spec().queries.values().next().unwrap()).unwrap())
        .unwrap()
        .descriptor
        .reference;
    let task = enterprise_task(&client, &local);
    let read: ReadPlan = task.clone().try_into().unwrap();
    let reference = data.register(read.clone()).unwrap().descriptor.reference;
    assert_ne!(reference, local_ref);
    data.native(read.clone(), None, &evidence(&read, true), 100_000)
        .unwrap();
    let key = format!("latest/projected:{}", reference.id.as_str());
    let fields: BTreeMap<String, crate::collection_projection::FieldResult> =
        decode(data.store.read(&key).unwrap().unwrap()).unwrap();
    assert_eq!(fields["custom.native"].value, Some(json!(4294967296i64)));
    assert!(data
        .directory("501")
        .unwrap()
        .iter()
        .any(|d| d.reference == reference && d.origin == QueryOrigin::Enterprise));
    let value: CollectedData = decode(
        data.store
            .read(&format!("latest/{}", reference.id.as_str()))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value.quality, DataQuality::Complete);
    assert!(!value.fields.contains_key("custom.native"));
    assert_eq!(
        value.fields["hardware.memory.total_bytes"],
        json!("4294967296")
    );
    // A different delivery attempt retains the mapping identity; a different mapping does not.
    let mut again = task.clone();
    again.attempt_id = uuid::Uuid::new_v4();
    let refreshed_attempt = again.attempt_id;
    assert_eq!(
        data.register(again.try_into().unwrap())
            .unwrap()
            .descriptor
            .reference,
        reference
    );
    let mut changed = task.clone();
    changed.fields[0].key = w::FieldKey::parse("custom.other").unwrap();
    let binding = changed.mappings.remove("custom.native").unwrap();
    changed.mappings.insert("custom.other".into(), binding);
    let changed_ref = data
        .register(changed.try_into().unwrap())
        .unwrap()
        .descriptor
        .reference;
    assert_ne!(changed_ref, reference);
    for context in 0..4 {
        let mut changed = task.clone();
        match context {
            0 => changed.generation += 1,
            1 => changed.device_id = "other-device".into(),
            2 => changed.tenant_id = uuid::Uuid::new_v4(),
            _ => changed.resource_digest = [4; 32],
        }
        assert_ne!(
            data.register(changed.try_into().unwrap())
                .unwrap()
                .descriptor
                .reference,
            reference
        );
    }
    let mut corrupted = read.clone();
    corrupted.deadline += 1;
    assert_eq!(corrupted.validate(), Err(Error::Untrusted));
    drop(data);
    let data = projection(&client);
    let restored: Definition = decode(
        data.store
            .read(&format!("definition/{}", reference.id.as_str()))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let mut expected = read.source.clone();
    let crate::collection_worker::ReadSource::Task { task, .. } = &mut expected else {
        panic!("task source");
    };
    task.attempt_id = refreshed_attempt;
    assert_eq!(
        encode(&restored.read.source).unwrap(),
        encode(&expected).unwrap()
    );
    assert_eq!(
        data.store.read(&key).unwrap().unwrap(),
        encode(&fields).unwrap()
    );
    assert!(data.local_definition(&reference, "501").is_err());
}

#[test]
fn shared_projection_preserves_missing_partial_unknown_and_complete_empty_list() {
    use crate::collection_projection::{self as p, FieldResult};
    let read = read_plan();
    let (mut fields, mut mappings) = crate::protocol_test_support::projection::native(&read.plan);
    let mut result: w::NativeCollectionResult =
        serde_json::from_slice(&evidence(&read, true).stdout).unwrap();
    for (status, complete, expected) in [
        (w::NativeQueryStatus::Partial, false, DataQuality::Partial),
        (
            w::NativeQueryStatus::Truncated,
            false,
            DataQuality::Truncated,
        ),
        (w::NativeQueryStatus::Missing, true, DataQuality::Missing),
    ] {
        let row = result.queries.get_mut("arbitrarySourceName").unwrap();
        row.status = status;
        row.enumeration_complete = complete;
        row.rows.clear();
        row.row_count = 0;
        let v = p::native(&fields, &mappings, &read.plan, &result, 100).unwrap();
        assert_eq!(v["custom.native"].quality, expected);
        assert!(!v["custom.native"].deleted);
    }
    fields[0].nullable = true;
    mappings.get_mut("custom.native").unwrap().missing = w::NativeMissing::Null;
    let v = p::native(&fields, &mappings, &read.plan, &result, 100).unwrap();
    assert_eq!(v["custom.native"].value, Some(Value::Null));
    mappings.get_mut("custom.native").unwrap().missing = w::NativeMissing::Delete;
    let v = p::native(&fields, &mappings, &read.plan, &result, 100).unwrap();
    assert!(v["custom.native"].deleted);
    result.queries.clear();
    let v = p::native(&fields, &mappings, &read.plan, &result, 100).unwrap();
    assert_eq!(v["custom.native"].quality, DataQuality::Unknown);
    let mut spec: w::NativePlanSpec = read.plan.into();
    let q = spec.queries.get_mut("arbitrarySourceName").unwrap();
    q.definition = w::NativeQueryDefinition::WindowsWmi {
        namespace: r"root\cimv2".into(),
        class: "Vendor_Items".into(),
        properties: [(
            "Id".into(),
            w::WmiProperty {
                cim_type: w::CimType::String,
                array: false,
            },
        )]
        .into(),
        filter: None,
    };
    q.cardinality = w::NativeCardinality::List;
    q.instance_keys = vec!["Id".into()];
    let plan = w::NativeCollectionPlan::new(spec).unwrap();
    let (fields, mappings) = crate::protocol_test_support::projection::native(&plan);
    result.plan_digest = plan.digest();
    result.queries.insert(
        "arbitrarySourceName".into(),
        w::NativeQueryResult {
            scan: None,
            query_digest: plan.spec().queries["arbitrarySourceName"].digest(),
            collected_at: 100,
            status: w::NativeQueryStatus::Complete,
            enumeration_complete: true,
            row_count: 0,
            rows: vec![],
            users: None,
        },
    );
    let v: BTreeMap<String, FieldResult> =
        p::native(&fields, &mappings, &plan, &result, 100).unwrap();
    assert_eq!(v["custom.native"].value, Some(json!([])));
    assert_eq!(v["custom.native"].quality, DataQuality::Complete);
}

#[tokio::test]
async fn packaged_baseline_and_product_owned_local_additions_coexist_without_registration() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let client = server.client(&root, OpenMode::Create);
    let mut data = Data::new(
        client
            .collected_store_for_installation(uuid::Uuid::new_v4())
            .unwrap(),
        client.output_policy().unwrap(),
        client.configuration().service_id,
    );
    data.install_platform(Platform::Macos).unwrap();
    let mac = data.directory("501").unwrap();
    assert_eq!(mac.len(), 15);
    data.install_platform(Platform::Windows).unwrap();
    let all = data.directory("501").unwrap();
    assert_eq!(all.len(), 32);
    assert!(all.iter().all(|d| d.origin == QueryOrigin::Local));
    let os = all
        .iter()
        .find(|d| {
            d.domains.contains(&"hardware.os".into()) && d.title.contains("Win32_OperatingSystem")
        })
        .unwrap();
    let read = data.definition(&os.reference, "501").unwrap().read;
    let crate::collection_worker::ReadSource::Local { projection } = &read.source else {
        panic!("local source")
    };
    let fields = projection.queries["read"]
        .fields(
            &[
                ("Caption".into(), json!("Windows")),
                ("Version".into(), json!("11")),
                ("BuildNumber".into(), json!("26100")),
                ("SystemDrive".into(), json!("C:")),
            ]
            .into(),
        )
        .unwrap();
    assert_eq!(fields["hardware.os.system_drive"], json!("C:"));
    let local = read_plan();
    let q = local.plan.spec().queries.values().next().unwrap();
    let single = single(&local, q).unwrap();
    let reference = data.register(single).unwrap().descriptor.reference;
    data.native(local.clone(), None, &evidence(&local, true), 100_000)
        .unwrap();
    let result = data
        .read(
            &DataRead {
                selection: DataSelection::Query { query: reference },
                after: None,
                limit: 16,
                max_age_seconds: None,
            },
            "501",
            &id("login/1"),
            100,
        )
        .unwrap();
    assert_eq!(
        result.fields["hardware.memory.total_bytes"],
        json!("4294967296")
    );
    assert!(
        rss_mdm_agent_wire::packaged_collections::definitions()
            .unwrap()
            .len()
            > 32
    );
    let crate::collection_worker::ReadSource::Local { projection } = local.source else {
        panic!("local source")
    };
    assert!(!projection.queries["arbitrarySourceName"]
        .baseline_fields(
            &local.plan,
            &serde_json::from_slice(&evidence(&read_plan(), true).stdout).unwrap(),
            100
        )
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn frozen_script_projection_retains_typed_results_and_requires_the_original_policy_grant() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut task = {
        let mut d = server.data.lock().unwrap();
        d.script();
        let w::TaskPayload::Script(task) = d.offer.as_ref().unwrap().payload.clone() else {
            panic!("script")
        };
        *task
    };
    let (mut fields, _) = crate::protocol_test_support::projection::native(&read_plan().plan);
    fields[0].key = w::FieldKey::parse("custom.created").unwrap();
    fields[0].sources = [(w::Source::AgentScript, 0)].into();
    task.projection = w::ScriptProjection::Collection {
        fields,
        mappings: [(
            "custom.created".into(),
            w::ValueMapping {
                pointer: "/count".into(),
                conversion: w::Conversion::Integer,
                columns: Default::default(),
            },
        )]
        .into(),
        output_schema: json!({"type":"object","properties":{"count":{"type":"string"}},"required":["count"],"additionalProperties":false}),
        scope: rss_mdm_agent_wire::CollectionScope::Device,
    };
    task.validate().unwrap();
    let mut process = evidence(&read_plan(), true);
    process.stdout = br#"{"count":"17"}"#.to_vec();
    process.stderr.clear();
    process.total_output_bytes = process.stdout.len() as u64;
    let mut data = projection(&client);
    let automatic = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    data.capture_script(&task, &automatic, &process, 100_000)
        .unwrap();
    assert!(data.directory("501").unwrap().is_empty());
    let saved: CollectedData = decode(
        data.store
            .read(&format!("result/{}", automatic.as_str()))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved.quality, DataQuality::Denied);
    assert!(saved.fields.is_empty());
    let protected = data
        .store
        .read(&format!("latest/projected:{}", saved.query.id.as_str()))
        .unwrap()
        .unwrap();
    assert_eq!(protected[1]["custom.created"]["value"], json!(17));
    let subject = execution_ipc::host::current_subject().unwrap();
    let session = id("login/1");
    let context = NativeContextRef {
        id: id("context"),
        generation: id("generation"),
    };
    let item = published_item();
    let query = data
        .publish(item.clone(), &subject, &session, &context)
        .unwrap()
        .reference;
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let association = Association {
        query: query.clone(),
        subject: subject.clone(),
        session: session.clone(),
        parameters: digest(&json!({})).unwrap(),
        kind: QueryKind::Policy,
        created_at: 100,
        origin: digest(&json!("desktop")).unwrap(),
    };
    data.associate(&request, &association).unwrap();
    let w::SelfServiceTask::Script { resource, .. } = &item.task else {
        panic!("script")
    };
    task.resource = resource.clone();
    task.authorization = w::TaskAuthorization::SelfService {
        request_id: uuid::Uuid::parse_str(request.as_str()).unwrap(),
        subject: item.subject,
        source: w::SelfServiceCaller::Human,
        policy_id: item.policy_id,
        policy_revision: item.policy_revision,
        policy_version: item.policy_version,
        allow_ai: true,
        risk_level: 1,
        confirmed: true,
    };
    data.capture_script(&task, &request, &process, 100_000)
        .unwrap();
    drop(data);
    let data = projection(&client);
    let read = DataRead {
        selection: DataSelection::Request {
            request: request.clone(),
        },
        after: None,
        limit: 16,
        max_age_seconds: None,
    };
    assert_eq!(
        data.read(&read, &subject, &session, 100).unwrap().fields["custom.created"],
        json!(17)
    );
    assert!(data.read(&read, "other-account", &session, 100).is_err());
    let mut data = data;
    task.resource.version = "other".into();
    assert!(data
        .capture_script(&task, &request, &process, 101_000)
        .is_err());
}

#[tokio::test]
async fn journal_delivery_capture_checks_success_hash_and_confirms_terminal_failures_after_reopen()
{
    use crate::backend::test_support::{local, CaptureSpec, CapturingRunner};
    use execution_app::{AppConfig, DeterministicTestRunner, RequestContext, TestScenario};
    let _budget = crate::scenario_budget::enter();
    for mode in 0..4 {
        let server = Server::new().await;
        let root = Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        server.register(&mut client).await;
        let read = read_plan();
        let mut data = projection(&client);
        let mut spec = local::plan().spec().clone();
        spec.request.request_id = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
        let input = FrozenExecution::freeze(spec, &execution_app::test_execution_limits()).unwrap();
        let mut result: w::NativeCollectionResult =
            serde_json::from_slice(&evidence(&read, true).stdout).unwrap();
        result.collected_at = 1;
        for r in result.queries.values_mut() {
            r.collected_at = 1;
        }
        let stdout = serde_json::to_vec(&result).unwrap();
        let stderr = if mode < 2 {
            serde_json::to_vec(&CollectionCompletion {
                input_sha256: if mode == 1 {
                    "d".repeat(64)
                } else {
                    input.spec().launch.artifact.sha256.as_str().into()
                },
                output_sha256: format!("{:x}", Sha256::digest(&stdout)),
                closed: true,
                native_output_bytes: stdout.len() as u64,
            })
            .unwrap()
        } else {
            b"worker stopped before completion".to_vec()
        };
        let runner = CapturingRunner {
            inner: DeterministicTestRunner::new(id("test-runner"), TestScenario::Complete, 16)
                .unwrap(),
            ready: Default::default(),
            progress: Default::default(),
            capture: std::sync::Arc::new(std::sync::Mutex::new(Some(CaptureSpec {
                exit_code: if mode == 2 { 1 } else { 0 },
                quiescent: true,
                stdout,
                stderr,
                quality: if mode == 3 {
                    OutputQuality::Truncated
                } else if mode == 2 {
                    OutputQuality::Failed
                } else {
                    OutputQuality::Complete
                },
                end: ProcessEnd::Exited,
            }))),
        };
        let db = local::Database::new();
        let mut host = local::TestHost::new();
        host.template = input.clone();
        let caller = RequestContext {
            actor: input.spec().request.actor.clone(),
        };
        let mut app = local::create_app(
            &db.path,
            host.clone(),
            runner.clone(),
            AppConfig::test_defaults(1),
        )
        .unwrap();
        app.request_execution(&caller, &input).unwrap();
        app.reconcile(&input.spec().request.request_id).unwrap();
        let request = &input.spec().request.request_id;
        let consumer = id("local-collected-data");
        let pending = app.service_delivery(request, &consumer, 64).unwrap().len();
        assert!(pending > 1);
        if mode == 1 {
            assert_eq!(
                crate::service::Core::capture_native_delivery(&mut app, &mut data, &input, &read),
                Err(Error::Untrusted)
            );
            assert!(!data.captured(request).unwrap());
            assert_eq!(
                app.service_delivery(request, &consumer, 64).unwrap().len(),
                pending
            );
            continue;
        }
        crate::service::Core::capture_native_delivery(&mut app, &mut data, &input, &read).unwrap();
        assert!(data.captured(request).unwrap());
        drop(app);
        drop(data);
        let mut app =
            local::reopen_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
        let mut data = projection(&client);
        for _ in 0..32 {
            crate::service::Core::capture_native_delivery(&mut app, &mut data, &input, &read)
                .unwrap();
        }
        assert!(app
            .service_delivery(request, &consumer, 64)
            .unwrap()
            .is_empty());
        assert_eq!(runner.inner.dispatch_count(), 1);
        let saved: CollectedData = decode(
            data.store
                .read(&format!("result/{}", request.as_str()))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            saved.quality,
            match mode {
                0 => DataQuality::Complete,
                2 => DataQuality::Failed,
                _ => DataQuality::Truncated,
            }
        );
        if mode == 0 {
            assert_eq!(
                saved.fields["hardware.memory.total_bytes"],
                json!("4294967296")
            );
        }
    }
}

#[tokio::test]
async fn frozen_view_domain_survives_recovery_without_changing_the_current_read_grant() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let mut read = read_plan();
    let crate::collection_worker::ReadSource::Local { projection: frozen } = &mut read.source
    else {
        panic!("local")
    };
    frozen
        .queries
        .get_mut("arbitrarySourceName")
        .unwrap()
        .domain = Some("historical.memory".into());
    let mut data = projection(&client);
    let request = RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    data.remember(&request, read.clone()).unwrap();
    drop(data);
    let mut data = projection(&client);
    let read = data.remembered(&request).unwrap().unwrap();
    data.native(
        read.clone(),
        Some(request.clone()),
        &evidence(&read, true),
        100_000,
    )
    .unwrap();
    let saved: CollectedData = decode(
        data.store
            .read(&format!("result/{}", request.as_str()))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved.rows[0]["domain"], json!("historical.memory"));
    assert!(read_granted(&read));
}

#[tokio::test]
async fn software_coverage_joins_delivery_identities_but_separates_physical_and_account_boundaries()
{
    let _budget = crate::scenario_budget::enter();
    let server = Server::new().await;
    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    let r = server.register(&mut client).await;
    let mut data = projection(&client);
    let mut local = read_plan();
    let mut spec: w::NativePlanSpec = local.plan.clone().into();
    let query = spec.queries.values_mut().next().unwrap();
    query.definition = w::NativeQueryDefinition::MacosApplications {
        root: w::MacosApplicationRoot::SystemApplications,
        max_depth: 4,
    };
    query.cardinality = w::NativeCardinality::List;
    query.instance_keys = vec!["id".into()];
    local.plan = w::NativeCollectionPlan::new(spec).unwrap();
    local.source = crate::collection_worker::ReadSource::local(&local.plan).unwrap();
    let local_ref = data.register(local.clone()).unwrap().descriptor.reference;
    let expected = data
        .software_coverage(&local_ref, "0")
        .unwrap()
        .into_values()
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 1);
    let task = enterprise_task(&client, &local);
    let task_ref = data
        .register(task.try_into().unwrap())
        .unwrap()
        .descriptor
        .reference;
    assert_ne!(local_ref, task_ref);
    assert_eq!(
        expected,
        data.software_coverage(&task_ref, "0")
            .unwrap()
            .into_values()
            .collect::<Vec<_>>()
    );
    let mut periodic =
        crate::protocol_test_support::telemetry::native_performance_configuration(&server, &r)
            .configuration;
    let (fields, mappings) = crate::protocol_test_support::projection::native(&local.plan);
    periodic.fields = fields;
    periodic.collector = w::TelemetryCollector::Native {
        plan_digest: local.plan.digest(),
        plan: local.plan.clone(),
        mappings,
    };
    let periodic_ref = data
        .register(ReadPlan::periodic(&periodic, periodic.expires_at).unwrap())
        .unwrap()
        .descriptor
        .reference;
    assert_ne!(periodic_ref, local_ref);
    assert_eq!(
        expected,
        data.software_coverage(&periodic_ref, "0")
            .unwrap()
            .into_values()
            .collect::<Vec<_>>()
    );
    let mut changed = local.clone();
    let mut spec: w::NativePlanSpec = changed.plan.clone().into();
    spec.queries.values_mut().next().unwrap().definition =
        w::NativeQueryDefinition::MacosPkgReceipts {};
    changed.plan = w::NativeCollectionPlan::new(spec).unwrap();
    changed.source = crate::collection_worker::ReadSource::local(&changed.plan).unwrap();
    let changed_ref = data.register(changed).unwrap().descriptor.reference;
    assert_ne!(
        expected,
        data.software_coverage(&changed_ref, "0")
            .unwrap()
            .into_values()
            .collect::<Vec<_>>()
    );
    let caller = execution_ipc::host::current_subject().unwrap();
    let mut spec: w::NativePlanSpec = local.plan.clone().into();
    spec.scope = rss_mdm_agent_wire::CollectionScope::User {
        identity: caller.clone(),
    };
    spec.queries.values_mut().next().unwrap().definition =
        w::NativeQueryDefinition::MacosApplications {
            root: w::MacosApplicationRoot::UserApplications,
            max_depth: 4,
        };
    local.plan = w::NativeCollectionPlan::new(spec).unwrap();
    local.source = crate::collection_worker::ReadSource::local(&local.plan).unwrap();
    local.execution = w::NativeExecutionBinding {
        identity: w::ExecutionIdentity::LoggedInUser,
        context_revision: 1,
        target: w::SoftwareExecutionTarget::User {
            identity: caller.clone(),
            session_id: uuid::Uuid::new_v4(),
        },
        generated_uid: account_uid(&caller).unwrap(),
    };
    let user_ref = data.register(local).unwrap().descriptor.reference;
    assert_ne!(
        expected,
        data.software_coverage(&user_ref, &caller)
            .unwrap()
            .into_values()
            .collect::<Vec<_>>()
    );
    if caller != "0" {
        assert!(data.software_coverage(&user_ref, "0").is_err());
    }
}
