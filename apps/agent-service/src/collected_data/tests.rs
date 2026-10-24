use super::*;
use crate::protocol_test_support::{Root, Server};
use agent_client::OpenMode;
fn projection<S: agent_client::IdentityProvider, C: agent_client::Clock>(
    client: &agent_client::Client<S, C>,
) -> Data {
    let mut data = Data::new(
        client.collected_store().unwrap(),
        client.output_policy().unwrap(),
    );
    let (binding, _) = crate::backend::plan::context(
        client.configuration().service_id,
        client.configuration().tenant,
        &client.registration().unwrap(),
    )
    .unwrap();
    data.bind_enterprise(Some(&binding.authority)).unwrap();
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
            .publish(
                published_item(),
                PolicyLocator { after: None },
                "501",
                &session,
                &context,
            )
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
    read.plan = w::NativeCollectionPlan::new(spec).unwrap();
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
async fn telemetry_ack_removes_delivery_but_preserves_the_normalized_local_projection() {
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
    let query = data
        .directory("501")
        .unwrap()
        .into_iter()
        .find(|d| d.kind == QueryKind::Telemetry)
        .unwrap()
        .reference;
    spool.freeze_all(&c).unwrap();
    let pending = spool.pending(at / 1_000_000, false).unwrap().unwrap();
    spool.attempting(&pending, at / 1_000_000).unwrap();
    let receipt = client.telemetry_submit(&pending).await.unwrap();
    spool.acknowledge(&pending, &receipt).unwrap();
    assert_eq!(spool.summary().unwrap().pending, 0);
    drop(data);
    let data = projection(&client);
    let result = data
        .read(
            &DataRead {
                selection: DataSelection::Query { query },
                after: None,
                limit: 16,
                max_age_seconds: None,
            },
            "501",
            &id("login/1"),
            (at / 1_000_000) as u64,
        )
        .unwrap();
    assert_eq!(
        result.fields["custom.load"]["value"],
        json!({"kind":"integer","value":17})
    );
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
    let disk = w::NativeQueryDefinition::WindowsWmi {
        namespace: r"root\cimv2".into(),
        class: "Win32_DiskDrive".into(),
        properties: Default::default(),
        filter: None,
    };
    assert_eq!(
        mapped_row(
            &disk,
            &[
                ("DeviceID".into(), json!("disk0")),
                ("Model".into(), json!("native")),
                ("Size".into(), json!("1024"))
            ]
            .into()
        ),
        [
            ("id".into(), json!("disk0")),
            ("model".into(), json!("native")),
            ("size_bytes".into(), json!("1024"))
        ]
        .into()
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
        data.publish(
            item,
            PolicyLocator { after: None },
            "501",
            &session,
            &context,
        )
        .unwrap();
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
        .publish(
            published_item(),
            PolicyLocator { after: None },
            &subject,
            &old_session,
            &old_context,
        )
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
        .publish(
            published_item(),
            PolicyLocator { after: None },
            &subject,
            &session,
            &context,
        )
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
