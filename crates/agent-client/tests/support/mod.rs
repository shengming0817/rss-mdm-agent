#![allow(dead_code)]
use agent_client::wire::*;
use agent_client::*;
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use base64::Engine;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    },
};
use uuid::Uuid;
pub struct Root {
    pub path: std::path::PathBuf,
}
impl Root {
    pub fn new() -> Self {
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("agent-test-{}", Uuid::new_v4()));
        native_process::private_storage::directory(&path).unwrap();
        Self { path }
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
#[derive(Clone)]
pub struct Time(pub Arc<AtomicI64>);
impl Clock for Time {
    fn now(&self) -> Result<i64, Error> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}
impl Time {
    pub fn set(&self, v: i64) {
        self.0.store(v, Ordering::SeqCst);
    }
}
#[derive(Clone)]
pub struct Secrets(pub Arc<Mutex<BTreeMap<String, String>>>);
impl Secrets {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(BTreeMap::from([
            (
                "password".into(),
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
            ),
            (
                "credential".into(),
                "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE".into(),
            ),
        ]))))
    }
}
impl SecretProvider for Secrets {
    fn resolve(&self, id: &str) -> Result<Secret, Error> {
        Secret::parse(self.0.lock().unwrap().get(id).ok_or(Error::Identity)?)
            .map_err(|_| Error::Identity)
    }
    fn credential(&self, id: &str) -> Result<Secret, Error> {
        self.resolve(id)
    }
}
pub struct Data {
    pub tenant: Uuid,
    pub registration: Uuid,
    pub epoch: Uuid,
    pub task: Uuid,
    pub attempt: Uuid,
    pub operation: Option<Uuid>,
    pub bytes: Vec<u8>,
    pub signer: Ed25519KeyPair,
    pub offer: Option<SignedTask>,
    pub time: Time,
    pub registration_failure: bool,
    pub report_failure: bool,
    pub start_failure: bool,
    pub result_failure: bool,
    pub bad_ack: bool,
    pub bad_etag: bool,
    pub bad_range: bool,
    pub range_ignored: bool,
    pub denied: bool,
    pub content_failure: bool,
    pub started: bool,
    pub received: bool,
    pub content_calls: Vec<Option<String>>,
    pub reports: BTreeMap<String, Value>,
    pub results: BTreeMap<String, Value>,
    pub start_ops: Vec<String>,
    pub forged_start: bool,
    pub start_permit: Option<SignedTask>,
}
impl Data {
    pub fn signed(&self, payload: TaskPayload) -> SignedTask {
        SignedTask {
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
                self.signer
                    .sign(&payload.signing_bytes("test").unwrap())
                    .as_ref(),
            ),
            payload,
            key_id: "test".into(),
        }
    }
    pub fn script(&mut self) {
        self.attempt = Uuid::new_v4();
        self.started = false;
        self.received = false;
        self.start_permit = None;
        let spec = TaskSpec {
            wire_version: 4,
            tenant_id: self.tenant,
            device_id: "device-1".into(),
            platform: TaskPlatform::Macos,
            architecture: TaskArchitecture::Aarch64,
            registration_id: self.registration,
            generation: 1,
            task_id: self.task,
            attempt_id: self.attempt,
            permit: TaskPermit::Offer,
            expires_at: self.time.now().unwrap() + 60,
            resource_digest: [3; 32],
            content: TaskContent {
                length: self.bytes.len() as u64,
                sha256: Sha256::digest(&self.bytes).into(),
            },
            profile: ExecutorProfile::PosixSh,
            run_as: ExecutionIdentity::System,
            arguments: vec![],
            environment: BTreeMap::new(),
            timeout_seconds: 30,
            output_bytes: 65536,
            max_rows: 1,
        };
        self.offer = Some(self.signed(TaskPayload::Script(spec)));
    }
    pub fn software(&mut self, steps: usize, user: bool) {
        let command = SoftwareTaskCommand {
            executor: SoftwareTaskExecutor::PackageInstaller,
            entry: None,
            run_as: ExecutionIdentity::System,
            arguments: vec![],
            environment: BTreeMap::new(),
            timeout_seconds: 30,
            output_bytes: 65536,
        };
        let steps: Vec<_> = (0..steps)
            .map(|i| SoftwareTaskStep {
                action: SoftwareTaskAction {
                    package: format!("fixture-{i}"),
                    version: "1.0".into(),
                    format: SoftwareTaskFormat::Pkg,
                    primary: "package".into(),
                    install: command.clone(),
                    uninstall: None,
                    detect: SoftwareTaskDetection::PkgReceipt {
                        receipt: format!("fixture-{i}"),
                        version: "1.0".into(),
                    },
                    reboot: SoftwareTaskReboot::Report,
                    downgrade: SoftwareTaskDowngrade::Deny,
                    ownership: SoftwareTaskOwnership::ManagedOnly,
                    bundle: None,
                },
                artifacts: vec![SoftwareTaskArtifact {
                    key: format!("{i}/package"),
                    length: self.bytes.len() as u64,
                    sha256: Sha256::digest(&self.bytes).into(),
                }],
                export_identity: None,
            })
            .collect();
        let spec = SoftwareTaskSpec {
            wire_version: 4,
            tenant_id: self.tenant,
            device_id: "device-1".into(),
            platform: TaskPlatform::Macos,
            architecture: TaskArchitecture::Aarch64,
            registration_id: self.registration,
            generation: 1,
            task_id: self.task,
            attempt_id: self.attempt,
            permit: TaskPermit::Offer,
            expires_at: self.time.now().unwrap() + 60,
            definition_digest: Sha256::digest(serde_json::to_vec(&steps).unwrap()).into(),
            steps,
            intent: SoftwareTaskIntent::Install,
            start_mode: if user {
                SoftwareStartMode::UserInitiated
            } else {
                SoftwareStartMode::Automatic
            },
        };
        self.offer = Some(self.signed(TaskPayload::Software(spec)));
    }
}
pub struct Server {
    pub url: url::Url,
    pub data: Arc<Mutex<Data>>,
    pub time: Time,
    pub secrets: Secrets,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    pub async fn new() -> Self {
        let time = Time(Arc::new(AtomicI64::new(1)));
        let signer = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let data = Arc::new(Mutex::new(Data {
            tenant: Uuid::new_v4(),
            registration: Uuid::new_v4(),
            epoch: Uuid::new_v4(),
            task: Uuid::new_v4(),
            attempt: Uuid::new_v4(),
            operation: None,
            bytes: b"{\"ok\":true}\n".repeat(4096),
            signer,
            offer: None,
            time: time.clone(),
            registration_failure: false,
            report_failure: false,
            start_failure: false,
            result_failure: false,
            bad_ack: false,
            bad_etag: false,
            bad_range: false,
            range_ignored: false,
            denied: false,
            content_failure: false,
            started: false,
            received: false,
            content_calls: vec![],
            reports: BTreeMap::new(),
            results: BTreeMap::new(),
            start_ops: vec![],
            forged_start: false,
            start_permit: None,
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = url::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let router = axum::Router::new()
            .fallback(handler)
            .with_state(data.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            url,
            data,
            time,
            secrets: Secrets::new(),
            task,
        }
    }
    pub fn config(&self) -> Config {
        let data = self.data.lock().unwrap();
        Config {
            origin: self.url.clone(),
            tenant: data.tenant,
            platform: TaskPlatform::Macos,
            architecture: TaskArchitecture::Aarch64,
            keys: BTreeMap::from([("test".into(), data.signer.public_key().as_ref().to_vec())]),
            limits: Limits::test_defaults(),
            transport: Transport::TestLoopback,
            ca_pem: None,
        }
    }
    pub fn client(&self, root: &Root, mode: OpenMode) -> Client<Secrets, Time> {
        Client::open(
            &root.path,
            self.config(),
            mode,
            self.secrets.clone(),
            self.time.clone(),
        )
        .unwrap()
    }
    pub async fn register(&self, client: &mut Client<Secrets, Time>) -> RegistrationReceipt {
        client
            .register(
                Uuid::new_v4(),
                Uuid::new_v4(),
                "password",
                "credential",
                vec![
                    Capability::InventoryBasicV4,
                    Capability::TaskExecuteV4,
                    Capability::SoftwareExecuteV4,
                ],
            )
            .await
            .unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn handler(
    State(data): State<Arc<Mutex<Data>>>,
    uri: Uri,
    headers: HeaderMap,
    bytes: Bytes,
) -> Response {
    let mut d = data.lock().unwrap();
    let path = uri.path();
    if path != "/api/agent/v4/registrations"
        && (d.denied
            || headers.get("authorization").and_then(|v| v.to_str().ok())
                != Some("Bearer AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"))
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if path.ends_with("/registrations") {
        let operation = Uuid::parse_str(value["operationId"].as_str().unwrap()).unwrap();
        if d.operation.is_some_and(|old| old != operation) {
            return StatusCode::CONFLICT.into_response();
        }
        d.operation = Some(operation);
        if std::mem::take(&mut d.registration_failure) {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        return (StatusCode::CREATED,axum::Json(json!({"wireVersion":4,"operationId":operation,"deviceId":"device-1","registrationId":d.registration,"generation":1,"source":"agent.builtin","epoch":d.epoch,"capabilities":value["capabilities"]}))).into_response();
    }
    if path.ends_with("/reports") {
        let id = value["reportId"].as_str().unwrap().to_owned();
        if let Some(old) = d.reports.get(&id) {
            if old != &value {
                return StatusCode::CONFLICT.into_response();
            }
        }
        d.reports.insert(id.clone(), value);
        if std::mem::take(&mut d.report_failure) {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        return (StatusCode::ACCEPTED,axum::Json(json!({"wireVersion":4,"reportId":if d.bad_ack{Uuid::new_v4().to_string()}else{id},"receivedAt":1,"intake":"durable"}))).into_response();
    }
    if path.ends_with("/claim") {
        if d.offer.is_none() {
            d.script();
        }
        return axum::Json(TaskClaimResponse::new(d.offer.clone(), vec![]).unwrap())
            .into_response();
    }
    if path.ends_with("/events") {
        let request: TaskEventRequest = serde_json::from_value(value.clone()).unwrap();
        if request.attempt_id() != d.attempt {
            return StatusCode::CONFLICT.into_response();
        }
        let operation = request.operation_id().to_string();
        match request.event() {
            TaskEvent::Received => {
                d.received = true;
                axum::Json(TaskEventAck::new(None, false)).into_response()
            }
            TaskEvent::Start => {
                d.started = true;
                d.start_ops.push(operation);
                if d.start_permit.is_none() {
                    let mut payload = d.offer.as_ref().unwrap().payload.clone();
                    let expiry = d.time.now().unwrap() + 15;
                    match &mut payload {
                        TaskPayload::Script(v) => {
                            v.permit = TaskPermit::Start;
                            v.expires_at = expiry;
                        }
                        TaskPayload::Software(v) => {
                            v.permit = TaskPermit::Start;
                            v.expires_at = expiry;
                        }
                        _ => unreachable!(),
                    }
                    d.start_permit = Some(d.signed(payload));
                }
                if std::mem::take(&mut d.start_failure) {
                    return StatusCode::SERVICE_UNAVAILABLE.into_response();
                }
                let mut signed = d.start_permit.clone().unwrap();
                if d.forged_start {
                    let mut payload = signed.payload.clone();
                    if let TaskPayload::Script(v) = &mut payload {
                        v.arguments.push("changed".into());
                    }
                    signed = d.signed(payload);
                }
                axum::Json(TaskEventAck::new(Some(signed), false)).into_response()
            }
            _ => {
                if d.results.values().any(|old| old != &value) {
                    return StatusCode::CONFLICT.into_response();
                }
                d.results.insert(operation, value);
                if std::mem::take(&mut d.result_failure) {
                    return StatusCode::SERVICE_UNAVAILABLE.into_response();
                }
                axum::Json(TaskEventAck::new(None, false)).into_response()
            }
        }
    } else if path.ends_with("/content") {
        let range = headers
            .get("range")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        d.content_calls.push(range.clone());
        if std::mem::take(&mut d.content_failure) {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        let start = if d.range_ignored {
            0
        } else {
            range
                .as_deref()
                .and_then(|v| v.strip_prefix("bytes="))
                .and_then(|v| v.strip_suffix('-'))
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
        };
        if start >= d.bytes.len() {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        }
        let etag = if d.bad_etag {
            "\"wrong\"".into()
        } else {
            format!("\"{:x}\"", Sha256::digest(&d.bytes))
        };
        let mut response = Response::builder()
            .status(if start > 0 { 206 } else { 200 })
            .header("content-length", d.bytes.len() - start)
            .header("etag", etag);
        if start > 0 {
            response = response.header(
                "content-range",
                format!(
                    "bytes {}-{}/{}",
                    if d.bad_range { start + 1 } else { start },
                    d.bytes.len() - 1,
                    d.bytes.len()
                ),
            );
        }
        response
            .body(Body::from(d.bytes[start..].to_vec()))
            .unwrap()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

pub mod execution;
