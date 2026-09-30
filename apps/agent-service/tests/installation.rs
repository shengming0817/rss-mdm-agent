#[test]
#[cfg(target_os = "macos")]
fn launchd_installation_permissions_and_partial_uninstall() {
    assert!(std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/service/execution-macos.test.py"
        ))
        .status()
        .unwrap()
        .success());
}

#[test]
#[cfg(windows)]
fn windows_installer_stops_uncertain_start_before_removing_registration() {
    let pwsh = std::env::var_os("RSS_TEST_PWSH7")
        .unwrap_or_else(|| r"C:\Program Files\PowerShell\7\pwsh.exe".into());
    assert!(std::process::Command::new(pwsh)
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/service/execution-windows.test.ps1"
        ))
        .status()
        .unwrap()
        .success());
}

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
