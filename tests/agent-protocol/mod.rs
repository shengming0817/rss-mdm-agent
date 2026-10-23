#![allow(dead_code)]
pub mod current_user;
pub(crate) mod pki;
pub mod software_native;
pub mod telemetry;
use agent_client::*;
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use base64::Engine;
use ring::signature::{Ed25519KeyPair, KeyPair};
use rss_mdm_agent_wire::{enroll, runtime::*};
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
#[derive(Clone, Debug)]
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
impl rustls::time_provider::TimeProvider for Time {
    fn current_time(&self) -> Option<rustls::pki_types::UnixTime> {
        self.now()
            .ok()
            .and_then(|now| u64::try_from(now).ok())
            .map(|now| {
                rustls::pki_types::UnixTime::since_unix_epoch(std::time::Duration::from_secs(now))
            })
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
    fn software_read_token(
        &self,
        credential: &SoftwareReadCredential,
    ) -> Result<zeroize::Zeroizing<String>, Error> {
        let values = self.0.lock().unwrap();
        Ok(zeroize::Zeroizing::new(
            values
                .get(&agent_client::native_source::credential_key(credential)?)
                .ok_or(Error::Identity)?
                .clone(),
        ))
    }
    fn native_identities(&self) -> Result<Vec<agent_client::NativeIdentity>, Error> {
        let values = self.0.lock().unwrap();
        let Some(installation) = values.get("managed-installation") else {
            return Ok(vec![]);
        };
        let installation = Uuid::parse_str(installation).map_err(|_| Error::Identity)?;
        let attempt = Uuid::parse_str(values.get("managed-attempt").ok_or(Error::Identity)?)
            .map_err(|_| Error::Identity)?;
        Ok(vec![agent_client::NativeIdentity {
            chain: pki::bootstrap_chain(installation, attempt),
            key: pki::renewal_key(),
        }])
    }

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
    pub current_user: current_user::RelationFixture,
    pub telemetry: telemetry::State,
    pub software_source: Option<software_native::Source>,
    pub trust: Value,
    pub task_trust: Value,
    pub task_trust_ack_calls: Vec<Value>,
    pub task_trust_failure: bool,
    pub task_trust_ack_failure: bool,
    pub observations: Vec<Value>,
    pub trust_calls: usize,
    pub trust_ack_calls: Vec<Value>,
    pub trust_ack_failure: bool,
    pub trust_failure: bool,
    pub renewal_lookup_calls: Vec<Value>,
    pub renewal_lookup_authenticated: Vec<bool>,
    pub tls_required: bool,
    pub employee_calls: Vec<(String, Value)>,
    pub employee_credentials: Vec<bool>,
    pub self_service_directory: Option<SelfServiceDirectoryResponse>,
    pub invocations: BTreeMap<Uuid, (Value, SelfServiceAccepted)>,
    pub invocation_reply_lost: bool,
    pub invocation_unaccepted: bool,
    pub response_override: Option<(String, Value)>,
    pub configuration_rejection: Option<enroll::ErrorCode>,
    pub configuration_file: Option<enroll::configuration::ConfigurationFile>,
    pub packages: Vec<enroll::configuration::Package>,
    pub configuration_calls: Vec<Value>,
    pub configuration_status_calls: Vec<Value>,
    pub grant: Option<enroll::GrantReceipt>,
    pub issuance: Uuid,
    pub candidate_leaf: Option<Vec<u8>>,
    pub candidate_not_after: u64,
    pub candidate_rotated: bool,
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
    pub task_key_id: String,
    pub offer: Option<SignedTask>,
    pub time: Time,
    pub native_inputs: Vec<Value>,
    pub native_receipt: Option<enroll::NativeEnrollmentReceipt>,
    pub native_failure: bool,
    pub managed_installation: Option<Uuid>,
    pub registration_failure: bool,
    pub certificate_failure: bool,
    pub activation_failure: bool,
    pub enrollment_paused: bool,
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
    pub collections: Option<Vec<CollectionDefinition>>,
    pub results: BTreeMap<String, Value>,
    pub credential_escrows: Vec<Value>,
    pub escrow_reply_lost: bool,
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
    pub status_pause: Option<Arc<tokio::sync::Notify>>,
    pub status_waiting: bool,
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
                    .sign(&payload.signing_bytes(&self.task_key_id).unwrap())
                    .as_ref(),
            ),
            payload,
            key_id: self.task_key_id.clone(),
        }
    }
    pub fn unmanage(&mut self) {
        self.reset_current_task();
        self.offer = Some(
            self.signed(TaskPayload::Unmanage(Box::new(UnmanageTaskSpec {
                wire_version: 1,
                tenant_id: self.tenant,
                device_id: "device-1".into(),
                registration_id: self.registration,
                generation: 1,
                retirement_id: Uuid::new_v4(),
                task_id: self.task,
                attempt_id: self.attempt,
                permit: TaskPermit::Offer,
                expires_at: self.time.now().unwrap() + 60,
                deadline: self.time.now().unwrap() + 300,
                platform: TaskPlatform::Windows,
                architecture: TaskArchitecture::Aarch64,
                authorization: TaskAuthorization::Policy {
                    policy_id: self.task,
                    policy_version: self.task,
                },
            }))),
        );
    }
    pub fn native_enrollment(&mut self) {
        self.reset_current_task();
        self.offer = Some(
            self.signed(
                EnrollmentTaskSpec {
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
                    organization: self.tenant,
                    authorization: TaskAuthorization::Policy {
                        policy_id: self.task,
                        policy_version: self.task,
                    },
                    entry: EnrollmentEntry::Macos {
                        url: "https://enroll.example.test/".into(),
                    },
                }
                .try_into()
                .unwrap(),
            ),
        );
    }
    pub fn native_collection(&mut self) {
        self.reset_current_task();
        self.attempt = Uuid::new_v4();
        let query = NativeQuery {
            version: 1,
            definition: NativeQueryDefinition::WindowsWmi {
                namespace: "root\\cimv2".into(),
                class: "Vendor_Inventory".into(),
                properties: [
                    (
                        "Id".into(),
                        WmiProperty {
                            cim_type: CimType::String,
                            array: false,
                        },
                    ),
                    (
                        "Name".into(),
                        WmiProperty {
                            cim_type: CimType::String,
                            array: false,
                        },
                    ),
                ]
                .into(),
                filter: None,
            },
            cardinality: NativeCardinality::List,
            instance_keys: vec!["Id".into()],
            budget: NativeBudget {
                timeout_seconds: 30,
                output_bytes: 1048576,
                max_rows: 4096,
            },
        };
        let plan = NativeCollectionPlan::new(NativePlanSpec {
            version: 1,
            scope: rss_mdm_agent_wire::CollectionScope::Device,
            budget: query.budget,
            queries: [("items".into(), query)].into(),
        })
        .unwrap();
        let task = CollectionTaskSpec {
            wire_version: COLLECTION_TASK_VERSION,
            tenant_id: self.tenant,
            device_id: "device-1".into(),
            platform: TaskPlatform::Windows,
            architecture: TaskArchitecture::Aarch64,
            registration_id: self.registration,
            generation: 1,
            task_id: self.task,
            attempt_id: self.attempt,
            permit: TaskPermit::Offer,
            expires_at: self.time.now().unwrap() + 60,
            resource_digest: [3; 32],
            resource: TaskResource {
                id: "fixture-native".into(),
                version: "v1".into(),
                variant: "windows".into(),
            },
            authorization: TaskAuthorization::Policy {
                policy_id: self.task,
                policy_version: self.task,
            },
            deadline: self.time.now().unwrap() + 300,
            plan_digest: plan.digest(),
            execution: rss_mdm_agent_wire::runtime::NativeExecutionBinding::bind(
                &plan,
                &execution_context(),
            )
            .unwrap(),
            plan,
        };
        self.offer = Some(self.signed(TaskPayload::Collection(Box::new(task))));
    }
    pub fn native_system_collection(&mut self, query: NativeSystemQuery) {
        self.native_collection();
        let platform = query.platform();
        let TaskPayload::Collection(mut task) = self.offer.as_ref().unwrap().payload.clone() else {
            unreachable!()
        };
        let definition = NativeQueryDefinition::System {
            query: query.clone(),
        };
        let query = NativeQuery {
            version: query.engine().contract_version(),
            cardinality: query.cardinality(),
            instance_keys: if query.cardinality() == NativeCardinality::List {
                vec!["id".into()]
            } else {
                vec![]
            },
            definition,
            budget: task.plan.spec().budget,
        };
        task.plan = NativeCollectionPlan::new(NativePlanSpec {
            version: 1,
            scope: rss_mdm_agent_wire::CollectionScope::Device,
            budget: query.budget,
            queries: [("items".into(), query)].into(),
        })
        .unwrap();
        task.platform = platform;
        task.resource.variant = if platform == TaskPlatform::Macos {
            "macos"
        } else {
            "windows"
        }
        .into();
        task.plan_digest = task.plan.digest();
        task.execution = NativeExecutionBinding::bind(&task.plan, &execution_context()).unwrap();
        self.offer = Some(self.signed(TaskPayload::Collection(task)));
    }
    pub fn native_macos_collection(&mut self) {
        self.native_collection();
        let TaskPayload::Collection(mut task) = self.offer.as_ref().unwrap().payload.clone() else {
            unreachable!()
        };
        let old = task.plan.spec();
        let query = NativeQuery {
            version: 1,
            definition: NativeQueryDefinition::MacosHardware {
                section: MacosHardwareSection::Network,
            },
            cardinality: NativeCardinality::List,
            instance_keys: vec!["id".into()],
            budget: old.budget,
        };
        task.plan = NativeCollectionPlan::new(NativePlanSpec {
            version: 1,
            scope: rss_mdm_agent_wire::CollectionScope::Device,
            budget: old.budget,
            queries: [("items".into(), query)].into(),
        })
        .unwrap();
        task.plan_digest = task.plan.digest();
        task.platform = TaskPlatform::Macos;
        task.resource.variant = "macos".into();
        task.execution = NativeExecutionBinding::bind(&task.plan, &execution_context()).unwrap();
        self.offer = Some(self.signed(TaskPayload::Collection(task)));
    }
    pub fn script(&mut self) {
        self.attempt = Uuid::new_v4();
        self.reset_current_task();
        let spec = TaskSpec {
            execution_target: SoftwareExecutionTarget::Device,
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
            resource: TaskResource {
                id: "fixture-resource".into(),
                version: "v1".into(),
                variant: "default".into(),
            },
            authorization: TaskAuthorization::Policy {
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
        self.offer = Some(self.signed(TaskPayload::Script(Box::new(spec))));
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
            authorization: if user {
                TaskAuthorization::SelfService {
                    request_id: self.task,
                    subject: SelfServiceSubject::Device {},
                    source: SelfServiceCaller::Human,
                    policy_id: self.task,
                    policy_revision: 1,
                    policy_version: self.task,
                    allow_ai: false,
                    risk_level: 2,
                    confirmed: true,
                }
            } else {
                TaskAuthorization::Policy {
                    policy_id: self.task,
                    policy_version: self.task,
                }
            },
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
        self.offer = Some(self.signed(TaskPayload::try_from(spec).unwrap()));
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
    _runtime: ServerRuntime,
}

// axum spawns each accepted connection. Cancelling only its listener leaves those
// tasks alive; a dedicated runtime owns and cancels the entire server together.
// ref: axum 0.8.9 src/serve/mod.rs; tokio 1.53.1 src/runtime/runtime.rs.
struct ServerRuntime {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for ServerRuntime {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            if let Err(panic) = thread.join() {
                if !std::thread::panicking() {
                    std::panic::resume_unwind(panic);
                }
            }
        }
    }
}
impl Server {
    pub async fn new() -> Self {
        Self::listen(true).await
    }
    pub async fn tls() -> Self {
        Self::listen(true).await
    }
    async fn listen(tls: bool) -> Self {
        let time = Time(Arc::new(AtomicI64::new(1)));
        let signer = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let data = Arc::new(Mutex::new(Data {
            current_user: Default::default(),
            telemetry: Default::default(),
            software_source: None,
            trust: Value::Null,
            task_trust: Value::Null,
            task_trust_ack_calls: Vec::new(),
            task_trust_failure: false,
            task_trust_ack_failure: false,
            observations: Vec::new(),
            trust_calls: 0,
            trust_ack_calls: Vec::new(),
            trust_ack_failure: false,
            trust_failure: false,
            renewal_lookup_calls: Vec::new(),
            renewal_lookup_authenticated: Vec::new(),
            tls_required: tls,
            employee_calls: vec![],
            employee_credentials: vec![],
            self_service_directory: None,
            invocations: BTreeMap::new(),
            invocation_reply_lost: false,
            invocation_unaccepted: false,
            response_override: None,
            configuration_rejection: None,
            configuration_file: None,
            packages: vec![],
            configuration_calls: vec![],
            configuration_status_calls: vec![],
            grant: None,
            issuance: Uuid::new_v4(),
            candidate_leaf: None,
            candidate_not_after: 4_070_908_800,
            candidate_rotated: false,
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
            task_key_id: "test".into(),
            offer: None,
            time: time.clone(),
            native_inputs: vec![],
            native_receipt: None,
            native_failure: false,
            managed_installation: None,
            registration_failure: false,
            certificate_failure: false,
            activation_failure: false,
            enrollment_paused: false,
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
            collections: None,
            results: BTreeMap::new(),
            credential_escrows: vec![],
            escrow_reply_lost: false,
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
            status_pause: None,
            status_waiting: false,
            explicit_offers: false,
            claim_ops: vec![],
            claim_inputs: vec![],
            claims: BTreeMap::new(),
        }));
        let (ready, started) = tokio::sync::oneshot::channel();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server_data = data.clone();
        let server_time = time.clone();
        let thread = std::thread::Builder::new()
            .name("agent-test-server".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    let url = url::Url::parse(&format!(
                        "{}://{}/",
                        if tls { "https" } else { "http" },
                        listener.local_addr().unwrap()
                    ))
                    .unwrap();
                    {
                        let mut state = server_data.lock().unwrap();
                        state.trust = pki::trust(url.as_str(), state.tenant);
                    }
                    let router = axum::Router::new()
                        .fallback(handler)
                        .with_state(server_data);
                    let serving = async move {
                        if tls {
                            let mut config = pki::server();
                            config.time_provider = Arc::new(server_time);
                            let listener = TlsListener {
                                listener,
                                acceptor: tokio_rustls::TlsAcceptor::from(Arc::new(config)),
                            };
                            axum::serve(
                                listener,
                                router.into_make_service_with_connect_info::<TlsIdentity>(),
                            )
                            .await
                        } else {
                            axum::serve(listener, router).await
                        }
                    };
                    if ready.send(url).is_err() {
                        return;
                    }
                    tokio::select! {
                        result = serving => result.unwrap(),
                        _ = stopped => {},
                    }
                });
                // Drop outside the caller's async context, cancelling pending handlers
                // and closing their sockets before the owner can release its budget.
            })
            .unwrap();
        let runtime = ServerRuntime {
            stop: Some(stop),
            thread: Some(thread),
        };
        let url = started.await.expect("test server failed to start");
        Self {
            url,
            data,
            time,
            secrets: Secrets::new(),
            _runtime: runtime,
        }
    }
    pub fn config(&self) -> Config {
        let data = self.data.lock().unwrap();
        Config {
            task_trust_origins: vec![],
            software_read_grants: data
                .software_source
                .as_ref()
                .and_then(|s| s.credential.clone())
                .map(|credential| agent_client::SoftwareReadGrant {
                    credential,
                    origin: self.url.origin().ascii_serialization(),
                })
                .into_iter()
                .collect(),
            execution_context: if data.software_source.is_some() {
                match &data.offer.as_ref().unwrap().payload {
                    TaskPayload::Software(s) => s.execution_context.clone(),
                    _ => unreachable!(),
                }
            } else {
                execution_context()
            },
            origin: self.url.clone(),
            tenant: data.tenant,
            platform: data
                .offer
                .as_ref()
                .map_or(TaskPlatform::Macos, |s| s.payload.platform()),
            architecture: TaskArchitecture::Aarch64,
            registration_keys: BTreeMap::from([(
                "test".into(),
                Ed25519KeyPair::from_seed_unchecked(&[7; 32])
                    .unwrap()
                    .public_key()
                    .as_ref()
                    .to_vec(),
            )]),
            limits: Limits::test_defaults(),
            transport: if data.tls_required {
                Transport::Https
            } else {
                Transport::TestLoopback
            },
            ca_pem: data.tls_required.then(pki::ca),
        }
    }
    pub fn configuration(&self) -> enroll::configuration::ConfigurationFile {
        use enroll::configuration as w;
        let cfg = self.config();
        let mut data = self.data.lock().unwrap();
        let configuration = w::Configuration {
            wire_version: 1,
            configuration_id: Uuid::new_v4(),
            tenant_id: cfg.tenant,
            origin: cfg.origin.origin().ascii_serialization(),
            targets: vec![w::Target {
                platform: cfg.platform,
                architecture: cfg.architecture,
            }],
            expires_at: cfg!(test)
                .then_some(100)
                .unwrap_or(data.time.now().unwrap() + 3600),
            quantity: w::Quantity::Finite {
                count: std::num::NonZeroU32::new(2).unwrap(),
            },
            key_id: "test".into(),
        };
        let secret = Secret::parse("AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE").unwrap();
        let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            data.signer
                .sign(&configuration.signing_bytes(&secret).unwrap())
                .as_ref(),
        );
        let file = w::ConfigurationFile {
            configuration,
            secret,
            signature,
        };
        data.packages = vec![w::Package {
            release_id: Uuid::new_v4(),
            format: w::InstallerFormat::MacosPkg,
            tenant_id: cfg.tenant,
            target: w::Target {
                platform: cfg.platform,
                architecture: cfg.architecture,
            },
            version: "1.0.0".into(),
            filename: "fixture.pkg".into(),
            length: 1,
            installer_sha256: [9; 32],
        }];
        data.configuration_file = Some(file.clone());
        file
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
        if client.configuration().platform == TaskPlatform::Windows {
            client.set_profiles(vec![]).unwrap();
            client
                .set_collection_environment(rss_mdm_agent_wire::CollectionEnvironment {
                    edition: None,
                    tools: Default::default(),
                    native_engines: [
                        (NativeEngine::WindowsWmi, 1),
                        (NativeEngine::WindowsUpdates, 1),
                        (NativeEngine::WindowsCertificates, 1),
                        (NativeEngine::WindowsPerformance, 1),
                    ]
                    .into(),
                })
                .unwrap();
        } else {
            client.set_profiles(vec![ExecutorProfile::PosixSh]).unwrap();
            client
                .set_collection_environment(rss_mdm_agent_wire::CollectionEnvironment {
                    edition: None,
                    tools: Default::default(),
                    native_engines: [
                        NativeEngine::MacosHardware,
                        NativeEngine::MacosSecurity,
                        NativeEngine::MacosAccounts,
                        NativeEngine::MacosCertificates,
                        NativeEngine::MacosUpdates,
                        NativeEngine::MacosPerformance,
                    ]
                    .into_iter()
                    .map(|engine| (engine, engine.contract_version()))
                    .collect(),
                })
                .unwrap();
        }
        client
    }
    pub async fn register(&self, client: &mut Client<Secrets, Time>) -> BindingProfile {
        client
            .request_authorization(vec![
                Capability::InventoryCollectionV1,
                Capability::TaskExecuteV1,
                Capability::SoftwarePkgSystemV1,
                Capability::TelemetryCollectV1,
                Capability::TelemetryCounterLocalV1,
            ])
            .await
            .unwrap();
        client.drive_pki().await.unwrap();
        client.drive_task_trust().await.unwrap();
        client.registration().unwrap()
    }
}
fn native_enrollment_response(d: &mut Data, value: Value) -> Response {
    if !d.started {
        return runtime_error(StatusCode::FORBIDDEN, ErrorCode::PermissionDenied);
    }
    let request: enroll::NativeEnrollmentRequest = serde_json::from_value(value.clone()).unwrap();
    if request.attempt_id != d.attempt || d.native_inputs.first().is_some_and(|old| old != &value) {
        return runtime_error(StatusCode::CONFLICT, ErrorCode::OperationConflict);
    }
    d.native_inputs.push(value);
    if d.native_receipt.is_none() {
        let TaskPayload::Enrollment(task) = &d.offer.as_ref().unwrap().payload else {
            panic!("enrollment task");
        };
        let id = Uuid::new_v4();
        d.native_receipt = Some(enroll::NativeEnrollmentReceipt {
            enrollment_id: id,
            expires_at: d.time.now().unwrap() + 300,
            entry: task.entry.clone(),
            username: None,
            profile_url: Some(format!(
                "https://enroll.example.test/api/v1/enrollments/{id}/profile"
            )),
        });
    }
    if std::mem::take(&mut d.native_failure) {
        return runtime_error(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::ServiceUnavailable,
        );
    }
    axum::Json(d.native_receipt.clone().unwrap()).into_response()
}

