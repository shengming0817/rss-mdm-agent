#![allow(dead_code)]
use agent_client::protocol::*;
pub(crate) mod pki;
use agent_api::{enroll, runtime::*};
use agent_client::*;
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, State},
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
        platform_private_storage::directory(&path).unwrap();
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
impl IdentityProvider for Secrets {
    fn resolve(&self, id: &str) -> Result<Secret, Error> {
        Secret::parse(self.0.lock().unwrap().get(id).ok_or(Error::Identity)?)
            .map_err(|_| Error::Identity)
    }
    fn import_secret(&self, id: &str, secret: &Secret) -> Result<(), Error> {
        let mut values = self.0.lock().unwrap();
        if values.get(id).is_some_and(|old| old != secret.expose()) {
            return Err(Error::Conflict);
        }
        values.insert(id.into(), secret.expose().into());
        Ok(())
    }
    fn create_secret(&self, id: &str) -> Result<Secret, Error> {
        self.0
            .lock()
            .unwrap()
            .entry(id.into())
            .or_insert_with(|| "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into());
        self.resolve(id)
    }
    fn create_key(&self, id: &str) -> Result<Arc<dyn rustls::sign::SigningKey>, Error> {
        let mut values = self.0.lock().unwrap();
        let next = if values.values().any(|v| v == "fixture-key") {
            "fixture-renewal-key"
        } else {
            "fixture-key"
        };
        values.entry(id.into()).or_insert_with(|| next.into());
        drop(values);
        self.key(id)
    }
    fn key(&self, id: &str) -> Result<Arc<dyn rustls::sign::SigningKey>, Error> {
        match self.0.lock().unwrap().get(id).map(String::as_str) {
            Some("fixture-key") => Ok(pki::key()),
            Some("fixture-renewal-key") => Ok(pki::renewal_key()),
            _ => Err(Error::Identity),
        }
    }
}
pub struct Data {
    pub tls_required: bool,
    pub employee_calls: Vec<(String, Value)>,
    pub self_service_directory: Option<SelfServiceDirectoryResponse>,
    pub invocations: BTreeMap<Uuid, (Value, SelfServiceAccepted)>,
    pub invocation_reply_lost: bool,
    pub invocation_unaccepted: bool,
    pub response_override: Option<(String, Value)>,
    pub grant: Option<enroll::GrantReceipt>,
    pub issuance: Uuid,
    pub candidate_leaf: Option<Vec<u8>>,
    pub profile: Option<enroll::RuntimeProfile>,
    pub activation: Option<enroll::ActivationReceipt>,
    pub binding: Option<BindingProfile>,
    pub bootstrap: Option<String>,
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
    pub certificate_failure: bool,
    pub activation_failure: bool,
    pub issuance_unknown: bool,
    pub certificate_calls: Vec<Value>,
    pub activation_calls: Vec<Value>,
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
    pub start_validity_seconds: i64,
    pub result_calls: usize,
    pub chunks: BTreeMap<u16, OutputChunk>,
    pub chunk_calls: Vec<Value>,
    pub chunk_failure: bool,
    pub acknowledged: std::collections::BTreeSet<String>,
    pub cancellations: Vec<TaskCancellation>,
    pub result_hook: Option<Arc<dyn Fn() + Send + Sync>>,
    pub claim_failure: bool,
    pub claim_pause: Option<Arc<tokio::sync::Notify>>,
    pub claim_waiting: bool,
    pub explicit_offers: bool,
    pub claim_ops: Vec<String>,
    pub claim_inputs: Vec<Value>,
    pub claims: BTreeMap<String, SignedTask>,
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
        self.reset_current_task();
        let spec = TaskSpec {
            wire_version: 1,
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
            resource: ScriptResource {
                id: "fixture-resource".into(),
                version: "v1".into(),
                variant: "default".into(),
            },
            authorization: ScriptAuthorization::Policy {
                policy_id: self.task,
                policy_version: self.task,
            },
            parameters: BTreeMap::new(),
            deadline: self.time.now().unwrap() + 300,
            content: TaskContent {
                length: self.bytes.len() as u64,
                sha256: Sha256::digest(&self.bytes).into(),
            },
            profile: ExecutorProfile::PosixSh,
            sql_parameters: None,
            run_as: ExecutionIdentity::System,
            arguments: vec![],
            environment: BTreeMap::new(),
            timeout_seconds: 30,
            output_bytes: 65536,
            max_rows: 1,
        };
        self.offer = Some(self.signed(TaskPayload::TaskSpec(Box::new(spec))));
    }
    pub fn software(&mut self, steps: usize, user: bool) {
        self.reset_current_task();
        let command = SoftwareTaskInvocation {
            run_as: ExecutionIdentity::System,
            arguments: vec![],
            environment: BTreeMap::new(),
            timeout_seconds: 30,
            output_bytes: 65536,
            exit_codes: SoftwareTaskExitCodes {
                success: [0].into(),
                reboot: Default::default(),
            },
        };
        let steps: Vec<_> = (0..steps)
            .map(|i| SoftwareTaskStep {
                action: SoftwareTaskAction {
                    package: format!("fixture-{i}"),
                    version: "1.0".into(),
                    behavior: SoftwareTaskBehavior::Pkg(SoftwareTaskNative {
                        installer: "package".into(),
                        scope: SoftwareTaskScope::System,
                        install: command.clone(),
                        upgrade_invocation: command.clone(),
                        upgrade: SoftwareTaskUpgrade::InPlace,
                        uninstall: None,
                        detect: SoftwareTaskDetection::PkgReceipt {
                            receipt: format!("fixture-{i}"),
                            version: "1.0".into(),
                        },
                    }),
                    signatures: vec![],
                    reboot: SoftwareTaskReboot::Report,
                    downgrade: SoftwareTaskDowngrade::Deny,
                },
                artifacts: vec![SoftwareTaskArtifact {
                    key: format!("{i}/package"),
                    length: self.bytes.len() as u64,
                    sha256: Sha256::digest(&self.bytes).into(),
                }],
                export: SoftwareTaskExport::Direct,
                target: SoftwareExecutionTarget::Device,
            })
            .collect();
        let spec = SoftwareTaskSpec {
            execution_context: execution_context(),
            wire_version: 1,
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
        self.offer = Some(self.signed(TaskPayload::SoftwareTaskSpec(spec)));
    }
    fn reset_current_task(&mut self) {
        self.started = false;
        self.received = false;
        self.start_permit = None;
        self.chunks.clear();
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
        Self::listen(false).await
    }
    pub async fn tls() -> Self {
        Self::listen(true).await
    }
    async fn listen(tls: bool) -> Self {
        let time = Time(Arc::new(AtomicI64::new(1)));
        let signer = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let data = Arc::new(Mutex::new(Data {
            tls_required: tls,
            employee_calls: vec![],
            self_service_directory: None,
            invocations: BTreeMap::new(),
            invocation_reply_lost: false,
            invocation_unaccepted: false,
            response_override: None,
            grant: None,
            issuance: Uuid::new_v4(),
            candidate_leaf: None,
            profile: None,
            activation: None,
            binding: None,
            bootstrap: None,
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
            certificate_failure: false,
            activation_failure: false,
            issuance_unknown: false,
            certificate_calls: vec![],
            activation_calls: vec![],
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
            start_validity_seconds: 15,
            result_calls: 0,
            chunks: BTreeMap::new(),
            chunk_calls: vec![],
            chunk_failure: false,
            acknowledged: Default::default(),
            cancellations: Vec::new(),
            result_hook: None,
            claim_failure: false,
            claim_pause: None,
            claim_waiting: false,
            explicit_offers: false,
            claim_ops: vec![],
            claim_inputs: vec![],
            claims: BTreeMap::new(),
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = url::Url::parse(&format!(
            "{}://{}/",
            if tls { "https" } else { "http" },
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let router = axum::Router::new()
            .fallback(handler)
            .with_state(data.clone());
        let task = tokio::spawn(async move {
            if tls {
                let listener = TlsListener {
                    listener,
                    acceptor: tokio_rustls::TlsAcceptor::from(Arc::new(pki::server())),
                };
                axum::serve(
                    listener,
                    router.into_make_service_with_connect_info::<TlsIdentity>(),
                )
                .await
                .unwrap();
            } else {
                axum::serve(listener, router).await.unwrap();
            }
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
            execution_context: execution_context(),
            origin: self.url.clone(),
            tenant: data.tenant,
            platform: TaskPlatform::Macos,
            architecture: TaskArchitecture::Aarch64,
            keys: BTreeMap::from([("test".into(), data.signer.public_key().as_ref().to_vec())]),
            limits: Limits::test_defaults(),
            transport: if data.tls_required {
                Transport::Https
            } else {
                Transport::TestLoopback
            },
            ca_pem: data.tls_required.then(pki::ca),
            device_ca_pem: pki::ca(),
        }
    }
    pub fn client(&self, root: &Root, mode: OpenMode) -> Client<Secrets, Time> {
        let mut client = Client::open(
            &root.path,
            self.config(),
            mode,
            self.secrets.clone(),
            self.time.clone(),
        )
        .unwrap();
        client.set_profiles(vec![ExecutorProfile::PosixSh]).unwrap();
        client
    }
    pub async fn register(&self, client: &mut Client<Secrets, Time>) -> BindingProfile {
        client
            .request_authorization(vec![
                Capability::InventoryCollectionV1,
                Capability::TaskExecuteV1,
                Capability::SoftwarePkgSystemV1,
            ])
            .await
            .unwrap();
        client.registration().unwrap()
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
    peer: Result<ConnectInfo<TlsIdentity>, axum::extract::rejection::ExtensionRejection>,
    bytes: Bytes,
) -> Response {
    let pause = {
        let mut d = data.lock().unwrap();
        if uri.path().ends_with("/claim") && d.claim_pause.is_some() {
            d.claim_waiting = true;
            d.claim_pause.clone()
        } else {
            None
        }
    };
    if let Some(pause) = pause {
        pause.notified().await;
    }
    let mut d = data.lock().unwrap();
    let path = uri.path();
    let peer_leaf = peer.ok().and_then(|v| v.0 .0);
    let authenticated = peer_leaf.is_some();
    if d.tls_required
        && (path.ends_with("/activate")
            || path.ends_with("/renewal-grants")
            || path.starts_with("/api/v1/agent/runtime/"))
        && !authenticated
    {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if headers.contains_key("authorization") {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    if path.contains("/self-service/") {
        return employee_response(&mut d, &uri, &headers, value);
    }
    if let Some((suffix, body)) = &d.response_override {
        if path.ends_with(suffix) {
            return axum::Json(body.clone()).into_response();
        }
    }
    if let Some(action) = path.strip_prefix("/api/v1/agent/enroll/") {
        return enrollment_response(&mut d, action, value, peer_leaf.as_deref());
    }
    if !path.starts_with("/api/v1/agent/runtime/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if d.denied {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::CredentialRevoked);
    }
    if d.activation.is_none() {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    let response = if path.ends_with("/reports") {
        report_response(&mut d, value)
    } else if path.ends_with("/claim") {
        claim_response(&mut d, value)
    } else if path.ends_with("/events") {
        event_response(&mut d, value)
    } else if path.ends_with("/content") {
        content_response(&mut d, headers)
    } else {
        StatusCode::NOT_FOUND.into_response()
    };
    if response.status().is_success() {
        response
    } else {
        let status = response.status();
        runtime_error(
            status,
            if status == StatusCode::CONFLICT {
                ErrorCode::OperationConflict
            } else {
                ErrorCode::ServiceUnavailable
            },
        )
    }
}
fn runtime_error(status: StatusCode, code: ErrorCode) -> Response {
    (status, axum::Json(ErrorBody { code })).into_response()
}
fn enrollment_response(d: &mut Data, action: &str, value: Value, peer: Option<&[u8]>) -> Response {
    match action {
        "requests" | "renewal-grants" => {
            let request: enroll::GrantRequest = serde_json::from_value(value).unwrap();
            if action == "renewal-grants" && d.operation != Some(request.operation_id) {
                assert!(d.binding.is_some(), "renewal requires an active binding");
                d.operation = None;
                d.grant = None;
                d.activation = None;
                d.profile = None;
                d.issuance = Uuid::new_v4();
                d.candidate_leaf = None;
            }
            if d.operation.is_some_and(|old| old != request.operation_id) {
                return (
                    StatusCode::CONFLICT,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::OperationConflict,
                        issuance_id: None,
                    }),
                )
                    .into_response();
            }
            d.operation = Some(request.operation_id);
            d.bootstrap = Some(request.secret.expose().into());
            let grant = d.grant.get_or_insert_with(|| enroll::GrantReceipt {
                wire_version: 1,
                grant_id: Uuid::new_v4(),
                state: enroll::GrantState::Available,
                platform: request.platform,
                expires_at: d.time.now().unwrap() + 300,
            });
            if std::mem::take(&mut d.registration_failure) {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::ServiceUnavailable,
                        issuance_id: None,
                    }),
                )
                    .into_response();
            }
            axum::Json(grant.clone()).into_response()
        }
        "status" => {
            let request: enroll::StatusRequest = serde_json::from_value(value).unwrap();
            assert_eq!(Some(request.operation_id), d.operation);
            assert_eq!(Some(request.secret.expose()), d.bootstrap.as_deref());
            axum::Json(enroll::Progress {
                grant: d.grant.clone().unwrap(),
                issuance_id: d.profile.as_ref().map(|_| d.issuance),
                issuance_unknown: d.issuance_unknown && d.profile.is_some(),
                activation: d.activation.clone(),
            })
            .into_response()
        }
        "certificates" => {
            d.certificate_calls.push(value.clone());
            let request: enroll::CertificateRequest = serde_json::from_value(value).unwrap();
            assert_eq!(Some(request.secret.expose()), d.bootstrap.as_deref());
            d.profile = Some(request.runtime.clone());
            if d.issuance_unknown {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::IssuanceUnknown,
                        issuance_id: Some(d.issuance),
                    }),
                )
                    .into_response();
            }
            d.profile = Some(request.runtime);
            d.grant.as_mut().unwrap().state = enroll::GrantState::Consumed;
            if std::mem::take(&mut d.certificate_failure) {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::ServiceUnavailable,
                        issuance_id: Some(d.issuance),
                    }),
                )
                    .into_response();
            }
            let chain = {
                use der::{Decode, Encode};
                let csr = x509_cert::request::CertReq::from_der(
                    &base64::engine::general_purpose::STANDARD
                        .decode(&request.csr)
                        .unwrap(),
                )
                .unwrap();
                pki::chain_for(d.tenant, csr.info.public_key.to_der().unwrap())
            };
            d.candidate_leaf = Some(
                base64::engine::general_purpose::STANDARD
                    .decode(&chain[0])
                    .unwrap(),
            );
            axum::Json(enroll::Candidate {
                wire_version: 1,
                issuance_id: d.issuance,
                operation_id: request.operation_id,
                certificate_chain: chain,
                activate_before: d.grant.as_ref().unwrap().expires_at,
            })
            .into_response()
        }
        "activate" => {
            if d.tls_required && (peer.is_none() || peer != d.candidate_leaf.as_deref()) {
                return (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::InvalidIdentity,
                        issuance_id: Some(d.issuance),
                    }),
                )
                    .into_response();
            }
            d.activation_calls.push(value.clone());
            let request: enroll::ActivationRequest = serde_json::from_value(value).unwrap();
            assert_eq!(request.issuance_id, d.issuance);
            let profile: BindingProfile = d.binding.clone().unwrap_or_else(|| serde_json::from_value(json!({"wireVersion":1,"operationId":d.operation,"deviceId":"device-1","registrationId":d.registration,"generation":1,"source":"agent.builtin","epoch":d.epoch,"capabilities":d.profile.as_ref().unwrap().capabilities,"collections":collections()})).unwrap());
            d.binding = Some(profile.clone());
            let receipt = enroll::ActivationReceipt {
                wire_version: 1,
                issuance_id: d.issuance,
                operation_id: request.operation_id,
                runtime: profile,
            };
            d.activation = Some(receipt.clone());
            if std::mem::take(&mut d.activation_failure) {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::ActivationUnknown,
                        issuance_id: Some(d.issuance),
                    }),
                )
                    .into_response();
            }
            axum::Json(receipt).into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
