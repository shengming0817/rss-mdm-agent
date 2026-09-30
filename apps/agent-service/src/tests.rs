//! Real HTTP/signatures/SQLite feeding the production compiler; OS execution is separately tested.
#[path = "../../../crates/agent-client/tests/support/mod.rs"]
mod protocol;
use super::*;
use agent_client::OpenMode;
use execution_admission::AuthorityVerifier;
use execution_contract::*;
use sha2::{Digest as _, Sha256};

#[tokio::test]
async fn signed_start_compiles_exactly_and_never_creates_a_local_enterprise_approval() {
    let server = protocol::Server::new().await;
    let root = protocol::Root::new();
    let clock = SystemClock::new().unwrap();
    server.time.set(clock.now().unwrap());
    let mut client = server.client(&root, OpenMode::Create);
    let receipt = server.register(&mut client).await;
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    let (binding, actor) =
        plan::context(server.url.as_str(), server.config().tenant, &receipt).unwrap();
    let interpreters = [Interpreter {
        profile: wire::ExecutorProfile::PosixSh,
        image: local_service::Artifact {
            path: "/bin/sh".into(),
            sha256: format!("{:x}", Sha256::digest(std::fs::read("/bin/sh").unwrap())),
            cdhash: None,
        },
    }];
    let compile = |payload: &wire::TaskSpec| {
        plan::script(
            &offer,
            &materials,
            payload,
            (&binding, &actor),
            &interpreters,
            (&root.path, &root.path.join("exact-source")),
            None,
        )
    };
    let wire::TaskPayload::Script(offered) = offer.payload() else {
        panic!("script")
    };
    let (preview, _) = compile(offered).unwrap();
    let host = host::EnterpriseHost {
        binding: binding.clone(),
        actor: actor.clone(),
        clock,
        materials: execution_runner::MaterialRegistry::new(4).unwrap(),
        subject: execution_runner::host::current_subject().unwrap(),
        current: Arc::new(Mutex::new(None)),
        revoked: Default::default(),
    };
    let attempt = AttemptId::new("local-attempt").unwrap();
    assert!(
        host.verify(&preview, &attempt).is_err(),
        "an Offer cannot authorize execution"
    );
    client.received(&offer).await.unwrap();
    let start = client.request_start(&offer, &materials).await.unwrap();
    let wire::TaskPayload::Script(payload) = start.payload() else {
        panic!("script")
    };
    let (frozen, _) = compile(payload).unwrap();
    assert_eq!(frozen.digest(), compile(payload).unwrap().0.digest());
    assert_eq!(frozen.spec().budget.max_attempts, 1);
    *host.current.lock().unwrap() = Some(Arc::new(host::BackendPermit {
        plan: frozen.clone(),
        start: start.clone(),
    }));
    assert!(host.verify(&frozen, &attempt).is_ok());
    assert!(execution_app::AppHost::approval_bindings(&host, &frozen)
        .unwrap()
        .is_empty());
    for change in 0..7 {
        let mut changed = payload.clone();
        match change {
            0 => changed.generation += 1,
            1 => changed.device_id = "another-device".into(),
            2 => changed.attempt_id = uuid::Uuid::new_v4(),
            3 => changed.content.sha256[0] ^= 1,
            4 => changed.arguments.push("replaced".into()),
            5 => changed.tenant_id = uuid::Uuid::new_v4(),
            _ => changed.run_as = wire::ExecutionIdentity::LoggedInUser,
        }
        assert!(compile(&changed).is_err(), "replacement {change}");
    }
    host.revoked
        .store(true, std::sync::atomic::Ordering::Release);
    assert!(host.verify(&frozen, &attempt).is_err());
    let mut next = receipt.clone();
    next.generation += 1;
    assert_ne!(
        binding.authority,
        plan::context(server.url.as_str(), server.config().tenant, &next)
            .unwrap()
            .0
            .authority
    );
    server.time.set(payload.expires_at);
    assert!(client.validate_start(&start).is_err());
}
