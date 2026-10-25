use agent_client::{Client, OpenMode};
use execution_contract::{ActorId, AttemptId, Digest, EventId, RequestId};
use rss_mdm_agent_wire::runtime as w;
use uuid::Uuid;
#[path = "../../../tests/agent-protocol/mod.rs"]
mod support;
use support::*;
#[tokio::test]
async fn native_partial_observation_replays_exact_request_and_never_freezes_terminal_projection() {
    let server = Server::new().await;
    let root = Root::new();
    let task = {
        let mut d = server.data.lock().unwrap();
        let id = Uuid::new_v4();
        let spec = w::AccessTaskSpec {
            wire_version: 1,
            tenant_id: d.tenant,
            device_id: "device-1".into(),
            registration_id: d.registration,
            generation: 1,
            platform: w::TaskPlatform::Windows,
            architecture: w::TaskArchitecture::X86_64,
            task_id: d.task,
            attempt_id: d.attempt,
            permit: w::TaskPermit::Offer,
            expires_at: 60,
            deadline: 300,
            binding_id: id,
            instance_id: Uuid::new_v4(),
            principal_id: Uuid::new_v4(),
            binding_generation: 1,
            factor_generation: 1,
            revision: 1,
            definition_digest: [7; 32],
            action: w::AccessAction::Apply {
                account: w::AccessAccount::Windows {
                    sid: "S-1-12-1-1-2-3-4".into(),
                    authority: w::AccessWindowsAuthority::Entra {
                        tenant_id: d.tenant,
                        object_id: Uuid::from_bytes_le([
                            1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0,
                        ]),
                    },
                },
                configuration: w::AccessConfiguration {
                    windows: w::AccessWindowsConfiguration::Whfb,
                    mac: None,
                    self_enrollment: true,
                    enabled: true,
                    role: None,
                },
            },
            windows: Some(w::AccessWindowsPlan::Whfb {
                retire: false,
                previous_key: None,
            }),
            authorization: w::TaskAuthorization::Policy {
                policy_id: id,
                policy_version: id,
            },
        };
        d.offer = Some(d.signed(w::TaskPayload::DeviceAccess(Box::new(spec.clone()))));
        spec
    };
    let mut cfg = server.config();
    cfg.architecture = w::TaskArchitecture::X86_64;
    let mut client = Client::open(
        &root.path,
        cfg.clone(),
        OpenMode::Create,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    let association = client
        .reserve_association(
            &start,
            &RequestId::new("native-task").unwrap(),
            &Digest::new("ab".repeat(32)).unwrap(),
            &ActorId::new("native-owner").unwrap(),
        )
        .unwrap();
    let association = client
        .bind_local_attempt(
            &association,
            Some(&AttemptId::new("native-attempt").unwrap()),
        )
        .unwrap();
    let w::AccessAction::Apply { account, .. } = &task.action else {
        unreachable!()
    };
    let w::AccessAccount::Windows {
        authority:
            w::AccessWindowsAuthority::Entra {
                tenant_id,
                object_id,
            },
        ..
    } = account
    else {
        unreachable!()
    };
    let mut report = w::AccessResult {
        binding_id: task.binding_id,
        binding_generation: 1,
        revision: 1,
        definition_digest: task.definition_digest,
        effect: w::AccessEffect::Applied {
            account: account.clone(),
            role: w::AccessRole::Standard,
            mac: None,
            windows: Some(w::AccessWindowsFacts::Whfb(w::AccessWhfbFacts {
                factor_generation: 1,
                tenant_id: *tenant_id,
                object_id: *object_id,
                policy: w::AccessObservation::Present,
                registration: w::AccessObservation::Absent,
                hardware_key: w::AccessObservation::Unknown,
                key_id: None,
                observed_at: 1,
            })),
        },
    };
    let event = EventId::new("native-result").unwrap();
    assert!(client
        .freeze_result(
            &association,
            &event,
            std::slice::from_ref(&event),
            w::TaskEvent::DeviceAccessResult(report.clone())
        )
        .is_err());
    server.data.lock().unwrap().result_failure = true;
    assert!(client
        .access_observation(task.task_id, report.clone())
        .await
        .is_err());
    let original = server.data.lock().unwrap().results.clone();
    drop(client);
    let mut client = Client::open(
        &root.path,
        cfg,
        OpenMode::Existing,
        server.secrets.clone(),
        server.time.clone(),
    )
    .unwrap();
    if let w::AccessEffect::Applied {
        windows: Some(w::AccessWindowsFacts::Whfb(f)),
        ..
    } = &mut report.effect
    {
        f.observed_at = 2;
        f.registration = w::AccessObservation::Present;
        f.key_id = Some(Uuid::new_v4());
    }
    client
        .access_observation(task.task_id, report.clone())
        .await
        .unwrap();
    assert_eq!(server.data.lock().unwrap().results, original);
    assert!(client.pending_result(&association).unwrap().is_none());
    client
        .access_observation(task.task_id, report.clone())
        .await
        .unwrap();
    assert_eq!(server.data.lock().unwrap().results.len(), 2);
    if let w::AccessEffect::Applied {
        windows: Some(w::AccessWindowsFacts::Whfb(f)),
        ..
    } = &mut report.effect
    {
        f.observed_at = 3;
    }
    client
        .access_observation(task.task_id, report.clone())
        .await
        .unwrap();
    assert_eq!(server.data.lock().unwrap().result_calls, 3);
    if let w::AccessEffect::Applied {
        windows: Some(w::AccessWindowsFacts::Whfb(f)),
        ..
    } = &mut report.effect
    {
        f.hardware_key = w::AccessObservation::Present;
    }
    client
        .access_observation(task.task_id, report.clone())
        .await
        .unwrap();
    assert_eq!(server.data.lock().unwrap().result_calls, 3);
    let frozen = client
        .freeze_result(
            &association,
            &event,
            std::slice::from_ref(&event),
            w::TaskEvent::DeviceAccessResult(report),
        )
        .unwrap();
    client.send_result(frozen).await.unwrap();
    assert_eq!(server.data.lock().unwrap().result_calls, 4);
}