fn report_response(d: &mut Data, value: Value) -> Response {
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
    (StatusCode::ACCEPTED,axum::Json(json!({"wireVersion":1,"reportId":if d.bad_ack{Uuid::new_v4().to_string()}else{id},"receivedAt":1,"intake":"durable"}))).into_response()
}
fn claim_response(d: &mut Data, value: Value) -> Response {
    let _: TaskClaimRequest = serde_json::from_value(value.clone()).unwrap();
    d.claim_inputs.push(value.clone());
    let op = value["operationId"].as_str().unwrap().to_owned();
    d.claim_ops.push(op.clone());
    let signed = if let Some(old) = d.claims.get(&op) {
        if old.payload.expires_at() <= d.time.now().unwrap() {
            return StatusCode::CONFLICT.into_response();
        }
        old.clone()
    } else {
        // A new claim never re-offers an already acknowledged terminal attempt. Retain
        // the old-operation branch above for exact idempotent claim replay.
        if d.explicit_offers
            && d.offer.as_ref().is_some_and(|offer| {
                let attempt = offer.payload.attempt_id().to_string();
                d.results.iter().any(|(operation, result)| {
                    d.acknowledged.contains(operation) && result["attemptId"] == attempt
                })
            })
        {
            return axum::Json(TaskClaimResponse::new(None, d.cancellations.clone()).unwrap())
                .into_response();
        }
        if d.offer
            .as_ref()
            .is_none_or(|v| v.payload.expires_at() <= d.time.now().unwrap())
        {
            if d.explicit_offers {
                return axum::Json(TaskClaimResponse::new(None, d.cancellations.clone()).unwrap())
                    .into_response();
            }
            d.script();
        }
        let mut signed = d.offer.clone().unwrap();
        if d.explicit_offers
            && !d
                .claims
                .values()
                .any(|v| v.payload.task_id() == signed.payload.task_id())
        {
            if let TaskPayload::SoftwareTaskSpec(mut spec) = signed.payload.clone() {
                spec.execution_context =
                    serde_json::from_value(value["executionContext"].clone()).unwrap();
                signed = d.signed(TaskPayload::SoftwareTaskSpec(spec));
                d.offer = Some(signed.clone());
            }
        }
        d.claims.insert(op, signed.clone());
        signed
    };
    if std::mem::take(&mut d.claim_failure) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    axum::Json(TaskClaimResponse::new(Some(signed), d.cancellations.clone()).unwrap())
        .into_response()
}
fn event_response(d: &mut Data, value: Value) -> Response {
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
        TaskEvent::Start => start_event(d, operation),
        TaskEvent::OutputChunk(chunk) => {
            d.chunk_calls.push(value);
            if d.chunks.get(&chunk.index()).is_some_and(|v| v != chunk) {
                return StatusCode::CONFLICT.into_response();
            }
            d.chunks.insert(chunk.index(), chunk.clone());
            if std::mem::take(&mut d.chunk_failure) {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            axum::Json(TaskEventAck::new(None, false)).into_response()
        }
        _ => {
            d.result_calls += 1;
            if d.results
                .values()
                .any(|old| old["attemptId"] == value["attemptId"] && old != &value)
            {
                return StatusCode::CONFLICT.into_response();
            }
            d.results.insert(operation.clone(), value);
            if std::mem::take(&mut d.result_failure) {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            if let Some(hook) = d.result_hook.take() {
                hook();
            }
            d.acknowledged.insert(operation);
            axum::Json(TaskEventAck::new(None, false)).into_response()
        }
    }
}
fn start_event(d: &mut Data, operation: String) -> Response {
    d.started = true;
    d.start_ops.push(operation);
    if d.start_permit.is_none() {
        let mut payload = d.offer.as_ref().unwrap().payload.clone();
        let expiry = d.time.now().unwrap() + d.start_validity_seconds;
        match &mut payload {
            TaskPayload::TaskSpec(v) => {
                v.permit = TaskPermit::Start;
                v.expires_at = expiry;
            }
            TaskPayload::SoftwareTaskSpec(v) => {
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
        if let TaskPayload::TaskSpec(v) = &mut payload {
            v.arguments.push("changed".into());
        }
        signed = d.signed(payload);
    }
    axum::Json(TaskEventAck::new(Some(signed), false)).into_response()
}
fn content_response(d: &mut Data, headers: HeaderMap) -> Response {
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
}

pub fn collections() -> Vec<agent_api::runtime::CollectionDefinition> {
    let fields:Vec<_>=["device.model","device.os.version"].into_iter().map(|key|json!({
        "key":key,"version":1,"valueType":{"kind":"string","maxLength":256,"allowEmpty":false},
        "nullable":false,"manual":false,"sources":{"agent.builtin":0},"platforms":["windows","macos"],
        "sensitivity":"standard","unit":null,"searchable":true,"itemKey":null
    })).collect();
    vec![serde_json::from_value(
        json!({"dataset":"inventory","version":"1","source":"agent.builtin","fields":fields}),
    )
    .unwrap()]
}

pub fn execution_context() -> SoftwareExecutionContext {
    SoftwareExecutionContext {
        revision: 1,
        os_version: [14, 0, 0, 0],
        system_broker: true,
        interactive_user: None,
        source_credentials: vec![],
        msix_sideload: false,
        msix_unsigned: false,
    }
}

#[derive(Clone, Debug)]
struct TlsIdentity(Option<Vec<u8>>);
struct TlsListener {
    listener: tokio::net::TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
}
impl axum::serve::Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
    type Addr = TlsIdentity;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (stream, _) = self.listener.accept().await.unwrap();
            if let Ok(Ok(stream)) = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                self.acceptor.accept(stream),
            )
            .await
            {
                // ref: rustls 0.23.45 common_state.rs peer_certificates: the first entry
                // is the authenticated peer leaf, including resumed handshakes.
                let leaf = stream
                    .get_ref()
                    .1
                    .peer_certificates()
                    .and_then(|v| v.first())
                    .map(|leaf| leaf.as_ref().to_vec());
                return (stream, TlsIdentity(leaf));
            }
        }
    }
    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Ok(TlsIdentity(None))
    }
}
impl axum::extract::connect_info::Connected<axum::serve::IncomingStream<'_, TlsListener>>
    for TlsIdentity
{
    fn connect_info(stream: axum::serve::IncomingStream<'_, TlsListener>) -> Self {
        stream.remote_addr().clone()
    }
}

