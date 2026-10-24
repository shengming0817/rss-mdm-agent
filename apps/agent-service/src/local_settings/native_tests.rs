//! Explicit current-host component acceptance; never part of unattended CI.
use super::*;
use serde_json::{json, Value};
fn field<'a>(target: &'a SettingsTarget, key: &str) -> &'a Value {
    target
        .fields
        .iter()
        .find(|f| f.key == key)
        .unwrap()
        .value
        .as_ref()
        .unwrap()
}
fn display_change(before: &SettingsSnapshot) -> SettingsPatch {
    let target = before
        .targets
        .iter()
        .find(|t| t.writable && t.options.len() > 1)
        .expect("requires an unmanaged interactive display with alternate modes");
    let mode = target
        .options
        .iter()
        .find(|m| Some(m.id.as_str()) != field(target, "mode").as_str())
        .unwrap()
        .id
        .clone();
    SettingsPatch::Display {
        target: target.id.clone(),
        mode: Some(mode),
        origin: None,
    }
}
struct DisplayRestore(SettingsSnapshot, SettingsPatch, bool);
impl Drop for DisplayRestore {
    fn drop(&mut self) {
        let _ = macos::finish_display(&self.1, &self.0, false);
        if self.2 {
            let target = match &self.1 {
                SettingsPatch::Display { target, .. } => target,
                _ => return,
            };
            if let Some(original) = self.0.targets.iter().find(|t| &t.id == target) {
                let patch = SettingsPatch::Display {
                    target: target.clone(),
                    mode: field(original, "mode").as_str().map(str::to_owned),
                    origin: serde_json::from_value(field(original, "origin").clone()).ok(),
                };
                let _ = macos::finish_display(&patch, &self.0, true);
            }
        }
    }
}

