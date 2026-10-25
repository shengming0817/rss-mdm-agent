//! Real protected state, communication SQLite, local journal and TLS. Native OS reads are
//! explicit fixtures: this proves settlement wiring, not signed-extension or actual-login T3.
use super::*;
use crate::{
    backend::{
        bridge::ExecutionBridge,
        test_support::{
            local::{create_app, reopen_app},
            *,
        },
    },
    protocol_test_support::*,
};
use execution_app::*;
use execution_contract::*;
use rss_mdm_agent_wire::runtime as r;
use sha2::{Digest as _, Sha256};
use std::sync::{Arc, Mutex};

fn readback(c: &Coordinate) -> ::platform_sso::NativeConfiguration {
    let endpoints = w::Endpoints::new(&c.context.origin, c.context.tenant).unwrap();
    ::platform_sso::NativeConfiguration {
        authentication: 1,
        client_id: c.client_id.to_string(),
        issuer: endpoints.issuer.clone(),
        audience: endpoints.issuer,
        token_endpoint: endpoints.token,
        jwks_endpoint: endpoints.jwks,
        nonce_endpoint: endpoints.nonce,
        nonce_response_keypath: "nonce".into(),
        server_nonce_claim: "request_nonce".into(),
        additional_scopes: w::SCOPE.into(),
        login_request_values: vec![("platform_sso_version".into(), "1.0".into())],
        login_user_name: Some(c.source.to_string()),
    }
}
fn observation(c: &Coordinate, actual: ::platform_sso::NativeConfiguration) -> Operation {
    Operation::Observe {
        scope: w::Kind::User,
        callback_generation: c.callback_generation,
        configuration: Some(actual),
        source: c.source,
        source_version: c.source_version,
        device_registered: true,
        user_registered: true,
    }
}
#[tokio::test]
async fn native_observation_refresh_settles_original_journal_after_restart_and_ack_loss() {
    let _budget = crate::scenario_budget::enter();
    let server = Server::tls().await;
    let account = Uuid::new_v4();
    let installation = Uuid::new_v4();
    let binding_id = Uuid::new_v4();
    let login = w::LoginConfiguration {
        authentication: w::AuthenticationMode::Password,
        password_provider: Some(Uuid::new_v4()),
        filevault: w::LoginPolicy::Local,
        ..w::LoginConfiguration::default()
    };
    {
        let mut d = server.data.lock().unwrap();
        let task = r::AccessTaskSpec {
            wire_version: 1,
            tenant_id: d.tenant,
            device_id: "device-1".into(),
            registration_id: d.registration,
            generation: 1,
            platform: r::TaskPlatform::Macos,
            architecture: r::TaskArchitecture::Aarch64,
            task_id: d.task,
            attempt_id: d.attempt,
            permit: r::TaskPermit::Offer,
            expires_at: 60,
            deadline: 300,
            binding_id,
            instance_id: Uuid::new_v4(),
            principal_id: Uuid::new_v4(),
            binding_generation: 1,
            factor_generation: 1,
            revision: 1,
            definition_digest: [7; 32],
            action: r::AccessAction::Apply {
                account: r::AccessAccount::Macos {
                    generated_uid: account,
                },
                configuration: r::AccessConfiguration {
                    mac: Some(login.clone()),
                    windows: r::AccessWindowsConfiguration::Disabled,
                    self_enrollment: true,
                    enabled: true,
                    role: Some(r::AccessRole::Standard),
                },
            },
            windows: None,
            authorization: r::TaskAuthorization::Policy {
                policy_id: Uuid::new_v4(),
                policy_version: Uuid::new_v4(),
            },
        };
        d.offer = Some(d.signed(r::TaskPayload::DeviceAccess(Box::new(task))));
    }
    let root = Root::new();
    let mut client = server.client(&root, agent_client::OpenMode::Create);
    server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    let r::TaskPayload::DeviceAccess(task) = start.payload() else {
        panic!()
    };
    let task = (**task).clone();
    let access_root = root.path.join("access");
    platform_private_storage::directory(&access_root).unwrap();
    let local_binding = ::platform_sso::LocalBinding {
        installation,
        tenant: task.tenant_id,
        agent_registration: task.registration_id,
        registration_generation: 1,
        binding: binding_id,
        binding_generation: 1,
        revision: 1,
        generated_uid: account.to_string(),
        enabled: true,
        configuration: login,
    };
    let coordinate = Coordinate {
        client_id: Uuid::new_v4(),
        authentication: w::AuthenticationMode::Password,
        callback_generation: 7,
        source: Uuid::new_v4(),
        source_version: 1,
        context: ::platform_sso::Context {
            installation,
            tenant: task.tenant_id,
            agent_registration: task.registration_id,
            origin: "https://mdm.example".into(),
            kind: w::Kind::User,
            binding: Some(binding_id),
            binding_generation: Some(1),
            binding_revision: 1,
            generated_uid: Some(account.to_string()),
            configuration: Some(local_binding.configuration.clone()),
        },
    };
    // Native prepare and registered-source state are legal preconditions. The observed facts
    // below go through the exact production acceptance function, not an Applied fixture write.
    state::write(
        &access_root,
        &binding_name(&account.to_string()).unwrap(),
        &local_binding,
    )
    .unwrap();
    state::write(
        &access_root,
        &coordinate.file("source").unwrap(),
        &coordinate,
    )
    .unwrap();
    state::write(
        &access_root,
        &crate::access_worker::name(&task, "prepared"),
        &r::AccessPrepared {
            binding_id,
            binding_generation: 1,
            revision: 1,
            definition_digest: task.definition_digest,
            account: r::AccessAccount::Macos {
                generated_uid: account,
            },
        },
    )
    .unwrap();
    state::write(
        &access_root,
        &crate::access_worker::name(&task, "result"),
        &r::AccessResult {
            binding_id,
            binding_generation: 1,
            revision: 1,
            definition_digest: task.definition_digest,
            effect: r::AccessEffect::Unknown,
        },
    )
    .unwrap();
    let session = Id::new("fixture-boot/1").unwrap();
    let mut actual = readback(&coordinate);
    actual.token_endpoint = "https://wrong.example/token".into();
    record_observation(
        &access_root,
        &coordinate.context,
        &observation(&coordinate, actual),
        "501",
        1,
        Some(&session),
    )
    .unwrap();
    let facts = |role| {
        let native = native_facts_in_session(&access_root, &local_binding, |_| true).unwrap();
        (
            role,
            r::AccessMacFacts {
                platform_sso: native,
                secure_token: r::AccessObservation::Present,
                volume_owner: r::AccessObservation::Unknown,
                filevault_unlock: r::AccessObservation::Unknown,
            },
        )
    };
    crate::access_worker::macos::refresh_with(&access_root, installation, &task, 1, |_| {
        Ok(facts(r::AccessRole::Standard))
    })
    .unwrap();
    assert!(!crate::access_worker::complete(
        &task,
        &crate::access_worker::stored_result(&access_root, &task)
            .unwrap()
            .unwrap()
            .effect
    ));
    let mut stale = coordinate.clone();
    stale.callback_generation += 1;
    assert!(record_observation(
        &access_root,
        &coordinate.context,
        &observation(&stale, readback(&coordinate)),
        "501",
        1,
        Some(&session)
    )
    .is_err());
    stale = coordinate.clone();
    stale.source_version += 1;
    assert!(record_observation(
        &access_root,
        &coordinate.context,
        &observation(&stale, readback(&coordinate)),
        "501",
        1,
        Some(&session)
    )
    .is_err());
    record_observation(
        &access_root,
        &coordinate.context,
        &observation(&coordinate, readback(&coordinate)),
        "501",
        1,
        Some(&session),
    )
    .unwrap();
    assert!(
        native_facts_in_session(&access_root, &local_binding, |_| false)
            .unwrap()
            .is_none(),
        "changed native session invalidates old readback"
    );
    let mut changed = local_binding.clone();
    changed.binding_generation += 1;
    assert!(native_facts_in_session(&access_root, &changed, |_| true)
        .unwrap()
        .is_none());
    crate::access_worker::macos::refresh_with(&access_root, installation, &task, 1, |_| {
        Ok(facts(r::AccessRole::Administrator))
    })
    .unwrap();
    assert!(
        !crate::access_worker::complete(
            &task,
            &crate::access_worker::stored_result(&access_root, &task)
                .unwrap()
                .unwrap()
                .effect
        ),
        "wrong role cannot complete"
    );

    let mut spec = adapted_plan(false, "device-1", &offer).spec().clone();
    spec.constraints = IsolationPolicy::OsIdentity {};
    spec.launch.interpreter.profile =
        crate::backend::plan::reference(crate::access_worker::PROFILE, "1").unwrap();
    spec.request.operation.resource = crate::backend::plan::reference(
        "backend-resource",
        crate::backend::plan::hex(&task.definition_digest),
    )
    .unwrap();
    spec.validity.expires_at_unix_ms = task.expires_at as u64 * 1000 + spec.budget.total_timeout_ms;
    spec.request.parameters.insert(
        "access-root".into(),
        InputValue::Literal {
            value: serde_json::to_value(&access_root).unwrap(),
        },
    );
    spec.request.parameters.insert(
        "access-installation".into(),
        InputValue::Literal {
            value: serde_json::json!(installation),
        },
    );
    let worker = crate::access_worker::Input {
        task: task.clone(),
        root: access_root.clone(),
        installation,
        image_sha256: spec.launch.interpreter.artifact.sha256.clone(),
        run_as: spec.run_as.clone(),
        session: spec.session_requirement.clone(),
    };
    spec.launch.artifact.sha256 = Digest::new(crate::backend::plan::hex(&Sha256::digest(
        serde_json::to_vec(&worker).unwrap(),
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
            stdout: vec![],
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
    app.reconcile(&plan.spec().request.request_id).unwrap();
    bridge
        .flush(&mut client, task.task_id, &mut app, 64)
        .await
        .unwrap();
    assert_eq!(
        bridge.finish(&mut client, task.task_id, &app, &caller),
        Err(agent_client::Error::Conflict)
    );
    crate::access_worker::macos::refresh_with(&access_root, installation, &task, 1, |_| {
        Ok(facts(r::AccessRole::Standard))
    })
    .unwrap();
    server.data.lock().unwrap().result_failure = true;
    assert!(bridge
        .flush(&mut client, task.task_id, &mut app, 64)
        .await
        .is_err());
    let sent = server.data.lock().unwrap().results.clone();
    drop(app);
    drop(client);
    let mut app = reopen_app(&db.path, host, runner.clone(), AppConfig::test_defaults(1)).unwrap();
    let mut client = server.client(&root, agent_client::OpenMode::Existing);
    bridge
        .flush(&mut client, task.task_id, &mut app, 64)
        .await
        .unwrap();
    bridge
        .flush(&mut client, task.task_id, &mut app, 64)
        .await
        .unwrap();
    assert_eq!(server.data.lock().unwrap().results, sent);
    assert_eq!(runner.inner.dispatch_count(), 1);
    bridge
        .finish(&mut client, task.task_id, &app, &caller)
        .unwrap();
    assert!(client.association(task.task_id).unwrap().is_none());
    queue_retirement(&access_root, coordinate).unwrap();
    assert!(
        native_facts_in_session(&access_root, &local_binding, |_| true)
            .unwrap()
            .is_none(),
        "retired/cancelled registration cannot reuse the previous observation"
    );
}
