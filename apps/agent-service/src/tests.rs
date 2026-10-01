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

#[tokio::test]
async fn osquery_compiler_revalidates_the_signed_query_and_uses_only_literal_arguments() {
    for (query, valid) in [
        ("SELECT version FROM osquery_info", true),
        ("SELECT load_extension('x') FROM osquery_info", false),
        (
            "SELECT version FROM osquery_info; DELETE FROM programs",
            false,
        ),
    ] {
        let server = protocol::Server::new().await;
        {
            let mut data = server.data.lock().unwrap();
            data.bytes = query.as_bytes().to_vec();
            data.script();
            let wire::TaskPayload::Script(mut spec) = data.offer.as_ref().unwrap().payload.clone()
            else {
                panic!("script")
            };
            spec.profile = wire::ExecutorProfile::Osquery;
            spec.sql_parameters = Some(Default::default());
            data.offer = Some(data.signed(wire::TaskPayload::Script(spec)));
        }
        let root = protocol::Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        let receipt = server.register(&mut client).await;
        let offer = client.claim().await.unwrap().offer.unwrap();
        let materials = client.prepare(&offer).await.unwrap();
        let (binding, actor) =
            plan::context(server.url.as_str(), server.config().tenant, &receipt).unwrap();
        let wire::TaskPayload::Script(spec) = offer.payload() else {
            panic!("script")
        };
        // This test verifies the production compiler, not osquery execution.
        let executable = std::path::PathBuf::from("/bin/sh");
        let interpreters = [Interpreter {
            profile: wire::ExecutorProfile::Osquery,
            image: local_service::Artifact {
                sha256: format!("{:x}", Sha256::digest(std::fs::read(&executable).unwrap())),
                path: executable,
                cdhash: None,
            },
        }];
        let result = plan::script(
            &offer,
            &materials,
            spec,
            (&binding, &actor),
            &interpreters,
            (&root.path, &root.path.join("query")),
            None,
        );
        assert_eq!(result.is_ok(), valid, "{:?}", result.as_ref().err());
        if let Ok((frozen, _)) = result {
            let args = &frozen.spec().launch.argv;
            assert!(args.iter().any(
                |a| matches!(a,LaunchArg::Literal{value} if value==&format!("{query} LIMIT 2"))
            ));
            assert!(args.iter().any(
                |a| matches!(a,LaunchArg::Literal{value} if value=="--disable_extensions=true")
            ));
            assert!(!args.iter().any(|a| matches!(a, LaunchArg::ArtifactPath {})));
        }
    }
}

#[test]
fn production_storage_budgets_open_with_large_output_capture_limits() {
    let root = protocol::Root::new();
    let authority = Authority::Test {
        id: Id::new("collection-storage").unwrap(),
    };
    let limits = plan::storage_limits();
    assert_eq!(limits.input.max_output_bytes, 16_777_216);
    execution_sqlite::Store::initialize_test(&root.path.join("budget.sqlite"), authority, limits)
        .unwrap();
}

#[tokio::test]
#[ignore = "native System/root material staging; run in the platform T3 service environment"]
async fn current_software_steps_compile_from_exact_prefixed_artifacts() {
    for supported_codes in [true, false] {
        let server = protocol::Server::new().await;
        {
            let mut data = server.data.lock().unwrap();
            data.software(2, false);
            if !supported_codes {
                let wire::TaskPayload::Software(mut spec) =
                    data.offer.as_ref().unwrap().payload.clone()
                else {
                    panic!("software")
                };
                for step in &mut spec.steps {
                    let wire::SoftwareTaskBehavior::Pkg(n) = &mut step.action.behavior else {
                        panic!("pkg")
                    };
                    n.install.exit_codes.success = [7].into();
                    n.upgrade_invocation = n.install.clone();
                }
                spec.definition_digest =
                    Sha256::digest(serde_json::to_vec(&spec.steps).unwrap()).into();
                data.offer = Some(data.signed(wire::TaskPayload::Software(spec)));
            }
        }
        let root = protocol::Root::new();
        let mut client = server.client(&root, OpenMode::Create);
        let receipt = server.register(&mut client).await;
        let offer = client.claim().await.unwrap().offer.unwrap();
        let materials = client.prepare(&offer).await.unwrap();
        let (binding, actor) =
            plan::context(server.url.as_str(), server.config().tenant, &receipt).unwrap();
        let executable = local_service::Artifact {
            path: "/bin/sh".into(),
            sha256: format!("{:x}", Sha256::digest(std::fs::read("/bin/sh").unwrap())),
            cdhash: None,
        };
        let material_root = root.path.join("materials");
        execution_runner::staging::initialize(&material_root).unwrap();
        let config = ExecutionConfig {
            work_root: root.path.clone(),
            material_root,
            interpreters: vec![Interpreter {
                profile: wire::ExecutorProfile::PosixSh,
                image: executable.clone(),
            }],
            managers: vec![SoftwareManager {
                executor: SoftwareManagerKind::PackageInstaller,
                image: executable,
            }],
            processes: 1,
        };
        let wire::TaskPayload::Software(spec) = offer.payload() else {
            panic!("software")
        };
        // Pins and compiles actual material; it does not pretend /bin/sh is a real installer.
        let compiled = software::compile(&offer, &materials, spec, &binding, &actor, &config, None);
        if supported_codes {
            let (plan, _) = compiled.unwrap();
            assert_eq!(
                plan.spec()
                    .execution
                    .software_program()
                    .unwrap()
                    .steps
                    .len(),
                2
            );
        } else {
            assert!(matches!(compiled, Err(Error::Unsupported)));
        }
    }
}