async fn handler(
    State(data): State<Arc<Mutex<Data>>>,
    uri: Uri,
    method: axum::http::Method,
    headers: HeaderMap,
    peer: Result<ConnectInfo<TlsIdentity>, axum::extract::rejection::ExtensionRejection>,
    bytes: Bytes,
) -> Response {
    let pause = response_pause(&mut data.lock().unwrap(), uri.path());
    if let Some(pause) = pause {
        pause.notified().await;
    }
    let mut d = data.lock().unwrap();
    let path = uri.path();
    let peer_leaf = peer.ok().and_then(|v| v.0 .0);
    let authenticated = peer_leaf.is_some();
    if path.starts_with("/native/") {
        return d
            .software_source
            .as_mut()
            .map(|source| source.response(path, &headers, &bytes, authenticated))
            .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response());
    }
    if d.tls_required
        && (path.ends_with("/activate")
            || path.ends_with("/native-grants")
            || path.ends_with("/renewal-grants")
            || path.ends_with("/trust/acknowledgements")
            || path.ends_with("/trust/observations")
            || path.starts_with("/api/v1/agent/runtime/"))
        && !authenticated
    {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if headers.contains_key("authorization") {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    if path.ends_with("/current-user") {
        if d.denied || d.activation.is_none() {
            return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
        }
        let tenant = d.tenant;
        return current_user::response(&mut d.current_user, tenant, &method, &headers, value);
    }
    if path.contains("/self-service") {
        return self_service_response(&mut d, &uri, &headers, value);
    }
    if let Some((suffix, body)) = &d.response_override {
        if path.ends_with(suffix) {
            return axum::Json(body.clone()).into_response();
        }
    }
    if path.ends_with("/task-trust/observations") || path.ends_with("/trust/observations") {
        d.observations.push(value.clone());
        return axum::Json(value).into_response();
    }
    if let Some(action) = path.strip_prefix("/api/v1/agent/enroll/") {
        return enrollment_response(&mut d, action, value, peer_leaf.as_deref());
    }
    if !path.starts_with("/api/v1/agent/runtime/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    runtime_response(&mut d, path, headers, value)
}

fn runtime_response(d: &mut Data, path: &str, headers: HeaderMap, value: Value) -> Response {
    if d.denied {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::CredentialRevoked);
    }
    if d.activation.is_none() {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    if path.contains("/runtime/telemetry/") {
        return telemetry::response(d, path, value);
    }
    let response = if path.ends_with("/runtime/task-trust") {
        task_trust_response(d, value)
    } else if path.ends_with("/enrollment") {
        native_enrollment_response(d, value)
    } else if path.ends_with("/reports") {
        report_response(d, value)
    } else if path.ends_with("/claim") {
        claim_response(d, value)
    } else if path.ends_with("/events") {
        event_response(d, value)
    } else if path.ends_with("/content") {
        content_response(d, headers)
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
fn response_pause(d: &mut Data, path: &str) -> Option<Arc<tokio::sync::Notify>> {
    if path.ends_with("/claim") && d.claim_pause.is_some() {
        d.claim_waiting = true;
        d.claim_pause.clone()
    } else if path.ends_with("/enroll/status") && d.status_pause.is_some() {
        d.status_waiting = true;
        d.status_pause.clone()
    } else {
        None
    }
}
fn runtime_error(status: StatusCode, code: ErrorCode) -> Response {
    (status, axum::Json(ErrorBody { code })).into_response()
}
fn configuration_response(d: &mut Data, action: &str, value: Value) -> Response {
    match action {
        "packages" => axum::Json(d.packages.clone()).into_response(),
        "configurations/status" => {
            d.configuration_status_calls.push(value.clone());
            let request: enroll::configuration::ExchangeStatus =
                serde_json::from_value(value).unwrap();
            if d.operation != Some(request.operation_id)
                || d.bootstrap.as_deref() != Some(request.secret.expose())
            {
                return (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::InvalidIdentity,
                        issuance_id: None,
                    }),
                )
                    .into_response();
            }
            axum::Json(d.grant.clone().unwrap()).into_response()
        }
        "configurations/exchange" => {
            d.configuration_calls.push(value.clone());
            if let Some(code) = d.configuration_rejection {
                return (
                    StatusCode::FORBIDDEN,
                    axum::Json(enroll::ErrorBody {
                        code,
                        issuance_id: None,
                    }),
                )
                    .into_response();
            }
            let request: enroll::configuration::Exchange = serde_json::from_value(value).unwrap();
            assert_eq!(
                serde_json::to_value(&request.file).unwrap(),
                serde_json::to_value(d.configuration_file.as_ref().unwrap()).unwrap()
            );
            assert_eq!(
                request.installation.release_id,
                d.packages
                    .iter()
                    .find(|p| p.release_id == request.installation.release_id)
                    .unwrap()
                    .release_id
            );
            assert_ne!(request.secret.expose(), request.file.secret.expose());
            if d.operation.is_some_and(|v| v != request.operation_id) {
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
            let grant = d
                .grant
                .get_or_insert_with(|| enroll::GrantReceipt {
                    wire_version: 1,
                    grant_id: Uuid::new_v4(),
                    state: enroll::GrantState::Available,
                    platform: request.runtime.platform,
                    expires_at: d.time.now().unwrap() + 300,
                })
                .clone();
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
            axum::Json(grant).into_response()
        }

        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
fn enrollment_response(d: &mut Data, action: &str, value: Value, peer: Option<&[u8]>) -> Response {
    match action {
        "packages" | "configurations/status" | "configurations/exchange" => {
            configuration_response(d, action, value)
        }
        "trust" => {
            d.trust_calls += 1;
            if std::mem::take(&mut d.trust_failure) {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(serde_json::json!({"code":"service_unavailable"})),
                )
                    .into_response();
            }
            axum::Json(d.trust.clone()).into_response()
        }
        "trust/acknowledgements" => {
            assert!(d.activation.is_some());
            if d.tls_required && peer.is_none() {
                return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
            }
            d.trust_ack_calls.push(value);
            if std::mem::take(&mut d.trust_ack_failure) {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::ServiceUnavailable,
                        issuance_id: None,
                    }),
                )
                    .into_response();
            }
            StatusCode::NO_CONTENT.into_response()
        }
        "renewal-grants/status" => {
            d.renewal_lookup_calls.push(value.clone());
            d.renewal_lookup_authenticated.push(peer.is_some());
            if value["operationId"].as_str() != d.operation.map(|id| id.to_string()).as_deref()
                || value["secret"].as_str() != d.bootstrap.as_deref()
            {
                return (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::InvalidIdentity,
                        issuance_id: None,
                    }),
                )
                    .into_response();
            }
            axum::Json(d.grant.clone().unwrap()).into_response()
        }
        "native-grants" => {
            let request: enroll::NativeGrantRequest =
                serde_json::from_value(value.clone()).unwrap();
            let coordinates = peer.and_then(|der| agent_client::bootstrap_coordinates(der).ok());
            if coordinates.map(|c| c.0) != d.managed_installation
                || coordinates.map(|c| c.0) != Some(request.installation_operation)
            {
                return (
                    StatusCode::FORBIDDEN,
                    axum::Json(enroll::ErrorBody {
                        code: enroll::ErrorCode::PermissionDenied,
                        issuance_id: None,
                    }),
                )
                    .into_response();
            }
            d.native_inputs.push(value);
            enrollment_response(
                d,
                "requests",
                serde_json::to_value(enroll::GrantRequest {
                    wire_version: 1,
                    operation_id: request.operation_id,
                    secret: request.secret,
                    platform: request.runtime.platform,
                    csr: request.csr,
                })
                .unwrap(),
                None,
            )
        }
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
                if d.candidate_rotated {
                    pki::rotation_chain(
                        d.tenant,
                        csr.info.public_key.to_der().unwrap(),
                        d.candidate_not_after,
                    )
                } else {
                    pki::chain_until(
                        d.tenant,
                        csr.info.public_key.to_der().unwrap(),
                        d.candidate_not_after,
                    )
                }
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
        "activate" => activation_response(d, value, peer),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
fn activation_response(d: &mut Data, value: Value, peer: Option<&[u8]>) -> Response {
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
    if d.enrollment_paused {
        return (
            StatusCode::CONFLICT,
            axum::Json(enroll::ErrorBody {
                code: enroll::ErrorCode::EnrollmentPaused,
                issuance_id: None,
            }),
        )
            .into_response();
    }
    let request: enroll::ActivationRequest = serde_json::from_value(value).unwrap();
    assert_eq!(request.issuance_id, d.issuance);
    let profile: BindingProfile = d.binding.clone().unwrap_or_else(|| serde_json::from_value(json!({"wireVersion":1,"operationId":d.operation,"deviceId":"device-1","registrationId":d.registration,"generation":1,"source":"agent.builtin","epoch":d.epoch,"capabilities":d.profile.as_ref().unwrap().capabilities,"collections":d.collections.clone().unwrap_or_else(collections)})).unwrap());
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
            if let TaskPayload::Software(mut spec) = signed.payload.clone() {
                spec.execution_context =
                    serde_json::from_value(value["executionContext"].clone()).unwrap();
                signed = d.signed(TaskPayload::Software(spec));
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
        TaskEvent::CredentialEscrow(_) => {
            if d.credential_escrows.iter().any(|old| old != &value) {
                return StatusCode::CONFLICT.into_response();
            }
            d.credential_escrows.push(value);
            if std::mem::take(&mut d.escrow_reply_lost) {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            axum::Json(TaskEventAck::new(None, false)).into_response()
        }
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
            TaskPayload::Unmanage(v) => {
                v.permit = TaskPermit::Start;
                v.expires_at = expiry;
            }
            TaskPayload::Credential(v) => {
                v.permit = TaskPermit::Start;
                v.expires_at = expiry;
            }
            TaskPayload::Collection(v) => {
                v.permit = TaskPermit::Start;
                v.expires_at = expiry;
            }
            TaskPayload::Script(v) => {
                v.permit = TaskPermit::Start;
                v.expires_at = expiry;
            }
            TaskPayload::Software(v) => {
                v.permit = TaskPermit::Start;
                v.expires_at = expiry;
            }
            TaskPayload::Enrollment(v) => {
                v.permit = TaskPermit::Start;
                v.expires_at = expiry;
            }
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

pub fn collections() -> Vec<rss_mdm_agent_wire::runtime::CollectionDefinition> {
    let fields:Vec<_>=["device.model","device.os.version"].into_iter().map(|key|json!({
        "key":key,"version":1,"valueType":{"kind":"string","maxLength":256,"allowEmpty":false},
        "nullable":false,"manual":false,"sources":{"agent.builtin":0},"platforms":["windows","macos"],
        "sensitivity":"standard","unit":null,"searchable":true,"itemKey":null
    })).collect();
    vec![serde_json::from_value(
        json!({"dataset":"inventory","version":"1","source":"agent.builtin","scope":rss_mdm_agent_wire::CollectionScope::Device,"fields":fields}),
    )
    .unwrap()]
}

pub fn execution_context() -> SoftwareExecutionContext {
    SoftwareExecutionContext {
        revision: 1,
        os_version: [14, 0, 0, 0],
        system_broker: true,
        user_helper: true,
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
pub fn policy_directory_fixture(tenant: Uuid) -> SelfServiceDirectoryItem {
    serde_json::from_value(json!({
        "policyId": Uuid::new_v4(), "policyRevision": 2, "policyVersion": Uuid::new_v4(),
        "subject":{"kind":"user","user":{"tenantId":tenant,"instanceId":Uuid::from_u128(2),"principalId":Uuid::from_u128(3)}},
        "selfService":{"published": true, "allowAi": true, "riskLevel": 2, "presentation": {"displayName": "Repair", "description": "Test repair", "prerequisites": "Employee session", "sideEffects": "Restarts a fixture", "category": "Support", "keywords": ["repair"]}},
        "task":{"kind":"script","runAs":"system","resource":{"id":Uuid::new_v4().to_string(),"version":"v1","variant":"default"},
        "parameterSources":{"name":{"kind":"input"}, "path":{"kind":"fixed","value":"/Applications"}},
        "parameterSchema":{"type":"object","properties":{"name":{"type":"string","minLength":1,"maxLength":32},"path":{"type":"string","const":"/Applications"}},"required":["name","path"],"additionalProperties":false}}
    })).unwrap()
}

fn self_service_response(d: &mut Data, uri: &Uri, headers: &HeaderMap, value: Value) -> Response {
    let path = uri.path();
    if d.activation.is_none()
        || headers
            .get("x-rss-user-session")
            .is_some_and(|value| value.to_str().ok() != Some(&"c".repeat(64)))
        || headers.contains_key("cookie")
    {
        return runtime_error(StatusCode::UNAUTHORIZED, ErrorCode::InvalidIdentity);
    }
    d.employee_calls.push((uri.to_string(), value.clone()));
    d.employee_credentials
        .push(headers.contains_key("x-rss-user-session"));
    if let Some((suffix, body)) = &d.response_override {
        if path.ends_with(suffix) {
            return axum::Json(body.clone()).into_response();
        }
    }
    if path.ends_with("/self-service") {
        let mut directory =
            d.self_service_directory
                .clone()
                .unwrap_or(SelfServiceDirectoryResponse {
                    items: vec![],
                    next_cursor: None,
                    as_of: d.time.now().unwrap(),
                });
        // Explicit transport fixture projection, not enterprise qualification authority.
        directory.items.retain(|item| {
            matches!(item.subject, SelfServiceSubject::Device {})
                || headers.contains_key("x-rss-user-session")
        });
        return axum::Json(directory).into_response();
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
                state: TaskRunState {
                    delivery: TaskDelivery::Queued,
                    execution: TaskExecution::NotStarted,
                    cancellation: RunCancellation::None,
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

fn task_trust_response(d: &mut Data, value: Value) -> Response {
    if value.is_null() {
        if std::mem::take(&mut d.task_trust_failure) {
            return runtime_error(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::ServiceUnavailable,
            );
        }
        if d.task_trust.is_null() {
            d.task_trust = serde_json::json!({"version":1,"origin":d.trust["origin"],"tenant":d.tenant,"revision":1,
                    "keys":[{"id":"test","public_key":base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(d.signer.public_key().as_ref()),"state":"active"}]});
        }
        return axum::Json(d.task_trust.clone()).into_response();
    }
    d.task_trust_ack_calls.push(value.clone());
    if std::mem::take(&mut d.task_trust_ack_failure) {
        return runtime_error(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::ServiceUnavailable,
        );
    }
    axum::Json(value).into_response()
}
