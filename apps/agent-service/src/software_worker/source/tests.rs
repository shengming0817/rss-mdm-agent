//! Actual source worker/Git/filesystem lifecycle with a pinned fixture manager; no OS install claim.
use super::*;
use crate::protocol_test_support::{software_native, Root, Server};
use std::os::unix::fs::PermissionsExt;

fn material(path: &Path) -> SoftwareMaterial {
    SoftwareMaterial {
        path: path.to_str().unwrap().into(),
        artifact: ExactArtifactRef {
            resource: VersionedRef {
                id: Id::new("fixture").unwrap(),
                revision: Id::new("1").unwrap(),
            },
            sha256: Digest::new(format!("{:x}", Sha256::digest(fs::read(path).unwrap()))).unwrap(),
        },
    }
}
pub(super) fn fixture(root: &Root) -> WorkerRequest {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.block_on(Server::tls());
    server
        .data
        .lock()
        .unwrap()
        .software_native(&server.url, true);
    let data = server.data.lock().unwrap();
    let wire::TaskPayload::Software(spec) = &data.offer.as_ref().unwrap().payload else {
        unreachable!()
    };
    let repository = root.path.join("repository");
    fs::create_dir_all(repository.join("Library/Taps")).unwrap();
    let manager = root.path.join("brew-fixture");
    let wire::SoftwareTaskExport::Brew { binding, .. } = &spec.steps[0].export else {
        unreachable!()
    };
    let cache_leaf = format!(
        "{:x}--fixture.bottle.tar.gz",
        Sha256::digest(binding.artifacts[0].url.as_bytes())
    );
    fs::write(
        &manager,
        format!(
            "#!/bin/sh\ncase \"$1\" in\n--repository) printf '%s\\n' '{}' ;;\ninfo) printf '%s\\n' '{{\"formulae\":[{{\"tap\":\"rss/fixture\",\"versions\":{{\"stable\":\"1.0\"}}}}]}}' ;;\n--cache) printf '%s/downloads/%s\\n' \"$HOMEBREW_CACHE\" '{cache_leaf}' ;;\ninstall) test -f \"$HOMEBREW_CACHE/downloads/{cache_leaf}\" || exit 92; printf '%s\\n' \"$*\" >> '{}'; if test -f '{}'; then exit 17; fi; printf '%s\\n' fixture-native-install ;;\n*) exit 91 ;;\nesac\n",
            repository.display(), root.path.join("native-calls").display(), root.path.join("fail-install").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&manager, fs::Permissions::from_mode(0o700)).unwrap();
    let pack = root.path.join("source.pack");
    platform_private_storage::write_new(&pack, &data.software_source.as_ref().unwrap().pack)
        .unwrap();
    let installer = root.path.join("package");
    platform_private_storage::write_new(&installer, &data.bytes).unwrap();
    WorkerRequest {
        intent: SoftwareOperation::Install,
        export: spec.steps[0].export.clone(),
        action: spec.steps[0].action.clone(),
        operation: WorkerOperation::Prepare,
        native_output: Default::default(),
        native_pending: Default::default(),
        materials: [
            ("native-source".into(), material(&pack)),
            ("package".into(), material(&installer)),
        ]
        .into(),
        tools: [
            ("manager".into(), material(&manager)),
            ("git".into(), material(Path::new("/usr/bin/git"))),
        ]
        .into(),
        resource_root: root.path.join("attempt"),
        run_as: RunAs::User {
            account: OsAccountRef {
                platform: Platform::Macos,
                subject: Id::new(unsafe { libc::geteuid() }.to_string()).unwrap(),
            },
        },
        session: SessionRequirement::NotRequired {},
        architecture: wire::TaskArchitecture::Aarch64,
        output_bytes: 1_048_576,
        step: 0,
    }
}

#[test]
fn script_detector_honors_arguments_and_rejects_a_declared_invalid_publisher_before_execution() {
    let root = Root::new();
    let mut request = fixture(&root);
    request.operation = WorkerOperation::Detect;
    let script = root.path.join("detector.sh");
    let marker = root.path.join("detector-ran");
    fs::write(
        &script,
        b"#!/bin/sh\nprintf '%s' \"$2\" > \"$1\"\nprintf '%s\\n' '{\"kind\":\"absent\"}'\n",
    )
    .unwrap();
    request
        .materials
        .insert("detector".into(), material(&script));
    request
        .tools
        .insert("detect-interpreter".into(), material(Path::new("/bin/sh")));
    request
        .tools
        .insert("codesign".into(), material(Path::new("/usr/bin/codesign")));
    let wire::SoftwareTaskBehavior::Brew(native) = &mut request.action.behavior else {
        unreachable!()
    };
    let mut invocation = native.install.clone();
    invocation.arguments = vec![marker.to_str().unwrap().into(), " literal ; $(x) ".into()];
    native.detect = wire::SoftwareTaskDetection::Script {
        command: wire::SoftwareTaskScript {
            entry: "detector".into(),
            interpreter: wire::SoftwareTaskInterpreter::PosixSh,
            invocation,
        },
    };
    assert_eq!(detect(&request).unwrap(), SoftwareState::Absent {});
    assert_eq!(fs::read_to_string(&marker).unwrap(), " literal ; $(x) ");
    fs::remove_file(&marker).unwrap();
    request.action.signatures.push(wire::SoftwareTaskSignature {
        artifact: "detector".into(),
        mechanism: wire::SoftwareTaskSignatureMechanism::AppleDeveloperId,
        publisher: "UNTRUSTED".into(),
    });
    assert!(matches!(detect(&request), Err(Error::Untrusted)));
    assert!(!marker.exists());
}

#[test]
fn native_install_consumes_verified_cache_and_preserves_real_failure_before_cleanup() {
    for fail in [false, true] {
        let root = Root::new();
        let mut request = fixture(&root);
        if fail {
            fs::write(root.path.join("fail-install"), b"fail").unwrap();
        }
        let before = SoftwareState::Absent {};
        execute(&request, Some(&before)).unwrap();
        let tap = state(&request).unwrap().unwrap().path.unwrap();
        request.operation = WorkerOperation::Install;
        let (code, observed) = execute(&request, Some(&before)).unwrap();
        assert_eq!(code, if fail { 17 } else { 0 });
        assert!(observed.detected.is_none());
        assert!(observed.closed);
        assert_eq!(
            fs::read_to_string(root.path.join("native-calls")).unwrap(),
            "install --formula --force-bottle --ignore-dependencies rss/fixture/fixture-0\n"
        );
        let cached = fs::read_dir(request.resource_root.join("cache/downloads"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read(cached).unwrap(),
            fs::read(root.path.join("package")).unwrap()
        );
        request.operation = WorkerOperation::Cleanup;
        assert_eq!(execute(&request, None).unwrap().0, 0);
        assert!(!tap.exists());
        assert_eq!(
            fs::read_to_string(root.path.join("native-calls"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
}
#[test]
fn changed_artifact_fails_before_native_install_and_still_closes_owned_source() {
    let root = Root::new();
    let mut request = fixture(&root);
    execute(&request, Some(&SoftwareState::Absent {})).unwrap();
    let length = fs::metadata(root.path.join("package")).unwrap().len() as usize;
    fs::write(root.path.join("package"), vec![b'x'; length]).unwrap();
    request.operation = WorkerOperation::Install;
    assert!(execute(&request, Some(&SoftwareState::Absent {})).is_err());
    assert!(!root.path.join("native-calls").exists());
    request.operation = WorkerOperation::Cleanup;
    execute(&request, None).unwrap();
    assert!(!request.resource_root.exists());
}
#[test]
fn source_creation_borrowing_and_cleanup_are_owned_by_the_original_attempt() {
    let root = Root::new();
    let mut first = fixture(&root);
    let before = SoftwareState::Absent {};
    assert_eq!(execute(&first, Some(&before)).unwrap().0, 0);
    let original = state(&first).unwrap().unwrap();
    assert!(original.created);
    let tap = original.path.clone().unwrap();
    assert!(tap.join("Formula/fixture-0.rb").is_file());
    let mut second: WorkerRequest =
        serde_json::from_slice(&serde_json::to_vec(&first).unwrap()).unwrap();
    second.resource_root = root.path.join("second-attempt");
    execute(&second, Some(&before)).unwrap();
    assert!(!state(&second).unwrap().unwrap().created);
    second.operation = WorkerOperation::Cleanup;
    execute(&second, None).unwrap();
    assert!(tap.is_dir());
    // Original activity still being alive cannot be erased by a cleanup worker.
    let mut pending = original;
    pending.active = true;
    pending.activity = execution_runner::software::NativeActivity::current().unwrap();
    save(&first, &pending).unwrap();
    first.operation = WorkerOperation::Cleanup;
    assert!(!execute(&first, None).unwrap().1.closed);
    assert!(tap.is_dir());
    pending.active = false;
    save(&first, &pending).unwrap();
    execute(&first, None).unwrap();
    assert!(!tap.exists());
    assert!(!first.resource_root.exists());
}
#[test]
fn incomplete_preparation_can_close_but_changed_foreign_content_is_retained() {
    let root = Root::new();
    let mut request = fixture(&root);
    let source = request.material("native-source").unwrap();
    fs::write(&source, b"bad-pack").unwrap();
    assert!(execute(&request, Some(&SoftwareState::Absent {})).is_err());
    let record = state(&request).unwrap().unwrap();
    assert!(record.created);
    let tap = record.path.unwrap();
    request.operation = WorkerOperation::Cleanup;
    execute(&request, None).unwrap();
    assert!(!tap.exists());

    let root = Root::new();
    let mut request = fixture(&root);
    execute(&request, Some(&SoftwareState::Absent {})).unwrap();
    let record = state(&request).unwrap().unwrap();
    let tap = record.path.unwrap();
    fs::write(tap.join("Formula/fixture-0.rb"), b"foreign Ruby").unwrap();
    request.operation = WorkerOperation::Cleanup;
    assert!(execute(&request, None).is_err());
    assert!(tap.is_dir());
}
#[test]
fn read_credentials_are_independent_from_device_bootstrap_encoding() {
    let credential = wire::SoftwareReadCredential {
        source: "target".into(),
        reference: "read".into(),
    };
    let key = agent_client::native_source::credential_key(&credential).unwrap();
    assert!(key.starts_with("software-read-"));
    assert!(agent_client::native_source::validate_read_token(software_native::TOKEN).is_ok());
    assert!(agent_client::native_source::validate_read_token("short").is_err());
    assert!(agent_client::native_source::validate_read_token(&" ".repeat(32)).is_err());
}

#[test]
fn interrupted_git_checkout_is_private_and_recovery_closes_the_attempt() {
    let root = Root::new();
    let mut request = fixture(&root);
    let documents = [
        (
            "Formula/fixture-0.rb",
            b"class Fixture < Formula\nend\n".as_slice(),
            "100644",
        ),
        (
            "Formula/fixture-1.rb",
            b"class Other < Formula\nend\n".as_slice(),
            "100644",
        ),
    ];
    let (frozen_commit, pack) = software_native::pack(&documents, false);
    let wire::SoftwareTaskExport::Brew {
        commit, binding, ..
    } = &mut request.export
    else {
        unreachable!()
    };
    *commit = frozen_commit;
    binding.document_sha256 =
        wire::brew_documents_digest(documents.iter().map(|(p, b, _)| (*p, *b))).unwrap();
    let source = root.path.join("source.pack");
    fs::write(&source, pack).unwrap();
    request
        .materials
        .insert("native-source".into(), material(&source));
    let git = root.path.join("git-checkout-interrupted");
    // Real Git writes one of two frozen paths, then this pinned process exits unsuccessfully.
    // This deterministic interruption leaves the same partially populated checkout as a kill.
    fs::write(&git, b"#!/bin/sh\nif test \"$1\" = --git-dir; then\n /usr/bin/git \"$1\" \"$2\" \"$3\" \"$4\" checkout \"$7\" -- Formula/fixture-0.rb || exit 92\n exit 17\nfi\nexec /usr/bin/git \"$@\"\n").unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o700)).unwrap();
    request.tools.insert("git".into(), material(&git));
    assert!(execute(&request, Some(&SoftwareState::Absent {})).is_err());
    let record = state(&request).unwrap().unwrap();
    let tap = record.path.unwrap();
    assert!(!tap.exists());
    let partial = request.resource_root.join("tap/Formula");
    assert!(partial.join("fixture-0.rb").is_file());
    assert!(!partial.join("fixture-1.rb").exists());
    request.operation = WorkerOperation::Cleanup;
    assert_eq!(execute(&request, None).unwrap().0, 0);
    assert!(!request.resource_root.exists());
    assert!(!tap.exists());
    assert!(!tap.parent().unwrap().exists());
}

#[test]
fn identical_foreign_replacement_does_not_inherit_created_tap_ownership() {
    let root = Root::new();
    let mut request = fixture(&root);
    execute(&request, Some(&SoftwareState::Absent {})).unwrap();
    let record = state(&request).unwrap().unwrap();
    let tap = record.path.unwrap();
    let original = root.path.join("original-tap");
    fs::rename(&tap, &original).unwrap();
    software_native::git(
        &root.path,
        &[
            "clone",
            "--no-local",
            original.to_str().unwrap(),
            tap.to_str().unwrap(),
        ],
        &[],
    );
    let wire::SoftwareTaskExport::Brew { uri, .. } = &request.export else {
        unreachable!()
    };
    software_native::git(&tap, &["config", "remote.origin.url", uri], &[]);
    fs::set_permissions(&tap, fs::Permissions::from_mode(0o700)).unwrap();
    request.operation = WorkerOperation::Cleanup;
    assert!(execute(&request, None).is_err());
    assert!(tap.join("Formula/fixture-0.rb").is_file());
    assert!(request.resource_root.is_dir());
}
