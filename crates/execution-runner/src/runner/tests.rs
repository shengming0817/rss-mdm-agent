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
    while f.runner.evidence(&f.plan, &id).unwrap().is_none() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(5));
    }
    f.runner.stop(&f.plan, &id).unwrap();
    assert_eq!(finish(&f, &id).end, ProcessEnd::Cancelled);
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
