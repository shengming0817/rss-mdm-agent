//! Fixed physical software operations run by the existing service image and authenticated helper.
//! This module has no journal, network client, authorization decision or task queue.
use agent_client::Error;
use execution_contract::*;
use rss_mdm_agent_wire::runtime as wire;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/// One physical operation within an already admitted software attempt.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkerOperation {
    /// Independently query the installed target.
    Detect,
    /// Execute the exact install command.
    Install,
    /// Execute the separately approved update command.
    Upgrade,
    /// Configure only the exact source frozen by the original publication.
    Prepare,
    /// Execute only the explicit removal.
    Uninstall,
    /// Remove the exact previously observed version before installing.
    RemovePrevious,
    /// Acquire the exact read-only disk image.
    Attach,
    /// Verify and copy an image payload into private staging.
    Stage,
    /// Close only this attempt's image and staging resources.
    Cleanup,
}
/// Frozen local worker input; its bytes are pinned by the original execution plan.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerRequest {
    /// Original desired operation; auxiliary preparation cannot invent install authority.
    pub intent: SoftwareOperation,
    /// Producer-owned native source authority, retained with the original frozen input.
    pub export: wire::SoftwareTaskExport,
    /// Runtime accounting shared by all native checks in this one physical invocation.
    #[serde(skip)]
    pub native_output: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Native work that has started without verified completion.
    #[serde(skip)]
    pub native_pending: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Current producer-owned behavior and signatures, already frozen in the signed task.
    pub action: wire::SoftwareTaskAction,
    /// Exact physical operation.
    pub operation: WorkerOperation,
    /// Artifact-key mapping to immutable retained files.
    pub materials: BTreeMap<String, SoftwareMaterial>,
    /// Existing trusted platform tools, frozen by content identity.
    pub tools: BTreeMap<String, SoftwareMaterial>,
    /// Protected attempt-specific resource directory; no shared mounts or candidate scanning.
    pub resource_root: PathBuf,
    /// Exact OS execution account.
    pub run_as: RunAs,
    /// Exact original login binding, if required.
    pub session: SessionRequirement,
    /// Device hardware selection.
    pub architecture: wire::TaskArchitecture,
    /// One cumulative physical operation diagnostic bound.
    pub output_bytes: u64,
    /// Original zero-based backend step.
    pub step: u32,
}
impl WorkerRequest {
    fn material(&self, key: &str) -> Result<PathBuf, Error> {
        let material = self.materials.get(key).ok_or(Error::Untrusted)?;
        Ok(PathBuf::from(&material.path))
    }
    fn tool(&self, key: &str) -> Result<PathBuf, Error> {
        let tool = self.tools.get(key).ok_or(Error::Configuration)?;
        Ok(PathBuf::from(&tool.path))
    }
}
/// Run a bounded pinned input file under the actual OS account selected by the original runner.
/// This entry returns the real native command exit, with separate completion and detection facts.
pub fn run(path: &std::path::Path) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 4 * 1024 * 1024)
        .map_err(crate::error::app_error)?;
    let request: WorkerRequest = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    if request.output_bytes == 0 || request.output_bytes > 1_048_576 {
        return Err(Error::Protocol);
    }
    execution_runner::staging::verify_worker_identity(&request.run_as, &request.session)
        .map_err(crate::error::app_error)?;
    let _leases = request
        .materials
        .values()
        .chain(request.tools.values())
        .map(|material| {
            execution_runner::staging::verify_retained(
                std::path::Path::new(&material.path),
                &material.artifact.sha256,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(crate::error::app_error)?;
    let before = if matches!(
        request.operation,
        WorkerOperation::Prepare
            | WorkerOperation::Install
            | WorkerOperation::Upgrade
            | WorkerOperation::Uninstall
            | WorkerOperation::RemovePrevious
    ) {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::io::stdin().take(4097).read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            return Err(Error::Capacity);
        }
        Some(serde_json::from_slice::<SoftwareState>(&bytes).map_err(|_| Error::Protocol)?)
    } else {
        None
    };
    let scripted_detector = request.operation == WorkerOperation::Detect
        && matches!(
            execution_contract::software_commands(&request.action).detection,
            Some(wire::SoftwareTaskDetection::Script { .. })
        );
    #[cfg(target_os = "macos")]
    let outcome = if scripted_detector {
        source::detect(&request).map(|state| result(0, Some(state), String::new()))
    } else if matches!(
        request.action.behavior,
        wire::SoftwareTaskBehavior::Pkg(_) | wire::SoftwareTaskBehavior::Bundle(_)
    ) {
        native_package(&request, before.as_ref())
    } else if !matches!(request.export, wire::SoftwareTaskExport::Direct) {
        source::execute(&request, before.as_ref())
    } else {
        macos::execute(&request, before.as_ref())
    };
    #[cfg(windows)]
    let outcome = if scripted_detector {
        source::detect(&request).map(|state| result(0, Some(state), String::new()))
    } else if matches!(
        request.action.behavior,
        wire::SoftwareTaskBehavior::Msi(_) | wire::SoftwareTaskBehavior::Bundle(_)
    ) {
        native_package(&request, before.as_ref())
    } else if !matches!(request.export, wire::SoftwareTaskExport::Direct) {
        source::execute(&request, before.as_ref())
    } else {
        windows::execute(&request, before.as_ref())
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let outcome: Result<(i32, SoftwareWorkerResult), Error> = Err(Error::Unsupported);
    let mut result = outcome.unwrap_or_else(|error| {
        let mut failed = result(1, None, error.to_string());
        failed.1.closed = !request
            .native_pending
            .load(std::sync::atomic::Ordering::Acquire);
        failed
    });
    if matches!(request.export, wire::SoftwareTaskExport::Direct)
        && !matches!(request.action.behavior, wire::SoftwareTaskBehavior::Dmg(_))
        && request.resource_root.exists()
    {
        result.1.closed = false;
    }
    #[cfg(target_os = "macos")]
    if scripted_detector
        || (request.operation != WorkerOperation::Detect
            && matches!(
                request.action.behavior,
                wire::SoftwareTaskBehavior::Bundle(_)
            ))
    {
        // Arbitrary shell completion does not close escaped descendants.
        result.1.closed = false;
    }
    result.1.native_output_bytes = request
        .native_output
        .load(std::sync::atomic::Ordering::Acquire);
    let output = serde_json::to_vec(&result.1).map_err(|_| Error::Protocol)?;
    if (output.len() as u64)
        .saturating_add(result.1.native_output_bytes)
        .saturating_add(1)
        > request.output_bytes
    {
        return Err(Error::Capacity);
    }
    println!(
        "{}",
        std::str::from_utf8(&output).map_err(|_| Error::Protocol)?
    );
    Ok(result.0)
}
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(windows, test))]
mod material;
mod source;
#[cfg(windows)]
mod windows;

