//! Product-private fixed worker launcher and read-only process-scope probes.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};
pub mod private_storage;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Scope {
    ProcessGroup { root: u32 },
    JobObject { name: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Ready {
    pub version: u32,
    pub launch_id: String,
    pub launcher_pid: u32,
    pub worker_pid: u32,
    pub scope: Scope,
    pub artifact: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    node: String,
    bootstrap: String,
    node_sha256: String,
    bootstrap_sha256: String,
}
fn file(root: &Path, path: &str, expected: &str) -> io::Result<PathBuf> {
    let relative = Path::new(path);
    if expected.len() != 64
        || relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(io::Error::other("invalid fixed artifact"));
    }
    let path = root.join(relative);
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || format!("{:x}", Sha256::digest(std::fs::read(&path)?)) != expected
    {
        return Err(io::Error::other("fixed artifact mismatch"));
    }
    Ok(path)
}
/// Missing is the only affirmative recovery evidence. Access denied and unknown
/// platform never authorize clearing a durable fence.
pub fn absent(scope: &Scope) -> bool {
    platform::absent(scope)
}

pub fn launch(id: &str) -> io::Result<()> {
    if id.len() != 36 || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return Err(io::Error::other("launch identity"));
    }
    let executable = std::env::current_exe()?;
    let root = executable
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| io::Error::other("runtime root"))?;
    let bytes = std::fs::read(root.join("worker-manifest.json"))?;
    if bytes.len() > 16384 {
        return Err(io::Error::other("manifest limit"));
    }
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    if manifest.version != 1 {
        return Err(io::Error::other("manifest version"));
    }
    let node = file(root, &manifest.node, &manifest.node_sha256)?;
    let bootstrap = file(root, &manifest.bootstrap, &manifest.bootstrap_sha256)?;
    let mut command = Command::new(node);
    command.arg(bootstrap).arg(id).env_clear();
    for name in [
        "PATH",
        "HOME",
        "TMPDIR",
        "SystemRoot",
        "USERPROFILE",
        "LOCALAPPDATA",
        "TEMP",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::null());
    let mut owner = platform::Owner::spawn(&mut command, id)?;
    let ready = Ready {
        version: 1,
        launch_id: id.into(),
        launcher_pid: std::process::id(),
        worker_pid: owner.child().id(),
        scope: owner.scope(),
        artifact: format!("{:x}", Sha256::digest(bytes)),
    };
    eprintln!("{}", serde_json::to_string(&ready)?);
    loop {
        if owner.parent_gone() || owner.child().try_wait()?.is_some() {
            owner.terminate()?;
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Native owns the current Host child and its scope, never a persisted process ID.
pub struct OwnedHost {
    inner: platform::HostScope,
}
impl OwnedHost {
    pub fn prepare(command: &mut Command) -> io::Result<Self> {
        platform::HostScope::prepare(command).map(|inner| Self { inner })
    }
    pub fn attach(&mut self, pid: u32) -> io::Result<()> {
        self.inner.attach(pid)
    }
    pub fn request_stop(&self) -> io::Result<()> {
        self.inner.request_stop()
    }
    pub fn terminate(&self) -> io::Result<()> {
        self.inner.terminate()
    }
    pub fn empty(&self) -> bool {
        self.inner.empty()
    }
}
