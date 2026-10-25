//! Real SQLite/TLS settlement; the runner and protected result producer are explicit fixtures.
use super::{
    bridge::ExecutionBridge,
    test_support::{
        local::{create_app, reopen_app},
        *,
    },
};
use crate::protocol_test_support::*;
use agent_client::{Error, OpenMode};
use base64::{engine::general_purpose::STANDARD, Engine};
use execution_app::*;
use execution_contract::*;
use rss_mdm_agent_wire::runtime as w;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn write(
    root: &std::path::Path,
    task: &w::CredentialTaskSpec,
    suffix: &str,
    value: &impl serde::Serialize,
) {
    use std::io::Write;
    let name = format!(
        "{}-{}-{}-{}.{}",
        task.registration_id, task.generation, task.task_id, task.attempt_id, suffix
    );
    let dir = platform_private_storage::PrivateDirectory::open(root).unwrap();
    let mut file = dir.create_file(std::path::Path::new(&name)).unwrap();
    file.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
    file.sync_all().unwrap();
}
#[tokio::test]
async fn credential_success_settles_after_reopen_and_ack_then_releases_pending() {
    settlement(0).await;
}
#[tokio::test]
async fn credential_no_effect_settles_but_unknown_retains_recovery() {
    settlement(1).await;
    settlement(2).await;
}

