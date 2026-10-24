//! Native registration observation shared by Inventory and settings; it never initiates enrollment.
use rss_mdm_agent_wire::runtime::MdmEnrollmentState as State;

pub(crate) fn classify(enrolled: bool, actual: Option<&str>, expected: Option<&str>) -> State {
    if !enrolled {
        return State::Unenrolled;
    }
    let (Some(actual), Some(expected)) = (actual, expected) else {
        return State::Unknown;
    };
    let (Ok(actual), Ok(expected)) = (url::Url::parse(actual), url::Url::parse(expected)) else {
        return State::Unknown;
    };
    if actual.scheme() != "https" || expected.scheme() != "https" {
        return State::Unknown;
    }
    if actual.origin() == expected.origin() {
        State::ThisOrganization
    } else {
        State::OtherOrganization
    }
}
pub(crate) async fn observe(expected: Option<&str>) -> State {
    #[cfg(target_os = "macos")]
    {
        macos(expected).await.unwrap_or(State::Unknown)
    }
    #[cfg(windows)]
    {
        windows(expected).unwrap_or(State::Unknown)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = expected;
        State::Unknown
    }
}
#[cfg(target_os = "macos")]
async fn profiles(args: &[&str]) -> Option<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut child = tokio::process::Command::new("/usr/bin/profiles")
        .args(args)
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let output = child.stdout.take()?;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        output
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .await
            .ok()?;
        if bytes.len() > 1024 * 1024 {
            return None;
        }
        if !child.wait().await.ok()?.success() {
            return None;
        }
        Some(bytes)
    })
    .await
    .ok()
    .flatten()
}
#[cfg(any(test, target_os = "macos"))]
pub(crate) fn macos_enrollment_flag(status: &str) -> Option<bool> {
    let values = status
        .lines()
        .filter_map(|line| {
            let value = line.trim().strip_prefix("MDM enrollment: ")?;
            if value == "No" {
                Some(false)
            } else if value == "Yes" || value.starts_with("Yes (") && value.ends_with(')') {
                Some(true)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if values.len() == 1 {
        Some(values[0])
    } else {
        None
    }
}
#[cfg(target_os = "macos")]
async fn macos(expected: Option<&str>) -> Option<State> {
    let status = profiles(&["status", "-type", "enrollment"]).await?;
    let status = std::str::from_utf8(&status).ok()?;
    if !macos_enrollment_flag(status)? {
        return Some(State::Unenrolled);
    }
    let expected = expected?;
    let document = profiles(&["show", "-type", "configuration", "-output", "stdout-xml"]).await?;
    let value = plist::Value::from_reader_xml(document.as_slice()).ok()?;
    fn find(value: &plist::Value) -> Option<&str> {
        match value {
            plist::Value::Dictionary(d) => {
                if d.get("PayloadType").and_then(plist::Value::as_string) == Some("com.apple.mdm") {
                    return d.get("ServerURL").and_then(plist::Value::as_string);
                }
                d.values().find_map(find)
            }
            plist::Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    Some(classify(true, find(&value), Some(expected)))
}
#[cfg(windows)]
fn windows(expected: Option<&str>) -> Option<State> {
    // ref: chromium chrome/credential_provider/gaiacp/mdm_utils.cc GetDeviceRegistrationInfo.
    use windows::Win32::Management::MobileDeviceManagementRegistration::{
        DeviceRegistrationBasicInfo, GetDeviceRegistrationInfo, MANAGEMENT_REGISTRATION_INFO,
    };
    struct Allocation(*mut std::ffi::c_void);
    impl Drop for Allocation {
        fn drop(&mut self) {
            unsafe {
                windows_sys::Win32::System::Memory::HeapFree(
                    windows_sys::Win32::System::Memory::GetProcessHeap(),
                    0,
                    self.0,
                );
            }
        }
    }
    let mut pointer = std::ptr::null_mut();
    unsafe { GetDeviceRegistrationInfo(DeviceRegistrationBasicInfo, &mut pointer) }.ok()?;
    if pointer.is_null() {
        return None;
    }
    let allocation = Allocation(pointer);
    let info = unsafe { &*allocation.0.cast::<MANAGEMENT_REGISTRATION_INFO>() };
    let actual = if info.pszMDMServiceUri.is_null() {
        None
    } else {
        unsafe { info.pszMDMServiceUri.to_string() }.ok()
    };
    Some(classify(
        info.fDeviceRegisteredWithManagement.as_bool(),
        actual.as_deref(),
        expected,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absence_and_organization_are_observed_not_assumed() {
        assert_eq!(classify(false, None, None), State::Unenrolled);
        assert_eq!(
            classify(true, None, Some("https://ours.test")),
            State::Unknown
        );
        assert_eq!(
            classify(true, Some("https://ours.test/mdm"), None),
            State::Unknown
        );
        assert_eq!(
            classify(
                true,
                Some("https://ours.test/mdm"),
                Some("https://ours.test/enroll")
            ),
            State::ThisOrganization
        );
        assert_eq!(
            classify(
                true,
                Some("https://other.test/mdm"),
                Some("https://ours.test")
            ),
            State::OtherOrganization
        );
    }
}
