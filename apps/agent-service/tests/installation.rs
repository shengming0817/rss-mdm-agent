#[test]
fn native_acceptance_requires_real_success_and_remote_acknowledgement() {
    assert!(std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/service/verify-execution-macos.test.py"
        ))
        .status()
        .unwrap()
        .success());
}
