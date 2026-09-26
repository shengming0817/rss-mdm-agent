use execution_app::Error;
use execution_contract::*;
use std::path::Path;

pub(super) fn arguments(
    s: &SoftwareSpec,
    manager: &Path,
    payload: &Path,
) -> Result<Vec<String>, Error> {
    let path = |p: &Path| p.to_str().map(str::to_owned).ok_or(Error::InvalidInput);
    let mut args = vec![path(manager)?, path(payload)?];
    args.push(
        match s.mutation {
            MutationKind::Install => "install",
            MutationKind::Upgrade => "upgrade",
            MutationKind::Downgrade => "downgrade",
            MutationKind::Uninstall => "uninstall",
        }
        .into(),
    );
    args.extend([
        s.source.as_str().into(),
        s.resource.as_str().into(),
        match &s.desired {
            DesiredState::Present { version, .. } => version.as_str().into(),
            DesiredState::Absent => String::new(),
        },
        s.package.architecture.as_str().into(),
    ]);
    if s.adapter == SoftwareKind::Pkg {
        args.push("/usr/sbin/pkgutil".into());
    }
    Ok(args)
}
/// Fixed MSI wrapper: Authenticode validity is separate from production publisher approval.
pub(super) const MSI: &str = r#"param($Manager,$Payload,$Operation,$Source,$Resource,$Version,$Architecture)
$ErrorActionPreference = 'Stop'
if ((Get-AuthenticodeSignature -LiteralPath $Payload).Status -ne 'Valid') { exit 87 }
if ($Operation -eq 'uninstall') { & $Manager /x $Resource /qn /norestart } else { & $Manager /i $Payload /qn /norestart }
exit $LASTEXITCODE
"#;
pub(super) const WINGET: &str = r#"param($Manager,$Payload,$Operation,$Source,$Resource,$Version,$Architecture)
$ErrorActionPreference = 'Stop'
if ($Operation -eq 'uninstall') { & $Manager uninstall --id $Resource --exact --source $Source --silent --disable-interactivity }
else { & $Manager install --manifest $Payload --architecture $Architecture --silent --disable-interactivity }
exit $LASTEXITCODE
"#;
pub(super) const PKG: &str = r#"#!/bin/sh
set -eu
manager=$1; payload=$2; operation=$3
[ "$operation" != uninstall ] || exit 64
"${8}" --check-signature "$payload" >/dev/null
exec "$manager" -pkg "$payload" -target /
"#;
pub(super) const BREW: &str = r#"#!/bin/sh
set -eu
[ "$(/usr/bin/id -u)" != 0 ] || exit 77
manager=$1; payload=$2; operation=$3; source=$4; resource=$5
export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_INSTALLED_DEPENDENTS_CHECK=1 HOMEBREW_NO_INSTALL_UPGRADE=1 HOMEBREW_NO_ANALYTICS=1
case "$operation" in
 uninstall) exec "$manager" uninstall --formula "$resource" ;;
 install) exec "$manager" install --formula "$payload" ;;
 upgrade|downgrade) exec "$manager" reinstall --formula "$payload" ;;
 *) exit 64 ;;
