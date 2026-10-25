//! Current-user native diagnostics, never a device-key or application WebAuthn proof.
//! ref: MicrosoftDocs/entra-docs identity/devices/troubleshoot-device-dsregcmd.md.
use execution_ipc::device_access::WindowsHelloUser;
use rss_mdm_agent_wire::runtime::AccessObservation as O;

fn parse(bytes: &[u8], now: i64) -> WindowsHelloUser {
    let mut result = WindowsHelloUser {
        registration: O::Unknown,
        key_id: None,
        hardware_key: O::Unknown,
        observed_at: now,
    };
    if bytes.len() > 32768 || now <= 0 {
        return result;
    }
    let text = String::from_utf8_lossy(bytes);
    let mut user = false;
    let mut sections = 0;
    let mut set = None;
    let mut key = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('|') && line.ends_with('|') {
            user = line.trim_matches('|').trim() == "User State";
            if user {
                sections += 1;
            }
        }
        if !user {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            let slot = match name.trim() {
                "NgcSet" => &mut set,
                "NgcKeyId" => &mut key,
                _ => continue,
            };
            if slot.replace(value.trim()).is_some() {
                return result;
            }
        }
    }
    if sections != 1 {
        return result;
    }
    if set == Some("NO") && key.is_none() {
        result.registration = O::Absent;
    }
    if set == Some("YES") {
        result.key_id = key
            .and_then(|v| v.trim_matches(['{', '}']).parse().ok())
            .filter(|v: &uuid::Uuid| !v.is_nil());
        if result.key_id.is_some() {
            result.registration = O::Present;
        }
    }
    // TpmProtected refers to the device registration key, not NgcKeyId.
    // No public, verified per-key Passport mapping is available in this profile.
    // Do not infer hardware provenance from TPM availability or policy configuration.
    result
}
#[cfg(windows)]
fn system_root() -> Result<std::path::PathBuf, execution_app::Error> {
    let mut text = [0u16; 32768];
    let n = unsafe {
        windows_sys::Win32::System::SystemInformation::GetWindowsDirectoryW(
            text.as_mut_ptr(),
            text.len() as u32,
        )
    } as usize;
    if n == 0 || n >= text.len() {
        return Err(execution_app::Error::Unavailable);
    }
    Ok(String::from_utf16(&text[..n])
        .map_err(|_| execution_app::Error::Unavailable)?
        .into())
}
#[cfg(windows)]
pub(super) fn observe() -> Result<WindowsHelloUser, execution_app::Error> {
    use execution_app::Error;
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    let root = system_root()?;
    let mut command = Command::new(root.join("System32/dsregcmd.exe"));
    command
        .arg("/status")
        .env_clear()
        .env("SystemRoot", &root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut scope =
        native_process::OwnedHost::prepare(&mut command).map_err(|_| Error::Unavailable)?;
    let mut child = command.spawn().map_err(|_| Error::Unavailable)?;
    if scope.attach(child.id()).is_err() {
        let _ = child.kill();
        return Err(Error::Unavailable);
    }
    let output = child.stdout.take().ok_or(Error::Unavailable)?;
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        let result = output.take(32769).read_to_end(&mut bytes).map(|_| bytes);
        let _ = send.send(result);
    });
    let end = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(20)),
            _ => break None,
        }
    };
    let _ = scope.terminate();
    let bytes = receive
        .recv_timeout(Duration::from_millis(500))
        .map_err(|_| Error::Unavailable)?
        .map_err(|_| Error::Unavailable)?;
    if !status.is_some_and(|s| s.success()) {
        return Err(Error::Unavailable);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Unavailable)?
        .as_secs()
        .try_into()
        .map_err(|_| Error::Unavailable)?;
    Ok(parse(&bytes, now))
}
#[cfg(windows)]
pub(super) fn open_settings() -> Result<(), execution_app::Error> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let verb: Vec<u16> = "open".encode_utf16().chain([0]).collect();
    let uri: Vec<u16> = "ms-settings:signinoptions"
        .encode_utf16()
        .chain([0])
        .collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            uri.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err(execution_app::Error::Unavailable)
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_tpm_is_not_user_key_provenance() {
        let sample=b"| Device Details |\n TpmProtected : YES\n| User State |\n NgcSet : YES\n NgcKeyId : {00000000-0000-0000-0000-000000000042}\n| SSO State |\n";
        let value = parse(sample, 100);
        assert_eq!(value.registration, O::Present);
        assert!(value.key_id.is_some());
        assert_eq!(value.hardware_key, O::Unknown);
        assert_eq!(
            parse(b"| Device Details |\n NgcSet : NO\n", 100).registration,
            O::Unknown
        );
        assert_eq!(
            parse(b"| User State |\n NgcSet : NO\n", 100).registration,
            O::Absent
        );
        assert_eq!(
            parse(b"| User State |\n NgcSet : YES\n", 100).registration,
            O::Unknown
        );
        assert_eq!(
            parse(b"| User State |\n NgcSet : NO\n NgcSet : YES\n", 100).registration,
            O::Unknown
        );
    }
}
