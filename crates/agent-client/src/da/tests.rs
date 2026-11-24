//! Real TLS and SQLite consumer test. The disposable endpoint issues the exact CSR public key
//! and requires the candidate leaf on activation and on every subsequent device operation.
use super::*;
use crate::test_support::{
    pki as fixture, Root, Secrets, Time, TlsIdentity, TlsListener, SERVICE_ID,
};
use axum::{
    extract::{ConnectInfo, State},
    http::Uri,
    response::{IntoResponse, Response},
    Json,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use der::{Decode, Encode};
use ring::signature::{Ed25519KeyPair, KeyPair};
use rss_mdm_agent_wire::runtime::AccessConfiguration;
use serde_json::{json, Value};
use std::sync::{atomic::AtomicI64, Mutex};
use wire::{ApplicationState, DeliveryPermission, Health, SourceState};

impl Credentials for Secrets {
    fn release(&self, reference: &str, _key: bool) -> Result<(), Error> {
        self.0.lock().unwrap().remove(reference);
        Ok(())
    }
}
struct Server {
    grant: Option<wire::Grant>,
    candidate: Option<enroll::Candidate>,
    registration: Option<wire::Registration>,
    active: Option<Vec<u8>>,
    status: Vec<Value>,
    reports: Vec<Value>,
    trust: wire::ConfigurationTrust,
    config: wire::SignedConfiguration,
    origin: String,
    lose_activation: bool,
    lose_report: bool,
    fail_trust_ack: bool,
    lose_renewal_reply: bool,
    expire_after_issue: bool,
}
async fn endpoint(
    State(state): State<Arc<Mutex<Server>>>,
    ConnectInfo(peer): ConnectInfo<TlsIdentity>,
    uri: Uri,
    body: axum::body::Bytes,
) -> Response {
    let mut s = state.lock().unwrap();
    let input: Value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
    let base = "/api/v1/da/";
    let route = uri.path().strip_prefix(base).expect("DA listener only");
    let grant = s.grant.clone().unwrap();
    let tenant = s.trust.tenant;
    let output = match route {
        "pki/trust" => {
            assert_eq!(uri.query(), Some(format!("tenantId={tenant}").as_str()));
            let mut v = fixture::trust(&s.origin, tenant);
            v["purpose"] = "da_device".into();
            v["issuers"][0]["provisioner"] = "rss-da-device".into();
            v
        }
        "registrations/status" => {
            assert_eq!(input["operationId"], grant.operation_id.to_string());
            assert_eq!(input["grantId"], grant.grant_id.to_string());
            assert_eq!(
                input["secret"],
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            );
            json!({"grant":grant,"issuanceId":s.candidate.as_ref().map(|v|v.issuance_id),"issuanceUnknown":false,"registration":s.registration})
        }
        "certificates" => return certificate_response(&mut s, &grant, tenant, input),
        "activate" => {
            let c = s.candidate.clone().unwrap();
            assert_eq!(
                peer.0.as_ref(),
                Some(&STANDARD.decode(&c.certificate_chain[0]).unwrap())
            );
            assert_eq!(input["issuanceId"], c.issuance_id.to_string());
            s.active = peer.0;
            let r = wire::Registration {
                service_id: SERVICE_ID,
                tenant,
                device: grant.device,
                credential: Uuid::new_v4(),
                issuance_id: c.issuance_id,
                operation_id: serde_json::from_value(input["operationId"].clone()).unwrap(),
            };
            s.registration = Some(r.clone());
            if std::mem::take(&mut s.lose_activation) {
                return (
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"code":"activation_unknown","issuanceId":null})),
                )
                    .into_response();
            }
            json!(r)
        }
        other => {
            assert!(s.active.is_some() && peer.0 == s.active);
            match other {
                "renewal-grants" => return renewal_response(&mut s, &grant, input),
                "configuration-trust" => json!(s.trust),
                "configuration-trust/acknowledgements" => {
                    assert_eq!(input["digest"], s.trust.installation_digest().unwrap());
                    if s.fail_trust_ack {
                        return (
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            Json(json!({"code":"service_unavailable","issuanceId":null})),
                        )
                            .into_response();
                    }
                    return ().into_response();
                }
                "pki/acknowledgements" => return ().into_response(),
                "configuration" => {
                    let d = s.config.decode().unwrap();
                    json!({"identity":{"serviceId":SERVICE_ID,"tenant":tenant,"deviceId":grant.device},"authorities":[{"policy":d.id,"version":d.version,"revision":d.revision,"enabled":true,"fields":{}}],"state":"present","artifact":s.config,"publication":{"id":d.id,"version":d.version,"revision":d.revision,"enabled":true,"managedFields":[],"configuration":d.content,"target":{"kind":"tenant_default"}}})
                }
                "status-reports" | "configuration-reports" => {
                    let seq = input["sequence"].as_i64().unwrap();
                    let history = if other == "status-reports" {
                        &mut s.status
                    } else {
                        &mut s.reports
                    };
                    if let Some(old) = history.iter().find(|v| v["sequence"] == seq) {
                        assert_eq!(old, &input);
                    } else {
                        history.push(input);
                    }
                    if other == "configuration-reports" && std::mem::take(&mut s.lose_report) {
                        return (
                            axum::http::StatusCode::SERVICE_UNAVAILABLE,
                            Json(json!({"code":"service_unavailable","issuanceId":null})),
                        )
                            .into_response();
                    }
                    json!({"sequence":seq,"receivedAt":2_000_000_000,"evidence":"reported"})
                }
                _ => panic!("unexpected {other}"),
            }
        }
    };
    Json(output).into_response()
}
fn renewal_response(s: &mut Server, grant: &wire::Grant, input: Value) -> Response {
    let req: wire::GrantRequest = serde_json::from_value(input).unwrap();
    let next = wire::Grant {
        grant_id: Uuid::new_v4(),
        device: grant.device,
        operation_id: req.operation_id,
        expires_at: 2_000_010_000,
    };
    s.grant = Some(next.clone());
    s.registration = None;
    s.candidate = None;
    if s.lose_renewal_reply {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"code":"service_unavailable","issuanceId":null})),
        )
            .into_response();
    }
    Json(next).into_response()
}
fn certificate_response(
    s: &mut Server,
    grant: &wire::Grant,
    tenant: Uuid,
    input: Value,
) -> Response {
    if !s.expire_after_issue {
        return Json(issue_candidate(s, grant, tenant, input)).into_response();
    }
    let (status, code) = if s.candidate.is_some() {
        (axum::http::StatusCode::FORBIDDEN, "grant_expired")
    } else {
        issue_candidate(s, grant, tenant, input);
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
    };
    (status, Json(json!({"code":code,"issuanceId":null}))).into_response()
}
fn issue_candidate(s: &mut Server, grant: &wire::Grant, tenant: Uuid, input: Value) -> Value {
    let req: wire::CertificateRequest = serde_json::from_value(input).unwrap();
    assert_eq!(req.grant_id, grant.grant_id);
    let csr = x509_cert::request::CertReq::from_der(&STANDARD.decode(req.csr).unwrap()).unwrap();
    assert_eq!(csr.info.subject.to_string(), "CN=rss-da-device");
    let san = format!(
        "urn:rss:da:device:v1:{SERVICE_ID}:{tenant}:{}",
        URL_SAFE_NO_PAD.encode(grant.device.to_string())
    );
    let chain = fixture::device_chain(
        "CN=rss-da-device",
        &san,
        csr.info.public_key.to_der().unwrap(),
        4_070_908_800,
    );
    json!(s.candidate.get_or_insert_with(|| enroll::Candidate {
        wire_version: 1,
        operation_id: req.operation_id,
        issuance_id: Uuid::new_v4(),
        certificate_chain: chain,
        activate_before: 2_000_010_000
    }))
}
async fn contract_exchange(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    use axum::body::{to_bytes, Body};
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.unwrap();
    let contract = crate::test_support::contract::fixed();
    contract
        .request(parts.method.as_str(), &parts.uri, &parts.headers, &body)
        .unwrap();
    let endpoint = contract
        .endpoint(parts.method.as_str(), parts.uri.path())
        .unwrap();
    let response = next
        .run(axum::http::Request::from_parts(parts, Body::from(body)))
        .await;
    let (parts, body) = response.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.unwrap();
    contract
        .response(endpoint, parts.status.as_u16(), &parts.headers, &body)
        .unwrap();
    Response::from_parts(parts, Body::from(body))
}
struct Consumer(Option<AccessConfiguration>);
impl PolicyConsumer for Consumer {
    fn managed_snapshot(&self) -> ManagedSnapshot {
        ManagedSnapshot {
            state: SourceState::Missing,
            sources: vec![],
        }
    }
    fn apply(&mut self, s: Option<&Selection>) -> Result<Option<AccessConfiguration>, Error> {
        self.0 = s.map(|selection| selection.settings.clone());
        Ok(self.0.clone())
    }
    fn health(&self) -> (Health, Health) {
        (Health::Unknown, Health::Unknown)
    }
}
fn signed(tenant: Uuid, key: &Ed25519KeyPair) -> wire::SignedConfiguration {
    let content=serde_json::from_value(json!({"windows":{"kind":"whfb"},"mac":null,"role":null,"selfEnrollment":false,"enabled":true})).unwrap();
    let native = wire::NativeExpectation::new(&content, vec![], None).unwrap();
    let config = wire::Configuration {
        service_id: SERVICE_ID,
        tenant,
        id: Uuid::new_v4(),
        version: Uuid::new_v4(),
        revision: 7,
        trust_revision: 1,
        operation_id: Uuid::new_v4(),
        audience: wire::Audience::Tenant,
        delivery: DeliveryPermission::Da,
        managed_fields: vec![],
        issued_at: 1_999_000_000,
        expires_at: 2_001_000_000,
        content,
        native,
    };
    let payload = config.payload().unwrap();
    let bytes = wire::SignedConfiguration::signing_bytes(&payload, "da").unwrap();
    wire::SignedConfiguration {
        digest: wire::SignedConfiguration::payload_digest(&payload).unwrap(),
        key_id: "da".into(),
        signature: URL_SAFE_NO_PAD.encode(key.sign(&bytes).as_ref()),
        payload,
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_tls_recovers_activation_reports_and_rotates_the_key() {
    let root = Root::new();
    let tenant = Uuid::new_v4();
    let install = Uuid::new_v4();
    let key = Ed25519KeyPair::from_seed_unchecked(&[49; 32]).unwrap();
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("https://{}", socket.local_addr().unwrap());
    let backend = Backend {
        instance: SERVICE_ID,
        tenant,
        origin: origin.clone(),
        ca_pem: String::from_utf8(fixture::ca()).unwrap(),
    };
    let trust = wire::ConfigurationTrust {
        service_id: SERVICE_ID,
        tenant,
        revision: 1,
        keys: vec![rss_mdm_agent_wire::task_trust::PublicKey {
            id: "da".into(),
            public_key: URL_SAFE_NO_PAD.encode(key.public_key().as_ref()),
            state: rss_mdm_agent_wire::task_trust::KeyState::Active,
        }],
    };
    let state = Arc::new(Mutex::new(Server {
        grant: None,
        candidate: None,
        registration: None,
        active: None,
        status: vec![],
        reports: vec![],
        config: signed(tenant, &key),
        trust,
        origin,
        lose_activation: true,
        lose_report: true,
        fail_trust_ack: false,
        lose_renewal_reply: false,
        expire_after_issue: false,
    }));
    let listener = TlsListener {
        listener: socket,
        acceptor: tokio_rustls::TlsAcceptor::from(Arc::new(fixture::server())),
    };
    let router = axum::Router::new()
        .fallback(endpoint)
        .layer(axum::middleware::from_fn(contract_exchange))
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<TlsIdentity>(),
        )
        .await
        .unwrap()
    });
    let time = Time(Arc::new(AtomicI64::new(2_000_000_000)));
    let secrets = Secrets::new();
    let open = || {
        Client::open(
            &root.path,
            backend.clone(),
            install,
            secrets.clone(),
            time.clone(),
        )
        .unwrap()
    };
    let mut consumer = Consumer(None);
    let first = open();
    let id = Uuid::new_v4();
    let req = first.prepare(id, Ok).unwrap();
    assert_eq!(req.csr, first.prepare(id, Ok).unwrap().csr);
    let grant = wire::Grant {
        operation_id: id,
        grant_id: Uuid::new_v4(),
        device: rss_mdm_agent_wire::DeviceId::new_v4(),
        expires_at: 2_000_000_001,
    };
    state.lock().unwrap().grant = Some(grant.clone());
    first.import(grant.clone()).unwrap();
    assert!(first.drive(&mut consumer).await.is_err());
    assert!(state.lock().unwrap().registration.is_some());
    drop(first);
    time.set(2_000_000_002);
    let second = open();
    assert_eq!(
        second.prepare(Uuid::new_v4(), Ok).err(),
        Some(Error::Conflict)
    );
    assert!(second.drive(&mut consumer).await.is_err());
    drop(second);
    let client = open();
    client.drive(&mut consumer).await.unwrap();
    let active = client.active().unwrap().unwrap();
    assert_eq!(active.receipt.device, grant.device);
    assert_eq!(
        state
            .lock()
            .unwrap()
            .reports
            .iter()
            .map(|v| v["state"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["received", "validated", "applied"]
    );
    assert_eq!(
        inspect(&root.path, &backend, install, time.now().unwrap())
            .unwrap()
            .unwrap()
            .application_sequence,
        3
    );
    assert!(inspect(&root.path, &backend, Uuid::new_v4(), time.now().unwrap()).is_err());
    client.start_renewal().unwrap();
    client.drive(&mut consumer).await.unwrap();
    let renewed = client.active().unwrap().unwrap();
    assert_ne!(renewed.receipt.credential, active.receipt.credential);
    assert_eq!(renewed.receipt.device, grant.device);
    assert!(consumer.0.is_some());
    assert_eq!(state.lock().unwrap().status.len(), 1);
    assert!(serde_json::from_value::<ApplicationState>(json!("verified")).is_err());
    let archived = inspect(&root.path, &backend, install, time.now().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(archived.application_receipt.unwrap()["sequence"], 3);
    client.release_settled().unwrap();
    assert!(secrets.key(&active.key).is_err());
    assert!(secrets.key(&renewed.key).is_ok());
    super::validate_keys(&root.path, &backend, install, &secrets).unwrap();
    let rows: i64 = client
        .journal
        .db
        .query_row(
            "SELECT count(*) FROM reports WHERE receipt IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 2);
    state.lock().unwrap().expire_after_issue = true;
    let pending = client.start_renewal().unwrap();
    assert_eq!(
        client.prepare(pending, |_| Err::<(), _>(Error::Storage)),
        Err(Error::Storage)
    );
    client.drive(&mut consumer).await.unwrap();
    assert!(client.pending().unwrap().is_some() && state.lock().unwrap().candidate.is_some());
    assert_eq!(
        client
            .prepare::<()>(pending, |_| panic!("must resume renewal"))
            .err(),
        Some(Error::DaGrantPending)
    );
    client.drive(&mut consumer).await.unwrap();
    assert!(client.pending().unwrap().is_none());
    assert_eq!(
        client.active().unwrap().unwrap().receipt.device,
        renewed.receipt.device
    );
    let last_selection = consumer.0.clone();
    {
        let mut producer = state.lock().unwrap();
        producer.trust.revision += 1;
        producer.trust.keys[0].id = "replacement".into();
        producer.fail_trust_ack = true;
    }
    assert!(client.drive(&mut consumer).await.is_err());
    assert!(consumer.0.is_none());
    state.lock().unwrap().lose_renewal_reply = true;
    let lost_renewal = client.start_renewal().unwrap();
    assert!(client.drive(&mut consumer).await.is_err());
    assert_eq!(
        client
            .prepare::<()>(lost_renewal, |_| panic!("unsettled renewal"))
            .err(),
        Some(Error::DaGrantPending)
    );
    server.abort();
    // Simulate a crash after durable invalidation but before the selected file was removed.
    consumer.0 = last_selection;
    drop(client);
    let client = open();
    assert!(client.drive(&mut consumer).await.is_err());
    assert!(consumer.0.is_none());
    time.set(renewed.validity.1 + 1);
    assert!(client.drive(&mut consumer).await.is_err());
    assert!(consumer.0.is_none());
    assert!(inspect(&root.path, &backend, install, time.now().unwrap())
        .unwrap()
        .unwrap()
        .effective_digest
        .is_none());
    let op = Uuid::new_v4();
    client.prepare(op, Ok).unwrap();
    let grant = wire::Grant {
        operation_id: op,
        device: renewed.receipt.device,
        grant_id: Uuid::new_v4(),
        expires_at: time.now().unwrap() + 3600,
    };
    let mut wrong = grant.clone();
    wrong.device = rss_mdm_agent_wire::DeviceId::new_v4();
    assert_eq!(client.import(wrong), Err(Error::Identity));
    client.import(grant).unwrap();
    assert_eq!(
        client
            .prepare::<()>(op, |_| panic!("grant already imported"))
            .err(),
        Some(Error::DaGrantPending)
    );
}

#[test]
fn expired_unsubmitted_grant_allows_new_authorization_without_reusing_its_key() {
    let root = Root::new();
    let time = Time(Arc::new(AtomicI64::new(2_000_000_000)));
    let secrets = Secrets::new();
    let backend = Backend {
        instance: SERVICE_ID,
        tenant: Uuid::new_v4(),
        origin: "https://da.test".into(),
        ca_pem: String::from_utf8(fixture::ca()).unwrap(),
    };
    let installation = Uuid::new_v4();
    let client = Client::open(
        &root.path,
        backend,
        installation,
        secrets.clone(),
        time.clone(),
    )
    .unwrap();
    let original = client.prepare(Uuid::new_v4(), Ok).unwrap();
    let expired = wire::Grant {
        grant_id: Uuid::new_v4(),
        operation_id: original.operation_id,
        expires_at: 1_999_999_999,
        device: rss_mdm_agent_wire::DeviceId::new_v4(),
    };
    assert_eq!(client.import(expired), Err(Error::Expired));
    assert!(has_live_credentials(&root.path, installation).unwrap());
    client.release_settled().unwrap();
    assert!(!has_live_credentials(&root.path, installation).unwrap());
    assert!(has_live_credentials(&root.path, Uuid::new_v4()).is_err());
    assert!(client.prepare(original.operation_id, Ok).is_err());
    let next = client.prepare(Uuid::new_v4(), Ok).unwrap();
    let valid = wire::Grant {
        grant_id: Uuid::new_v4(),
        operation_id: next.operation_id,
        expires_at: 2_000_000_010,
        device: rss_mdm_agent_wire::DeviceId::new_v4(),
    };
    client.import(valid).unwrap();
    time.set(2_000_000_010);
    let fresh = client.prepare(Uuid::new_v4(), Ok).unwrap();
    assert_ne!(fresh.csr, next.csr);
}
#[test]
fn field_locks_survive_disappearing_sources_and_reject_rollback_and_foreign_signatures() {
    use wire::{ConfigurationField as Field, ReportIdentity};
    let root = Root::new();
    let tenant = Uuid::new_v4();
    let key = Ed25519KeyPair::from_seed_unchecked(&[48; 32]).unwrap();
    let signed = signed(tenant, &key);
    let backend_config = signed.decode().unwrap();
    let client = Client::open(
        &root.path,
        Backend {
            instance: SERVICE_ID,
            tenant,
            origin: "https://da.test".into(),
            ca_pem: String::from_utf8(fixture::ca()).unwrap(),
        },
        Uuid::new_v4(),
        Secrets::new(),
        Time(Arc::new(AtomicI64::new(2_000_000_000))),
    )
    .unwrap();
    let trust = wire::ConfigurationTrust {
        service_id: SERVICE_ID,
        tenant,
        revision: 3,
        keys: vec![rss_mdm_agent_wire::task_trust::PublicKey {
            id: "da".into(),
            public_key: URL_SAFE_NO_PAD.encode(key.public_key().as_ref()),
            state: rss_mdm_agent_wire::task_trust::KeyState::VerifyOnly,
        }],
    };
    let identity = ReportIdentity {
        service_id: SERVICE_ID,
        tenant,
        device_id: rss_mdm_agent_wire::DeviceId::new_v4(),
    };
    let mut managed = backend_config.clone();
    managed.delivery = DeliveryPermission::ManagedMandatory;
    managed.operation_id = Uuid::new_v4();
    managed.revision -= 1;
    managed.managed_fields = vec![Field::Windows];
    managed.content.windows =
        serde_json::from_value(json!({"kind":"rss_factor","accounts":["local"]})).unwrap();
    managed.native = wire::NativeExpectation::new(&managed.content, vec![], None).unwrap();
    managed.content.role = Some(rss_mdm_agent_wire::runtime::AccessRole::Administrator);
    let wrap = |doc: &wire::Configuration| {
        let payload = doc.payload().unwrap();
        let signature = URL_SAFE_NO_PAD.encode(
            key.sign(&wire::SignedConfiguration::signing_bytes(&payload, "da").unwrap())
                .as_ref(),
        );
        wire::SignedConfiguration {
            key_id: "da".into(),
            signature,
            digest: wire::SignedConfiguration::payload_digest(&payload).unwrap(),
            payload,
        }
    };
    let snapshot = |e| ManagedSnapshot {
        state: SourceState::Present,
        sources: vec![ManagedSource { envelope: e }],
    };
    let absent = || ManagedSnapshot {
        state: SourceState::Missing,
        sources: vec![],
    };
    let authority = |doc: &wire::Configuration, authorized| wire::PolicyAuthority {
        policy: doc.id,
        version: doc.version,
        enabled: true,
        revision: doc.revision,
        fields: [(
            Field::Windows,
            wire::FieldAuthority {
                authorized,
                revision: if authorized { 1 } else { doc.revision },
            },
        )]
        .into(),
    };
    let published = [authority(&backend_config, true)];
    let run = |doc: &wire::Configuration, input, policies: &[wire::PolicyAuthority]| {
        client.observe_authorities(policies)?;
        client
            .resolve(&identity, &trust, doc, policies, input, &mut vec![])
            .map(|(settings, _)| settings)
    };
    let selected = run(&backend_config, snapshot(wrap(&managed)), &published).unwrap();
    assert_eq!(selected.windows, managed.content.windows);
    assert_ne!(selected.windows, backend_config.content.windows);
    let mut older = managed.clone();
    older.revision -= 1;
    assert_eq!(
        run(&backend_config, snapshot(wrap(&older)), &published),
        Err(Error::Conflict)
    );
    assert_eq!(
        run(&backend_config, snapshot(wrap(&managed)), &published)
            .unwrap()
            .role,
        backend_config.content.role
    );
    assert_eq!(
        run(&backend_config, absent(), &published),
        Err(Error::Unavailable)
    );
    let mut foreign = managed.clone();
    foreign.tenant = Uuid::new_v4();
    assert_eq!(
        run(&backend_config, snapshot(wrap(&foreign)), &published),
        Err(Error::Untrusted)
    );
    let mut foreign = managed.clone();
    foreign.service_id = Uuid::new_v4();
    assert_eq!(
        run(&backend_config, snapshot(wrap(&foreign)), &published),
        Err(Error::Untrusted)
    );
    let mut wrong = wrap(&managed);
    wrong.signature = URL_SAFE_NO_PAD.encode(key.sign(wrong.payload.as_bytes()).as_ref());
    assert_eq!(
        run(&backend_config, snapshot(wrong), &published),
        Err(Error::Untrusted)
    );
    let mut foreign = managed.clone();
    foreign.audience = wire::Audience::Device {
        device: rss_mdm_agent_wire::DeviceId::new_v4(),
    };
    assert_eq!(
        run(&backend_config, snapshot(wrap(&foreign)), &published),
        Err(Error::Untrusted)
    );
    let mut old = backend_config.clone();
    old.revision -= 1;
    assert_eq!(run(&old, absent(), &published), Err(Error::Conflict));
    let mut released = backend_config.clone();
    released.revision += 1;
    released.version = Uuid::new_v4();
    assert_eq!(
        run(&released, absent(), &[authority(&released, false)]).unwrap(),
        backend_config.content
    );
    let mut other = managed.clone();
    other.id = Uuid::new_v4();
    let policies = [authority(&released, false), authority(&other, true)];
    assert_eq!(
        run(&released, snapshot(wrap(&other)), &policies)
            .unwrap()
            .windows,
        other.content.windows
    );
    assert_eq!(run(&released, absent(), &policies), Err(Error::Unavailable));
    assert_eq!(run(&released, absent(), &published), Err(Error::Conflict));
}
