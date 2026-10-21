//! Local TLS fixture for periodic collection; not a deployed MDM or platform T3.
use super::*;
#[derive(Default)]
pub struct State {
    pub configuration: Option<SignedTelemetryConfiguration>,
    pub receipts: BTreeMap<Uuid, TelemetryReceipt>,
    pub gap_receipts: BTreeMap<Uuid, TelemetryGapReceipt>,
    pub gap_bodies: BTreeMap<Uuid, Value>,
    pub gap_attempts: Vec<Value>,
    pub bodies: BTreeMap<Uuid, Value>,
    pub uploads: Vec<Value>,
    pub upload_attempts: Vec<Value>,
    pub lose_response: bool,
    pub throttled: bool,
    pub timed_out: bool,
}
pub fn response(d: &mut Data, path: &str, value: Value) -> Response {
    if path.ends_with("/batches") {
        d.telemetry.upload_attempts.push(value.clone());
    }
    if d.telemetry.timed_out {
        return StatusCode::REQUEST_TIMEOUT.into_response();
    }
    if d.telemetry.throttled {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "90")],
            axum::Json(json!({"code":"service_unavailable"})),
        )
            .into_response();
    }
    if path.ends_with("/gaps") {
        d.telemetry.gap_attempts.push(value.clone());
        let Ok(report) = serde_json::from_value::<TelemetryGapReport>(value.clone()) else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        let now = d.time.now().unwrap() * 1_000_000;
        if report.validate().is_err()
            || report.created_at + TELEMETRY_GAP_WINDOW_MICROS <= now
            || report.created_at > now + 300_000_000
        {
            return StatusCode::BAD_REQUEST.into_response();
        }
        if d.telemetry
            .gap_bodies
            .get(&report.report_id)
            .is_some_and(|old| old != &value)
        {
            return StatusCode::CONFLICT.into_response();
        }
        let replayed = d.telemetry.gap_receipts.contains_key(&report.report_id);
        d.telemetry.gap_bodies.insert(report.report_id, value);
        let mut receipt = d
            .telemetry
            .gap_receipts
            .entry(report.report_id)
            .or_insert(TelemetryGapReceipt {
                report_id: report.report_id,
                received_at: now,
                replayed: false,
            })
            .clone();
        receipt.replayed = replayed;
        if std::mem::take(&mut d.telemetry.lose_response) {
            return Response::builder()
                .status(200)
                .body(Body::from_stream(futures_util::stream::once(async {
                    Err::<Bytes, _>(std::io::Error::other("lost gap response"))
                })))
                .unwrap();
        }
        return axum::Json(receipt).into_response();
    }
    if path.contains("/gaps/") {
        return path
            .rsplit('/')
            .next()
            .and_then(|v| Uuid::parse_str(v).ok())
            .and_then(|id| d.telemetry.gap_receipts.get(&id))
            .map(|r| axum::Json(r.clone()).into_response())
            .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response());
    }
    if path.ends_with("/configurations") {
        let assignments = d
            .telemetry
            .configuration
            .as_ref()
            .map(|s| {
                vec![TelemetryAssignment {
                    policy: s.configuration.policy,
                    version: s.configuration.policy_version,
                    supported: true,
                }]
            })
            .unwrap_or_default();
        return axum::Json(TelemetryDirectory { assignments }).into_response();
    }
    if path.contains("/configurations/") {
        return d
            .telemetry
            .configuration
            .clone()
            .map(|s| axum::Json(s).into_response())
            .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response());
    }
    if path.ends_with("/batches") {
        let batch: TelemetryUpload = serde_json::from_value(value.clone()).unwrap();
        if d.telemetry
            .bodies
            .get(&batch.batch_id)
            .is_some_and(|b| b != &value)
        {
            return StatusCode::CONFLICT.into_response();
        }
        d.telemetry.uploads.push(value.clone());
        d.telemetry.bodies.insert(batch.batch_id, value);
        let receipt = d
            .telemetry
            .receipts
            .entry(batch.batch_id)
            .or_insert(TelemetryReceipt {
                batch_id: batch.batch_id,
                received_at: d.time.now().unwrap() * 1_000_000,
                replayed: false,
                delivery: TelemetryDelivery::Pending,
            })
            .clone();
        if std::mem::take(&mut d.telemetry.lose_response) {
            return Response::builder()
                .status(200)
                .body(Body::from_stream(futures_util::stream::once(async {
                    Err::<Bytes, _>(std::io::Error::other("lost response"))
                })))
                .unwrap();
        }
        return axum::Json(receipt).into_response();
    }
    let id = path
        .rsplit('/')
        .next()
        .and_then(|v| Uuid::parse_str(v).ok());
    id.and_then(|id| d.telemetry.receipts.get(&id))
        .map(|r| axum::Json(r.clone()).into_response())
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}
pub fn configuration(
    server: &Server,
    registration: &BindingProfile,
) -> SignedTelemetryConfiguration {
    let d = server.data.lock().unwrap();
    let now = server.time.now().unwrap();
    let policy = Uuid::new_v4();
    let configuration = TelemetryConfiguration {
        tenant_id: d.tenant,
        device_id: registration.device_id.clone(),
        registration_id: registration.registration_id,
        generation: registration.generation,
        source_epoch: registration.epoch,
        issued_at: now,
        expires_at: now + 300,
        policy,
        policy_version: Uuid::new_v4(),
        source: Source::AgentScript,
        binding: "a".repeat(64),
        template: TaskResource {
            id: "metric-script".into(),
            version: "v1".into(),
            variant: "default".into(),
        },
        collector: TelemetryCollector::Script {
            content: TaskContent {
                length: 4,
                sha256: [7; 32],
            },
            profile: ExecutorProfile::PosixSh,
            arguments: vec![],
            environment: Default::default(),
            sql_parameters: None,
            output_schema: json!({"type":"object","properties":{"load":{"type":"integer"}},"required":["load"],"additionalProperties":false}),
            timeout_seconds: 1,
            max_rows: 1,
            mappings: [(
                "custom.load".into(),
                TelemetryValueMapping {
                    pointer: "/load".into(),
                    conversion: TelemetryConversion::Identity,
                    columns: Default::default(),
                },
            )]
            .into(),
        },
        fields: vec![FieldDefinition {
            time_series: Some(TimeSeriesBinding {
                kind: TelemetryKind::Gauge,
                value_path: vec![],
                interval_start_path: None,
                dimensions: Default::default(),
                aggregations: [TelemetryAggregation::Mean].into(),
                retention: RetentionReference {
                    key: "standard".into(),
                    revision: 1,
                },
                max_resources: 1,
            }),
            key: FieldKey::parse("custom.load").unwrap(),
            version: 1,
            value_type: ValueType::Integer,
            nullable: false,
            manual: false,
            sources: [(Source::AgentScript, 0)].into(),
            platforms: [Platform::Macos].into(),
            sensitivity: Sensitivity::Standard,
            unit: Some("percent".into()),
            searchable: false,
            item_key: None,
        }],
        sample_millis: 5000,
        normal_sample_millis: 5000,
        boost_expires_at: None,
        upload_millis: 10000,
        max_bytes: 65536,
        max_points: 64,
        max_resources: 64,
        execution_millis: 1000,
        output_bytes: 4096,
        backfill_seconds: 3600,
    };
    sign(&d, configuration)
}
pub fn sign(d: &Data, configuration: TelemetryConfiguration) -> SignedTelemetryConfiguration {
    let key_id = "test".to_owned();
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        d.signer
            .sign(&configuration.signing_bytes(&key_id).unwrap())
            .as_ref(),
    );
    SignedTelemetryConfiguration {
        configuration,
        key_id,
        signature,
    }
}

