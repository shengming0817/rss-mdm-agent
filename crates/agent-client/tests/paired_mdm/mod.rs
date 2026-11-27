//! Manual pairing against a disposable, revision-bound MDM deployment.
//! AGENT_REPORT_PAIR_DIR points to a private directory with endpoint.json
//! (origin, tenant and service_id) and the ephemeral gateway ca.crt.
//! Run this ignored test in separate processes with AGENT_REPORT_PAIR_PHASE
//! set to prepare, queue, submit, then verify. Authorize the emitted grant-id after
//! prepare and set up the two expired Observation receipts after submit.
use crate::test_support::{self as support, Secrets};
use agent_client::{Client, Clock, Config, Error, Limits, OpenMode, Transport};
use ring::signature::{Ed25519KeyPair, KeyPair};
use rss_mdm_agent_wire::runtime::*;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::SystemTime};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct WallClock;
impl Clock for WallClock {
    fn now(&self) -> Result<i64, Error> {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| Error::Clock)?
            .as_secs()
            .try_into()
            .map_err(|_| Error::Clock)
    }
}

#[derive(Deserialize)]
struct Endpoint {
    origin: String,
    tenant: Uuid,
    service_id: Uuid,
}

#[derive(Deserialize, Serialize)]
struct Reports {
    snapshot: ReportAck,
    failed: ReportAck,
}

#[derive(Deserialize, Serialize)]
struct Queued {
    snapshot: Uuid,
    failed: Uuid,
}

#[tokio::test]
#[ignore = "requires a disposable MDM deployment, real CA and database state transitions"]
async fn paired_mdm_reports() {
    // Inputs and credentials are provided by the disposable backend driver only.
    let root = PathBuf::from(std::env::var_os("AGENT_REPORT_PAIR_DIR").unwrap());
    let phase = std::env::var("AGENT_REPORT_PAIR_PHASE").unwrap();
    let endpoint: Endpoint =
        serde_json::from_slice(&std::fs::read(root.join("endpoint.json")).unwrap()).unwrap();
    let origin = Url::parse(&endpoint.origin).unwrap();
    assert!(
        origin
            .host_str()
            .is_some_and(|host| host.ends_with(".localhost")),
        "the pairing fixture requires a disposable local endpoint"
    );
    let mut connections =
        support::signed_connections(origin.origin().ascii_serialization(), endpoint.tenant, 1);
    connections.configuration.service_id = endpoint.service_id;
    let config = Config {
        software_read_grants: vec![],
        origin,
        service_id: endpoint.service_id,
        connections: support::sign_connections(connections.configuration),
        tenant: endpoint.tenant,
        platform: TaskPlatform::Macos,
        execution_context: support::execution_context(),
        architecture: TaskArchitecture::Aarch64,
        registration_keys: [(
            "test".into(),
            Ed25519KeyPair::from_seed_unchecked(&[7; 32])
                .unwrap()
                .public_key()
                .as_ref()
                .to_vec(),
        )]
        .into(),
        limits: Limits::test_defaults(),
        transport: Transport::Https,
        ca_pem: Some(std::fs::read(root.join("ca.crt")).unwrap()),
    };
    let identity_path = root.join("fixture-identity.json");
    let (secrets, mode) = if phase == "prepare" {
        (Secrets::new(), OpenMode::Create)
    } else {
        let mut secrets = Secrets::new();
        *Arc::get_mut(&mut secrets.0).unwrap().get_mut().unwrap() =
            serde_json::from_slice(&std::fs::read(&identity_path).unwrap()).unwrap();
        (secrets, OpenMode::Existing)
    };
    let mut client = Client::open(&root, config, mode, secrets.clone(), WallClock).unwrap();
    match phase.as_str() {
        "prepare" => {
            let status = client
                .request_authorization(vec![Capability::InventoryCollectionV1])
                .await
                .unwrap();
            let identity = serde_json::to_vec(&*secrets.0.lock().unwrap()).unwrap();
            platform_private_storage::write_new(&identity_path, &identity).unwrap();
            std::fs::write(root.join("grant-id"), status.grant_id.unwrap().to_string()).unwrap();
        }
        "queue" => {
            client.drive_enrollment().await.unwrap();
            let registration = client.registration().unwrap();
            let field = FieldKey::parse("channel.mdm.enrollment").unwrap();
            let collection = registration
                .collections
                .iter()
                .find(|c| c.field(field).is_ok())
                .unwrap();
            let snapshot = client
                .queue_report(
                    collection.dataset(),
                    ReportBody::Snapshot(vec![FieldValue {
                        field,
                        value: CollectedValue::Value(Scalar::String("unenrolled".into())),
                    }]),
                    WallClock.now().unwrap(),
                )
                .unwrap();
            let failed = client
                .queue_report(
                    collection.dataset(),
                    ReportBody::Failed {
                        code: FailureCode::CollectionFailed,
                    },
                    WallClock.now().unwrap(),
                )
                .unwrap();
            std::fs::write(
                root.join("queued.json"),
                serde_json::to_vec(&Queued { snapshot, failed }).unwrap(),
            )
            .unwrap();
        }
        "submit" => {
            let Queued { snapshot, failed } =
                serde_json::from_slice(&std::fs::read(root.join("queued.json")).unwrap()).unwrap();
            assert_eq!(client.report_status(snapshot).await, Err(Error::NotFound));
            assert_eq!(client.report_status(failed).await, Err(Error::NotFound));
            let reports = Reports {
                snapshot: client.flush_next_report().await.unwrap().unwrap(),
                failed: client.flush_next_report().await.unwrap().unwrap(),
            };
            assert_eq!(reports.snapshot.report_id, snapshot);
            assert_eq!(reports.failed.report_id, failed);
            assert_eq!(
                client.report_status(snapshot).await.unwrap().ack,
                reports.snapshot
            );
            assert_eq!(
                client.report_status(failed).await.unwrap().ack,
                reports.failed
            );
            std::fs::write(
                root.join("reports.json"),
                serde_json::to_vec(&reports).unwrap(),
            )
            .unwrap();
        }
        "verify" => {
            let reports: Reports =
                serde_json::from_slice(&std::fs::read(root.join("reports.json")).unwrap()).unwrap();
            let snapshot = client
                .report_status(reports.snapshot.report_id)
                .await
                .unwrap();
            let failed = client
                .report_status(reports.failed.report_id)
                .await
                .unwrap();
            assert_eq!(snapshot.ack, reports.snapshot);
            assert_eq!(failed.ack, reports.failed);
            assert_eq!(snapshot.observation, ObservationStatus::Expired);
            assert_eq!(failed.observation, ObservationStatus::Expired);
            assert_eq!(snapshot.projection, ProjectionStatus::Applied);
            assert_eq!(failed.projection, ProjectionStatus::Unconfirmed);
            assert_eq!(
                client.report_status(Uuid::new_v4()).await,
                Err(Error::NotFound)
            );
            assert_eq!(client.flush_reports(2).await, Ok(0));
            std::fs::write(
                root.join("verified.json"),
                serde_json::to_vec(&[snapshot, failed]).unwrap(),
            )
            .unwrap();
        }
        _ => panic!("expected prepare, queue, submit or verify pairing phase"),
    }
}
