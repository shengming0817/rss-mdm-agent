//! One installation owner; packages and lab harnesses only invoke this boundary.
//! ref: tauri tauri-cli-v2.11.2 crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi.
mod lifecycle;
mod unmanage;
pub(crate) use unmanage::{launch as launch_unmanage, supported as unmanage_supported};
#[cfg(windows)]
mod msix;
#[cfg(windows)]
pub use msix::initialize as initialize_packaged;
#[cfg(target_os = "macos")]
mod macos;
mod outcome;
#[cfg(any(windows, test))]
mod task_document;
#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
mod worker_evidence;
use crate::{deployment::Deployment, ExecutionConfig, SystemClock};
use agent_client::{Clock, Error, GrantImport};
use installation_security::{Artifact, Target};
#[cfg(target_os = "macos")]
use macos as platform;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[cfg(windows)]
use windows as platform;
#[cfg(windows)]
pub use windows::worker;

/// Public fixed package identity. No device identity, grant or secret is part of this candidate.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub version: u8,
    pub publication_sha256: String,
    pub product_version: String,
    pub target: Target,
    pub root: PathBuf,
    pub service: Artifact,
    pub desktop: Artifact,
    pub runtime_root: PathBuf,
    pub runtime_sha256: String,
    pub origin: String,
    pub tenant: uuid::Uuid,
    pub signing_keys: BTreeMap<String, String>,
    pub ca_pem: Option<String>,
    pub signing_mode: SigningMode,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SigningMode {
    Development,
    Release,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub subjects: Vec<String>,
    pub helper_work_roots: BTreeMap<String, PathBuf>,
}
pub fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl Candidate {
    fn retained(config: &Path, deployment: &Deployment) -> Result<Self, Error> {
        let path = config
            .parent()
            .ok_or(Error::Configuration)?
            .join("install-manifests")
            .join(format!("{}.json", deployment.installation_artifact));
        let bytes = installation_security::read_protected(&path).map_err(|_| Error::Identity)?;
        if hash(&bytes) != deployment.installation_artifact {
            return Err(Error::Identity);
        }
        let candidate: Self = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        if candidate.version != 3
            || candidate.service.path != deployment.service.path
            || candidate.origin != deployment.origin
            || candidate.tenant != deployment.tenant
            || candidate.target != Target::current().map_err(|_| Error::Unsupported)?
        {
            return Err(Error::Identity);
        }
        Ok(candidate)
    }
    fn load(path: &Path) -> Result<(Self, String), Error> {
        let bytes =
            installation_security::read_protected(path).map_err(|_| Error::Configuration)?;
        let candidate: Self = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        candidate.validate()?;
        Ok((candidate, hash(&bytes)))
    }
    fn validate(&self) -> Result<(), Error> {
        if self.version != 3
            || self.target != Target::current().map_err(|_| Error::Unsupported)?
            || !self.root.is_absolute()
            || self.tenant.is_nil()
            || self.product_version.is_empty()
            || self.product_version.len() > 64
            || !digest_valid(&self.publication_sha256)
            || !digest_valid(&self.runtime_sha256)
            || !self.service.path.starts_with(&self.root)
            || !self.desktop.path.starts_with(&self.root)
            || !self.runtime_root.starts_with(&self.root)
            || self.service.path == self.desktop.path
            || self.signing_keys.is_empty()
        {
            return Err(Error::Configuration);
        }
        agent_client::validate_public_ca(
            self.ca_pem.as_ref().ok_or(Error::Configuration)?.as_bytes(),
        )?;
        if self.signing_keys.len() > 32
            || self.signing_keys.iter().any(|(id, key)| {
                use base64::Engine;
                id.is_empty()
                    || id.len() > 128
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    || base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .decode(key)
                        .map_or(true, |k| k.len() != 32)
            })
        {
            return Err(Error::Configuration);
        }
        let origin = url::Url::parse(&self.origin).map_err(|_| Error::Configuration)?;
        if origin.scheme() != "https"
            || origin.origin().ascii_serialization() != self.origin
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err(Error::Configuration);
        }
        for image in [&self.service, &self.desktop] {
            image.verify(&image.path).map_err(|_| Error::Identity)?;
            self.target
                .verify_image(&image.path)
                .map_err(|_| Error::Identity)?;
        }
        installation_security::protected(&self.runtime_root).map_err(|_| Error::Identity)?;
        installation_security::runtime::verify(
            &self.runtime_root,
            &self.runtime_sha256,
            Some(&self.runtime_sha256),
        )?;
        let suffix = if self.target.platform() == "win32" {
            ".exe"
        } else {
            ""
        };
        for name in ["node", "rss-ai-worker-launcher", "rss-private-storage"] {
            self.target
                .verify_image(
                    &self
                        .runtime_root
                        .join("bin")
                        .join(format!("{name}{suffix}")),
                )
                .map_err(|_| Error::Identity)?;
        }
        verify_runtime_files(&self.runtime_root, self.target)?;
        platform::verify_signatures(self)?;
        Ok(())
    }
    fn format(&self) -> Result<String, Error> {
        let bytes = installation_security::read_protected(&self.root.join("publication.json"))
            .map_err(|_| Error::Identity)?;
        if hash(&bytes) != self.publication_sha256 {
            return Err(Error::Identity);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
        Ok(value["packaging"]["kind"]
            .as_str()
            .ok_or(Error::Protocol)?
            .into())
    }
    fn deployment(
        &self,
        artifact: String,
        profile: Profile,
        config: &Path,
    ) -> Result<Deployment, Error> {
        let base = config.parent().ok_or(Error::Configuration)?;
        if profile.subjects.len() > 64
            || profile
                .subjects
                .iter()
                .any(|s| s.is_empty() || matches!(s.as_str(), "0" | "S-1-5-18"))
            || profile
                .helper_work_roots
                .iter()
                .any(|(s, p)| !profile.subjects.contains(s) || !p.is_absolute())
        {
            return Err(Error::Configuration);
        }
        let mut interpreters = vec![];
        let mut managers = vec![];
        platform::execution_resources(&mut interpreters, &mut managers)?;
        let clients = execution_ipc::host::PeerPolicy {
            images: vec![self.desktop.clone(), self.service.clone()],
            subjects: profile.subjects.clone(),
            interactive: true,
        };
        Ok(Deployment {
            version: execution_ipc::host::DEPLOYMENT_VERSION,
            ipc_version: execution_ipc::host::IPC_VERSION,
            origin: self.origin.clone(),
            tenant: self.tenant,
            signing_keys: self.signing_keys.clone(),
            ca_file: self.ca_pem.as_ref().map(|_| base.join("https-ca.pem")),
            installation_id: uuid::Uuid::new_v4(),
            installation_artifact: artifact,
            state_root: base.join("state"),
            service: self.service.clone(),
            clients,
            registration_clients: execution_ipc::host::PeerPolicy {
                images: vec![self.desktop.clone()],
                subjects: profile.subjects,
                interactive: true,
            },
            execution: ExecutionConfig {
                work_root: base.join("work"),
                material_root: base.join("materials"),
                interpreters,
                managers,
                processes: 8,
            },
            helper_work_roots: profile.helper_work_roots,
        })
    }
}
/// Move only the immutable version selected by the protected deployment out of the install namespace.
/// Windows' NSIS carrier deletes this exact retired directory after the worker exits.
fn retire_payload(candidate: &Candidate) -> Result<PathBuf, Error> {
    let versions = candidate.root.parent().ok_or(Error::Configuration)?;
    let retired = versions
        .parent()
        .ok_or(Error::Configuration)?
        .join("retired");
    platform::public_directory(&retired)?;
    let target = retired.join(candidate.root.file_name().ok_or(Error::Configuration)?);
    match std::fs::symlink_metadata(&candidate.root) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.is_symlink() || target.exists() {
                return Err(Error::Identity);
            }
            installation_security::protected(&candidate.root).map_err(|_| Error::Identity)?;
            std::fs::rename(&candidate.root, &target)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if target.exists() {
                installation_security::protected(&target).map_err(|_| Error::Identity)?;
            }
        }
        Err(e) => return Err(e.into()),
    }
    Ok(target)
}
/// Immutable publication identity retained in the protected local installation manifest.
pub(crate) fn publication_digest(deployment: &Deployment) -> Result<[u8; 32], Error> {
    let path = deployment
        .service
        .path
        .parent()
        .ok_or(Error::Configuration)?
        .join("candidate.json");
    let manifest = installation_security::read_protected(&path).map_err(|_| Error::Identity)?;
    if hash(&manifest) != deployment.installation_artifact {
        return Err(Error::Identity);
    }
    let candidate: Candidate =
        serde_json::from_slice(&manifest).map_err(|_| Error::Configuration)?;
    if candidate.version != 3
        || candidate.origin != deployment.origin
        || candidate.tenant != deployment.tenant
        || candidate.service.path != deployment.service.path
        || !digest_valid(&candidate.publication_sha256)
    {
        return Err(Error::Identity);
    }
    let bytes = installation_security::read_protected(&candidate.root.join("publication.json"))
        .map_err(|_| Error::Identity)?;
    if hash(&bytes) != candidate.publication_sha256 {
        return Err(Error::Identity);
    }
    Ok(Sha256::digest(&bytes).into())
}
/// Every native utility uses a fixed absolute path and bounded execution; output is never logged.
fn utility(path: &Path, args: &[OsString]) -> Result<(std::process::ExitStatus, Vec<u8>), Error> {
    if !path.is_absolute() {
        return Err(Error::Configuration);
    }
    let mut child = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().ok_or(Error::Unavailable)?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(1_048_577)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err(Error::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let bytes = reader.join().map_err(|_| Error::Unavailable)??;
    if bytes.len() > 1_048_576 {
        return Err(Error::Unavailable);
    }
    Ok((status, bytes))
}
fn command(path: &Path, args: &[OsString]) -> Result<Vec<u8>, Error> {
    let (status, bytes) = utility(path, args)?;
    if !status.success() {
        outcome::native_failure(status.code());
        return Err(Error::Unavailable);
    }
    Ok(bytes)
}
fn read_image(path: &Path, digest: &str) -> Result<Vec<u8>, Error> {
    const IMAGE_BYTES: u64 = 256 * 1024 * 1024;
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > IMAGE_BYTES {
        return Err(Error::Capacity);
    }
    let mut bytes = Vec::new();
    file.take(IMAGE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > IMAGE_BYTES {
        return Err(Error::Capacity);
    }
    if hash(&bytes) != digest {
        return Err(Error::Identity);
    }
    Ok(bytes)
}
fn json<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(|_| Error::Configuration)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    installation_security::protected(path.parent().ok_or(Error::Configuration)?)
        .map_err(|_| Error::Identity)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        installation_security::protected(path).map_err(|_| Error::Identity)
    })();
    if result.is_err() {
        drop(file);
        let _ = std::fs::remove_file(path);
    }
    result
}
/// Publish a complete immutable file. Interrupted staging never occupies the final name.
fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temporary = path.with_extension(format!("pending-{}", uuid::Uuid::new_v4()));
    write_new(&temporary, bytes)?;
    let result = (|| {
        std::fs::hard_link(&temporary, path)?;
        #[cfg(unix)]
        std::fs::File::open(path.parent().ok_or(Error::Configuration)?)?.sync_all()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}
fn retain_public(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {
            if installation_security::read_protected(path).map_err(|_| Error::Identity)? != bytes {
                return Err(Error::Identity);
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => publish_new(path, bytes),
        Err(e) => Err(e.into()),
    }
}
fn publish(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temporary = path.with_extension(format!("pending-{}", uuid::Uuid::new_v4()));
    write_new(&temporary, bytes)?;
    let result = platform::replace(&temporary, path);
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
/// Closed CLI used by both fixed installers and platform acceptance harnesses.
pub fn run(args: &[OsString]) -> Result<(), Error> {
    #[cfg(windows)]
    let _apartment = windows::ComApartment::new()?;
    let result = lifecycle::run(args);
    if let Err(error) = result {
        outcome::failed(error);
        eprintln!(
            "{}",
            serde_json::to_string(&outcome::snapshot()).map_err(|_| Error::Protocol)?
        );
    }
    result
}

#[cfg(test)]
mod tests;

pub(crate) fn validate_native_artifact(image: &Artifact) -> Result<(), Error> {
    Target::current()
        .map_err(|_| Error::Unsupported)?
        .verify_image(&image.path)
        .map_err(|_| Error::Identity)?;
    #[cfg(target_os = "macos")]
    {
        let cdhash = image.cdhash.as_deref().ok_or(Error::Identity)?;
        if cdhash.len() != 40 || !cdhash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Identity);
        }
        command(
            Path::new("/usr/bin/codesign"),
            &[
                "--verify".into(),
                "--strict".into(),
                "-R".into(),
                format!("=cdhash H\"{cdhash}\"").into(),
                image.path.clone().into(),
            ],
        )?;
    }
    Ok(())
}

fn verify_runtime_files(root: &Path, target: Target) -> Result<(), Error> {
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let kind = entry.file_type()?;
            let actual = if kind.is_symlink() {
                std::fs::canonicalize(&path)?
            } else {
                path.clone()
            };
            if !actual.starts_with(root) {
                return Err(Error::Identity);
            }
            installation_security::protected(&actual).map_err(|_| Error::Identity)?;
            if kind.is_dir() {
                directories.push(path);
            } else if target.platform() == "win32"
                && path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| matches!(s, "exe" | "dll" | "node"))
            {
                target.verify_image(&actual).map_err(|_| Error::Identity)?;
            }
        }
    }
    Ok(())
}
