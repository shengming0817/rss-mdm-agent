//! Ordered software uses the original SQLite intent; unknown process activity never advances.
use super::*;
#[test]
fn unknown_detector_activity_keeps_real_exit_and_cannot_skip_to_mutation() {
    let script = "printf '{\"kind\":\"absent\"}'\n";
    let f = fixture(script, vec![LaunchArg::ArtifactPath {}], 4096, 5000);
    let input = f.plan.spec();
    let invocation = SoftwareInvocation {
        launch: input.launch.clone(),
        run_as: input.run_as.clone(),
        session_requirement: input.session_requirement.clone(),
        timeout_ms: 5000,
        output_bytes: 4096,
        exit_codes: SoftwareExitCodes {
            success: [0].into_iter().collect(),
            reboot: Default::default(),
        },
    };
    let mut p = input.clone();
    p.request.parameters.clear();
    p.request.initiator = Initiator::Policy {
        policy: p.policy.clone(),
    };
    p.request.operation.action = Id::new("software.install").unwrap();
    p.launch.interpreter.profile.id = Id::new("native-software-sequence").unwrap();
    p.execution = ExecutionSpec::SoftwareProgram {
        program: Box::new(SoftwareProgram {
            definition_digest: Digest::new("ab".repeat(32)).unwrap(),
            intent: SoftwareOperation::Install,
            steps: vec![SoftwareProgramStep {
                format: SoftwareFormat::Pkg {},
                package: PackageValue::new("fixture").unwrap(),
                version: PackageValue::new("1").unwrap(),
                architecture: PackageValue::new("aarch64").unwrap(),
                payload: invocation.launch.artifact.clone(),
                materials: Vec::new(),
                signatures: Vec::new(),
                upgrade: SoftwareUpgrade::Deny {},
                install: invocation.clone(),
                auxiliary: Default::default(),
                uninstall: None,
                detection: SoftwareDetector::Script {
                    invocation: Box::new(invocation),
                },

                allow_downgrade: false,
                allow_reboot: false,
            }],
        }),
    };
    let limits = execution_sqlite::test_store_limits();
    let plan = FrozenExecution::freeze(p, &limits.input).unwrap();
    let source = || Artifacts {
        program: vec![],
        delegate: None,
        interpreter: "/bin/sh".into(),
        content: f.root.join("source"),
        work_root: f.root.clone(),
        controlled_input: None,
        fixture_owned: true,
    };
    let mut materials = source();
    materials.program.push(crate::SoftwareStepArtifacts {
        mutations: [(SoftwarePhase::Mutation, Box::new(source()))].into(),
        detection: Some(Box::new(source())),
        files: vec![],
    });
    let runner = NativeRunner::new(
        Id::new("mechanism-runner").unwrap(),
        BTreeMap::from([(plan.digest().as_str().into(), materials)]),
        8,
    )
    .unwrap();
    let mut host = app_support::TestHost::new();
    host.template = plan.clone();
    let carrier = TestCarrier(
        Arc::new(runner),
        Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    );
    let caller = execution_app::RequestContext {
        actor: plan.spec().request.actor.clone(),
    };
    let request = plan.spec().request.request_id.clone();
    let mut app = create_app(
        &f.root.join("software.sqlite"),
        host,
        carrier.clone(),
        execution_app::AppConfig::test_defaults(1),
    )
    .unwrap();
    let status = app.request_execution(&caller, &plan).unwrap();
    let attempt = status.attempt_id.unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        app.reconcile(&request).unwrap();
        if carrier
            .0
            .evidence(&plan, &attempt)
            .unwrap()
            .is_some_and(|v| v.finished)
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let delivery = app
        .service_delivery(&request, &Id::new("fixture-delivery").unwrap(), 64)
        .unwrap();
    let progress = delivery
        .iter()
        .find_map(|e| e.software_progress.as_ref())
        .unwrap();
    assert!(progress.checkpoints.iter().all(|c| !matches!(
        c,
        SoftwareCheckpoint::Begin {
            phase: SoftwarePhase::Mutation,
            ..
        }
    )));
    assert!(progress
        .checkpoints
        .iter()
        .any(|c| matches!(c, SoftwareCheckpoint::End {
 process: Some(p), quiescent: false, .. } if p.exit_code == Some(0))));
    assert!(!progress.complete(&plan));
    assert_eq!(carrier.1.load(Ordering::SeqCst), 1);
    let sqlite = rusqlite::Connection::open(f.root.join("software.sqlite")).unwrap();
    assert_eq!(
        sqlite
            .query_row("SELECT count(*) FROM software_claims", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn current_observation_drives_real_runner_without_historical_installation_rows() {
    // A controlled physical worker supplies bounded observations; the production
    // runner, phase acknowledgements, claim owner and SQLite journal are real.
    for (intent, before, target, expected, expected_phase) in [
        (
            SoftwareOperation::Install,
            r#"{"kind":"present","version":"1.0"}"#,
            "1.0",
            true,
            None,
        ),
        (
            SoftwareOperation::Uninstall,
            r#"{"kind":"absent"}"#,
            "1.0",
            true,
            None,
        ),
        (
            SoftwareOperation::Install,
            r#"{"kind":"present","version":"0.9"}"#,
            "1.0",
            true,
            Some(SoftwarePhase::Upgrade),
        ),
        (
            SoftwareOperation::Uninstall,
            r#"{"kind":"present","version":"1.1"}"#,
            "1.1",
            true,
            Some(SoftwarePhase::Mutation),
        ),
        (
            SoftwareOperation::Install,
            r#"{"kind":"unknown","reason":"unavailable"}"#,
            "1.0",
            false,
            None,
        ),
    ] {
        let after = if intent == SoftwareOperation::Uninstall {
            r#"{"kind":"absent"}"#.to_owned()
        } else {
            format!(r#"{{"kind":"present","version":"{target}"}}"#)
        };
        let script = format!(
            r#"case "$1" in
observe) if [ -f changed ]; then state='{after}'; else state='{before}'; fi ;;
*) printf '%s\n' "$1" >> calls; : > changed; state=null ;;
esac
printf '{{"nativeOutputBytes":0,"detected":%s,"closed":true,"diagnostics":""}}' "$state"
"#
        );
        let f = fixture(&script, vec![LaunchArg::ArtifactPath {}], 32768, 10000);
        let mut p = f.plan.spec().clone();
        p.request.parameters.clear();
        p.request.initiator = Initiator::Policy {
            policy: p.policy.clone(),
        };
        p.request.operation.action = Id::new(if intent == SoftwareOperation::Uninstall {
            "software.uninstall"
        } else {
            "software.install"
        })
        .unwrap();
        let command = |label: &str| {
            let mut launch = p.launch.clone();
            launch.interpreter.profile.id = Id::new("native-software-worker").unwrap();
            launch.argv = vec![
                LaunchArg::ArtifactPath {},
                LaunchArg::Literal {
                    value: label.into(),
                },
            ];
            SoftwareInvocation {
                launch,
                run_as: p.run_as.clone(),
                session_requirement: p.session_requirement.clone(),
                timeout_ms: 2000,
                output_bytes: 4096,
                exit_codes: SoftwareExitCodes {
                    success: [0].into(),
                    reboot: Default::default(),
                },
            }
        };
        let install = command("install");
        let uninstall = command("uninstall");
        let upgrade = command("upgrade");
        let detect = command("observe");
        p.launch.interpreter.profile.id = Id::new("native-software-sequence").unwrap();
        p.execution = ExecutionSpec::SoftwareProgram {
            program: Box::new(SoftwareProgram {
                definition_digest: Digest::new("ab".repeat(32)).unwrap(),
                intent,
                steps: vec![SoftwareProgramStep {
                    format: SoftwareFormat::Pkg {},
                    package: PackageValue::new("independent-current-object").unwrap(),
                    version: PackageValue::new(target).unwrap(),
                    architecture: PackageValue::new("aarch64").unwrap(),
                    payload: install.launch.artifact.clone(),
                    materials: vec![],
                    signatures: vec![],
                    upgrade: SoftwareUpgrade::InPlace {
                        invocation: Box::new(upgrade),
                    },
                    install,
                    uninstall: Some(uninstall),
                    auxiliary: Default::default(),
                    detection: SoftwareDetector::Script {
                        invocation: Box::new(detect),
                    },
                    allow_downgrade: false,
                    allow_reboot: false,
                }],
            }),
        };
        let plan =
            FrozenExecution::freeze(p, &execution_sqlite::test_store_limits().input).unwrap();
        let source = || Artifacts {
            program: vec![],
            delegate: None,
            interpreter: "/bin/sh".into(),
            content: f.root.join("source"),
            work_root: f.root.clone(),
            controlled_input: None,
            fixture_owned: true,
        };
        let mut materials = source();
        materials.program.push(crate::SoftwareStepArtifacts {
            mutations: [
                (SoftwarePhase::Mutation, Box::new(source())),
                (SoftwarePhase::Upgrade, Box::new(source())),
            ]
            .into(),
            detection: Some(Box::new(source())),
            files: vec![],
        });
        let runner = NativeRunner::new(
            Id::new("mechanism-runner").unwrap(),
            [(plan.digest().as_str().into(), materials)].into(),
            8,
        )
        .unwrap();
        let mut host = app_support::TestHost::new();
        host.template = plan.clone();
        let carrier = TestCarrier(
            Arc::new(runner),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        );
        let caller = execution_app::RequestContext {
            actor: plan.spec().request.actor.clone(),
        };
        let request = &plan.spec().request.request_id;
        let database = f.root.join("software.sqlite");
        let mut app = create_app(
            &database,
            host,
            carrier.clone(),
            execution_app::AppConfig::test_defaults(1),
        )
        .unwrap();
        let attempt = app
            .request_execution(&caller, &plan)
            .unwrap()
            .attempt_id
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            app.reconcile(request).unwrap();
            if carrier
                .0
                .evidence(&plan, &attempt)
                .unwrap()
                .is_some_and(|v| v.finished)
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        let delivery = app
            .service_delivery(request, &Id::new("fixture-delivery").unwrap(), 64)
            .unwrap();
        let progress = delivery
            .iter()
            .find_map(|e| e.software_progress.as_ref())
            .unwrap();
        assert_eq!(progress.complete(&plan), expected);
        let phases: Vec<_> = progress
            .checkpoints
            .iter()
            .filter_map(|c| match c {
                SoftwareCheckpoint::Begin { phase, .. } => Some(*phase),
                _ => None,
            })
            .collect();
        let expected_phases = if let Some(phase) = expected_phase {
            vec![SoftwarePhase::Before, phase, SoftwarePhase::After]
        } else {
            vec![SoftwarePhase::Before]
        };
        assert_eq!(phases, expected_phases);
        if let Some(phase) = expected_phase {
            assert_eq!(
                std::fs::read_to_string(f.root.join("calls")).unwrap(),
                if phase == SoftwarePhase::Upgrade {
                    "upgrade\n"
                } else {
                    "uninstall\n"
                }
            );
        } else {
            assert!(!f.root.join("calls").exists());
        }
        let sqlite = rusqlite::Connection::open(database).unwrap();
        assert_eq!(
            sqlite
                .query_row("SELECT count(*) FROM software_claims", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            if expected { 0 } else { 1 }
        );
    }
}
