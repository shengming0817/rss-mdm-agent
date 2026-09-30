//! Real local OS mechanics; no production AppHost or test authority is promoted.

use super::*;
use sha2::{Digest as _, Sha256};
use std::{path::PathBuf, sync::atomic::AtomicU64};
fn now() -> Result<u64, Error> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64)
}
fn limits() -> ExecutionLimits {
    execution_app::test_store_limits().input
}
fn interpreter() -> PathBuf {
    std::env::var_os("RSS_TEST_PWSH7")
        .map(PathBuf::from)
        .unwrap_or_else(|| r"C:\Program Files\PowerShell\7\pwsh.exe".into())
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    plan: FrozenExecution,
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
    native_process::private_storage::directory(&root).unwrap();
    let root = root.canonicalize().unwrap();

    let content = root.join("source");
    std::fs::write(&content, script).unwrap();

    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../execution-contract/tests/fixtures/plan.json"
    ))
    .unwrap();
    value["request"]["target"] = serde_json::json!({"device":"mechanism-device","platform":"windows","scope":{"kind":"device"}});
    value["runAs"] = serde_json::json!({"kind":"user","account":{"platform":"windows","subject":crate::windows::token_identity().unwrap().0}});
    value["sessionRequirement"] = serde_json::json!({"kind":"activeUser", "account":value["runAs"]["account"].clone(), "session":crate::host::current_session_binding().unwrap()});
    value["constraints"] = serde_json::json!({"kind":"osIdentity"});
    value["launch"]["artifact"]["sha256"] =
        format!("{:x}", Sha256::digest(script.as_bytes())).into();
    value["launch"]["interpreter"]["artifact"]["sha256"] =
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(interpreter()).expect(
                "Windows tests require protected PowerShell 7; see execution-runner README"
            ))
        )
        .into();
    value["launch"]["interpreter"]["profile"] =
        serde_json::json!({"id":"native-pwsh7-file","revision":"1"});
    let mut args: Vec<LaunchArg> = ["-NoLogo", "-NoProfile", "-NonInteractive", "-File"]
        .into_iter()
        .map(|s| LaunchArg::Literal { value: s.into() })
        .collect();
    args.extend(argv);
    value["launch"]["argv"] = serde_json::to_value(args).unwrap();
    value["launch"]["stdin"] = serde_json::json!({"kind":"closed"});
    value["launch"]["env"] = serde_json::json!({});
    value["launch"]["cwd"] = root.to_str().unwrap().into();
    value["budget"] =
        serde_json::json!({"totalTimeoutMs":timeout,"totalOutputBytes":budget,"maxAttempts":1});
    let time = now().unwrap();
    value["validity"] = serde_json::json!({"notBeforeUnixMs":time-1,"expiresAtUnixMs":time+60000});
    let limits = ExecutionLimits {
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
    let plan = FrozenExecution::freeze(
        decode_execution(&serde_json::to_vec(&value).unwrap(), &limits).unwrap(),
        &limits,
    )
    .unwrap();
    let artifacts = Artifacts {
        program: Vec::new(),
        delegate: None,
        interpreter: interpreter(),
        content,
        work_root: root.clone(),
        controlled_input: None,
        fixture_owned: true,
    };
    let runner = NativeRunner::new(
        Id::new("test-runner").unwrap(),
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
    let until = Instant::now() + Duration::from_secs(20);
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

fn replan(f: &mut Fixture, change: impl FnOnce(&mut ExecutionInput)) {
    let mut spec = f.plan.spec().clone();
    change(&mut spec);
    let plan = FrozenExecution::freeze(spec, &limits()).unwrap();
    let artifacts = f
        .runner
        .artifacts
        .entries
        .lock()
        .unwrap()
        .remove(f.plan.digest().as_str())
        .unwrap();
    f.runner
        .artifacts
        .entries
        .lock()
        .unwrap()
        .insert(plan.digest().as_str().into(), artifacts);
    f.plan = plan;
}
fn wait_spawned(f: &Fixture, id: &AttemptId) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let facts = f.runner.evidence(&f.plan, id).unwrap().unwrap();
        if matches!(facts.scope, ProcessScope::JobObject { .. }) {
            return;
        }
        assert!(!facts.finished, "process failed before spawn: {facts:?}");
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn powershell_literals_dual_streams_and_exit_evidence() {
    let f = fixture(
        "param([string]$Value) [Console]::Out.Write($Value); [Console]::Error.Write('err')",
        vec![
            LaunchArg::ArtifactPath {},
            LaunchArg::Literal {
                value: "-Value:".into(),
            },
            LaunchArg::Literal {
                value: "$(Write-Host bad); spaced".into(),
            },
        ],
        4096,
        15000,
    );
    let id = start(&f, 4096, 15000);
    let facts = finish(&f, &id);
    assert_eq!(facts.stdout, b"$(Write-Host bad); spaced");
    assert_eq!(facts.stderr, b"err");
    assert_eq!(facts.end, ProcessEnd::Exited);
    assert_eq!(facts.exit_code, Some(0));
    assert!(facts.quiescent);
    assert_eq!(facts.quality, OutputQuality::Complete);
    assert!(f
        .runner
        .observe(&f.plan, &id, ObservationStage::Assessment, now().unwrap())
        .unwrap()
        .is_none());
}
#[test]
fn windows_output_cap_running_cancel_timeout_and_restart() {
    let f=fixture("while ($true) {[Console]::Out.Write('xxxxxxxxxxxxxxxx');[Console]::Error.Write('yyyyyyyyyyyyyyyy')}",vec![LaunchArg::ArtifactPath{}],4096,15000);
    let id = start(&f, 128, 15000);
    let facts = finish(&f, &id);
    assert!(facts.stdout.len() + facts.stderr.len() <= 128);
    assert!(facts.total_output_bytes >= 128);
    assert_eq!(facts.quality, OutputQuality::Truncated);
    assert!(facts.quiescent);
    let empty = NativeRunner::new(Id::new("test-runner").unwrap(), BTreeMap::new(), 8).unwrap();
    assert!(empty.evidence(&f.plan, &id).unwrap().is_none());
    for cancel in [false, true] {
        let f = fixture(
            "Start-Sleep -Seconds 60",
            vec![LaunchArg::ArtifactPath {}],
            4096,
            15000,
        );
        let id = start(&f, 4096, if cancel { 15000 } else { 3000 });
        wait_spawned(&f, &id);
        if cancel {
            f.runner.stop(&f.plan, &id).unwrap();
        }
        let facts = finish(&f, &id);
        assert_eq!(
            facts.end,
            if cancel {
                ProcessEnd::Cancelled
            } else {
                ProcessEnd::TimedOut
            }
        );
        assert!(facts.quiescent);
        assert_ne!(facts.quality, OutputQuality::Complete);
    }
}
struct Input(Vec<u8>);
impl crate::InputResolver for Input {
    fn resolve(
        &self,
        _: &FrozenExecution,
        _: &AttemptId,
        _: &VersionedRef,
        _: u64,
    ) -> Result<crate::InputBytes, Error> {
        Ok(crate::InputBytes::new(self.0.clone()))
    }
}
#[test]
fn windows_controlled_stdin_and_early_close_quality() {
    for early in [false, true] {
        let mut f = fixture(
            if early {
                "exit 0"
            } else {
                "[Console]::Out.Write([Console]::In.ReadToEnd())"
            },
            vec![LaunchArg::ArtifactPath {}],
            4096,
            15000,
        );
        replan(&mut f, |s| {
            s.launch.stdin = StandardInput::Controlled {
                reference: VersionedRef {
                    id: Id::new("input").unwrap(),
                    revision: Id::new("1").unwrap(),
                },
                encoding: TextEncoding::Utf8,
                max_bytes: 65536,
            }
        });
        let bytes = if early {
            vec![b'x'; 65536]
        } else {
            b"literal input".to_vec()
        };
        Arc::get_mut(
            f.runner
                .artifacts
                .entries
                .lock()
                .unwrap()
                .get_mut(f.plan.digest().as_str())
                .unwrap(),
        )
        .unwrap()
        .controlled_input = Some(Arc::new(Input(bytes)));
        let id = start(&f, 4096, 15000);
        let facts = finish(&f, &id);
        assert!(facts.quiescent);
        if early {
            assert_ne!(facts.quality, OutputQuality::Complete)
        } else {
            assert_eq!(facts.stdout, b"literal input");
            assert_eq!(facts.quality, OutputQuality::Complete)
        }
    }
}
#[test]
fn windows_profiles_identity_and_content_fail_closed() {
    let mut f = fixture(
        "Write-Output ok",
        vec![LaunchArg::ArtifactPath {}],
        4096,
        15000,
    );
    replan(&mut f, |s| {
        s.run_as = RunAs::User {
            account: OsAccountRef {
                platform: Platform::Windows,
                subject: Id::new("S-1-5-21-0-0-0-99999").unwrap(),
            },
        }
    });
    let id = start(&f, 4096, 15000);
    assert!(matches!(finish(&f, &id).scope, ProcessScope::NotStarted {}));
    let f = fixture(
        "Write-Output ok",
        vec![LaunchArg::ArtifactPath {}],
        4096,
        15000,
    );
    std::fs::write(f.root.join("source"), "Write-Output replaced").unwrap();
    let id = start(&f, 4096, 15000);
    assert_eq!(finish(&f, &id).end, ProcessEnd::Rejected);
    let mut f = fixture(
        "SELECT name FROM arbitrary;",
        vec![LaunchArg::ArtifactPath {}],
        4096,
        15000,
    );
    replan(&mut f, |s| {
        s.launch.interpreter.profile.id = Id::new("native-osquery-info-v1").unwrap();
        s.launch.argv = vec![
            LaunchArg::Literal {
                value: "--json".into(),
            },
            LaunchArg::Literal {
                value: "SELECT name FROM arbitrary;".into(),
            },
        ];
        s.launch.output.format = OutputFormat::Json { max_rows: 1 };
    });
    let id = start(&f, 4096, 15000);
    assert_eq!(finish(&f, &id).end, ProcessEnd::Rejected);
}

use super::support as app_support;
use super::support::TestCarrier;
#[test]
fn windows_capture_is_durable_and_reopened_attempt_does_not_launch() {
    use execution_app::{AppConfig, ExecutionApp, RequestContext, Startup};
    let mut f = fixture(
        "[Console]::Out.Write('durable')",
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
        NativeRunner::new(Id::new("test-runner").unwrap(), BTreeMap::new(), 8).unwrap(),
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
    app.request_execution(&caller, &f.plan).unwrap();
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
        assert!(matches!(facts.scope, ProcessScope::JobObject { .. }));
        assert!(facts.quiescent);
        assert_eq!(output, b"durable");
    }
    drop(carrier);
    let empty = TestCarrier(
        Arc::new(NativeRunner::new(Id::new("test-runner").unwrap(), BTreeMap::new(), 8).unwrap()),
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
    let recovered = app.request_execution(&caller, &f.plan).unwrap();
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