esac
"#;
pub(crate) fn wrapper(kind: SoftwareKind) -> Option<&'static [u8]> {
    match kind {
        SoftwareKind::Msi => Some(MSI.as_bytes()),
        SoftwareKind::Winget => Some(WINGET.as_bytes()),
        SoftwareKind::Pkg => Some(PKG.as_bytes()),
        SoftwareKind::Homebrew => Some(BREW.as_bytes()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_adapters_keep_coordinates_as_individual_arguments() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../execution-contract/tests/fixtures/software.json"
        ))
        .unwrap();
        let mut spec: SoftwareSpec =
            serde_json::from_value(value["execution"]["software"].clone()).unwrap();
        for adapter in [
            SoftwareKind::Msi,
            SoftwareKind::Winget,
            SoftwareKind::Pkg,
            SoftwareKind::Homebrew,
            SoftwareKind::WindowsBundle,
            SoftwareKind::MacosBundle,
        ] {
            spec.adapter = adapter;
            spec.resource = PackageValue::new("name with spaces; $(no-shell)").unwrap();
            let args = arguments(
                &spec,
                Path::new("/fixed manager"),
                Path::new("/exact package"),
            )
            .unwrap();
            assert_eq!(args.len(), if adapter == SoftwareKind::Pkg { 8 } else { 7 });
            assert_eq!(args[0], "/fixed manager");
            assert_eq!(args[1], "/exact package");
            assert_eq!(args[4], spec.resource.as_str());
        }
    }
    #[test]
    fn manager_entries_preserve_identity_and_forbid_fallbacks() {
        assert!(MSI.contains("Get-AuthenticodeSignature"));
        assert!(MSI.contains("/norestart"));
        assert!(WINGET.contains("--manifest $Payload --architecture $Architecture"));
        assert!(PKG.contains("--check-signature"));
        assert!(BREW.contains("id -u"));
        assert!(BREW.contains("HOMEBREW_NO_AUTO_UPDATE=1"));
        assert!(BREW.contains("reinstall --formula"));
        for entry in [MSI, WINGET, PKG, BREW] {
            assert!(!entry.contains("latest"));
            assert!(!entry.contains("sudo"));
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod native_shell_tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    #[test]
    fn real_shell_preserves_brew_operations_arguments_environment_and_exit() {
        let root = std::env::temp_dir().join(format!("rss-brew-wrapper-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let manager = root.join("fake manager");
        let wrapper = root.join("wrapper.sh");
        fs::write(&wrapper, BREW).unwrap();
        fs::write(&manager, "#!/bin/sh\nprintf '%s\\n' \"$@\"\nprintf '%s' \"$HOMEBREW_NO_AUTO_UPDATE:$HOMEBREW_NO_INSTALL_UPGRADE\"\nexit 23\n").unwrap();
        fs::set_permissions(&manager, fs::Permissions::from_mode(0o700)).unwrap();
        for (operation, verb, target) in [
            ("install", "install", "/exact package; $(literal)"),
            ("upgrade", "reinstall", "/exact package; $(literal)"),
            ("downgrade", "reinstall", "/exact package; $(literal)"),
            ("uninstall", "uninstall", "tap/name@1"),
        ] {
            let output = Command::new("/bin/sh")
                .arg(&wrapper)
                .args([
                    manager.to_str().unwrap(),
                    "/exact package; $(literal)",
                    operation,
                    "fixed-tap",
                    "tap/name@1",
                    "1",
                    "arm64",
                ])
                .output()
                .unwrap();
            if unsafe { libc::geteuid() } == 0 {
                assert_eq!(output.status.code(), Some(77));
            } else {
                assert_eq!(output.status.code(), Some(23));
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap(),
                    format!("{verb}\n--formula\n{target}\n1:1")
                );
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pkg_wrapper_uses_verifier_before_exact_manager_and_propagates_failures() {
        let root = std::env::temp_dir().join(format!("rss-pkg-wrapper-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let manager = root.join("manager");
        let verifier = root.join("verifier");
        fs::write(&manager, "#!/bin/sh\nprintf '%s\\n' \"$@\"\nexit 19\n").unwrap();
        fs::set_permissions(&manager, fs::Permissions::from_mode(0o700)).unwrap();
        for verifier_exit in [0, 42] {
            fs::write(
                &verifier,
                format!(
                    "#!/bin/sh\n[ \"$1\" = --check-signature ] || exit 99\nexit {verifier_exit}\n"
                ),
            )
            .unwrap();
            fs::set_permissions(&verifier, fs::Permissions::from_mode(0o700)).unwrap();
            for operation in ["install", "upgrade", "downgrade", "uninstall"] {
                let output = Command::new("/bin/sh")
                    .arg("-c")
                    .arg(PKG)
                    .arg("wrapper")
                    .args([
                        manager.to_str().unwrap(),
                        "/exact payload.pkg",
                        operation,
                        "source",
                        "resource",
                        "1",
                        "arm64",
                        verifier.to_str().unwrap(),
                    ])
                    .output()
                    .unwrap();
                let expected = if operation == "uninstall" {
                    64
                } else if verifier_exit != 0 {
                    verifier_exit
                } else {
                    19
                };
                assert_eq!(output.status.code(), Some(expected));
                if expected == 19 {
                    assert_eq!(output.stdout, b"-pkg\n/exact payload.pkg\n-target\n/\n");
                } else {
                    assert!(output.stdout.is_empty());
                }
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pkg_unsigned_payload_never_reaches_manager() {
        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(PKG)
            .arg("wrapper")
            .args([
                "/usr/bin/true",
                "/nonexistent/rss-unsigned.pkg",
                "install",
                "source",
                "resource",
                "version",
                "arm64",
                "/usr/sbin/pkgutil",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}

#[cfg(all(test, windows))]
mod native_powershell_tests {
    use super::*;
    use std::{fs, process::Command};
    #[test]
    fn fixed_msi_and_winget_wrappers_preserve_manager_exit_and_fixed_arguments() {
        let root = std::env::temp_dir().join(format!("rss-manager-wrapper-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let manager = root.join("manager.cmd");
        let script = root.join("entry.ps1");
        for (wrapper, expected_install, expected_uninstall) in [
            (MSI, "/i", "/x"),
            (WINGET, "install --manifest", "uninstall --id"),
        ] {
            // Only this interpreter harness replaces the OS signature command; production
            // still invokes the unmodified wrapper and Get-AuthenticodeSignature.
            fs::write(&script, format!("function Get-AuthenticodeSignature {{ param($LiteralPath) @{{Status='Valid'}} }}\n& {{ {wrapper} }} @args")).unwrap();
            for code in [0, 3010, 1641, 37] {
                fs::write(
                    &manager,
                    format!("@echo off\r\necho %*\r\nexit /b {code}\r\n"),
                )
                .unwrap();
                for operation in ["install", "upgrade", "downgrade", "uninstall"] {
                    let output = Command::new(
                        std::env::var_os("RSS_TEST_PWSH7")
                            .unwrap_or_else(|| r"C:\Program Files\PowerShell\7\pwsh.exe".into()),
                    )
                    .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
                    .arg(&script)
                    .arg(&manager)
                    .args([
                        "C:\\exact package",
                        operation,
                        "fixed-source",
                        "fixed-id",
                        "1",
                        "x64",
                    ])
                    .output()
                    .unwrap();
                    assert_eq!(output.status.code(), Some(code));
                    let text = String::from_utf8_lossy(&output.stdout);
                    assert!(text.contains(if operation == "uninstall" {
                        expected_uninstall
                    } else {
                        expected_install
                    }));
                    if wrapper == WINGET && operation == "uninstall" {
                        assert!(text.contains("--source fixed-source"));
                    }
                }
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