/// One native performance lease for the real Agent wire/signature consumer seam.
pub fn native_performance_configuration(
    server: &Server,
    registration: &BindingProfile,
) -> SignedTelemetryConfiguration {
    let mut c = configuration(server, registration).configuration;
    let budget = NativeBudget {
        timeout_seconds: 2,
        output_bytes: 4096,
        max_rows: 1,
    };
    let section = PerformanceSection::Memory;
    let definition = if cfg!(target_os = "windows") {
        NativeQueryDefinition::WindowsPerformance { section }
    } else {
        NativeQueryDefinition::MacosPerformance { section }
    };
    let plan = NativeCollectionPlan::try_from(NativePlanSpec {
        version: 1,
        scope: rss_mdm_agent_wire::CollectionScope::Device,
        queries: [(
            "memory".into(),
            NativeQuery {
                version: 1,
                definition,
                cardinality: NativeCardinality::ExactlyOne,
                instance_keys: vec![],
                budget,
            },
        )]
        .into(),
        budget,
    })
    .unwrap();
    c.source = Source::AgentNative;
    c.fields[0].sources = [(Source::AgentNative, 0)].into();
    c.fields[0].unit = Some("bytes".into());
    c.execution_millis = 2000;
    c.collector = TelemetryCollector::Native {
        plan,
        mappings: [(
            "custom.load".into(),
            TelemetryNativeMapping {
                counter_normalization: None,
                query: "memory".into(),
                mapping: TelemetryValueMapping {
                    pointer: "/total_bytes".into(),
                    conversion: TelemetryConversion::Integer,
                    columns: Default::default(),
                },
            },
        )]
        .into(),
    };
    sign(&server.data.lock().unwrap(), c)
}

