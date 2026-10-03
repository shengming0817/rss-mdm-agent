//! Real HTTP/signatures/SQLite feeding the production compiler; OS execution is separately tested.
use super::*;
use crate::backend::{host, plan, software};
use crate::protocol_test_support as protocol;
use agent_client::OpenMode;
use execution_admission::AuthorityVerifier;
use execution_contract::*;
use sha2::{Digest as _, Sha256};

#[test]
fn production_network_io_progresses_while_the_owner_checks_synchronous_native_facts() {
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (release, waiting) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        stream.read_exact(&mut [0u8; 4]).unwrap();
        waiting
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        stream.write_all(b"ack").unwrap();
    });
    let sent = Arc::new(AtomicBool::new(false));
    let completed = Arc::new(AtomicBool::new(false));
    let progressed = crate::service::owner_runtime().unwrap().block_on(async {
        let ready = sent.clone();
        let acknowledged = completed.clone();
        let network = tokio::spawn(async move {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream.write_all(b"body").await.unwrap();
            ready.store(true, Ordering::Release);
            stream.read_exact(&mut [0u8; 3]).await.unwrap();
            acknowledged.store(true, Ordering::Release);
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !sent.load(Ordering::Acquire) {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        release.send(()).unwrap();
        // A real NSXPC fact check is synchronous on this sole SQLite/journal owner.
        std::thread::sleep(std::time::Duration::from_millis(250));
        let progressed = completed.load(Ordering::Acquire);
        network.await.unwrap();
        progressed
    });
    server.join().unwrap();
    assert!(
        progressed,
        "network reactor was blocked by native fact reconciliation"
    );
}

#[tokio::test]
async fn signed_start_compiles_exactly_and_never_creates_a_local_enterprise_approval() {
    let server = protocol::Server::new().await;
    let root = protocol::Root::new();
    let clock = SystemClock::new().unwrap();
    server.time.set(clock.now().unwrap());
    let mut client = server.client(&root, OpenMode::Create);
    let receipt = server.register(&mut client).await;
    {
        let mut data = server.data.lock().unwrap();
        data.script();
        let wire::TaskPayload::Script(mut spec) = data.offer.as_ref().unwrap().payload.clone()
        else {
            panic!("script")
        };
        spec.arguments = vec![
            "".into(),
            "-Name:".into(),
            "true".into(),
            " a b; $(x) ".into(),
        ];
        spec.environment = [("RSS_PARAM_VALUE".into(), " literal ; $(x) ".into())].into();
        data.offer = Some(data.signed(wire::TaskPayload::Script(spec)));
    }
    let offer = client.claim().await.unwrap().offer.unwrap();
    let materials = client.prepare(&offer).await.unwrap();
    let (binding, actor) =
        plan::context(server.url.as_str(), server.config().tenant, &receipt).unwrap();
    let interpreters = [Interpreter {
        profile: wire::ExecutorProfile::PosixSh,
        image: installation_security::Artifact {
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
        subject: execution_ipc::host::current_subject().unwrap(),
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
    // Expected pre-refactor calling fields are explicit; the compiler is the production entry.
    let expected_argv: Vec<_> = Some(LaunchArg::ArtifactPath {})
        .into_iter()
        .chain(payload.arguments.iter().map(|value| LaunchArg::Literal {
            value: value.clone(),
        }))
        .collect();
    assert_eq!(frozen.spec().launch.argv, expected_argv);
    assert_eq!(
        frozen.spec().launch.interpreter.profile,
        plan::reference("native-posix-sh-file", "1").unwrap()
    );
    assert_eq!(
        frozen.spec().launch.interpreter.artifact.resource,
        plan::reference("native-posix-sh-file", &interpreters[0].image.sha256).unwrap()
    );
    assert_eq!(
        frozen.spec().launch.env[&EnvironmentKey::new("RSS_PARAM_VALUE").unwrap()],
        InputValue::Literal {
            value: serde_json::json!(" literal ; $(x) ")
        }
    );
    assert_eq!(frozen.digest(), compile(payload).unwrap().0.digest());
    assert_eq!(frozen.spec().budget.max_attempts, 1);
    *host.current.lock().unwrap() = Some(Arc::new(host::BackendPermit {
        plan: frozen.clone(),
        start: start.clone(),
        gate: crate::backend::gate::ProductGateProof::verify_start(
            &offer,
            &start,
            &frozen,
            None,
            None,
            host.clock.millis().unwrap(),
        )
        .unwrap(),
    }));
    assert!(host.verify(&frozen, &attempt).is_ok());
    assert!(execution_app::AppHost::approval_bindings(&host, &frozen)
        .unwrap()
        .is_empty());
    // Journal reconciliation survives a later dispatch or process restart without granting
    // a new attempt. The short-lived backend Start remains mandatory for admission.
    let original_grant = host.current.lock().unwrap().take().unwrap();
    let snapshot = execution_app::AppHost::trusted_snapshot(&host, &frozen).unwrap();
    assert_eq!(
        snapshot.fresh_until_unix_ms,
        frozen.spec().validity.expires_at_unix_ms
    );
    assert!(snapshot.approvals.is_empty());
    assert!(host.verify(&frozen, &attempt).is_err());
    let mut foreign = frozen.spec().clone();
    foreign.request.actor = ActorId::new("foreign-registration").unwrap();
    let foreign = FrozenExecution::freeze(foreign, &plan::storage_limits().input).unwrap();
    assert!(execution_app::AppHost::trusted_snapshot(&host, &foreign).is_err());
    *host.current.lock().unwrap() = Some(original_grant);
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
            image: installation_security::Artifact {
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

#[cfg(unix)]
#[tokio::test]
async fn production_service_creates_and_reopens_journal_with_its_actual_budgets() {
    let server = protocol::Server::new().await;
    let root = protocol::Root::new();
    let clock = SystemClock::new().unwrap();
    server.time.set(clock.now().unwrap());
    let mut client = server.client(&root, OpenMode::Create);
    server.register(&mut client).await;
    drop(client);
    let limits = plan::storage_limits();
    assert_eq!(limits.input.max_output_bytes, 16_777_216);
    let image = installation_security::Artifact {
        path: "/bin/sh".into(),
        sha256: format!("{:x}", Sha256::digest(std::fs::read("/bin/sh").unwrap())),
        cdhash: None,
    };
    let config = ExecutionConfig {
        work_root: root.path.clone(),
        material_root: root.path.join("materials"),
        interpreters: vec![Interpreter {
            profile: wire::ExecutorProfile::PosixSh,
            image: image.clone(),
        }],
        managers: vec![],
        processes: 1,
    };
    let journal = root.path.join("execution.sqlite");
    // Exercise the production owner, NativeRunner and Enterprise authority. The loopback
    // backend supplies registration only; this test neither dispatches nor proves root IPC.
    for startup in [
        crate::ProductionStartup::Create,
        crate::ProductionStartup::Open,
    ] {
        let client = agent_client::Client::open(
            &root.path,
            server.config(),
            OpenMode::Existing,
            server.secrets.clone(),
            clock.clone(),
        )
        .unwrap();
        drop(
            DeviceService::open(
                client,
                &journal,
                startup,
                config.clone(),
                clock.clone(),
                UserResources {
                    image: image.clone(),
                    work_roots: Default::default(),
                },
            )
            .unwrap(),
        );
    }
    assert!(journal.is_file());
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
        let executable = installation_security::Artifact {
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
