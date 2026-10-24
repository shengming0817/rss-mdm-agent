//! Physical source lifecycle inside the existing frozen software attempt.
use super::*;
use sha2::{Digest as _, Sha256};
use std::{fs, path::Path, sync::atomic::Ordering};
#[cfg(target_os = "macos")]
mod brew;
#[cfg(all(test, target_os = "macos"))]
mod tests;
#[cfg(any(windows, all(test, target_os = "macos")))]
#[cfg_attr(not(windows), allow(dead_code))]
mod winget;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    owner: String,
    path: Option<PathBuf>,
    created: bool,
    parent_created: bool,
    created_directory: Option<(u64, u64)>,
    active: bool,
    activity: execution_runner::software::NativeActivity,
}
fn owner(request: &WorkerRequest) -> Result<String, Error> {
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                &request.intent,
                &request.action,
                &request.export,
                &request.materials,
                &request.tools,
                &request.resource_root,
                &request.run_as,
                &request.session,
                request.architecture,
                request.step,
            ))
            .map_err(|_| Error::Protocol)?
        )
    ))
}
fn state(request: &WorkerRequest) -> Result<Option<State>, Error> {
    if !request.resource_root.try_exists()? {
        return Ok(None);
    }
    let dir = platform_private_storage::PrivateDirectory::open(&request.resource_root)?;
    let bytes = match dir.read(Path::new("source.json"), 16384) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if fs::read_dir(&request.resource_root)?.next().is_none() {
                return Ok(None);
            }
            return Err(Error::Untrusted);
        }
        Err(e) => return Err(e.into()),
    };
    let state: State = serde_json::from_slice(&bytes).map_err(|_| Error::Untrusted)?;
    if state.owner != owner(request)? {
        return Err(Error::Untrusted);
    }
    Ok(Some(state))
}
fn save(request: &WorkerRequest, state: &State) -> Result<(), Error> {
    let bytes = serde_json::to_vec(state).map_err(|_| Error::Protocol)?;
    let dir = platform_private_storage::PrivateDirectory::open(&request.resource_root)?;
    let staging = format!("source-{}.next", uuid::Uuid::new_v4());
    let path = Path::new(&staging);
    let mut file = dir.create_file(path)?;
    use std::io::Write;
    file.set_len(0)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    dir.replace(path, Path::new("source.json"))?;
    Ok(())
}
fn prepared(request: &WorkerRequest) -> Result<State, Error> {
    let current = state(request)?.ok_or(Error::Untrusted)?;
    if current.active {
        return Err(Error::Unavailable);
    }
    Ok(current)
}
fn group_closed(record: &State) -> bool {
    !record.active || record.activity.closed().unwrap_or(false)
}
fn environment(request: &WorkerRequest) -> Result<BTreeMap<String, String>, Error> {
    let mut env = BTreeMap::new();
    #[cfg(unix)]
    {
        use std::ffi::CStr;
        let entry = unsafe { libc::getpwuid(libc::geteuid()) };
        if entry.is_null() {
            return Err(Error::Identity);
        }
        let home = unsafe { CStr::from_ptr((*entry).pw_dir) }
            .to_str()
            .map_err(|_| Error::Identity)?;
        env.insert("HOME".into(), home.into());
        env.insert("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into());
        env.insert("LC_ALL".into(), "C".into());
        env.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
        env.insert("GIT_CONFIG_GLOBAL".into(), "/dev/null".into());
        env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
        env.insert("GIT_NO_REPLACE_OBJECTS".into(), "1".into());
    }
    #[cfg(windows)]
    {
        for key in [
            "SystemRoot",
            "WINDIR",
            "TEMP",
            "TMP",
            "LOCALAPPDATA",
            "USERPROFILE",
            "APPDATA",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
            "PATHEXT",
        ] {
            if let Ok(value) = std::env::var(key) {
                env.insert(key.into(), value);
            }
        }
    }
    let commands = crate::backend::commands::software_commands(&request.action)?;
    let invocation = match request.operation {
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious => {
            commands.uninstall.ok_or(Error::Unsupported)?
        }
        WorkerOperation::Upgrade => commands.upgrade.ok_or(Error::Unsupported)?,
        _ => commands.install,
    };
    env.extend(invocation.environment.clone());
    Ok(env)
}
fn tool(
    request: &WorkerRequest,
    key: &str,
    args: Vec<String>,
    env: &BTreeMap<String, String>,
) -> Result<Vec<u8>, Error> {
    let (code, stdout, _) = run_tool_environment(request, &request.tool(key)?, &args, Some(env))?;
    if code != 0 {
        return Err(Error::Unavailable);
    }
    Ok(stdout)
}
fn text(bytes: Vec<u8>) -> Result<String, Error> {
    let text = String::from_utf8(bytes).map_err(|_| Error::Protocol)?;
    let text = text.trim_end_matches(['\r', '\n']);
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(Error::Untrusted);
    }
    Ok(text.into())
}
pub(super) fn detect(request: &WorkerRequest) -> Result<SoftwareState, Error> {
    let commands = crate::backend::commands::software_commands(&request.action)?;
    match commands.detection.ok_or(Error::Unsupported)? {
        wire::SoftwareTaskDetection::MsiProduct {
            product_code,
            version,
        } => execution_runner::software::observe_native(
            &SoftwareDetector::MsiProduct {
                product_code: product_code.clone(),
                version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
            },
            &request.run_as,
        )
        .map_err(crate::error::app_error),
        wire::SoftwareTaskDetection::PkgReceipt { receipt, version } => {
            execution_runner::software::observe_native(
                &SoftwareDetector::PkgReceipt {
                    receipt: receipt.clone(),
                    version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
                },
                &request.run_as,
            )
            .map_err(crate::error::app_error)
        }
        wire::SoftwareTaskDetection::File {
            path,
            version,
            sha256,
            ..
        } => {
            if !Path::new(path).is_absolute() {
                return Err(Error::Untrusted);
            }
            match fs::symlink_metadata(path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(SoftwareState::Absent {})
                }
                Err(e) => return Err(e.into()),
                _ => (),
            }
            let digest = execution_contract::Digest::new(crate::backend::plan::hex(sha256))
                .map_err(|_| Error::Protocol)?;
            Ok(
                match execution_runner::staging::verify_observed(
                    Path::new(path),
                    &digest,
                    &request.run_as,
                    &request.session,
                ) {
                    Ok(_) => SoftwareState::Present {
                        version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
                    },
                    Err(_) => SoftwareState::Unknown {
                        reason: SoftwareDetectionFailure::UnrecognizedVersion,
                    },
                },
            )
        }
        #[cfg(windows)]
        detection @ wire::SoftwareTaskDetection::Registry { .. } => {
            super::windows::detect(request, detection)
        }
        _ => Err(Error::Unsupported),
    }
}
pub(super) fn execute(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    if request.operation == WorkerOperation::Detect {
        return Ok(result(0, Some(detect(request)?), String::new()));
    }
    if request.operation == WorkerOperation::Cleanup {
        let Some(record) = state(request)? else {
            return Ok(result(0, None, String::new()));
        };
        if !group_closed(&record) {
            let mut output = result(
                1,
                None,
                "original native source operation has not closed".into(),
            );
            output.1.closed = false;
            return Ok(output);
        }
        cleanup(request, &record)?;
        return Ok(result(0, None, String::new()));
    }
    let mut record = if request.operation == WorkerOperation::Prepare {
        if let Some(record) = state(request)? {
            record
        } else {
            platform_private_storage::directory(&request.resource_root)?;
            State {
                owner: owner(request)?,
                path: None,
                created: false,
                parent_created: false,
                created_directory: None,
                active: false,
                activity: execution_runner::software::NativeActivity::current()
                    .map_err(crate::error::app_error)?,
            }
        }
    } else {
        prepared(request)?
    };
    if record.active {
        return Err(Error::Unavailable);
    }
    record.active = true;
    record.activity =
        execution_runner::software::NativeActivity::current().map_err(crate::error::app_error)?;
    save(request, &record)?;
    let outcome = physical(request, before, &mut record);
    if outcome.is_ok() || !request.native_pending.load(Ordering::Acquire) {
        record.active = false;
        save(request, &record)?;
    }
    outcome
}
fn physical(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
    record: &mut State,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    #[cfg(target_os = "macos")]
    if matches!(request.export, wire::SoftwareTaskExport::Brew { .. }) {
        return brew::execute(request, before, record);
    }
    #[cfg(any(windows, all(test, target_os = "macos")))]
    if matches!(request.export, wire::SoftwareTaskExport::Winget { .. }) {
        return winget::execute(request, before, record);
    }
    let _ = (request, before, record);
    Err(Error::Unsupported)
}
fn cleanup(request: &WorkerRequest, record: &State) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    if matches!(request.export, wire::SoftwareTaskExport::Brew { .. }) {
        brew::cleanup(request, record)?;
    }
    // The account-private attempt directory contains only source ownership and private cache data.
    platform_private_storage::PrivateDirectory::open(&request.resource_root)?;
    fs::remove_dir_all(&request.resource_root)?;
    Ok(())
}
