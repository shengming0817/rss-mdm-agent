use super::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: PathBuf,
    pub sha256: String,
    /// macOS ad-hoc code-directory identity pinned by the administrator.
    pub cdhash: Option<String>,
}
impl Artifact {
    pub(crate) fn verify(&self, actual: &Path) -> Result<(), Rejected> {
        if !self.path.is_absolute() || actual != self.path || self.sha256.len() != 64 {
            return Err(Rejected);
        }
        protected(&self.path)?;
        let bytes = std::fs::read(&self.path).map_err(|_| Rejected)?;
        if format!("{:x}", Sha256::digest(&bytes)) != self.sha256 {
            return Err(Rejected);
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn requirement(&self) -> Result<std::ffi::CString, Rejected> {
        let cdhash = self.cdhash.as_deref().ok_or(Rejected)?;
        if cdhash.len() != 40 || !cdhash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Rejected);
        }
        std::ffi::CString::new(format!("cdhash H\"{cdhash}\"")).map_err(|_| Rejected)
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    pub installation: String,
    pub build: String,
    pub platform: String,
    pub service_subject: String,
    pub allowed_users: Vec<String>,
    pub client: Artifact,
    pub service: Artifact,
}
impl Policy {
    pub fn load() -> Result<Self, Rejected> {
        let path = policy_path()?;
        protected(&path)?;
        let bytes = std::fs::read(path).map_err(|_| Rejected)?;
        if bytes.len() > MAX_FRAME {
            return Err(Rejected);
        }
        let p: Self = serde_json::from_slice(&bytes).map_err(|_| Rejected)?;
        let platform = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            "macos-arm64"
        } else if cfg!(all(windows, target_arch = "x86_64")) {
            "windows-x64"
        } else {
            return Err(Rejected);
        };
        if p.version != VERSION
            || p.platform != platform
            || p.allowed_users.len() > 64
            || p.installation.is_empty()
            || p.build.is_empty()
            || p.allowed_users
                .iter()
                .any(|u| u.len() > 256 || u.is_empty())
        {
            return Err(Rejected);
        }
        p.client.verify(&p.client.path)?;
        p.service.verify(&p.service.path)?;
        Ok(p)
    }
    pub(crate) fn status(&self) -> Status {
        Status {
            version: VERSION,
            installation: self.installation.clone(),
            build: self.build.clone(),
            platform: self.platform.clone(),
            capability: Capability::StatusOnly,
        }
    }
}
pub fn policy_path() -> Result<PathBuf, Rejected> {
    #[cfg(target_os = "macos")]
    {
        Ok(PathBuf::from(
            "/Library/Application Support/RSS MDM Agent/service/policy.json",
        ))
    }
    #[cfg(windows)]
    {
        super::windows::policy_path()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(Rejected)
    }
}
pub(crate) fn protected(path: &Path) -> Result<(), Rejected> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for entry in path.ancestors() {
            let metadata = std::fs::symlink_metadata(entry).map_err(|_| Rejected)?;
            if metadata.file_type().is_symlink()
                || metadata.uid() != 0
                || metadata.mode() & 0o022 != 0
            {
                return Err(Rejected);
            }
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        super::windows::protected(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(Rejected)
    }
}
