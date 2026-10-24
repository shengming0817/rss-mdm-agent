//! Real worker processes exercise the collection receipt boundary in NativeRunner.
use super::*;

#[test]
fn collection_receipt_binds_closure_to_input_output_exit_and_native_budget() {
    const OUTPUT: &[u8] = b"[{\"serial\":\"fixture\"}]";
    const CAP: u64 = 4096;
    let worker = fixture("", vec![LaunchArg::ArtifactPath {}], CAP, 5000);
    let executable = worker.root.join("collection-worker");
    let source = worker.root.join("collection-worker.rs");
    // This fixture emits caller-selected control bytes; it does not reimplement
    // receipt validation. Only the real runner decides whether they prove closure.
    std::fs::write(
        &source,
        r#"
use std::io::Write;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3);
    assert_eq!(args[1], "--collection-worker");
    let input = std::fs::read_to_string(&args[2]).unwrap();
    let exit: i32 = input.parse().unwrap();
    std::io::stdout().write_all(b"[{\"serial\":\"fixture\"}]").unwrap();
    std::io::stderr().write_all(&std::fs::read("receipt").unwrap()).unwrap();
    std::process::exit(exit);
}
"#,
    )
    .unwrap();
    assert!(std::process::Command::new("rustc")
        .arg("--edition=2024")
        .arg("--crate-name=collection_worker_fixture")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .status()
        .unwrap()
        .success());
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o500)).unwrap();
    let worker_digest = Digest::new(format!(
        "{:x}",
        Sha256::digest(std::fs::read(&executable).unwrap())
    ))
    .unwrap();

    for case in [
        "valid",
        "missing",
        "malformed",
        "open",
        "wrong-input",
        "wrong-output",
        "nonzero",
        "native-overflow",
    ] {
        let exit = if case == "nonzero" { 7 } else { 0 };
        let mut f = fixture(
            &exit.to_string(),
            vec![LaunchArg::ArtifactPath {}],
            CAP,
            5000,
        );
        replan(&mut f, |s| {
            s.launch.interpreter.profile.id = Id::new("native-collection-worker").unwrap();
            s.launch.interpreter.artifact.sha256 = worker_digest.clone();
            s.launch.argv = vec![
                LaunchArg::Literal {
                    value: "--collection-worker".into(),
                },
                LaunchArg::ArtifactPath {},
            ];
            s.launch.output = OutputSpec {
                stdout: TextEncoding::Utf8,
                stderr: TextEncoding::Utf8,
                format: OutputFormat::Json { max_rows: 1 },
            };
        });
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
        .interpreter = executable.clone();
        let native_bytes = if case == "native-overflow" { CAP } else { 17 };
        let receipt = CollectionCompletion {
            input_sha256: if case == "wrong-input" {
                "00".repeat(32)
            } else {
                f.plan.spec().launch.artifact.sha256.as_str().into()
            },
            output_sha256: if case == "wrong-output" {
                "00".repeat(32)
            } else {
                format!("{:x}", Sha256::digest(OUTPUT))
            },
            closed: case != "open",
            native_output_bytes: native_bytes,
        };
        let receipt = match case {
            "missing" => Vec::new(),
            "malformed" => b"{broken".to_vec(),
            _ => serde_json::to_vec(&receipt).unwrap(),
        };
        std::fs::write(f.root.join("receipt"), &receipt).unwrap();
        let id = start(&f, CAP, 5000);
        let facts = finish(&f, &id);
        assert!(
            matches!(facts.scope, ProcessScope::ProcessGroup { .. }),
            "{case}"
        );
        assert_eq!(facts.exit_code, Some(exit), "{case}");
        assert_eq!(facts.failure_kind, ProcessFailureKind::None, "{case}");
        assert_eq!(facts.stdout, OUTPUT, "{case}");
        assert_eq!(facts.stderr, receipt, "{case}");
        let accounted_native = if matches!(case, "missing" | "malformed") {
            0
        } else {
            native_bytes
        };
        assert_eq!(
            facts.total_output_bytes,
            (OUTPUT.len() + receipt.len()) as u64 + accounted_native,
            "{case}"
        );
        assert_eq!(
            facts.end,
            if case == "native-overflow" {
                ProcessEnd::OutputLimit
            } else {
                ProcessEnd::Exited
            },
            "{case}"
        );
        assert_eq!(facts.quiescent, case == "valid", "{case}");
        let terminal = f
            .runner
            .observe(&f.plan, &id, ObservationStage::Termination, now().unwrap())
            .unwrap();
        if case == "valid" {
            assert_eq!(facts.quality, OutputQuality::Complete);
            assert!(matches!(
                terminal.unwrap().observation,
                Observation::Exited { exit_code: 0, total_output_bytes }
                    if total_output_bytes == facts.total_output_bytes
            ));
        } else {
            // Fully captured JSON alone cannot establish a completed attempt.
            assert!(
                terminal.is_none(),
                "{case} must not publish terminal closure"
            );
        }
        if case == "nonzero" {
            assert_eq!(facts.quality, OutputQuality::Failed);
        }
        if case == "native-overflow" {
            assert_eq!(facts.quality, OutputQuality::Truncated);
        }
        assert!(f
            .runner
            .observe(&f.plan, &id, ObservationStage::Assessment, now().unwrap())
            .unwrap()
            .is_none());
        f.runner.acknowledge_capture(&f.plan, &facts).unwrap();
        assert_eq!(
            f.runner.records.lock().unwrap().is_empty(),
            facts.quiescent,
            "{case}"
        );
    }
}
