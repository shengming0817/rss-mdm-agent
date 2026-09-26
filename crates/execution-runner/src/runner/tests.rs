//! Real local OS mechanics; no production AppHost or test authority is promoted.
#![cfg(target_os = "macos")]
use super::*;
use sha2::{Digest as _, Sha256};
use std::{os::unix::fs::PermissionsExt, path::PathBuf, sync::atomic::AtomicU64};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    plan: FrozenPlan,
    runner: NativeRunner,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn fixture(script: &str, argv: Vec<LaunchArg>, budget: u64, timeout: u64) -> Fixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../.cache/runner-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let content = root.join("source");
    std::fs::write(&content, script).unwrap();
    std::fs::set_permissions(&content, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../execution-contract/tests/fixtures/plan.json"
    ))
    .unwrap();
    value["request"]["target"] = serde_json::json!({"device":"mechanism-device","platform":"macos","scope":{"kind":"device"}});
    value["runAs"] = serde_json::json!({"kind":"user","account":{"platform":"macos","subject":unsafe{libc::geteuid()}.to_string()}});
    value["sessionRequirement"] = serde_json::json!({"kind":"notRequired"});
    value["constraints"] = serde_json::json!({"kind":"osIdentity"});
    value["launch"]["artifact"]["sha256"] =
        format!("{:x}", Sha256::digest(script.as_bytes())).into();
    value["launch"]["interpreter"]["artifact"]["sha256"] =
        format!("{:x}", Sha256::digest(std::fs::read("/bin/sh").unwrap())).into();
    value["launch"]["interpreter"]["profile"] =
        serde_json::json!({"id":"native-posix-sh-file","revision":"1"});
    value["launch"]["argv"] = serde_json::to_value(argv).unwrap();
    value["launch"]["stdin"] = serde_json::json!({"kind":"closed"});
    value["launch"]["env"] = serde_json::json!({});
    value["launch"]["cwd"] = root.to_str().unwrap().into();
    value["budget"] =
        serde_json::json!({"totalTimeoutMs":timeout,"totalOutputBytes":budget,"maxAttempts":1});
    let time = now().unwrap();
    value["validity"] = serde_json::json!({"notBeforeUnixMs":time-1,"expiresAtUnixMs":time+60000});
    let limits = PlanLimits {
        max_input_bytes: 65536,
        max_depth: 32,
        max_nodes: 4096,
        max_string_bytes: 4096,
        max_collection_items: 128,
        max_timeout_ms: 60000,
        max_output_bytes: 1048576,
        max_stdin_bytes: 65536,
        max_attempts: 3,
    };
    let plan = FrozenPlan::freeze(
        decode_plan(&serde_json::to_vec(&value).unwrap(), &limits).unwrap(),
        &limits,
    )
    .unwrap();
    let artifacts = Artifacts {
        interpreter: "/bin/sh".into(),
        content,
        work_root: root.clone(),
        controlled_input: None,
        fixture_owned: true,
    };
    let runner = NativeRunner::new(
        Id::new("mechanism-runner").unwrap(),
        BTreeMap::from([(plan.digest().as_str().into(), artifacts)]),
        8,
    )
    .unwrap();
    Fixture { root, plan, runner }
}
fn start(f: &Fixture, cap: u64, time: u64) -> AttemptId {
    let id = AttemptId::new("attempt-1").unwrap();
    assert_eq!(
        f.runner
            .launch(
                &f.plan,
                &id,
                DispatchAllowance {
                    deadline_unix_ms: now().unwrap() + time,
                    remaining_timeout_ms: time,
                    remaining_output_bytes: cap
                }
            )
            .unwrap(),
        DispatchOutcome::Accepted
    );
    id
}
fn finish(f: &Fixture, id: &AttemptId) -> ProcessEvidence {
    let until = Instant::now() + Duration::from_secs(6);
    loop {
        if let Some(facts) = f.runner.evidence(&f.plan, id).unwrap() {
            if facts.finished {
                return facts;
            }
        }
        assert!(Instant::now() < until, "owner did not finish");
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn argv_is_literal_and_exit_is_not_effect_or_tree_proof() {
    let f = fixture(
        "printf '%s' \"$1\"; printf err >&2\n",
        vec![
            LaunchArg::ArtifactPath {},
            LaunchArg::Literal {
                value: "$(touch forbidden); spaced".into(),
            },
        ],
        4096,
        2000,
    );
    let id = start(&f, 4096, 2000);
    let facts = finish(&f, &id);
    assert_eq!(facts.stdout, b"$(touch forbidden); spaced");
    assert_eq!(facts.stderr, b"err");
    assert_eq!(facts.exit_code, Some(0));
    assert!(!facts.quiescent);
    assert_eq!(facts.quality, OutputQuality::Complete);
    assert!(!f.root.join("forbidden").exists());
    assert!(f
        .runner
        .observe(&f.plan, &id, ObservationStage::Termination, now().unwrap())
        .unwrap()
        .is_none());
    assert!(f
        .runner
        .launch(
            &f.plan,
            &id,
            DispatchAllowance {
                deadline_unix_ms: now().unwrap() + 1000,
                remaining_timeout_ms: 1000,
                remaining_output_bytes: 4096
            }
        )
        .is_err());
}
#[test]
fn output_and_time_are_bounded_and_restart_never_synthesizes_history() {
    let f = fixture(
        "while :; do printf 'xxxxxxxxxxxxxxxx'; printf 'yyyyyyyyyyyyyyyy' >&2; done\n",
        vec![LaunchArg::ArtifactPath {}],
        128,
        2000,
    );
    let id = start(&f, 64, 2000);
    let facts = finish(&f, &id);
    assert!(facts.stdout.len() + facts.stderr.len() <= 64);
    assert!(facts.total_output_bytes >= 64);
    assert_eq!(facts.quality, OutputQuality::Truncated);
    let empty =
        NativeRunner::new(Id::new("mechanism-runner").unwrap(), BTreeMap::new(), 8).unwrap();
    assert!(empty.evidence(&f.plan, &id).unwrap().is_none());
    let f = fixture(
        "while :; do :; done\n",
        vec![LaunchArg::ArtifactPath {}],
        128,
        2000,
    );
    let id = start(&f, 128, 50);
    assert_eq!(finish(&f, &id).end, ProcessEnd::TimedOut);
}
#[test]
fn cancel_and_materialized_content_mismatch_fail_closed() {
    let f = fixture(
        "while :; do :; done\n",
        vec![LaunchArg::ArtifactPath {}],
        128,
        3000,
    );
    let id = start(&f, 128, 3000);
    let until = Instant::now() + Duration::from_secs(2);
    while !f
        .runner
        .evidence(&f.plan, &id)
        .unwrap()
        .is_some_and(|facts| {
            matches!(facts.scope, ProcessScope::ProcessGroup { .. }) && !facts.finished
        })
    {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(5));
    }
    f.runner.stop(&f.plan, &id).unwrap();
    let cancelled = finish(&f, &id);
    assert_eq!(cancelled.end, ProcessEnd::Cancelled);
    assert_ne!(cancelled.quality, OutputQuality::Complete);
    let ProcessScope::ProcessGroup { group, .. } = cancelled.scope else {
        panic!("must cancel an actually running group")
    };
    let cleanup = Instant::now() + Duration::from_secs(1);
    while unsafe { libc::kill(-(group as i32), 0) } == 0 {
        assert!(
            Instant::now() < cleanup,
            "cooperative test group was not reclaimed"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !cancelled.quiescent,
        "empty group does not prove escaped descendants stopped"
    );
    let f = fixture("printf ok\n", vec![LaunchArg::ArtifactPath {}], 128, 1000);
    std::fs::write(f.root.join("source"), "printf replaced").unwrap();
    let id = start(&f, 128, 1000);
    let facts = finish(&f, &id);
    assert_eq!(facts.scope, ProcessScope::NotStarted {});
    assert_eq!(facts.end, ProcessEnd::Rejected);
    assert!(matches!(
        f.runner
            .observe(&f.plan, &id, ObservationStage::Termination, now().unwrap())
            .unwrap()
            .unwrap()
            .observation,
        Observation::NeverDispatched { .. }
    ));
}
#[test]
fn strict_output_cannot_promote_invalid_or_over_row_json() {
    let spec = OutputSpec {
        stdout: TextEncoding::Utf8,
        stderr: TextEncoding::Utf8,
        format: OutputFormat::Json { max_rows: 1 },
    };
    for bad in [b"[]".as_slice(), b"[{},{}]", b"null", b"\xff", b"{broken"] {
        assert_eq!(output::quality(bad, b"", spec), OutputQuality::Failed)
    }
    assert_eq!(
        output::quality(b"[{\"version\":\"1\"}]", b"", spec),
        OutputQuality::Complete
    );
}

#[test]
#[ignore = "subprocess-only owner-death fixture"]
fn owner_death_child() {
    let f = fixture(
        "while :; do :; done\n",
        vec![LaunchArg::ArtifactPath {}],
        128,
        30000,
    );
    let id = start(&f, 128, 30000);
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(facts) = f
            .runner
            .evidence(&f.plan, &id)
            .unwrap()
            .filter(|f| matches!(f.scope, ProcessScope::ProcessGroup { .. }))
        {
            std::fs::write(
                std::env::var_os("RSS_MECHANISM_MARKER").unwrap(),
                serde_json::to_vec(&(facts.scope, &f.root)).unwrap(),
            )
            .unwrap();
            // Simulate an uncatchable host death: no Rust owner destructor may perform cleanup.
            unsafe { libc::_exit(0) }
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn host_death_closes_owner_pipe_and_reaps_cooperative_group() {
    let marker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../.cache/owner-death-{}", std::process::id()));
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "runner::tests::owner_death_child", "--ignored"])
        .env("RSS_MECHANISM_MARKER", &marker)
        .status()
        .unwrap();
    assert!(result.success());
    let (scope, root): (ProcessScope, PathBuf) =
        serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    let ProcessScope::ProcessGroup { group, .. } = scope else {
        panic!("group")
    };
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if unsafe { libc::kill(-(group as i32), 0) } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            break;
        }
        assert!(
            Instant::now() < until,
            "cooperative group remained after owner death"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::remove_file(marker).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn acknowledged_final_capture_releases_slots_without_evicting_live_owners() {
    let mut f = fixture("printf ok\n", vec![LaunchArg::ArtifactPath {}], 128, 1000);
    f.runner.capacity = 1;
    for n in 0..4 {
        let id = AttemptId::new(format!("serial-{n}")).unwrap();
        f.runner
            .launch(
                &f.plan,
                &id,
                DispatchAllowance {
                    deadline_unix_ms: now().unwrap() + 1000,
                    remaining_timeout_ms: 1000,
                    remaining_output_bytes: 128,
                },
            )
            .unwrap();
        assert!(matches!(
            f.runner.launch(
                &f.plan,
                &AttemptId::new("extra").unwrap(),
                DispatchAllowance {
                    deadline_unix_ms: now().unwrap() + 1000,
                    remaining_timeout_ms: 1000,
                    remaining_output_bytes: 128
                }
            ),
            Err(Error::Capacity)
        ));
        let facts = finish(&f, &id);
        f.runner.acknowledge_capture(&f.plan, &facts).unwrap();
        assert!(f.runner.evidence(&f.plan, &id).unwrap().is_none());
    }
}

#[test]
fn opened_script_and_cwd_survive_path_replacement() {
    let f = fixture(
        "printf '%s' \"$1\"\n",
        vec![
            LaunchArg::ArtifactPath {},
            LaunchArg::Literal {
                value: "original".into(),
            },
        ],
        128,
        1000,
    );
    let source = f.runner.artifacts.get(f.plan.digest().as_str()).unwrap();
    let materialized = source
        .prepare(&f.plan, &AttemptId::new("object-binding").unwrap())
        .unwrap();
    std::fs::rename(f.root.join("source"), f.root.join("old-source")).unwrap();
    std::fs::write(f.root.join("source"), "printf replaced").unwrap();
    let original = f.root.with_extension("original");
    std::fs::rename(&f.root, &original).unwrap();
    std::fs::create_dir(&f.root).unwrap();
    let mut command = std::process::Command::new(&materialized.interpreter);
    command.args(&materialized.args).env_clear();
    materialized.configure(&mut command).unwrap();
    let output = command.output().unwrap();
    assert_eq!(output.stdout, b"original");
    assert!(output.status.success());
    drop(materialized);
    std::fs::remove_dir_all(original).unwrap();
}

#[test]
fn controlled_input_is_bound_once_and_partial_delivery_is_failed() {
    use crate::{InputBytes, InputResolver};
    struct Input {
        calls: AtomicU64,
    }
    impl InputResolver for Input {
        fn resolve(
            &self,
            _: &FrozenPlan,
            attempt: &AttemptId,
            reference: &VersionedRef,
            max: u64,
        ) -> Result<InputBytes, Error> {
            assert_eq!(attempt.as_str(), "attempt-1");
            assert_eq!(reference.id.as_str(), "secret-input");
            assert_eq!(max, 1_048_576);
            assert_eq!(self.calls.fetch_add(1, Ordering::Relaxed), 0);
            Ok(InputBytes::new(vec![b's'; 1_048_576]))
        }
    }
    let mut f = fixture(
        "exec 0<&-; printf ok\n",
        vec![LaunchArg::ArtifactPath {}],
        128,
        2000,
    );
    let old = f.plan.digest().as_str().to_owned();
    let source = f.runner.artifacts.remove(&old).unwrap();
    let mut source = Arc::try_unwrap(source).ok().unwrap();
    source.controlled_input = Some(Arc::new(Input {
        calls: AtomicU64::new(0),
    }));
    let mut spec = f.plan.spec().clone();
    spec.launch.stdin = StandardInput::Controlled {
        reference: VersionedRef {
            id: Id::new("secret-input").unwrap(),
            revision: Id::new("1").unwrap(),
        },
        encoding: TextEncoding::Utf8,
        max_bytes: 1_048_576,
    };
    let mut limits = execution_app::test_store_limits().plan;
    limits.max_stdin_bytes = 1_048_576;
    f.plan = FrozenPlan::freeze(spec, &limits).unwrap();
    f.runner
        .artifacts
        .insert(f.plan.digest().as_str().into(), Arc::new(source));
    let id = start(&f, 128, 2000);
    let facts = finish(&f, &id);
    assert_ne!(facts.quality, OutputQuality::Complete);
    assert!(facts.stdout.len() + facts.stderr.len() <= 128);
}

fn replan(f: &mut Fixture, change: impl FnOnce(&mut PlanSpec)) {
    let mut spec = f.plan.spec().clone();
    change(&mut spec);
    let plan = FrozenPlan::freeze(spec, &execution_app::test_store_limits().plan).unwrap();
    let artifacts = f.runner.artifacts.remove(f.plan.digest().as_str()).unwrap();
    f.runner
        .artifacts
        .insert(plan.digest().as_str().into(), artifacts);
    f.plan = plan;
}
use super::support::{self as app_support, TestCarrier};
#[test]
fn macos_capture_is_durable_and_reopened_attempt_does_not_launch() {
    use execution_app::{AppConfig, ExecutionApp, RequestContext, Startup};
    let mut f = fixture(
        "printf durable",
        vec![LaunchArg::ArtifactPath {}],
        4096,
        15000,
    );
    replan(&mut f, |s| {
        s.validity = ValidityWindow {
            not_before_unix_ms: 999,
            expires_at_unix_ms: 60000,
        }
    });
    let native = std::mem::replace(
        &mut f.runner,
        NativeRunner::new(Id::new("mechanism-runner").unwrap(), BTreeMap::new(), 8).unwrap(),
    );
    let carrier = TestCarrier(
        Arc::new(native),
        Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    );
    let mut host = app_support::TestHost::new();
    host.template = f.plan.clone();
    let caller = RequestContext {
        actor: f.plan.spec().request.actor.clone(),
    };
    let request = &f.plan.spec().request.request_id;
    let database = f.root.join("execution.sqlite");
    let mut app = ExecutionApp::start(
        &database,
        Startup::CreateTest,
        host.clone(),
        carrier.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    app.submit(&caller, request, &f.plan).unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let status = app.reconcile(request).unwrap();
        if status.process.as_ref().is_some_and(|p| p.finished) {
            assert_eq!(status.process.unwrap().exit_code, Some(0));
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        carrier.0.records.lock().unwrap().is_empty(),
        "capture acknowledgement retires process owner"
    );
    assert_eq!(carrier.1.load(Ordering::SeqCst), 1);
    drop(app);
    {
        let sql = rusqlite::Connection::open(&database).unwrap();
        let (body, output): (Vec<u8>, Vec<u8>) = sql
            .query_row("SELECT body,stdout FROM process_evidence", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        let facts: ProcessEvidence = serde_json::from_slice(&body).unwrap();
        assert!(matches!(facts.scope, ProcessScope::ProcessGroup { .. }));
        assert!(!facts.quiescent);
        assert_eq!(output, b"durable");
    }
    drop(carrier);
    let empty = TestCarrier(
        Arc::new(
            NativeRunner::new(Id::new("mechanism-runner").unwrap(), BTreeMap::new(), 8).unwrap(),
        ),
        Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    );
    let mut app = ExecutionApp::start(
        &database,
        Startup::OpenTest,
        host,
        empty.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let recovered = app.submit(&caller, request, &f.plan).unwrap();
    assert_eq!(recovered.attempts, 1);
    assert_eq!(
        app.status(&caller, request)
            .unwrap()
            .process
            .unwrap()
            .exit_code,
        Some(0)
    );
    assert_eq!(empty.1.load(Ordering::SeqCst), 0);
}

#[test]
fn input_binding_limit_encoding_and_platform_guards_refuse_before_spawn() {
    struct ValueInput(Vec<u8>);
    impl crate::InputResolver for ValueInput {
        fn resolve(
            &self,
            _: &FrozenPlan,
            _: &AttemptId,
            _: &VersionedRef,
            _: u64,
        ) -> Result<crate::InputBytes, Error> {
            Ok(crate::InputBytes::new(self.0.clone()))
        }
    }
    for (bytes, expected) in [
        (None, ProcessFailureKind::Unbound),
        (Some(vec![b'x'; 17]), ProcessFailureKind::Capacity),
        (Some(vec![0xff]), ProcessFailureKind::InvalidInput),
    ] {
        let mut f = fixture(
            "printf forbidden",
            vec![LaunchArg::ArtifactPath {}],
            128,
            1000,
        );
        replan(&mut f, |s| {
            s.launch.stdin = StandardInput::Controlled {
                reference: VersionedRef {
                    id: Id::new("input").unwrap(),
                    revision: Id::new("1").unwrap(),
                },
                encoding: TextEncoding::Utf8,
                max_bytes: 16,
            }
        });
        Arc::get_mut(
            f.runner
                .artifacts
                .get_mut(f.plan.digest().as_str())
                .unwrap(),
        )
        .unwrap()
        .controlled_input =
            bytes.map(|bytes| Arc::new(ValueInput(bytes)) as Arc<dyn crate::InputResolver>);
        let id = start(&f, 128, 1000);
        let facts = finish(&f, &id);
        assert_eq!(facts.scope, ProcessScope::NotStarted {});
        assert_eq!(facts.failure_kind, expected);
        assert!(facts.stdout.is_empty());
    }
    for case in 0..8 {
        let mut f = fixture(
            "printf forbidden",
            vec![LaunchArg::ArtifactPath {}],
            128,
            1000,
        );
        match case {
            0 => replan(&mut f, |s| {
                if let RunAs::User { account } = &mut s.run_as {
                    account.subject = Id::new("4294967294").unwrap()
                }
            }),
            1 => replan(&mut f, |s| {
                s.run_as = RunAs::System {
                    platform: Platform::Windows,
                };
                s.request.target.platform = Platform::Windows;
                s.launch.cwd = r"C:\not-used".into();
            }),
            2 => replan(&mut f, |s| {
                s.session_requirement = SessionRequirement::ActiveUser {
                    account: OsAccountRef {
                        platform: Platform::Macos,
                        subject: Id::new("4294967294").unwrap(),
                    },
                }
            }),
            3 => replan(&mut f, |s| {
                s.launch.interpreter.profile.id = Id::new("unknown").unwrap()
            }),
            4 => replan(&mut f, |s| {
                s.launch.interpreter.profile.revision = Id::new("99").unwrap()
            }),
            5 => replan(&mut f, |s| {
                s.launch
                    .argv
                    .insert(0, LaunchArg::Literal { value: "-c".into() })
            }),
            6 => {
                std::fs::rename(f.root.join("source"), f.root.join("saved")).unwrap();
                std::os::unix::fs::symlink(f.root.join("saved"), f.root.join("source")).unwrap();
            }
            7 => std::fs::set_permissions(
                f.root.join("source"),
                std::fs::Permissions::from_mode(0o666),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let id = start(&f, 128, 1000);
        let facts = finish(&f, &id);
        assert_eq!(facts.scope, ProcessScope::NotStarted {}, "case {case}");
        assert_eq!(facts.end, ProcessEnd::Rejected, "case {case}");
        assert_ne!(facts.failure_kind, ProcessFailureKind::None);
    }
}
#[test]
fn application_cancel_captures_real_process_and_reopen_does_not_dispatch() {
    use execution_app::{AppConfig, ExecutionApp, RequestContext, Startup};
    let mut f = fixture(
        "printf started; exec sleep 30",
        vec![LaunchArg::ArtifactPath {}],
        4096,
        10000,
    );
    let native = std::mem::replace(
        &mut f.runner,
        NativeRunner::new(Id::new("mechanism-runner").unwrap(), BTreeMap::new(), 8).unwrap(),
    );
    let carrier = TestCarrier(
        Arc::new(native),
        Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    );
    let host = app_support::TestHost {
        template: f.plan.clone(),
    };
    let caller = RequestContext {
        actor: f.plan.spec().request.actor.clone(),
    };
    let request = &f.plan.spec().request.request_id;
    let database = f.root.join("cancel.sqlite");
    let mut app = ExecutionApp::start(
        &database,
        Startup::CreateTest,
        host.clone(),
        carrier.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    let started = app.submit(&caller, request, &f.plan).unwrap();
    let attempt = started.attempt_id.unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let facts = carrier.0.evidence(&f.plan, &attempt).unwrap().unwrap();
        if !facts.finished
            && facts.stdout == b"started"
            && matches!(facts.scope, ProcessScope::ProcessGroup { .. })
        {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    app.cancel(&caller, request).unwrap();
    loop {
        let status = app.reconcile(request).unwrap();
        if status.process.as_ref().is_some_and(|p| p.finished) {
            let facts = status.process.unwrap();
            assert_eq!(facts.end, ProcessEnd::Cancelled);
            assert!(!facts.quiescent);
            assert_ne!(facts.quality, OutputQuality::Complete);
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(carrier.0.records.lock().unwrap().is_empty());
    drop(app);
    drop(carrier);
    let empty = TestCarrier(
        Arc::new(
            NativeRunner::new(Id::new("mechanism-runner").unwrap(), BTreeMap::new(), 8).unwrap(),
        ),
        Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    );
    let mut app = ExecutionApp::start(
        &database,
        Startup::OpenTest,
        host,
        empty.clone(),
        AppConfig::test_defaults(1),
    )
    .unwrap();
    assert_eq!(app.submit(&caller, request, &f.plan).unwrap().attempts, 1);
    assert_eq!(
        app.status(&caller, request).unwrap().process.unwrap().end,
        ProcessEnd::Cancelled
    );
    assert_eq!(empty.1.load(Ordering::SeqCst), 0);
}