/// Signed native Network mapping: the Agent owns only the generated start property.
pub fn native_network_configuration(
    server: &Server,
    registration: &BindingProfile,
) -> SignedTelemetryConfiguration {
    let mut c = native_performance_configuration(server, registration).configuration;
    let TelemetryCollector::Native { plan, mappings } = &mut c.collector else {
        unreachable!()
    };
    let mut spec = plan.spec().clone();
    let query = spec.queries.get_mut("memory").unwrap();
    query.definition = if cfg!(target_os = "windows") {
        NativeQueryDefinition::WindowsPerformance {
            section: PerformanceSection::Network,
        }
    } else {
        NativeQueryDefinition::MacosPerformance {
            section: PerformanceSection::Network,
        }
    };
    query.cardinality = NativeCardinality::List;
    query.instance_keys = vec!["id".into()];
    query.budget.max_rows = 4;
    spec.budget.max_rows = 4;
    *plan = NativeCollectionPlan::try_from(spec).unwrap();
    let m = mappings.get_mut("custom.load").unwrap();
    m.counter_normalization = Some(NativeCounterNormalization::LocalCumulativeV1);
    m.mapping = TelemetryValueMapping {
        pointer: String::new(),
        conversion: TelemetryConversion::Identity,
        columns: [
            (
                "id".into(),
                TelemetryValueMapping {
                    pointer: "/id".into(),
                    conversion: TelemetryConversion::Identity,
                    columns: Default::default(),
                },
            ),
            (
                "bytes".into(),
                TelemetryValueMapping {
                    pointer: "/received_bytes".into(),
                    conversion: TelemetryConversion::Integer,
                    columns: Default::default(),
                },
            ),
        ]
        .into(),
    };
    let f = &mut c.fields[0];
    f.value_type = ValueType::Array {
        items: Box::new(ValueType::Object {
            properties: [
                (
                    "id".into(),
                    ValueType::String {
                        max_length: 256,
                        allow_empty: false,
                    },
                ),
                ("bytes".into(), ValueType::Integer),
                ("start".into(), ValueType::Integer),
            ]
            .into(),
        }),
        max_items: 4,
    };
    f.item_key = Some("id".into());
    let binding = f.time_series.as_mut().unwrap();
    binding.kind = TelemetryKind::CumulativeCounter;
    binding.aggregations = [TelemetryAggregation::Increase, TelemetryAggregation::Rate].into();
    binding.value_path = vec!["bytes".into()];
    binding.interval_start_path = Some(vec!["start".into()]);
    binding.max_resources = 4;
    sign(&server.data.lock().unwrap(), c)
}
