//! Current prerequisite evidence, never inferred from an earlier inventory report.
//! ref: PowerShell 5a6ecbf8 src/System.Management.Automation/engine/PSVersionInfo.cs.
use crate::Interpreter;
use agent_client::Error;
use rss_mdm_agent_wire::{runtime::ExecutorProfile, CollectionEnvironment};
use std::{process::Stdio, time::Duration};
use tokio::io::AsyncReadExt;

pub(crate) async fn observe(interpreters: &[Interpreter]) -> Result<CollectionEnvironment, Error> {
    let mut environment = CollectionEnvironment {
        native_engines: crate::collection_worker::engines(),
        edition: edition(),
        tools: Default::default(),
    };
    for interpreter in interpreters {
        let (name, args): (&str, &[&str]) = match interpreter.profile {
            ExecutorProfile::PowerShell7 => (
                "powershell",
                &[
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "$PSVersionTable.PSVersion.ToString()",
                ],
            ),
            ExecutorProfile::Osquery => ("osquery", &["--version"]),
            _ => continue,
        };
        if interpreter.image.verify(&interpreter.image.path).is_err() {
            continue;
        }
        if let Some(version) = probe(&interpreter.image.path, args, name).await {
            if interpreter.image.verify(&interpreter.image.path).is_ok() {
                environment.tools.insert(name.into(), version);
            }
        }
    }
    Ok(environment)
}

async fn probe(path: &std::path::Path, args: &[&str], name: &str) -> Option<String> {
    let mut child = tokio::process::Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        let mut bytes = Vec::new();
        stdout.take(257).read_to_end(&mut bytes).await.ok()?;
        if bytes.len() > 256 {
            return None;
        }
        if !child.wait().await.ok()?.success() {
            return None;
        }
        version(&bytes, name)
    })
    .await
    .ok()
    .flatten();
    // Also reap oversized and timed-out probes. Unknown prerequisites remain absent.
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill().await;
    }
    result
}
fn version(bytes: &[u8], name: &str) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?.trim();
    let value = if name == "osquery" {
        // Official macOS packages pin the real osqueryd image; osqueryi may be a link.
        text.strip_prefix("osqueryi version ")
            .or_else(|| text.strip_prefix("osqueryd version "))?
    } else {
        text
    };
    let evidence = CollectionEnvironment {
        native_engines: Default::default(),
        edition: None,
        tools: [(name.into(), value.into())].into(),
    };
    evidence.validate().ok()?;
    Some(value.into())
}
#[cfg(not(windows))]
fn edition() -> Option<String> {
    None
}
#[cfg(windows)]
fn edition() -> Option<String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Registry::*;
    let key = HSTRING::from(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let name = HSTRING::from("EditionID");
    let mut buffer = [0u16; 129];
    let mut bytes = (buffer.len() * 2) as u32;
    // SAFETY: fixed read-only registry coordinates and a bounded writable UTF-16 buffer.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
        .ok()
        .ok()?;
    }
    if bytes < 2 || bytes as usize > buffer.len() * 2 || bytes % 2 != 0 {
        return None;
    }
    let end = bytes as usize / 2 - 1;
    if buffer[end] != 0 {
        return None;
    }
    let value = String::from_utf16(&buffer[..end]).ok()?;
    CollectionEnvironment {
        native_engines: Default::default(),
        edition: Some(value.clone()),
        tools: Default::default(),
    }
    .validate()
    .ok()?;
    Some(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_require_numeric_observed_output() {
        assert_eq!(
            version(b"7.4.13\n", "powershell").as_deref(),
            Some("7.4.13")
        );
        assert_eq!(
            version(b"osqueryi version 5.23.1\n", "osquery").as_deref(),
            Some("5.23.1")
        );
        for bad in [
            b"".as_slice(),
            b"7.4-preview",
            b"7.4\nextra",
            b"4294967296",
            b"7.4.1.2.3",
        ] {
            assert!(version(bad, "powershell").is_none());
        }
    }
    #[test]
    fn official_osquery_daemon_and_shell_share_the_same_tool_prerequisite() {
        assert_eq!(
            version(b"osqueryd version 5.23.1\n", "osquery").as_deref(),
            Some("5.23.1")
        );
        assert_eq!(
            version(b"osqueryi version 5.23.1\n", "osquery").as_deref(),
            Some("5.23.1")
        );
        for value in [
            "osquery version 5.23.1",
            "osqueryd version 5.23.1-preview",
            "osqueryd version 5.23.1\nextra",
            "5.23.1",
        ] {
            assert!(version(value.as_bytes(), "osquery").is_none());
        }
    }
}
