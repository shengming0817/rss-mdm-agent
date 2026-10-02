//! Fixed physical software operations run by the existing service image and authenticated helper.
//! This module has no journal, network client, authorization decision or task queue.
use agent_client::{wire, Error};
use execution_contract::*;
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
    /// Execute only the explicit removal.
    Uninstall,
    /// Remove the exact previously observed managed version before installing.
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
    let bytes = execution_runner::staging::read_worker_input(path, 4 * 1024 * 1024)?;
    let request: WorkerRequest = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    if request.output_bytes == 0 || request.output_bytes > 1_048_576 {
        return Err(Error::Protocol);
    }
    execution_runner::staging::verify_worker_identity(&request.run_as, &request.session)?;
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
        .collect::<Result<Vec<_>, _>>()?;
    let before = if matches!(
        request.operation,
        WorkerOperation::Install
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
    #[cfg(target_os = "macos")]
    let outcome = macos::execute(&request, before.as_ref());
    #[cfg(windows)]
    let outcome = windows::execute(&request, before.as_ref());
    #[cfg(not(any(target_os = "macos", windows)))]
    let outcome: Result<(i32, SoftwareWorkerResult), Error> = Err(Error::Unsupported);
    let mut result = outcome.unwrap_or_else(|error| {
        let mut failed = result(1, None, error.to_string());
        failed.1.closed = !request
            .native_pending
            .load(std::sync::atomic::Ordering::Acquire);
        failed
    });
    if !matches!(request.action.behavior, wire::SoftwareTaskBehavior::Dmg(_))
        && request.resource_root.exists()
    {
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
#[cfg(windows)]
mod windows;

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
    if image.starts_with(&request.resource_root) {
        command.current_dir(image.parent().ok_or(Error::Protocol)?);
    }
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    request.native_pending.store(true, Ordering::Release);
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
            let _ = child.kill();
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
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
