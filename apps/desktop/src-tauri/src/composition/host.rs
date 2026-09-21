//! The replaceable AI process. Execution, users and master-key ownership stay in DesktopRuntime.
// ref: tokio src/process/mod.rs@tokio-1.53.1 (explicit kill/wait rather than drop as exit evidence).
use super::{control::Control, credentials::MasterKey, execution::ExecutionHandle};
use crate::self_service::fixtures;
use execution_mcp::{ExecutionMcp, McpLimits};
use futures_util::StreamExt;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::{
    os::fd::AsRawFd,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{process::Command, task::JoinHandle};
use tokio_util::{
    codec::{FramedRead, LinesCodec},
    sync::CancellationToken,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Missing,
    Invalid,
    Version,
    Start,
    Exited,
    Timeout,
    Configuration,
    Authentication,
    Storage,
    Cleanup,
    Control,
}
impl Fault {
    pub fn diagnostic(
        self,
        source: ai_session_contract::HostStatusSource,
    ) -> ai_session_contract::HostDiagnostic {
        use ai_session_contract::{
            HostDiagnosticAction as A, HostDiagnosticCode as C, HostDiagnosticStage as S,
        };
        let package_action = if source == ai_session_contract::HostStatusSource::DevelopmentOverride
        {
            A::PrepareRuntime
        } else {
            A::ReinstallRuntime
        };
        let (stage, code, action) = match self {
            Self::Missing => (S::RuntimePackage, C::RuntimeMissing, package_action),
            Self::Invalid => (S::RuntimePackage, C::RuntimeInvalid, package_action),
            Self::Version => (S::RuntimePackage, C::UnsupportedVersion, package_action),
            Self::Start => (S::HostProcess, C::HostStartFailed, A::RestartHost),
            Self::Exited => (S::HostProcess, C::HostExited, A::RestartHost),
            Self::Timeout => (S::HostProcess, C::ReadinessTimeout, A::RestartHost),
            Self::Configuration => (
                S::Configuration,
                C::ConfigurationInvalid,
                A::CheckConfiguration,
            ),
            Self::Authentication => (
                S::Authentication,
                C::AuthenticationRequired,
                A::CheckCredentials,
            ),
            Self::Storage => (S::Storage, C::StorageCorrupt, A::CheckStorage),
            Self::Control => (S::HostProcess, C::ControlClosed, A::RestartHost),
            Self::Cleanup => (S::Shutdown, C::CleanupIncomplete, A::RestartHost),
        };
        ai_session_contract::HostDiagnostic {
            stage,
            code,
            action,
            at_ms: ai_session_contract::Counter(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .min(i64::MAX as u128) as i64,
            ),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Starting,
    Ready,
    Stopping,
    Stopped,
    Failed(Fault),
}
enum Waiter {
    Pending(JoinHandle<bool>),
    Reaped(bool),
}
pub struct Process {
    pub control: Arc<Control>,
    pub stop: CancellationToken,
    mcp_stop: CancellationToken,
    phase: Arc<Mutex<Phase>>,
    task: tokio::sync::Mutex<Waiter>,
}
impl Process {
    pub fn phase(&self) -> Phase {
        let phase = *self.phase.lock().unwrap();
        if phase == Phase::Ready && self.control.closed() {
            Phase::Failed(Fault::Control)
        } else {
            phase
        }
    }
    pub fn ready(&self) -> bool {
        self.phase() == Phase::Ready
    }
    pub async fn close(&self) -> bool {
        let mut waiter = self.task.lock().await;
        if let Waiter::Reaped(result) = *waiter {
            return result;
        }
        let Waiter::Pending(task) = std::mem::replace(&mut *waiter, Waiter::Reaped(false)) else {
            unreachable!()
        };
        {
            let mut phase = self.phase.lock().unwrap();
            // A startup diagnostic may arrive before health notices EOF. Do not
            // erase it while the waiter drains the remaining diagnostic pipe.
            if !matches!(*phase, Phase::Failed(_)) {
                *phase = Phase::Stopping;
            }
        }
        self.control.close();
        self.mcp_stop.cancel();
        self.stop.cancel();
        let reaped = task.await.unwrap_or(false);
        if !reaped {
            *self.phase.lock().unwrap() = Phase::Failed(Fault::Cleanup);
        }
        *waiter = Waiter::Reaped(reaped);
        reaped
    }
}
fn preflight(artifact: &Path, trusted_digest: Option<&str>) -> Result<(), Fault> {
    use std::os::unix::fs::PermissionsExt;
    let executable = artifact.join("bin/rss-ai-host");
    let meta = std::fs::symlink_metadata(executable).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Fault::Missing
        } else {
            Fault::Invalid
        }
    })?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.permissions().mode() & 0o111 == 0 {
        return Err(Fault::Invalid);
    }
    let root = artifact.canonicalize().map_err(|_| Fault::Invalid)?;
    for file in [
        "bin/node",
        "package.json",
        "pnpm-lock.yaml",
        "node_modules/@rss-mdm-agent/ai-host-app/dist/cli.js",
        "node_modules/@rss-mdm-agent/ai-host/dist/index.js",
        "node_modules/@rss-mdm-agent/ai-contract/dist/index.js",
        "node_modules/@rss-mdm-agent/ai-store-sqlite/dist/index.js",
        "node_modules/@rss-mdm-agent/ai-access/dist/index.js",
    ] {
        let path = artifact.join(file);
        let resolved = path.canonicalize().map_err(|_| Fault::Invalid)?;
        let metadata = std::fs::metadata(&resolved).map_err(|_| Fault::Invalid)?;
        if !resolved.starts_with(&root)
            || !metadata.is_file()
            || (file == "bin/node" && metadata.permissions().mode() & 0o111 == 0)
        {
            return Err(Fault::Invalid);
        }
    }
    let bytes = std::fs::read(artifact.join("manifest.json")).map_err(|_| Fault::Invalid)?;
    if bytes.len() > 1024 * 1024 {
        return Err(Fault::Invalid);
    }
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|_| Fault::Invalid)?;
    if manifest["status"] != "passed"
        || manifest["desktopProtocol"] != i64::from(ai_session_contract::HostHealthProtocol::VALUE)
        || manifest["contractVersion"] != 5
    {
        return Err(Fault::Version);
    }
    if manifest["verification"]["platform"] != "darwin"
        || manifest["verification"]["arch"] != "arm64"
        || !cfg!(all(target_os = "macos", target_arch = "aarch64"))
    {
        return Err(Fault::Version);
    }
    super::runtime_package::verify(
        artifact,
        manifest["runtimeTreeSha256"]
            .as_str()
            .ok_or(Fault::Invalid)?,
        trusted_digest,
    )
    .map_err(|_| Fault::Invalid)?;
    Ok(())
}
fn startup_fault(line: &str) -> Option<Fault> {
    use ai_session_contract::{HostProcessDiagnosticCode as C, WireRecord};
    let Ok(WireRecord::HostProcessDiagnostic(frame)) = ai_session_contract::decode(
        line.as_bytes(),
        &ai_session_contract::Limits {
            max_bytes: 256,
            max_text_bytes: 128,
            max_depth: 4,
            max_nodes: 16,
        },
    ) else {
        return None;
    };
    Some(match frame.code {
        C::ConfigurationInvalid => Fault::Configuration,
        C::AuthenticationRequired => Fault::Authentication,
        C::StorageCorrupt => Fault::Storage,
        C::UnsupportedVersion => Fault::Version,
        C::HostStartFailed => Fault::Start,
        C::CleanupIncomplete => Fault::Cleanup,
    })
}
pub async fn launch(
    artifact: &Path,
    configuration: &Path,
    execution: &ExecutionHandle,
    master: Arc<MasterKey>,
    trusted_digest: Option<&str>,
) -> Result<Arc<Process>, Fault> {
    let checked = artifact.to_path_buf();
    let trusted = trusted_digest.map(str::to_owned);
    tokio::task::spawn_blocking(move || preflight(&checked, trusted.as_deref()))
        .await
        .map_err(|_| Fault::Invalid)??;
    let (parent_pipe, child_pipe) =
        std::os::unix::net::UnixStream::pair().map_err(|_| Fault::Start)?;
    parent_pipe
        .set_nonblocking(true)
        .map_err(|_| Fault::Start)?;
    let control = Control::start(
        tokio::net::UnixStream::from_std(parent_pipe).map_err(|_| Fault::Start)?,
        master,
    );
    let mut command = Command::new(artifact.join("bin/rss-ai-host"));
    command
        .arg(configuration)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let fd = child_pipe.as_raw_fd();
    // SAFETY: only async-signal-safe descriptor operations occur in the child before exec.
    unsafe {
        command.pre_exec(move || {
            if fd != 3 && libc::dup2(fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let launched = command.spawn();
    drop(child_pipe);
    let mut child = match launched {
        Ok(child) => child,
        Err(_) => {
            control.close();
            return Err(Fault::Start);
        }
    };
    let reader = child.stdout.take().expect("piped stdout");
    let writer = child.stdin.take().expect("piped stdin");
    let diagnostics = child.stderr.take().expect("piped stderr");
    let phase = Arc::new(Mutex::new(Phase::Starting));
    let was_ready = Arc::new(AtomicBool::new(false));
    let waiter_was_ready = was_ready.clone();
    let diagnostic_phase = phase.clone();
    let cleanup = Arc::new(AtomicBool::new(false));
    let reported_cleanup = cleanup.clone();
    let diagnostic_task = tokio::spawn(async move {
        let mut lines = FramedRead::new(diagnostics, LinesCodec::new_with_max_length(256));
        while let Some(line) = lines.next().await {
            if let Ok(line) = line {
                if let Some(fault) = startup_fault(&line) {
                    if fault == Fault::Cleanup {
                        reported_cleanup.store(true, Ordering::Release);
                        continue;
                    }
                    let mut phase = diagnostic_phase.lock().unwrap();
                    if matches!(
                        *phase,
                        Phase::Starting
                            | Phase::Stopping
                            | Phase::Failed(Fault::Timeout | Fault::Exited)
                    ) {
                        *phase = Phase::Failed(fault);
                    }
                }
            }
        }
    });
    let limits = McpLimits {
        frame_bytes: 262144,
        response_bytes: 262144,
        json_depth: 64,
        json_nodes: 16384,
        in_flight: 16,
        session_frames: 100000,
        request_timeout: Duration::from_secs(10),
        io_timeout: Duration::from_secs(3600),
        catalog: fixtures::CATALOG_LIMITS,
        parameters: fixtures::PARAMETERS,
    };
    let server = ExecutionMcp::new(Arc::new(execution.clone()), limits).expect("fixed MCP limits");
    let mcp_stop = CancellationToken::new();
    let stop = mcp_stop.clone();
    let mcp_task = tokio::spawn(async move {
        let _ = server.serve(reader, writer, stop).await;
    });
    let stop = CancellationToken::new();
    let waiter_stop = stop.clone();
    let process_mcp_stop = mcp_stop.clone();
    let waiter_phase = phase.clone();
    let waiter_control = control.clone();
    let task = tokio::spawn(async move {
        let (result, forced) = tokio::select! {
            result = child.wait() => (result, false),
            _ = waiter_stop.cancelled() => {
                if let Some(pid) = child.id() {
                    // SAFETY: the Child is still owned and has not been reaped; this is not a persisted PID.
                    unsafe { libc::kill(pid as i32, libc::SIGTERM); }
                }
                match tokio::time::timeout(Duration::from_secs(8), child.wait()).await {
                    Ok(result) => (result, false),
                    Err(_) => { let _ = child.start_kill(); (child.wait().await, true) }
                }
            }
        };
        let requested = waiter_stop.is_cancelled();
        waiter_stop.cancel();
        waiter_control.shutdown().await;
        mcp_stop.cancel();
        let mut mcp_task = mcp_task;
        if tokio::time::timeout(Duration::from_secs(1), &mut mcp_task)
            .await
            .is_err()
        {
            mcp_task.abort();
            let _ = mcp_task.await;
        }
        // A descendant retaining stderr cannot hold the owner forever.
        let mut diagnostic_task = diagnostic_task;
        if tokio::time::timeout(Duration::from_secs(1), &mut diagnostic_task)
            .await
            .is_err()
        {
            diagnostic_task.abort();
            let _ = diagnostic_task.await;
        }
        let failed_exit = result
            .as_ref()
            .is_ok_and(|status| status.code().is_some_and(|code| code != 0));
        let mut phase = waiter_phase.lock().unwrap();
        if !matches!(
            *phase,
            Phase::Failed(
                Fault::Configuration
                    | Fault::Authentication
                    | Fault::Storage
                    | Fault::Version
                    | Fault::Start
            )
        ) {
            *phase = if result.is_err()
                || forced
                || cleanup.load(Ordering::Acquire)
                || (requested && waiter_was_ready.load(Ordering::Acquire) && failed_exit)
            {
                Phase::Failed(Fault::Cleanup)
            } else if failed_exit {
                // Startup EOF can request cleanup before the exit waiter runs.
                Phase::Failed(Fault::Exited)
            } else if requested {
                Phase::Stopped
            } else {
                Phase::Failed(Fault::Exited)
            };
        }
        result.is_ok()
    });
    let process = Arc::new(Process {
        control,
        stop,
        mcp_stop: process_mcp_stop,
        phase,
        task: tokio::sync::Mutex::new(Waiter::Pending(task)),
    });
    let health = process.control.health().await;
    if health.is_ok() {
        let mut phase = process.phase.lock().unwrap();
        if *phase == Phase::Starting {
            was_ready.store(true, Ordering::Release);
            *phase = Phase::Ready;
        }
    }
    if !process.ready() {
        let fault = match process.phase() {
            Phase::Failed(fault) => fault,
            _ if health.is_err_and(|e| e.code == "unsupported_version") => Fault::Version,
            _ if process.control.closed() => Fault::Exited,
            _ => Fault::Timeout,
        };
        // Revoke capabilities before waiting for exit. An uncertain reap retains
        // this failed owner so restart cannot admit a competing incarnation.
        if process.close().await {
            let fault = match process.phase() {
                Phase::Failed(Fault::Exited) if fault == Fault::Timeout => Fault::Exited,
                Phase::Failed(fault) if fault != Fault::Exited => fault,
                _ => fault,
            };
            return Err(fault);
        }
    }
    Ok(process)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_diagnostics_accept_only_the_generated_closed_frame() {
        for (code, fault) in [
            ("configuration_invalid", Fault::Configuration),
            ("authentication_required", Fault::Authentication),
            ("storage_corrupt", Fault::Storage),
            ("unsupported_version", Fault::Version),
            ("host_start_failed", Fault::Start),
            ("cleanup_incomplete", Fault::Cleanup),
        ] {
            let frame =
                json!({"schemaVersion":5,"kind":"hostProcessDiagnostic","code":code}).to_string();
            assert_eq!(startup_fault(&frame), Some(fault));
        }
        for raw in [
            "AI Host could not start: storage_corrupt".to_owned(),
            json!({"schemaVersion":5,"kind":"hostProcessDiagnostic","code":"CANARY"}).to_string(),
            json!({"schemaVersion":5,"kind":"hostProcessDiagnostic","code":"storage_corrupt","detail":"CANARY"}).to_string(),
            "x".repeat(1024),
        ] { assert_eq!(startup_fault(&raw), None); }
    }
    #[tokio::test]
    async fn unconfirmed_reaping_cannot_be_reclassified_as_stopped_on_a_later_close() {
        let (stream, _peer) = tokio::net::UnixStream::pair().unwrap();
        let control = Control::start(stream, Arc::new(MasterKey::default()));
        let task = tokio::spawn(std::future::pending::<bool>());
        task.abort();
        let process = Process {
            control,
            stop: CancellationToken::new(),
            mcp_stop: CancellationToken::new(),
            phase: Arc::new(Mutex::new(Phase::Ready)),
            task: tokio::sync::Mutex::new(Waiter::Pending(task)),
        };
        assert!(!process.close().await);
        assert!(!process.close().await);
        assert_eq!(process.phase(), Phase::Failed(Fault::Cleanup));
        process.control.shutdown().await;
    }
}
