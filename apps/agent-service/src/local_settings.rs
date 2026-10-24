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
        before: SettingsSnapshot,
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
}
impl Permit {
    pub(crate) fn new(
        plan: FrozenExecution,
        policy: crate::local_permissions::Policy,
        action: &Action,
    ) -> Result<Self, Error> {
        policy.current()?;
        if !matches!(plan.spec().request.authority, Authority::Local { .. })
            || plan.spec().launch.interpreter.profile.id.as_str() != PROFILE
            || plan.spec().policy.revision.as_str() != policy.revision.as_str()
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
            if before.revision != *expected
                || now()?.saturating_sub(before.collected_at_unix_ms) > 30_000
            {
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
        })
    }
    pub(crate) fn verify(&self, plan: &FrozenExecution, now: u64) -> Result<RiskLevel, Error> {
        if plan.digest() != self.plan.digest() || now >= plan.spec().validity.expires_at_unix_ms {
            return Err(Error::Denied);
        }
        self.policy.current()?;
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
    Ok(control_root(config)?.join(id.to_string()))
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
        let row = serde_json::to_vec(&SettingsResponse {
            write_allowed: false,
            request: Some(input.request.clone()),
            digest: None,
            finished: true,
            effect: if input.action.is_write() {
                SettingsEffect::Unknown
            } else {
                SettingsEffect::Failed
            },
            snapshot: snapshot(input.action.section()).ok(),
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
            if !patch.valid()
                || before.revision != *expected
                || now()?.saturating_sub(before.collected_at_unix_ms) > 30_000
            {
                return Err(Error::Conflict);
            }
            let current = snapshot(section)?;
            if current.revision != *expected {
                return Err(Error::Conflict);
            }
            let target = match patch {
                SettingsPatch::Proxy { target, .. }
                | SettingsPatch::Display { target, .. }
                | SettingsPatch::Power { target, .. } => target,
            };
            if !current
                .targets
                .iter()
                .any(|t| &t.id == target && t.writable)
            {
                return Err(Error::Denied);
            }
            *display_unsettled = matches!(patch, SettingsPatch::Display { .. });
            #[cfg(target_os = "macos")]
            macos::apply(patch, before)?;
            #[cfg(windows)]
            windows::apply(patch, before)?;
            let applied = snapshot(section)?;
            if !satisfied(patch, &applied) {
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
                let actual = snapshot(section)?;
                if actual.targets != applied.targets {
                    #[cfg(target_os = "macos")]
                    macos::preserve_display(&actual)?;
                    response.effect = SettingsEffect::Conflict;
                    response.snapshot = Some(actual);
                } else {
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