struct SettingRestore(SettingsSection, SettingsPatch);
impl Drop for SettingRestore {
    fn drop(&mut self) {
        let restored = snapshot(self.0).and_then(|current| {
            if satisfied(&self.1, &current) {
                Ok(())
            } else {
                macos::apply(&self.1, &current)
            }
        });
        if let Err(error) = restored {
            eprintln!("native_setting_restore_failed: {error:?}");
        }
    }
}
#[test]
#[ignore = "changes an actual display for 15 seconds; run explicitly in the logged-in macOS session"]
fn display_worker_times_out_without_ui_and_native_keep_reads_back() {
    let before = snapshot(SettingsSection::Display).unwrap();
    let patch = display_change(&before);
    let mut restore = DisplayRestore(before.clone(), patch.clone(), false);
    let account = OsAccountRef {
        platform: Platform::Macos,
        subject: Id::new(execution_ipc::host::current_subject().unwrap()).unwrap(),
    };
    let input = Input {
        request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        action: Action::Change {
            expected: before.revision.clone(),
            before: Box::new(before.clone()),
            patch: patch.clone(),
        },
        run_as: RunAs::User {
            account: account.clone(),
        },
        session: SessionRequirement::ActiveUser {
            account,
            session: execution_ipc::host::current_session_binding().unwrap(),
        },
        confirm: PathBuf::from("/var/empty/rss-no-display-confirmation"),
        finish_by_unix_ms: now().unwrap() + 25_000,
    };
    let started = std::time::Instant::now();
    let mut rows = Vec::new();
    let mut unsettled = false;
    execute(&input, &mut unsettled, &mut |r| {
        eprintln!("display_native_effect={:?}", r.effect);
        rows.push(r);
        Ok(())
    })
    .unwrap();
    assert_eq!(rows.first().unwrap().effect, SettingsEffect::Preview);
    assert_eq!(rows.last().unwrap().effect, SettingsEffect::Restored);
    assert!(!unsettled);
    assert!(
        started.elapsed() >= std::time::Duration::from_secs(14),
        "preview ended before its timeout; current session must remain active"
    );
    let fresh = snapshot(SettingsSection::Display).unwrap();
    assert_eq!(fresh.targets, before.targets);
    macos::apply(&patch, &fresh).unwrap();
    assert!(satisfied(
        &patch,
        &snapshot(SettingsSection::Display).unwrap()
    ));
    restore.2 = true;
    macos::finish_display(&patch, &before, true).unwrap();
    assert!(satisfied(
        &patch,
        &snapshot(SettingsSection::Display).unwrap()
    ));
    macos::finish_display(&patch, &before, false).unwrap();
    assert_eq!(
        snapshot(SettingsSection::Display).unwrap().targets,
        before.targets
    );
    drop(restore);
}
#[test]
#[ignore = "requires root; changes and restores real network bypass and AC/DC timers"]
fn machine_proxy_and_power_apply_preserve_unmodified_values_and_restore() {
    assert_eq!(
        unsafe { libc::geteuid() },
        0,
        "run the built test binary through native administrator authorization"
    );
    let before = snapshot(SettingsSection::Network).unwrap();
    let t = before
        .targets
        .iter()
        .find(|t| t.writable && t.fields.iter().any(|f| f.key == "bypass"))
        .expect("unmanaged native network service");
    let original: Vec<String> = serde_json::from_value(field(t, "bypass").clone()).unwrap();
    let mut bypass = original.clone();
    bypass.push("rss-settings-acceptance.invalid".into());
    let patch = SettingsPatch::Proxy {
        target: t.id.clone(),
        http: None,
        https: None,
        socks: None,
        bypass: Some(bypass),
    };
    let restoration = SettingsPatch::Proxy {
        target: t.id.clone(),
        http: None,
        https: None,
        socks: None,
        bypass: Some(original),
    };
    let _restore = SettingRestore(SettingsSection::Network, restoration.clone());
    macos::apply(&patch, &before).unwrap();
    let actual = snapshot(SettingsSection::Network).unwrap();
    let applied = actual.targets.iter().find(|v| v.id == t.id).unwrap();
    let preserved = t
        .fields
        .iter()
        .filter(|f| f.key != "bypass")
        .cloned()
        .collect::<Vec<_>>();
    let unchanged = applied
        .fields
        .iter()
        .filter(|f| f.key != "bypass")
        .cloned()
        .collect::<Vec<_>>();
    let restored = macos::apply(&restoration, &actual);
    restored.unwrap();
    assert_eq!(preserved, unchanged);
    assert!(satisfied(&patch, &actual));
    assert_eq!(
        snapshot(SettingsSection::Network).unwrap().targets,
        before.targets
    );
    let before = snapshot(SettingsSection::Power).unwrap();
    let targets = before
        .targets
        .iter()
        .filter(|t| t.writable && ["ac", "dc"].contains(&t.id.as_str()))
        .collect::<Vec<_>>();
    assert!(!targets.is_empty(), "requires writable native AC/DC timers");
    for t in targets {
        let minutes = field(t, "displayOffMinutes").as_u64().unwrap() as u16;
        let patch = SettingsPatch::Power {
            target: t.id.clone(),
            display_off_minutes: Some(if minutes == 10 { 11 } else { 10 }),
            sleep_minutes: None,
            mode: None,
        };
        let restoration = SettingsPatch::Power {
            target: t.id.clone(),
            display_off_minutes: Some(minutes),
            sleep_minutes: None,
            mode: None,
        };
        let _restore = SettingRestore(SettingsSection::Power, restoration.clone());
        let fresh = snapshot(SettingsSection::Power).unwrap();
        macos::apply(&patch, &fresh).unwrap();
        let actual = snapshot(SettingsSection::Power).unwrap();
        let restored = macos::apply(&restoration, &actual);
        restored.unwrap();
        assert!(satisfied(&patch, &actual));
        let applied = actual.targets.iter().find(|v| v.id == t.id).unwrap();
        assert_eq!(field(applied, "sleepMinutes"), field(t, "sleepMinutes"));
        assert_eq!(
            snapshot(SettingsSection::Power).unwrap().targets,
            fresh.targets
        );
        eprintln!("native_power_verified_target={}", t.id);
    }
    assert_eq!(
        json!(snapshot(SettingsSection::Power).unwrap().targets),
        json!(before.targets)
    );
}

#[test]
#[ignore = "reads actual macOS displays and asks the native adapter to reject an invalid mode"]
fn refused_native_display_transaction_proves_no_change_and_closes() {
    let before = snapshot(SettingsSection::Display).unwrap();
    let target = before
        .targets
        .iter()
        .find(|t| t.writable)
        .expect("unmanaged connected display");
    let input = Input {
        request: RequestId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        action: Action::Change {
            expected: before.revision.clone(),
            before: Box::new(before.clone()),
            patch: SettingsPatch::Display {
                target: target.id.clone(),
                mode: Some("not-a-native-mode".into()),
                origin: None,
            },
        },
        run_as: RunAs::System {
            platform: Platform::Macos,
        },
        session: SessionRequirement::NotRequired {},
        confirm: PathBuf::from("/var/empty/no-confirmation"),
        finish_by_unix_ms: now().unwrap() + 25000,
    };
    let mut unsettled = false;
    assert!(matches!(
        execute(&input, &mut unsettled, &mut |_| Ok(())),
        Err(Error::Unsupported)
    ));
    assert!(!unsettled);
    assert_eq!(
        snapshot(SettingsSection::Display).unwrap().targets,
        before.targets
    );
}
