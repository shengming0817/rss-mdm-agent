//! Native platform bindings; commands have fixed images and code-selected arguments.
//! ref: Apple PowerManagement/pmset/pmset.m@d415e45501842834a280930c3eed9186544a67f0.
use super::*;
use serde_json::{json, Value};
unsafe extern "C" {
    fn rss_settings_call(
        bytes: *const u8,
        length: usize,
        output: *mut *mut u8,
        size: *mut usize,
    ) -> i32;
}
fn call(value: Value) -> Result<Value, Error> {
    #[cfg(test)]
    let call_kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let bytes = serde_json::to_vec(&value).map_err(|_| Error::Protocol)?;
    let mut output = std::ptr::null_mut();
    let mut size = 0;
    // SAFETY: native owns a bounded malloc buffer, copied and freed exactly once here.
    let code = unsafe { rss_settings_call(bytes.as_ptr(), bytes.len(), &mut output, &mut size) };
    if output.is_null() {
        return Err(Error::Unavailable);
    }
    let value = if size <= 256 * 1024 {
        serde_json::from_slice(unsafe { std::slice::from_raw_parts(output, size) })
            .map_err(|_| Error::Protocol)
    } else {
        Err(Error::Capacity)
    };
    unsafe {
        libc::free(output.cast());
    }
    if code != 0 {
        #[cfg(test)]
        eprintln!(
            "native_settings: kind={} section=native code={code}",
            call_kind
        );
        return Err(match code {
            2 => Error::Denied,
            3 => Error::Conflict,
            4 => Error::Unsupported,
            _ => Error::Unavailable,
        });
    }
    value
}
pub(super) fn read(section: SettingsSection) -> Result<Vec<SettingsTarget>, Error> {
    if matches!(
        section,
        SettingsSection::About | SettingsSection::Storage | SettingsSection::Security
    ) {
        return Err(Error::Unsupported);
    }
    let mut targets: Vec<SettingsTarget> =
        if matches!(section, SettingsSection::Power | SettingsSection::Lock) {
            power(section)?
        } else {
            serde_json::from_value(call(json!({"kind":"read","section":section}))?)
                .map_err(|_| Error::Protocol)?
        };
    if matches!(
        section,
        SettingsSection::Network
            | SettingsSection::Display
            | SettingsSection::Power
            | SettingsSection::Lock
    ) {
        // The public preference APIs identify forced keys, but cannot prove that an MDM
        // payload outside those domains leaves a field editable. Unknown stays read-only.
        let unforced = command_image(
            "/usr/bin/profiles",
            &["status".into(), "-type".into(), "enrollment".into()],
        )
        .is_ok_and(|text| crate::mdm_state::macos_enrollment_flag(&text) == Some(false));
        if !unforced {
            for target in &mut targets {
                if target.writable {
                    target.writable = false;
                    target.reason = Some("无法确认此设置未受组织管理，请使用系统设置".into());
                }
            }
        }
    }
    Ok(targets)
}
pub(super) fn open(section: SettingsSection) -> Result<(), Error> {
    call(json!({"kind":"open","section":section})).map(|_| ())
}
pub(super) fn apply(patch: &SettingsPatch, before: &SettingsSnapshot) -> Result<(), Error> {
    if let SettingsPatch::Power {
        target,
        display_off_minutes,
        sleep_minutes,
        mode,
    } = patch
    {
        if power(SettingsSection::Power)? != before.targets {
            return Err(Error::Conflict);
        }
        let mut args = vec![if target == "ac" {
            "-c".into()
        } else {
            "-b".into()
        }];
        if let Some(n) = display_off_minutes {
            args.extend(["displaysleep".into(), n.to_string()]);
        }
        if let Some(n) = sleep_minutes {
            args.extend(["sleep".into(), n.to_string()]);
        }
        if let Some(mode) = mode {
            args.extend([
                "lowpowermode".into(),
                match mode.as_str() {
                    "normal" => "0",
                    "low" => "1",
                    _ => return Err(Error::Unsupported),
                }
                .into(),
            ]);
        }
        command(&args)?;
        return Ok(());
    }
    call(json!({"kind":"apply","patch":patch,"before":before.targets})).map(|_| ())
}
pub(super) fn finish_display(
    patch: &SettingsPatch,
    before: &SettingsSnapshot,
    keep: bool,
) -> Result<(), Error> {
    call(json!({"kind":if keep {"keepDisplay"} else {"restoreDisplay"},"patch":patch,"before":before.targets})).map(|_| ())
}
pub(super) fn preserve_display(actual: &SettingsSnapshot) -> Result<(), Error> {
    call(json!({"kind":"preserveDisplay","before":actual.targets})).map(|_| ())
}
fn command(args: &[String]) -> Result<String, Error> {
    command_image("/usr/bin/pmset", args)
}
fn command_image(image: &'static str, args: &[String]) -> Result<String, Error> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    installation_security::protected(Path::new(image)).map_err(|_| Error::Untrusted)?;
    let mut child = Command::new(image)
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let count = Arc::new(AtomicUsize::new(0));
    fn drain(
        mut source: impl Read + Send + 'static,
        count: Arc<AtomicUsize>,
    ) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>> {
        std::thread::spawn(move || {
            let mut result = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let n = source.read(&mut buffer)?;
                if n == 0 {
                    return Ok(result);
                }
                let total = count.fetch_add(n, Ordering::Relaxed);
                if total + n <= 65536 {
                    result.extend_from_slice(&buffer[..n]);
                }
            }
        })
    }
    let stdout = drain(
        child.stdout.take().ok_or(Error::Unavailable)?,
        count.clone(),
    );
    let stderr = drain(
        child.stderr.take().ok_or(Error::Unavailable)?,
        count.clone(),
    );
    let cutoff = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if std::time::Instant::now() >= cutoff || count.load(Ordering::Relaxed) > 65536 {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let out = stdout.join().map_err(|_| Error::Unavailable)??;
    let err = stderr.join().map_err(|_| Error::Unavailable)??;
    observe_native(count.load(Ordering::Relaxed) as u64);
    if count.load(Ordering::Relaxed) > 65536 {
        return Err(Error::Capacity);
    }
    if !status.is_some_and(|s| s.success()) || !err.is_empty() {
        return Err(Error::Unavailable);
    }
    String::from_utf8(out).map_err(|_| Error::Protocol)
}
fn field(key: &str, value: Value) -> SettingsField {
    SettingsField {
        key: key.into(),
        value: Some(value),
        quality: DataQuality::Complete,
        reason: None,
    }
}
fn power(section: SettingsSection) -> Result<Vec<SettingsTarget>, Error> {
    let text = command(&["-g".into(), "custom".into()])?;
    let forced = call(json!({"kind":"powerManaged"}))?
        .as_bool()
        .ok_or(Error::Protocol)?;
    let mut targets = Vec::new();
    let mut current = None;
    for line in text.lines() {
        match line.trim() {
            "Battery Power:" => {
                targets.push(SettingsTarget {
                    id: "dc".into(),
                    name: "电池供电".into(),
                    fields: vec![],
                    options: vec![],
                    writable: !forced,
                    reason: forced.then(|| "由组织策略管理".into()),
                });
                current = Some(targets.len() - 1);
                continue;
            }
            "AC Power:" => {
                targets.push(SettingsTarget {
                    id: "ac".into(),
                    name: "外接电源".into(),
                    fields: vec![],
                    options: vec![],
                    writable: !forced,
                    reason: forced.then(|| "由组织策略管理".into()),
                });
                current = Some(targets.len() - 1);
                continue;
            }
            _ => {}
        }
        let Some(index) = current else {
            continue;
        };
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        let Some(n) = parts.next().and_then(|n| n.parse::<u16>().ok()) else {
            continue;
        };
        match key {
            "displaysleep" => targets[index]
                .fields
                .push(field("displayOffMinutes", json!(n))),
            "sleep" => targets[index].fields.push(field("sleepMinutes", json!(n))),
            "lowpowermode" if n <= 1 => {
                targets[index]
                    .fields
                    .push(field("mode", json!(if n == 1 { "low" } else { "normal" })));
                targets[index].options = vec![
                    SettingsOption {
                        id: "normal".into(),
                        label: "正常".into(),
                    },
                    SettingsOption {
                        id: "low".into(),
                        label: "低电量".into(),
                    },
                ];
            }
            _ => {}
        }
    }
    if targets.is_empty() {
        return Err(Error::Unavailable);
    }
    let mut source: Vec<SettingsTarget> =
        serde_json::from_value(call(json!({"kind":"read","section":"power"}))?)
            .map_err(|_| Error::Protocol)?;
    targets.append(&mut source);
    if section == SettingsSection::Lock {
        let mut extra: Vec<SettingsTarget> =
            serde_json::from_value(call(json!({"kind":"read","section":"lock"}))?)
                .map_err(|_| Error::Protocol)?;
        targets.append(&mut extra);
    }
    Ok(targets)
}
