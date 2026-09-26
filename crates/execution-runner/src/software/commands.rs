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
/usr/sbin/pkgutil --check-signature "$payload" >/dev/null
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
            assert_eq!(args.len(), 7);
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
        assert!(PKG.contains("pkgutil --check-signature"));
        assert!(BREW.contains("id -u"));
        assert!(BREW.contains("HOMEBREW_NO_AUTO_UPDATE=1"));
        assert!(BREW.contains("reinstall --formula"));
        for entry in [MSI, WINGET, PKG, BREW] {
            assert!(!entry.contains("latest"));
            assert!(!entry.contains("sudo"));
        }
    }
}
