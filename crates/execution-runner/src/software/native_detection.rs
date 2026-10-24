use execution_contract::*;

fn unknown() -> SoftwareState {
    SoftwareState::Unknown {
        reason: SoftwareDetectionFailure::Unavailable,
    }
}
pub(crate) fn detect(detector: &SoftwareDetector, run_as: &RunAs) -> SoftwareState {
    let _ = run_as;
    match detector {
        #[cfg(target_os = "macos")]
        SoftwareDetector::PkgReceipt { receipt, .. } => pkg(receipt).unwrap_or_else(unknown),
        #[cfg(windows)]
        SoftwareDetector::MsiProduct { product_code, .. } => {
            msi(product_code, run_as).unwrap_or_else(unknown)
        }
        _ => unknown(),
    }
}
#[cfg(target_os = "macos")]
fn pkg(receipt: &str) -> Option<SoftwareState> {
    use std::{io::Read, os::unix::fs::MetadataExt};
    if receipt.is_empty()
        || !receipt
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
    {
        return None;
    }
    let root = std::path::Path::new("/private/var/db/receipts");
    crate::platform::open_directory(root).ok()?;
    let path = root.join(format!("{receipt}.plist"));
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Some(SoftwareState::Absent {})
        }
        Err(_) => return None,
        Ok(_) => (),
    }
    let mut file = crate::platform::open_file(&path).ok()?;
    let before = file.metadata().ok()?;
    if before.len() > 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let after = file.metadata().ok()?;
    let current = std::fs::symlink_metadata(&path).ok()?;
    if bytes.len() > 1024 * 1024
        || before.len() != after.len()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.mtime() != after.mtime()
        || current.dev() != before.dev()
        || current.ino() != before.ino()
    {
        return None;
    }
    let value = plist::Value::from_reader(std::io::Cursor::new(bytes)).ok()?;
    let dictionary = value.as_dictionary()?;
    if dictionary.get("PackageIdentifier")?.as_string()? != receipt {
        return None;
    }
    Some(SoftwareState::Present {
        version: PackageValue::new(dictionary.get("PackageVersion")?.as_string()?).ok()?,
    })
}
#[cfg(windows)]
fn msi(product: &str, run_as: &RunAs) -> Option<SoftwareState> {
    use windows_sys::Win32::System::ApplicationInstallationAndServicing::*;
    if product.len() != 38 || !product.starts_with('{') || !product.ends_with('}') {
        return None;
    }
    let product = crate::windows::wide(product);
    let property = crate::windows::wide("VersionString");
    let query = |sid: Option<&str>, context| -> Option<SoftwareState> {
        let sid = sid.map(crate::windows::wide);
        let mut buffer = [0u16; 1025];
        let mut length = 1024u32;
        let status = unsafe {
            MsiGetProductInfoExW(
                product.as_ptr(),
                sid.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                context,
                property.as_ptr(),
                buffer.as_mut_ptr(),
                &mut length,
            )
        };
        match status {
            0 if length <= 1024 => Some(SoftwareState::Present {
                version: PackageValue::new(String::from_utf16(&buffer[..length as usize]).ok()?)
                    .ok()?,
            }),
            1605 => Some(SoftwareState::Absent {}),
            _ => None,
        }
    };
    match run_as {
        RunAs::System { .. } => query(None, MSIINSTALLCONTEXT_MACHINE),
        RunAs::User { account } => {
            let managed = query(
                Some(account.subject.as_str()),
                MSIINSTALLCONTEXT_USERMANAGED,
            )?;
            let unmanaged = query(
                Some(account.subject.as_str()),
                MSIINSTALLCONTEXT_USERUNMANAGED,
            )?;
            match (managed, unmanaged) {
                (state, SoftwareState::Absent {}) | (SoftwareState::Absent {}, state) => {
                    Some(state)
                }
                _ => None,
            }
        }
    }
}
// ref: https://learn.microsoft.com/en-us/windows/win32/msi/productversion
// Native MSI ordering is not generalized to Homebrew, PKG, or arbitrary Bundle versions.
pub(crate) fn not_downgrade(step: &SoftwareProgramStep, state: &SoftwareState) -> bool {
    let SoftwareState::Present { version } = state else {
        return false;
    };
    if version == &step.version {
        return true;
    }
    // The authenticated native adapter checks Homebrew Version ordering in Prepare. WinGet's
    // upgrade workflow checks its own native ordering before invoking an installer. A separate
    // removal would bypass that guard, so it still requires a detector with known ordering below.
    if matches!(step.format, SoftwareFormat::Homebrew { .. })
        || (matches!(step.format, SoftwareFormat::Winget { .. })
            && matches!(
                step.upgrade,
                execution_contract::SoftwareUpgrade::InPlace { .. }
            ))
    {
        return true;
    }
    if let SoftwareFormat::Msix { identity, .. } = &step.format {
        let old = version
            .as_str()
            .split('.')
            .map(str::parse::<u16>)
            .collect::<Result<Vec<_>, _>>();
        return old
            .ok()
            .and_then(|v| <[u16; 4]>::try_from(v).ok())
            .is_some_and(|old| old <= identity.version);
    }
    if matches!(
        step.format,
        SoftwareFormat::DmgApp { .. } | SoftwareFormat::Exe { .. }
    ) {
        let parse = |v: &str| -> Option<[u64; 4]> {
            let fields = v.split('.').collect::<Vec<_>>();
            if fields.is_empty()
                || fields.len() > 4
                || (matches!(step.format, SoftwareFormat::DmgApp { .. }) && fields.len() != 3)
            {
                return None;
            }
            let mut values = [0; 4];
            for (index, field) in fields.iter().enumerate() {
                if field.is_empty() || !field.bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                values[index] = field.parse().ok()?;
            }
            Some(values)
        };
        return matches!((parse(version.as_str()),parse(step.version.as_str())),(Some(old),Some(new)) if old <= new);
    }
    if !matches!(step.detection, SoftwareDetector::MsiProduct { .. }) {
        return false;
    }
    fn msi_version(value: &str) -> Option<(u8, u8, u16)> {
        let fields = value.split('.').collect::<Vec<_>>();
        if !(3..=4).contains(&fields.len())
            || fields
                .iter()
                .any(|v| v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        Some((
            fields[0].parse().ok()?,
            fields[1].parse().ok()?,
            fields[2].parse().ok()?,
        ))
    }
    match (
        msi_version(version.as_str()),
        msi_version(step.version.as_str()),
    ) {
        (Some(old), Some(new)) => old <= new,
        _ => false,
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;
    #[test]
    fn exe_numeric_versions_support_upgrade_without_permitting_downgrade_or_guessing_labels() {
        let input: execution_contract::ExecutionInput = serde_json::from_str(include_str!(
            "../../../execution-contract/tests/fixtures/software.json"
        ))
        .unwrap();
        let mut step = input.execution.software_program().unwrap().steps[0].clone();
        step.format = SoftwareFormat::Exe {
            layout: Default::default(),
        };
        step.version = PackageValue::new("26.05").unwrap();
        for (installed, expected) in [
            ("26.04", true),
            ("26.05.0.0", true),
            ("26.06", false),
            ("preview.26", false),
            ("26..04", false),
        ] {
            let state = SoftwareState::Present {
                version: PackageValue::new(installed).unwrap(),
            };
            assert_eq!(not_downgrade(&step, &state), expected, "{installed}");
        }
    }
}
