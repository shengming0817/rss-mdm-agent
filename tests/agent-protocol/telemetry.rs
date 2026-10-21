//! Local TLS fixture for periodic collection; not a deployed MDM or platform T3.
use super::*;
#[derive(Default)]
pub struct State {
    pub configuration: Option<SignedTelemetryConfiguration>,
    pub receipts: BTreeMap<Uuid, TelemetryReceipt>,
    pub bodies: BTreeMap<Uuid, Value>,
    pub uploads: Vec<Value>,
    pub lose_response: bool,
    pub throttled: bool,
}
pub fn response(d: &mut Data, path: &str, value: Value) -> Response {
    if d.telemetry.throttled {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "90")],
            axum::Json(json!({"code":"service_unavailable"})),
        )
            .into_response();
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
