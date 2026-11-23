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
            json!({"grant":grant,"issuanceId":s.candidate.as_ref().map(|v|v.issuance_id),"issuanceUnknown":false,"registration":s.registration})
        }
        "certificates" => {
            let req: wire::CertificateRequest = serde_json::from_value(input).unwrap();
            assert_eq!(req.grant_id, grant.grant_id);
            let csr =
                x509_cert::request::CertReq::from_der(&STANDARD.decode(req.csr).unwrap()).unwrap();
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
                "renewal-grants" => {
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
                    json!(next)
                }
                "configuration-trust" => json!(s.trust),
                "configuration-trust/acknowledgements" => {
                    assert_eq!(input["digest"], s.trust.installation_digest().unwrap());
                    return ().into_response();
                }
                "pki/acknowledgements" => return ().into_response(),
                "configuration" => {
                    let d = s.config.decode().unwrap();
                    json!({"state":"present","artifact":s.config,"publication":{"id":d.id,"version":d.version,"revision":d.revision,"enabled":true,"managedFields":[],"configuration":d.content,"target":{"kind":"tenant_default"}}})
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
struct Consumer(Option<AccessConfiguration>);
impl PolicyConsumer for Consumer {
    fn managed_snapshot(&self) -> ManagedSnapshot {
        ManagedSnapshot {
            state: SourceState::Missing,
            sources: vec![],
        }
    }
    fn apply(
        &mut self,
        s: Option<&AccessConfiguration>,
    ) -> Result<Option<AccessConfiguration>, Error> {
        self.0 = s.cloned();
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
    }));
    let listener = TlsListener {
        listener: socket,
        acceptor: tokio_rustls::TlsAcceptor::from(Arc::new(fixture::server())),
    };
    let router = axum::Router::new()
        .fallback(endpoint)
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
    let req = first.prepare(id).unwrap();
    assert_eq!(req.csr, first.prepare(id).unwrap().csr);
    let grant = wire::Grant {
        operation_id: id,
        grant_id: Uuid::new_v4(),
        device: rss_mdm_agent_wire::DeviceId::new_v4(),
        expires_at: 2_000_010_000,
    };
    state.lock().unwrap().grant = Some(grant.clone());
    first.import(grant.clone()).unwrap();
    assert!(first.drive(&mut consumer).await.is_err());
    assert!(state.lock().unwrap().registration.is_some());
    drop(first);
    let second = open();
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
        inspect(&root.path, &backend, install)
            .unwrap()
            .unwrap()
            .application_sequence,
        3
    );
    assert!(inspect(&root.path, &backend, Uuid::new_v4()).is_err());
    client.start_renewal().unwrap();
    client.drive(&mut consumer).await.unwrap();
    let renewed = client.active().unwrap().unwrap();
    assert_ne!(renewed.receipt.credential, active.receipt.credential);
    assert_eq!(renewed.receipt.device, grant.device);
    assert!(consumer.0.is_some());
    assert_eq!(state.lock().unwrap().status.len(), 1);
    assert!(matches!(
        serde_json::from_value::<ApplicationState>(json!("verified")),
        Err(_)
    ));
    server.abort();
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
    let published = json!({"managedFields":["windows"]});
    let run = |doc: &wire::Configuration, input, publication: &Value| {
        client.resolve(&identity, &trust, doc, publication, input, &mut vec![])
    };
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
        run(&released, absent(), &json!({"managedFields":[]})).unwrap(),
        backend_config.content
    );
}
