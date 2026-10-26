//! Code-owned native settings worker and exact admission permit.
//! ref: Apple configd/SystemConfiguration.fproj/SCNetworkProtocol.c@585b7f2fca293f4642d21d15c5daf187f63c4796.
use crate::local_execution::hash;
use agent_client::Error;
use execution_contract::*;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
pub(crate) mod compile;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
static NATIVE_OUTPUT_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) fn observe_native(bytes: u64) {
    NATIVE_OUTPUT_BYTES.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
}
pub(crate) const PROFILE: &str = "native-settings-worker";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Action {
    Read {
        section: SettingsSection,
    },
    Change {
        expected: Digest,
        before: Box<SettingsSnapshot>,
        patch: SettingsPatch,
    },
    Open {
        section: SettingsSection,
    },
}
impl Action {
    pub(crate) fn section(&self) -> SettingsSection {
        match self {
            Self::Read { section } | Self::Open { section } => *section,
            Self::Change { patch, .. } => patch.section(),
        }
    }
    pub(crate) fn is_write(&self) -> bool {
        matches!(self, Self::Change { .. })
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Input {
    pub request: RequestId,
    pub action: Action,
    pub run_as: RunAs,
    pub session: SessionRequirement,
    pub confirm: PathBuf,
    pub finish_by_unix_ms: u64,
}
pub(crate) struct Permit {
    plan: FrozenExecution,
    policy: crate::local_permissions::Policy,
    write: bool,
    section: SettingsSection,
}
impl Permit {
    pub(crate) fn new(
        plan: FrozenExecution,
        policy: crate::local_permissions::Policy,
        action: &Action,
    ) -> Result<Self, Error> {
        Self::checked(plan, policy, action)
    }
    pub(crate) fn keep(
        plan: FrozenExecution,
        policy: crate::local_permissions::Policy,
        action: &Action,
    ) -> Result<Self, Error> {
        if !matches!(
            action,
            Action::Change {
                patch: SettingsPatch::Display { .. },
                ..
            }
        ) {
            return Err(Error::Denied);
        }
        Self::checked(plan, policy, action)
    }
    fn checked(
        plan: FrozenExecution,
        policy: crate::local_permissions::Policy,
        action: &Action,
    ) -> Result<Self, Error> {
        if !matches!(plan.spec().request.authority, Authority::Local { .. })
            || plan.spec().launch.interpreter.profile.id.as_str() != PROFILE
            || !matches!(plan.spec().request.initiator, Initiator::Human { .. })
        {
            return Err(Error::Denied);
        }
        if let Action::Change {
            expected,
            before,
            patch,
        } = action
        {
            if before.revision != *expected {
                return Err(Error::Conflict);
            }
            if !patch.valid()
                || !match patch {
                    SettingsPatch::Display { .. } => policy.rules.allow_display,
                    SettingsPatch::Proxy { .. } => policy.rules.allow_proxy,
                    SettingsPatch::Power { .. } => policy.rules.allow_power,
                }
            {
                return Err(Error::Denied);
            }
        }
        Ok(Self {
            plan,
            policy,
            write: action.is_write(),
            section: action.section(),
        })
    }
    pub(crate) fn verify(&self, plan: &FrozenExecution, now: u64) -> Result<RiskLevel, Error> {
        if plan.digest() != self.plan.digest() || now >= plan.spec().validity.expires_at_unix_ms {
            return Err(Error::Denied);
        }
        if self.write {
            let current = self.policy.current()?;
            if !match self.section {
                SettingsSection::Display => current.rules.allow_display,
                SettingsSection::Network => current.rules.allow_proxy,
                SettingsSection::Power => current.rules.allow_power,
                _ => false,
            } {
                return Err(Error::Denied);
            }
        }
        Ok(if self.write {
            RiskLevel::Two
        } else {
            RiskLevel::One
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DisplayDecision {
    pub request: RequestId,
    pub keep: bool,
}
pub(crate) fn decide(
    config: &crate::ExecutionConfig,
    request: &RequestId,
    keep: bool,
) -> Result<(), Error> {
    let path = control_path(config, request)?;
    installation_security::protected(path.parent().ok_or(Error::Configuration)?)
        .map_err(|_| Error::Denied)?;
    let bytes = serde_json::to_vec(&DisplayDecision {
        request: request.clone(),
        keep,
    })
    .map_err(|_| Error::Protocol)?;
    if path.exists() {
        if installation_security::read_protected(&path).map_err(|_| Error::Denied)? != bytes {
            return Err(Error::Conflict);
        }
    } else {
        if std::fs::read_dir(path.parent().ok_or(Error::Configuration)?)?
            .take(1024)
            .count()
            >= 1024
        {
            return Err(Error::Capacity);
        }
        crate::installation::publish_local_rules(&path, &bytes)?;
    }
    Ok(())
}
pub(crate) fn now() -> Result<u64, Error> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Clock)?
            .as_millis(),
    )
    .map_err(|_| Error::Clock)
}
pub(crate) fn control_root(config: &crate::ExecutionConfig) -> Result<PathBuf, Error> {
    Ok(config
        .material_root
        .parent()
        .ok_or(Error::Configuration)?
        .join("settings-confirmations"))
}
pub(crate) fn control_path(
    config: &crate::ExecutionConfig,
    request: &RequestId,
) -> Result<PathBuf, Error> {
    let id = uuid::Uuid::parse_str(request.as_str()).map_err(|_| Error::Protocol)?;
    if id.is_nil() || id.to_string() != request.as_str() {
        return Err(Error::Protocol);
    }
    Ok(control_root(config)?.join(id.to_string()))
}

fn edited_fields(patch: &SettingsPatch) -> Vec<&'static str> {
    match patch {
        SettingsPatch::Proxy {
            http,
            https,
            socks,
            bypass,
            ..
        } => [
            ("http", http.is_some()),
            ("https", https.is_some()),
            ("socks", socks.is_some()),
            ("bypass", bypass.is_some()),
        ]
        .into_iter()
        .filter_map(|(key, present)| present.then_some(key))
        .collect(),
        SettingsPatch::Display { mode, origin, .. } => {
            [("mode", mode.is_some()), ("origin", origin.is_some())]
                .into_iter()
                .filter_map(|(key, present)| present.then_some(key))
                .collect()
        }
        SettingsPatch::Power {
            display_off_minutes,
            sleep_minutes,
            mode,
            ..
        } => [
            ("displayOffMinutes", display_off_minutes.is_some()),
            ("sleepMinutes", sleep_minutes.is_some()),
            ("mode", mode.is_some()),
        ]
        .into_iter()
        .filter_map(|(key, present)| present.then_some(key))
        .collect(),
    }
}
fn patch_target(patch: &SettingsPatch) -> &str {
    match patch {
        SettingsPatch::Proxy { target, .. }
        | SettingsPatch::Display { target, .. }
        | SettingsPatch::Power { target, .. } => target,
    }
}
fn setting_value<'a>(target: &'a SettingsTarget, key: &str) -> Option<&'a serde_json::Value> {
    target
        .fields
        .iter()
        .find(|f| f.key == key)
        .and_then(|f| f.value.as_ref())
}
fn checked_target<'a>(
    patch: &SettingsPatch,
    targets: &'a [SettingsTarget],
) -> Result<&'a SettingsTarget, Error> {
    let target = targets
        .iter()
        .find(|t| t.id == patch_target(patch))
        .ok_or(Error::Conflict)?;
    if edited_fields(patch).iter().any(|key| {
        !target.fields.iter().any(|f| {
            f.key == *key && f.writable && f.quality == DataQuality::Complete && f.value.is_some()
        })
    }) {
        return Err(Error::Denied);
    }
    Ok(target)
}
fn display_state_equal(before: &[SettingsTarget], current: &[SettingsTarget]) -> bool {
    before.len() == current.len()
        && before.iter().all(|old| {
            current.iter().find(|t| t.id == old.id).is_some_and(|new| {
                [
                    "mode",
                    "origin",
                    "width",
                    "height",
                    "pixelWidth",
                    "pixelHeight",
                    "main",
                ]
                .iter()
                .all(|key| setting_value(old, key) == setting_value(new, key))
            })
        })
}
fn check_change(
    patch: &SettingsPatch,
    before: &SettingsSnapshot,
    current: &[SettingsTarget],
) -> Result<(), Error> {
    let actual = checked_target(patch, current)?;
    let original = before
        .targets
        .iter()
        .find(|t| t.id == actual.id)
        .ok_or(Error::Conflict)?;
    if edited_fields(patch).iter().any(|key| {
        setting_value(original, key).is_none()
            || setting_value(original, key) != setting_value(actual, key)
    }) || matches!(patch, SettingsPatch::Display { .. })
        && !display_state_equal(&before.targets, current)
        || matches!(patch, SettingsPatch::Power { .. })
            && setting_value(original, "scheme") != setting_value(actual, "scheme")
    {
        return Err(Error::Conflict);
    }
    Ok(())
}
pub(crate) fn snapshot(section: SettingsSection) -> Result<SettingsSnapshot, Error> {
    #[cfg(target_os = "macos")]
    let targets = macos::read(section)?;
    #[cfg(windows)]
    let targets = windows::read(section)?;
    #[cfg(not(any(windows, target_os = "macos")))]
    let targets: Vec<SettingsTarget> = return Err(Error::Unsupported);
    if targets.len() > 64
        || targets
            .iter()
            .any(|t| t.fields.len() > 64 || t.options.len() > 256)
    {
        return Err(Error::Capacity);
    }
    let revision = crate::collected_data::digest(&(section, &targets))?;
    Ok(SettingsSnapshot {
        section,
        revision,
        collected_at_unix_ms: now()?,
        source: if cfg!(windows) {
            "Windows native APIs"
        } else {
            "macOS native APIs"
        }
        .into(),
        targets,
    })
}
/// Fixed physical adapter entry. Invocation alone cannot authorize a service task.
pub fn run(path: &Path) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 65536)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    execution_runner::staging::verify_worker_identity(&input.run_as, &input.session)
        .map_err(crate::error::app_error)?;
    if now()? >= input.finish_by_unix_ms {
        return Err(Error::Expired);
    }
    let mut output = Vec::new();
    let mut display_unsettled = false;
    let result = execute(&input, &mut display_unsettled, &mut |value| {
        let mut row = serde_json::to_vec(&value).map_err(|_| Error::Protocol)?;
        row.push(b'\n');
        if output.len() + row.len() > 256 * 1024 {
            return Err(Error::Capacity);
        }
        std::io::stdout().write_all(&row)?;
        std::io::stdout().flush()?;
        output.extend(row);
        Ok(())
    });
    if let Err(error) = result {
        let observed = snapshot(input.action.section()).ok();
        let unchanged_display = !display_unsettled
            && matches!(&input.action, Action::Change {before,patch:SettingsPatch::Display {..},..} if observed.as_ref().is_some_and(|actual|display_state_equal(&actual.targets,&before.targets)));
        let row = serde_json::to_vec(&SettingsResponse {
            write_allowed: false,
            request: Some(input.request.clone()),
            digest: None,
            finished: true,
            effect: if matches!(error, Error::Conflict) {
                SettingsEffect::Conflict
            } else if input.action.is_write() && !unchanged_display {
                SettingsEffect::Unknown
            } else {
                SettingsEffect::Failed
            },
            snapshot: observed,
            keep_before_unix_ms: None,
            reason: Some(error.to_string()),
        })
        .map_err(|_| Error::Protocol)?;
        std::io::stdout().write_all(&row)?;
        std::io::stdout().write_all(b"\n")?;
        output.extend(row);
        output.push(b'\n');
    }
    // The trusted runner verifies closure and both hashes; JSON text alone grants no effect.
    eprintln!(
        "{}",
        serde_json::to_string(&CollectionCompletion {
            closed: !display_unsettled,
            input_sha256: hash(&bytes)?.as_str().into(),
            output_sha256: hash(&output)?.as_str().into(),
            native_output_bytes: NATIVE_OUTPUT_BYTES.load(std::sync::atomic::Ordering::Relaxed)
        })
        .map_err(|_| Error::Protocol)?
    );
    Ok(0)
}
fn execute(
    input: &Input,
    display_unsettled: &mut bool,
    emit: &mut impl FnMut(SettingsResponse) -> Result<(), Error>,
) -> Result<(), Error> {
    let result = execute_inner(input, display_unsettled, emit);
    if result.is_err() && *display_unsettled {
        if let Action::Change {
            before,
            patch: patch @ SettingsPatch::Display { .. },
            ..
        } = &input.action
        {
            // Native configuration calls are synchronous. A refused/no-op transaction can
            // close only after an independent observation; a known preview is restored.
            if let Ok(actual) = snapshot(SettingsSection::Display) {
                if actual.targets == before.targets {
                    *display_unsettled = false;
                } else if satisfied(patch, &actual) {
                    #[cfg(target_os = "macos")]
                    let restored = macos::finish_display(patch, before, false);
                    #[cfg(windows)]
                    let restored = windows::finish_display(patch, before, false);
                    if restored.is_ok()
                        && snapshot(SettingsSection::Display)
                            .is_ok_and(|s| s.targets == before.targets)
                    {
                        *display_unsettled = false;
                    }
                }
            }
        }
    }
    result
}
fn execute_inner(
    input: &Input,
    display_unsettled: &mut bool,
    emit: &mut impl FnMut(SettingsResponse) -> Result<(), Error>,
) -> Result<(), Error> {
    let section = input.action.section();
    let mut response = SettingsResponse {
        write_allowed: false,
        request: Some(input.request.clone()),
        digest: None,
        finished: true,
        effect: SettingsEffect::Observed,
        snapshot: None,
        keep_before_unix_ms: None,
        reason: None,
    };
    match &input.action {
        Action::Read { .. } => response.snapshot = Some(snapshot(section)?),
        Action::Open { .. } => {
            #[cfg(target_os = "macos")]
            macos::open(section)?;
            #[cfg(windows)]
            windows::open(section)?;
            response.effect = SettingsEffect::OpenRequested;
        }
        Action::Change {
            expected,
            before,
            patch,
        } => {
            if !patch.valid() || before.revision != *expected {
                return Err(Error::Conflict);
            }
            let current = snapshot(section)?;
            check_change(patch, before, &current.targets)?;
            *display_unsettled = matches!(patch, SettingsPatch::Display { .. });
            #[cfg(target_os = "macos")]
            macos::apply(patch, before)?;
            #[cfg(windows)]
            windows::apply(patch, before)?;
            let applied = snapshot(section)?;
            if !satisfied(patch, &applied)
                || matches!(patch, SettingsPatch::Display { .. })
                    && checked_target(patch, &applied.targets).is_err()
            {
                #[cfg(target_os = "macos")]
                if *display_unsettled {
                    macos::preserve_display(&applied)?;
                }
                *display_unsettled = false;
                response.effect = SettingsEffect::Conflict;
                response.snapshot = Some(applied);
            } else if matches!(patch, SettingsPatch::Display { .. }) {
                let deadline = now()?.checked_add(15_000).ok_or(Error::Clock)?;
                response.finished = false;
                response.effect = SettingsEffect::Preview;
                response.snapshot = Some(applied.clone());
                response.keep_before_unix_ms = Some(deadline);
                emit(response.clone())?;
                let mut keep = false;
                // Monotonic cutoff cannot be extended by wall-clock adjustments or UI lifecycle.
                let cutoff = std::time::Instant::now() + std::time::Duration::from_secs(15);
                while std::time::Instant::now() < cutoff && now()? < deadline {
                    if !can_keep(input) {
                        break;
                    }
                    if let Ok(bytes) = installation_security::read_protected(&input.confirm) {
                        if let Ok(decision) = serde_json::from_slice::<DisplayDecision>(&bytes) {
                            if decision.request == input.request {
                                keep = decision.keep;
                                break;
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                // A revoked login or expired execution cannot retain the preview, even
                // if a previously written keep marker remains readable.
                keep &= can_keep(input);
                let actual = snapshot(section)?;
                if actual.targets != applied.targets {
                    #[cfg(target_os = "macos")]
                    macos::preserve_display(&actual)?;
                    response.effect = SettingsEffect::Conflict;
                    response.snapshot = Some(actual);
                } else {
                    keep &= can_keep(input);
                    #[cfg(target_os = "macos")]
                    macos::finish_display(patch, before, keep)?;
                    #[cfg(windows)]
                    windows::finish_display(patch, before, keep)?;
                    let actual = snapshot(section)?;
                    response.effect = if keep && satisfied(patch, &actual) {
                        SettingsEffect::Applied
                    } else if !keep && actual.targets == before.targets {
                        SettingsEffect::Restored
                    } else {
                        SettingsEffect::Conflict
                    };
                    response.snapshot = Some(actual);
                }
                *display_unsettled = false;
                response.finished = true;
                response.keep_before_unix_ms = None;
            } else {
                response.effect = SettingsEffect::Applied;
                response.snapshot = Some(applied);
            }
        }
    }
    emit(response)
}
fn can_keep(input: &Input) -> bool {
    now().is_ok_and(|now| now < input.finish_by_unix_ms)
        && execution_runner::staging::verify_worker_identity(&input.run_as, &input.session).is_ok()
}
pub(crate) fn satisfied(patch: &SettingsPatch, snapshot: &SettingsSnapshot) -> bool {
    let target = match patch {
        SettingsPatch::Proxy { target, .. }
        | SettingsPatch::Display { target, .. }
        | SettingsPatch::Power { target, .. } => target,
    };
    let Some(t) = snapshot.targets.iter().find(|t| &t.id == target) else {
        return false;
    };
    let equal = |key: &str, value: serde_json::Value| {
        t.fields.iter().any(|f| {
            f.key == key && f.quality == DataQuality::Complete && f.value.as_ref() == Some(&value)
        })
    };
    match patch {
        SettingsPatch::Proxy {
            http,
            https,
            socks,
            bypass,
            ..
        } => {
            [("http", http), ("https", https), ("socks", socks)]
                .into_iter()
                .all(|(k, v)| {
                    v.as_ref().is_none_or(|v| {
                        if v.enabled {
                            equal(k, serde_json::to_value(v).unwrap_or_default())
                        } else {
                            t.fields.iter().any(|f| {
                                f.key == k
                                    && f.quality == DataQuality::Complete
                                    && f.value
                                        .as_ref()
                                        .and_then(|v| v.get("enabled"))
                                        .and_then(serde_json::Value::as_bool)
                                        == Some(false)
                            })
                        }
                    })
                })
                && bypass
                    .as_ref()
                    .is_none_or(|v| equal("bypass", serde_json::json!(v)))
        }
        SettingsPatch::Display { mode, origin, .. } => {
            mode.as_ref()
                .is_none_or(|v| equal("mode", serde_json::json!(v)))
                && origin
                    .as_ref()
                    .is_none_or(|v| equal("origin", serde_json::json!(v)))
        }
        SettingsPatch::Power {
            display_off_minutes,
            sleep_minutes,
            mode,
            ..
        } => {
            display_off_minutes.is_none_or(|v| equal("displayOffMinutes", serde_json::json!(v)))
                && sleep_minutes.is_none_or(|v| equal("sleepMinutes", serde_json::json!(v)))
                && mode
                    .as_ref()
                    .is_none_or(|v| equal("mode", serde_json::json!(v)))
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod native_tests;

#[cfg(test)]
mod control_tests {
    use super::*;
    fn proxy_snapshot() -> SettingsSnapshot {
        SettingsSnapshot {
            section: SettingsSection::Network,
            revision: Digest::new("a".repeat(64)).unwrap(),
            collected_at_unix_ms: 1,
            source: "native fixture".into(),
            targets: vec![SettingsTarget {
                id: "service".into(),
                name: "original name".into(),
                options: vec![],
                reason: None,
                fields: vec![
                    SettingsField {
                        key: "bypass".into(),
                        writable: true,
                        value: Some(serde_json::json!(["localhost"])),
                        quality: DataQuality::Complete,
                        reason: None,
                    },
                    SettingsField {
                        key: "http".into(),
                        writable: true,
                        value: Some(serde_json::json!({"enabled": false,"host":"","port":0})),
                        quality: DataQuality::Complete,
                        reason: None,
                    },
                ],
            }],
        }
    }
    #[test]
    fn local_patch_compares_only_written_values_and_current_field_permission() {
        let before = proxy_snapshot();
        let patch = SettingsPatch::Proxy {
            target: "service".into(),
            http: None,
            https: None,
            socks: None,
            bypass: Some(vec!["example.test".into()]),
        };
        let mut current = before.targets.clone();
        current[0].name = "renamed".into();
        current[0].reason = Some("unrelated diagnostic".into());
        current[0].fields[1].value =
            Some(serde_json::json!({"enabled":true,"host":"new-proxy","port":8080}));
        check_change(&patch, &before, &current).unwrap();
        current[0].fields[0].value = Some(serde_json::json!(["changed-by-other-writer"]));
        assert!(matches!(
            check_change(&patch, &before, &current),
            Err(Error::Conflict)
        ));
        current[0].fields[0].value = before.targets[0].fields[0].value.clone();
        current[0].fields[0].writable = false;
        assert!(matches!(
            check_change(&patch, &before, &current),
            Err(Error::Denied)
        ));
    }

    #[test]
    fn decision_paths_reject_uuid_aliases_and_nil_instead_of_sharing_another_task_marker() {
        let config = crate::ExecutionConfig {
            worker_image_fixture: None,
            local_permissions: crate::local_permissions::Config {
                policy_file: "unused".into(),
            },
            work_root: std::env::temp_dir(),
            material_root: std::env::temp_dir().join("settings-control-fixture/materials"),
            interpreters: vec![],
            managers: vec![],
            processes: 1,
        };
        let value = "a171d762-7c18-4038-b672-dc9838b869ab";
        assert_eq!(
            control_path(&config, &RequestId::new(value).unwrap())
                .unwrap()
                .file_name()
                .unwrap(),
            value
        );
        for value in [
            value.to_uppercase(),
            value.replace('-', ""),
            format!("urn:uuid:{value}"),
            "00000000-0000-0000-0000-000000000000".into(),
        ] {
            assert!(matches!(
                control_path(&config, &RequestId::new(value).unwrap()),
                Err(Error::Protocol)
            ));
        }
    }
}