fn native_package(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    let commands = execution_contract::software_commands(&request.action);
    if request.operation == WorkerOperation::Detect {
        return Ok(result(0, Some(source::detect(request)?), String::new()));
    }
    if !matches!(
        commands.detection,
        Some(wire::SoftwareTaskDetection::Script { .. })
    ) && before != Some(&source::detect(request)?)
    {
        return Err(Error::Conflict);
    }
    let removal = matches!(
        request.operation,
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
    );
    let command = match request.operation {
        WorkerOperation::Install => commands.install,
        WorkerOperation::Upgrade => commands.upgrade.ok_or(Error::Unsupported)?,
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious => {
            commands.uninstall.ok_or(Error::Unsupported)?
        }
        _ => return Err(Error::Unsupported),
    };
    for signature in request
        .action
        .signatures
        .iter()
        .filter(|s| request.materials.contains_key(&s.artifact))
    {
        #[cfg(windows)]
        windows::authenticode(request, &signature.artifact)?;
        #[cfg(target_os = "macos")]
        macos::verify_code(
            request,
            &request.material(&signature.artifact)?,
            if matches!(request.action.behavior, wire::SoftwareTaskBehavior::Pkg(_)) {
                "install"
            } else {
                "execute"
            },
            Some(&signature.artifact),
        )?;
    }
    let (image, args) = if let wire::SoftwareTaskBehavior::Bundle(b) = &request.action.behavior {
        let script = if removal {
            b.uninstall.as_ref().ok_or(Error::Unsupported)?
        } else {
            &b.install
        };
        let profile = crate::backend::plan::script_profile(match script.interpreter {
            wire::SoftwareTaskInterpreter::PowerShell7 => wire::ExecutorProfile::PowerShell7,
            wire::SoftwareTaskInterpreter::PosixSh => wire::ExecutorProfile::PosixSh,
            wire::SoftwareTaskInterpreter::Bash => wire::ExecutorProfile::Bash,
        })?;
        let args = profile.materialized_file_argv(
            &request.material(&script.entry)?.to_string_lossy(),
            &command.arguments,
        );
        (
            request.tool(if removal {
                "uninstall-interpreter"
            } else {
                "install-interpreter"
            })?,
            args,
        )
    } else {
        let key = if removal {
            commands.removal_artifact.ok_or(Error::Unsupported)?
        } else {
            request.action.behavior.installer()
        };
        let package = request.material(key)?.to_string_lossy().into_owned();
        let mut args: Vec<String> =
            if matches!(request.action.behavior, wire::SoftwareTaskBehavior::Msi(_)) {
                vec![
                    if removal { "/x".into() } else { "/i".into() },
                    package,
                    "/qn".into(),
                    "/norestart".into(),
                ]
            } else {
                vec!["-pkg".into(), package, "-target".into(), "/".into()]
            };
        args.extend(command.arguments.clone());
        (request.tool("installer")?, args)
    };
    let (code, stdout, stderr) = run_tool(request, &image, &args)?;
    Ok(result(
        code,
        None,
        format!(
            "{}{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        ),
    ))
}

fn result(
    code: i32,
    detected: Option<SoftwareState>,
    diagnostics: String,
) -> (i32, SoftwareWorkerResult) {
    (
        code,
        SoftwareWorkerResult {
            native_output_bytes: 0,
            detected,
            closed: true,
            diagnostics,
        },
    )
}
fn run_tool(
    request: &WorkerRequest,
    image: &std::path::Path,
    args: &[String],
) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
    run_tool_environment(request, image, args, None)
}
fn run_tool_environment(
    request: &WorkerRequest,
    image: &std::path::Path,
    args: &[String],
    environment: Option<&BTreeMap<String, String>>,
) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        },
        time::Duration,
    };
    let mut command = Command::new(image);
    if let Some(environment) = environment {
        command.env_clear().envs(environment);
    }
    if let wire::SoftwareTaskBehavior::Bundle(b) = &request.action.behavior {
        let script = if matches!(
            request.operation,
            WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
        ) {
            b.uninstall.as_ref().ok_or(Error::Unsupported)?
        } else {
            &b.install
        };
        let path = request.material(&script.entry)?;
        let depth = std::path::Path::new(&script.entry).components().count();
        command.current_dir(path.ancestors().nth(depth).ok_or(Error::Untrusted)?);
    } else if image.starts_with(&request.resource_root) {
        command.current_dir(image.parent().ok_or(Error::Protocol)?);
    }
    #[cfg(windows)]
    let mut scope = native_process::OwnedHost::prepare(&mut command)?;
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    request.native_pending.store(true, Ordering::Release);
    #[cfg(windows)]
    if let Err(error) = scope.attach(child.id()) {
        let _ = child.kill();
        if child.wait().is_ok() {
            request.native_pending.store(false, Ordering::Release);
        }
        return Err(error.into());
    }
    let observed = request.native_output.clone();
    let bound = request
        .output_bytes
        .saturating_sub(1024.min(request.output_bytes / 2));
    let reader = |mut stream: Box<dyn Read + Send>, observed: Arc<AtomicU64>| {
        std::thread::spawn(move || {
            let mut output = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let count = stream.read(&mut buffer).map_err(|_| Error::Unavailable)?;
                if count == 0 {
                    break;
                }
                let before = observed.fetch_add(count as u64, Ordering::AcqRel);
                let keep = (bound.saturating_sub(before) as usize).min(count);
                output.extend_from_slice(&buffer[..keep]);
            }
            Ok::<_, Error>(output)
        })
    };
    let stdout = reader(
        Box::new(child.stdout.take().ok_or(Error::Unavailable)?),
        observed.clone(),
    );
    let stderr = reader(
        Box::new(child.stderr.take().ok_or(Error::Unavailable)?),
        observed.clone(),
    );
    let status = loop {
        if observed.load(Ordering::Acquire) > bound {
            #[cfg(windows)]
            scope.terminate()?;
            #[cfg(not(windows))]
            let _ = child.kill();
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    #[cfg(windows)]
    while !scope.empty() {
        if observed.load(Ordering::Acquire) > bound {
            scope.terminate()?;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let stdout = stdout.join().map_err(|_| Error::Unavailable)??;
    let stderr = stderr.join().map_err(|_| Error::Unavailable)??;
    if observed.load(Ordering::Acquire) > bound {
        return Err(Error::Capacity);
    }
    let code = status.code().ok_or(Error::Unavailable)?;
    request.native_pending.store(false, Ordering::Release);
    Ok((code, stdout, stderr))
}

/// Read-only native sideload policy; no policy or certificate mutation.
pub(crate) fn sideload_allowed() -> Result<bool, Error> {
    #[cfg(windows)]
    {
        windows::sideload_allowed()
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}
