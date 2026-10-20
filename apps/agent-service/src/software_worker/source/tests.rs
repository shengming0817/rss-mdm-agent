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
    fs::write(
        &manager,
        format!(
            "#!/bin/sh\nif [ \"$1\" = --repository ]; then printf '%s\\n' '{}'; else exit 91; fi\n",
            repository.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&manager, fs::Permissions::from_mode(0o700)).unwrap();
    let pack = root.path.join("source.pack");
    platform_private_storage::write_new(&pack, &data.software_source.as_ref().unwrap().pack)
        .unwrap();
    WorkerRequest {
        intent: SoftwareOperation::Install,
        export: spec.steps[0].export.clone(),
        action: spec.steps[0].action.clone(),
        operation: WorkerOperation::Prepare,
        native_output: Default::default(),
        native_pending: Default::default(),
        external_pending: Default::default(),
        materials: [("native-source".into(), material(&pack))].into(),
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
    pending.group = group();
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
