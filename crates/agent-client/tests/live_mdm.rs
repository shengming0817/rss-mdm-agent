//! Real-server integration against isolated rss-mdm serve and PostgreSQL.
//! The live_mdm target uses a controlled Test runner; it does not run against production.
mod support;
use agent_client::wire::*;
use agent_client::*;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use support::execution::*;
use support::{Root, Secrets};
use uuid::Uuid;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveConfig {
    #[serde(deserialize_with = "parse_url")]
    origin: url::Url,
    tenant: Uuid,
    ca_file: PathBuf,
    admin_password_file: PathBuf,
    admin_login: String,
    key_id: String,
    public_key: String,
}
fn parse_url<'de, D: serde::Deserializer<'de>>(d: D) -> Result<url::Url, D::Error> {
    url::Url::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
}
struct Utc;
impl Clock for Utc {
    fn now(&self) -> Result<i64, Error> {
        i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Clock)?
                .as_secs(),
        )
        .map_err(|_| Error::Clock)
    }
}
struct Admin {
    http: reqwest::Client,
    origin: url::Url,
    cookies: BTreeMap<String, String>,
    csrf: Option<String>,
}
impl Admin {
    async fn request(
        &mut self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
        bytes: Option<&[u8]>,
    ) -> Value {
        let mut request = self
            .http
            .request(method, self.origin.join(path).unwrap())
            .header("origin", self.origin.as_str().trim_end_matches('/'))
            .header("x-identity-request", "1");
        if !self.cookies.is_empty() {
            request = request.header(
                "cookie",
                self.cookies
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            );
        }
        if let Some(csrf) = &self.csrf {
            request = request.header("x-csrf-token", csrf);
        }
        if path == "/api/v3/enrollments" {
            request = request.header("idempotency-key", Uuid::new_v4().to_string());
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        if let Some(bytes) = bytes {
            request = request
                .header("content-type", "application/octet-stream")
                .body(bytes.to_owned());
        }
        let response = request.send().await.expect("live management HTTP");
        let status = response.status();
        for cookie in response.headers().get_all("set-cookie") {
            if let Ok(cookie) = cookie.to_str() {
                if let Some((name, value)) = cookie.split(';').next().unwrap().split_once('=') {
                    self.cookies.insert(name.into(), value.into());
                }
            }
        }
        let bytes = response.bytes().await.unwrap();
        let result: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("live JSON response")
        };
        assert!(
            status.is_success(),
            "live {path} status={status} code={}",
            result.get("code").unwrap_or(&Value::Null)
        );
        if let Some(csrf) = result["csrfToken"].as_str() {
            self.csrf = Some(csrf.to_owned());
        }
        result
    }
    async fn write(&mut self, path: &str, revision: u64, input: Value) -> Value {
        self.request(
            reqwest::Method::POST,
            path,
            Some(json!({"operationId":Uuid::new_v4(),"expectedRevision":revision,"input":input})),
            None,
        )
        .await
    }
    async fn resource(&mut self, software: bool) -> (Uuid, Option<Value>) {
        let resource = Uuid::new_v4();
        let path = format!("/api/v3/resources/{resource}");
        let bytes = if software {
            b"controlled software fixture".as_slice()
        } else {
            b"{\"ok\":true,\"message\":\"fixture\"}\n".as_slice()
        };
        let digest = Sha256::digest(bytes).to_vec();
        let declaration = if software {
            let source = Uuid::new_v4().to_string();
            let source_path = format!("/api/v3/software/sources/{source}/revisions/1");
            let created=self.write(&source_path,0,json!({"action":"register","definition":{"id":source,"revision":"1","kind":"private","location":null,"publishers":[]}})).await;
            self.write(
                &source_path,
                1,
                json!({"action":"approve","evidence":["controlled-protocol-test"]}),
            )
            .await;
            json!({"kind":"software","definition":{"source":created["snapshot"],"package":"Agent.LiveFixture","version":"1.0","format":"pkg","primary":"package",
                "artifacts":{"package":{"reference":"installer","length":bytes.len(),"sha256":digest}},
                "install":{"executor":"package_installer","entry":null,"runAs":"system","arguments":[],"environment":{},"timeoutSeconds":60,"outputBytes":4096},
                "uninstall":null,"detect":{"kind":"pkg_receipt","receipt":"com.rss.livefixture","version":"1.0"},
                "reboot":"report","downgrade":"deny","ownership":"managed_only","dependencies":[],"bundle":null}})
        } else {
            json!({"kind":"script","artifact":{"reference":"fixture-script","length":bytes.len(),"sha256":digest},
                "definition":{"profile":"posix_sh","runAs":"system","encoding":"utf8",
                "parameters":{"type":"object","properties":{},"required":[],"additionalProperties":false},"bindings":{},
                "output":{"type":"object","properties":{"ok":{"type":"boolean"},"message":{"type":"string"}},"required":["ok","message"],"additionalProperties":false},
                "purpose":{"kind":"collection","mappings":{"custom.corporate_agent.healthy":"/ok","custom.corporate_agent.version":"/message"}},"timeoutSeconds":60,"outputBytes":4096,"maxRows":1}})
        };
        let kind = if software { "software" } else { "script" };
        self.write(&path, 0, json!({"action":"create","kind":kind}))
            .await;
        self.write(&path,1,json!({"action":"version","version":"v1","kind":kind,"variants":[{"platform":"macos","architecture":"aarch64","key":"default","declaration":declaration}]})).await;
        self.request(reqwest::Method::POST,&format!("{path}/content?version=v1&variant=default&platform=macos&architecture=aarch64&operation={}",Uuid::new_v4()),None,Some(bytes)).await;
        self.write(&path, 2, json!({"action":"activate","version":"v1"}))
            .await;
        let admission = if software {
            Some(
                self.write(
                    &format!("/api/v3/software/resources/{resource}/versions/v1"),
                    0,
                    json!({"action":"approve","evidence":["controlled-protocol-test"]}),
                )
                .await["admission"]["operation"]
                    .clone(),
            )
        } else {
            None
        };
        (resource, admission)
    }
}
/// Explicit live mode never skips a missing or failed environment.
#[tokio::test]
#[ignore = "requires AGENT_LIVE_CONFIG for an isolated real rss-mdm serve environment"]
async fn real_https_registration_reports_script_software_and_journal_results() {
    use execution_app::{
        AppConfig, DeterministicTestRunner, ExecutionApp, RequestContext, Startup, TestScenario,
    };
    let path = std::env::var("AGENT_LIVE_CONFIG").expect("AGENT_LIVE_CONFIG is required");
    let fixture: LiveConfig = serde_json::from_slice(
        &native_process::private_storage::read(std::path::Path::new(&path), 16384).unwrap(),
    )
    .unwrap();
    let ca = std::fs::read(&fixture.ca_file).unwrap();
    let config = Config {
        origin: fixture.origin.clone(),
        tenant: fixture.tenant,
        platform: TaskPlatform::Macos,
        architecture: TaskArchitecture::Aarch64,
        keys: BTreeMap::from([(
            fixture.key_id.clone(),
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(&fixture.public_key)
                .unwrap(),
        )]),
        limits: Limits::test_defaults(),
        transport: Transport::Https,
        ca_pem: Some(ca.clone()),
    };
    let mut admin = Admin {
        http: reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .add_root_certificate(reqwest::Certificate::from_pem(&ca).unwrap())
            .build()
            .unwrap(),
        origin: fixture.origin.clone(),
        cookies: BTreeMap::new(),
        csrf: None,
    };
    let password = String::from_utf8(
        native_process::private_storage::read(&fixture.admin_password_file, 4096).unwrap(),
    )
    .unwrap();
    admin
        .request(
            reqwest::Method::POST,
            &format!("/api/v2/tenants/{}/login", fixture.tenant),
            Some(json!({"login":fixture.admin_login,"password":password})),
            None,
        )
        .await;
    let authority = admin
        .request(reqwest::Method::GET, "/api/v1/authorization", None, None)
        .await;
    let mut grants: Vec<Value> = [
        "resource_read",
        "resource_write",
        "policy_read",
        "policy_write",
        "scope_read",
        "scope_write",
        "software_read",
        "software_write",
        "software_approve",
    ]
    .iter()
    .map(|p| json!({"operation":p,"scope":{"kind":"tenant"}}))
    .collect();
    grants.extend(
        [
            "enrollment",
            "inventory_read",
            "script_execute",
            "software_deploy",
            "operation_read",
            "operation_cancel",
        ]
        .iter()
        .map(|p| json!({"operation":p,"scope":{"kind":"all_devices"}})),
    );
    admin.request(reqwest::Method::PUT,&format!("/api/v1/authorization/rules/{}",Uuid::new_v4()),Some(json!({"operationId":Uuid::new_v4(),"expectedRevision":0,"value":{"subject":{"kind":"user","user":{"instanceId":authority["instanceId"],"tenantId":fixture.tenant,"principalId":authority["principalId"]}},"grants":grants}})),None).await;
    let device = format!("agent-live-{}", Uuid::new_v4());
    let secrets = Secrets::new();
    use ring::rand::SecureRandom;
    for key in ["password", "credential"] {
        let mut bytes = [0u8; 32];
        ring::rand::SystemRandom::new().fill(&mut bytes).unwrap();
        secrets.0.lock().unwrap().insert(
            key.into(),
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
        );
    }
    let enrollment=admin.request(reqwest::Method::POST,"/api/v3/enrollments",Some(json!({"deviceId":device,"password":secrets.resolve("password").unwrap().expose(),"source":"agent.builtin"})),None).await;
    let root = Root::new();
    let mut client = Client::open(&root.path, config, OpenMode::Create, secrets, Utc).unwrap();
    client.set_profiles(vec![ExecutorProfile::PosixSh]).unwrap();
    let receipt = client
        .register(
            Uuid::new_v4(),
            Uuid::parse_str(enrollment["enrollmentId"].as_str().unwrap()).unwrap(),
            "password",
            "credential",
            vec![
                Capability::InventoryCollectionV5,
                Capability::TaskExecuteV5,
                Capability::SoftwareExecuteV5,
            ],
        )
        .await
        .unwrap();
    assert_eq!(receipt.device_id, device);
    let report = client
        .queue_report(
            "inventory",
            ReportBody::Failed {
                code: FailureCode::CollectionFailed,
            },
            Utc.now().unwrap(),
        )
        .unwrap();
    assert_eq!(client.flush_reports(4).await.unwrap(), 1);
    assert_eq!(
        client.report_status(report).await.unwrap().ack.report_id,
        report
    );
    let scope = Uuid::new_v4();
    let scope_created=admin.write(&format!("/api/v2/scopes/{scope}"),0,json!({"action":"put","definition":{"targets":[{"kind":"device","id":device}],"limitations":null,"exclusions":[]}})).await;
    let task = scope_created["task"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let status = admin
                .request(
                    reqwest::Method::GET,
                    &format!("/api/v2/scopes/{scope}/tasks/{task}"),
                    None,
                    None,
                )
                .await;
            if status["status"] == "completed" {
                break;
            }
            assert_ne!(status["status"], "failed");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("live Scope resolution");
    for software in [false, true] {
        let (resource, admission) = admin.resource(software).await;
        let action = if software {
            json!({"kind":"software","resource":{"kind":"software","id":resource,"version":"v1","variants":{"macos_aarch64":"default"}},"intent":"required_install","admissionOperation":admission.unwrap(),"runLifetimeSeconds":600,"rollout":{"stages":[{"scope":scope,"opensAt":0}]}})
        } else {
            json!({"kind":"execution","resource":{"id":resource,"version":"v1","platform":"macos","architecture":"aarch64","variant":"default"},"parameters":{},"runLifetimeSeconds":300})
        };
        let policy = Uuid::new_v4();
        admin
            .write(
                &format!("/api/v2/policies/{policy}"),
                0,
                json!({"action":"put","enabled":true,"definition":{"scope":scope,"action":action}}),
            )
            .await;
        let offer = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(offer) = client.claim().await.unwrap().offer {
                    break offer;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("live task delivery");
        assert_eq!(
            matches!(offer.payload(), TaskPayload::Software(_)),
            software
        );
        let materials = client.prepare(&offer).await.unwrap();
        client.received(&offer).await.unwrap();
        let db = local::Database::new();
        let mut host = local::TestHost::new();
        let plan = adapted_plan(software, &device, &offer);
        host.template = plan.clone();
        let runner = CapturingRunner {
            inner: DeterministicTestRunner::new(
                local::id("test-runner"),
                TestScenario::Complete,
                16,
            )
            .unwrap(),
            ready: Default::default(),
            capture: Default::default(),
        };
        let mut app = ExecutionApp::start(
            &db.path,
            Startup::CreateTest,
            host,
            runner.clone(),
            AppConfig::test_defaults(1),
        )
        .unwrap();
        let caller = RequestContext {
            actor: plan.spec().request.actor.clone(),
        };
        let bridge = ExecutionBridge::new(local::id("live-agent-consumer"), FixtureOutput);
        let prepared = bridge
            .prepare(&offer, &materials, &app, &caller, &plan)
            .unwrap();
        let start = client.request_start(&offer, &materials).await.unwrap();
        bridge
            .dispatch(&mut client, start, &materials, &mut app, prepared)
            .unwrap();
        app.reconcile(&plan.spec().request.request_id).unwrap();
        assert_eq!(
            bridge
                .flush(&mut client, offer.task_id(), &mut app, 64)
                .await
                .unwrap(),
            1
        );
        assert_eq!(runner.inner.dispatch_count(), 1);
        assert_eq!(
            bridge
                .flush(&mut client, offer.task_id(), &mut app, 64)
                .await
                .unwrap(),
            0
        );
        bridge
            .finish(&mut client, offer.task_id(), &app, &caller)
            .unwrap();
        let runs = admin
            .request(
                reqwest::Method::GET,
                &format!("/api/v2/policies/{policy}/runs"),
                None,
                None,
            )
            .await;
        assert!(!runs["items"].as_array().unwrap().is_empty());
        if !software {
            let run = admin
                .request(
                    reqwest::Method::GET,
                    &format!(
                        "/api/v2/devices/{device}/collections/{}",
                        offer.payload().attempt_id()
                    ),
                    None,
                    None,
                )
                .await;
            assert_eq!(run["asset"]["run"]["result"], "snapshot");
            assert_eq!(run["asset"]["run"]["fields"].as_array().unwrap().len(), 2);
        }
        if software {
            let rollout = admin
                .request(
                    reqwest::Method::GET,
                    &format!("/api/v2/policies/{policy}/software/rollout"),
                    None,
                    None,
                )
                .await;
            assert_eq!(rollout["stages"][0]["verifiedSuccess"], 1);
        }
    }
}