/// Explicit protocol fixture, never production Policy authority.
pub fn policy_directory_fixture() -> SelfServiceDirectoryItem {
    serde_json::from_value(json!({
        "policyId": Uuid::new_v4(), "policyRevision": 2, "policyVersion": Uuid::new_v4(),
        "selfService": {"published":true, "displayName":"Repair", "description":"Test repair", "prerequisites":"Employee session", "sideEffects":"Restarts a fixture", "category":"Support", "keywords":["repair"], "allowAi":true, "riskLevel":2},
        "resource":{"id":Uuid::new_v4().to_string(),"version":"v1","variant":"default"},
        "parameterSources":{"name":{"kind":"input"}, "path":{"kind":"fixed","value":"/Applications"}},
        "parameterSchema":{"type":"object","properties":{"name":{"type":"string","minLength":1,"maxLength":32},"path":{"type":"string","const":"/Applications"}},"required":["name","path"],"additionalProperties":false}
    })).unwrap()
}

fn employee_response(d: &mut Data, uri: &Uri, headers: &HeaderMap, value: Value) -> Response {
    let path = uri.path();
    if d.activation.is_none()
        || headers
            .get("x-rss-user-session")
            .and_then(|v| v.to_str().ok())
            != Some(&"c".repeat(64))
        || headers.contains_key("cookie")
    {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    d.employee_calls.push((uri.to_string(), value.clone()));
    if let Some((suffix, body)) = &d.response_override {
        if path.ends_with(suffix) {
            return axum::Json(body.clone()).into_response();
        }
    }
    if path.ends_with("/scripts") {
        return axum::Json(d.self_service_directory.clone().unwrap_or(
            SelfServiceDirectoryResponse {
                items: vec![],
                next_cursor: None,
                as_of: d.time.now().unwrap(),
            },
        ))
        .into_response();
    }
    if path.ends_with("/invocations") {
        let Ok(input) = serde_json::from_value::<SelfServiceInvoke>(value.clone()) else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        if d.invocation_unaccepted {
            return runtime_error(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::ServiceUnavailable,
            );
        }
        let accepted = if let Some((old, accepted)) = d.invocations.get(&input.operation_id) {
            if old != &value {
                return StatusCode::CONFLICT.into_response();
            }
            accepted.clone()
        } else {
            let accepted = SelfServiceAccepted {
                request_id: input.operation_id,
                run_id: Uuid::new_v4(),
                policy_id: input.policy_id,
                policy_revision: input.policy_revision,
                policy_version: input.policy_version,
            };
            d.invocations
                .insert(input.operation_id, (value, accepted.clone()));
            accepted
        };
        if std::mem::take(&mut d.invocation_reply_lost) {
            return runtime_error(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::ServiceUnavailable,
            );
        }
        return (StatusCode::ACCEPTED, axum::Json(accepted)).into_response();
    }
    if let Some(id) = path
        .rsplit('/')
        .next()
        .and_then(|id| Uuid::parse_str(id).ok())
    {
        if let Some((_, accepted)) = d.invocations.get(&id) {
            return axum::Json(SelfServiceStatus {
                request_id: id,
                run_id: accepted.run_id,
                attempt_id: None,
                state: ScriptRunState {
                    delivery: ScriptDelivery::Queued,
                    execution: ScriptExecution::NotStarted,
                    cancellation: ScriptCancellation::None,
                    deadline: d.time.now().unwrap() + 300,
                    started_at: None,
                },
                available_at: d.time.now().unwrap(),
                deadline: d.time.now().unwrap() + 300,
                result: None,
            })
            .into_response();
        }
    }
    runtime_error(StatusCode::NOT_FOUND, ErrorCode::TaskNotFound)
}