async fn settlement(case: u8) {
    let _budget = crate::scenario_budget::enter();
    let server = Server::tls().await;
    {
        let mut data = server.data.lock().unwrap();
        let mut task = w::CredentialTaskSpec {
            wire_version: 1,
            tenant_id: data.tenant,
            device_id: "device-1".into(),
            platform: w::TaskPlatform::Windows,
            architecture: w::TaskArchitecture::Aarch64,
            registration_id: data.registration,
            generation: 1,
            task_id: data.task,
            attempt_id: data.attempt,
            permit: w::TaskPermit::Offer,
            expires_at: 60,
            deadline: 300,
            authorization: w::TaskAuthorization::Policy {
                policy_id: Uuid::new_v4(),
                policy_version: Uuid::new_v4(),
            },
            operation: w::CredentialOperation::RotateLocalAdministrator {
                sid: "S-1-5-21-1-2-3-1001".into(),
            },
            recipient: w::CredentialRecipient {
                key_id: "a".repeat(64),
                public_key: STANDARD.encode([1; 32]),
            },
            expected: vec![],
            recovery: None,
            definition_digest: [0; 32],
        };
        task.definition_digest = w::CredentialTaskSpec::digest(
            &task.operation,
            &task.recipient,
            &task.expected,
            &task.recovery,
        )
        .unwrap();
        data.offer = Some(data.signed(w::TaskPayload::Credential(Box::new(task))));
    }

    let root = Root::new();
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    let w::TaskPayload::Credential(task) = start.payload() else {
        panic!()
    };
    let task = (**task).clone();
    let credential_root = root.path.join("credentials");
    platform_private_storage::PrivateDirectory::create(&credential_root).unwrap();
    let subject = w::CredentialSubject::LocalAdministrator {
        sid: "S-1-5-21-1-2-3-1001".into(),
    };
    let result = if case == 1 {
        crate::credential_worker::failed(&task, w::CredentialFailure::OwnerConflict)
    } else {
        w::CredentialResult {
            definition_digest: task.definition_digest,
            recovery: None,
            complete: case == 0,
            failure: (case == 2).then_some(w::CredentialFailure::NativeFailure),
            items: vec![w::CredentialItemResult {
                subject: subject.clone(),
                outcome: if case == 0 {
                    w::CredentialOutcome::Verified
                } else {
                    w::CredentialOutcome::Unknown
                },
            }],
        }
    };
    let mut spec = adapted_plan(false, "device-1", &offer).spec().clone();
    spec.request.target.platform = Platform::Windows;
    spec.constraints = IsolationPolicy::OsIdentity {};
    spec.launch.cwd = r"C:\ProgramData\RSS".into();
    spec.run_as = RunAs::System {
        platform: Platform::Windows,
    };
    spec.launch.interpreter.profile =
        super::plan::reference(crate::credential_worker::PROFILE, "1").unwrap();
    spec.request.operation.resource = super::plan::reference(
        "backend-resource",
        super::plan::hex(&task.definition_digest),
    )
    .unwrap();
    spec.validity.expires_at_unix_ms = task.expires_at as u64 * 1000;
    spec.request.parameters.insert(
        "credential-root".into(),
        InputValue::Literal {
            value: serde_json::to_value(&credential_root).unwrap(),
        },
    );
    let input = crate::credential_worker::Input {
        task: task.clone(),
        root: credential_root.clone(),
        image_sha256: spec.launch.interpreter.artifact.sha256.clone(),
        run_as: spec.run_as.clone(),
        session: spec.session_requirement.clone(),
    };
    use sha2::{Digest as _, Sha256};
    spec.launch.artifact.sha256 = Digest::new(super::plan::hex(&Sha256::digest(
        serde_json::to_vec(&input).unwrap(),
    )))
    .unwrap();
    let plan = FrozenExecution::freeze(spec, &test_execution_limits()).unwrap();
    let db = local::Database::new();
    let mut host = local::TestHost::new();
    host.template = plan.clone();
    let runner = CapturingRunner {
        inner: DeterministicTestRunner::new(local::id("test-runner"), TestScenario::Complete, 16)
            .unwrap(),
        ready: Default::default(),
        progress: Default::default(),
        capture: Arc::new(Mutex::new(Some(CaptureSpec {
            exit_code: 0,
            quiescent: true,
            stdout: b"untrusted stdout cannot settle credentials".to_vec(),
            stderr: vec![],
            quality: OutputQuality::Complete,
            end: ProcessEnd::Exited,
        }))),
    };
    let caller = RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let mut app = create_app(
        &db.path,
        host.clone(),
        runner.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let bridge = ExecutionBridge::new(local::id("agent-consumer"), FixtureOutput);
    let prepared = bridge
        .prepare(&offer, &materials, &app, &caller, &plan)
        .unwrap();
    bridge
        .dispatch(&mut client, start, &materials, &mut app, prepared)
        .unwrap();
    let request = &plan.spec().request.request_id;
    let attempt = app.status(&caller, request).unwrap().attempt_id.unwrap();
    let evidence = super::credential::EffectEvidence {
        task: &task,
        attempt: &attempt,
    };
    let reference = super::plan::reference(task.task_id.to_string(), "credential-result").unwrap();
    assert!(!app
        .service_assess(request, &attempt, reference.clone(), &evidence)
        .unwrap());
    app.reconcile(request).unwrap();
    assert_eq!(
        app.status(&caller, request).unwrap().phase,
        TaskPhase::ExecutionEnded
    );
    assert!(app
        .service_assess(request, &attempt, reference.clone(), &evidence)
        .is_err()); // no durable result
    write(&credential_root, &task, "result", &result);
    if case != 1 {
        let escrow = w::CredentialEscrow {
            definition_digest: task.definition_digest,
            materials: vec![w::CredentialEnvelope {
                subject,
                expected_revision: 0,
                wrapped_key: STANDARD.encode([7; 256]),
                nonce: [9; 12],
                ciphertext: STANDARD.encode([3; 32]),
            }],
        };
        write(&credential_root, &task, "escrow", &escrow);
        client
            .escrow_credentials(task.task_id, escrow.clone())
            .await
            .unwrap();
        crate::credential_worker::acknowledge(&credential_root, &task, &escrow, &[]).unwrap();
    }
    let wrong = AttemptId::new("wrong-attempt").unwrap();
    assert!(app
        .service_assess(request, &wrong, reference.clone(), &evidence)
        .is_err());
    let mut wrong_task = task.clone();
    wrong_task.generation += 1;
    assert!(app
        .service_assess(
            request,
            &attempt,
            reference,
            &super::credential::EffectEvidence {
                task: &wrong_task,
                attempt: &attempt
            }
        )
        .is_err());
    server.data.lock().unwrap().result_failure = true;
    assert_eq!(
        bridge.flush(&mut client, task.task_id, &mut app, 64).await,
        Err(Error::Unavailable)
    );
    assert_eq!(
        bridge.finish(&mut client, task.task_id, &app, &caller),
        Err(Error::Conflict)
    );
    let original = server.data.lock().unwrap().results.clone();
    drop(app);
    drop(client);
    let mut app = reopen_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
    let mut client = server.client(&root, OpenMode::Existing);
    bridge
        .flush(&mut client, task.task_id, &mut app, 64)
        .await
        .unwrap();
    bridge
        .flush(&mut client, task.task_id, &mut app, 64)
        .await
        .unwrap();
    assert_eq!(server.data.lock().unwrap().results, original);
    assert_eq!(runner.inner.dispatch_count(), 1);
    if case == 2 {
        assert_eq!(
            app.status(&caller, request).unwrap().assessment,
            Some(execution_lifecycle::EffectAssessment::Unknown)
        );
        assert_eq!(
            bridge.finish(&mut client, task.task_id, &app, &caller),
            Err(Error::Conflict)
        );
        assert!(
            crate::credential_worker::stored_result(&credential_root, &task)
                .unwrap()
                .is_some()
        );
        assert!(client.association(task.task_id).unwrap().is_some());
    } else {
        assert!(super::bridge::settled(
            &app.status(&caller, request).unwrap()
        ));
        bridge
            .finish(&mut client, task.task_id, &app, &caller)
            .unwrap();
        assert!(client.association(task.task_id).unwrap().is_none());
        assert!(
            crate::credential_worker::stored_result(&credential_root, &task)
                .unwrap()
                .is_none()
        );
        assert!(crate::credential_worker::pending(&credential_root, &task)
            .unwrap()
            .is_none());
    }
}
