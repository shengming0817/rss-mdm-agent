use super::*;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: PathBuf,
    pub sha256: String,
    /// macOS ad-hoc code-directory identity pinned by the administrator.
    pub cdhash: Option<String>,
}
impl Artifact {
    /// Verify the protected installed image at its exact administrator-selected path.
    pub fn verify(&self, actual: &Path) -> Result<(), Rejected> {
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
    pub fn requirement(&self) -> Result<std::ffi::CString, Rejected> {
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
    pub probe: Artifact,
    pub service: Artifact,
}
impl Policy {
    pub fn load() -> Result<Self, Rejected> {
        let (_, bytes) = read_policy(&policy_path()?)?;
        Self::parse(&bytes)
    }
    fn parse(bytes: &[u8]) -> Result<Self, Rejected> {
        let p: Self = serde_json::from_slice(bytes).map_err(|_| Rejected)?;
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
        p.probe.verify(&p.probe.path)?;
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
/// Validate the complete administrator-owned path chain without following symlinks/reparse points.
pub fn protected(path: &Path) -> Result<(), Rejected> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for entry in path.ancestors() {
            #[cfg(target_os = "macos")]
            super::macos::acl_empty(entry)?;
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

fn bounded_read(file: std::fs::File) -> Result<Vec<u8>, Rejected> {
    let meta = file.metadata().map_err(|_| Rejected)?;
    if !meta.is_file() || meta.len() > MAX_FRAME as u64 {
        return Err(Rejected);
    }
    let mut bytes = Vec::new();
    file.take(MAX_FRAME as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Rejected)?;
    if bytes.len() > MAX_FRAME {
        return Err(Rejected);
    }
    Ok(bytes)
}

fn read_policy(path: &Path) -> Result<(PathBuf, Vec<u8>), Rejected> {
    #[cfg(windows)]
    {
        let (file, ancestors, actual) = super::windows::open_protected(path, true)?;
        let bytes = bounded_read(file)?;
        drop(ancestors);
        Ok((actual, bytes))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        protected(path)?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| Rejected)?;
        let meta = file.metadata().map_err(|_| Rejected)?;
        if meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            return Err(Rejected);
        }
        #[cfg(target_os = "macos")]
        {
            // Compare the opened inode with the ACL-checked path; unprivileged
            // subjects cannot replace any component of the protected chain.
            let current = std::fs::symlink_metadata(path).map_err(|_| Rejected)?;
            if current.dev() != meta.dev() || current.ino() != meta.ino() {
                return Err(Rejected);
            }
        }
        Ok((path.to_owned(), bounded_read(file)?))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(Rejected)
    }
}

/// Read bounded administrator-owned deployment data through the existing native protection checks.
pub fn read_protected(path: &Path) -> Result<Vec<u8>, Rejected> {
    read_policy(path).map(|(_, bytes)| bytes)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Verification {
    version: u32,
    helper_version: &'static str,
    phase: &'static str,
    policy_path: Option<PathBuf>,
    policy_version: Option<u32>,
    permission_check: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    executable: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    negative: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
}

/// Internal laboratory CLI projection; this never starts a candidate or service.
pub fn write_verification_candidate(output: impl std::io::Write) -> Result<(), Rejected> {
    write_candidate(output, policy_path(), read_policy, Policy::parse)
}
fn write_candidate(
    output: impl std::io::Write,
    path: Result<PathBuf, Rejected>,
    read: impl FnOnce(&Path) -> Result<(PathBuf, Vec<u8>), Rejected>,
    parse: impl FnOnce(&[u8]) -> Result<Policy, Rejected>,
) -> Result<(), Rejected> {
    let mut report = Verification {
        version: 1,
        helper_version: env!("CARGO_PKG_VERSION"),
        phase: "rejected",
        policy_path: path.as_ref().ok().cloned(),
        policy_version: None,
        permission_check: "unavailable",
        executable: None,
        negative: None,
        reason: Some("policyPath"),
    };
    let result = (|| {
        let path = path?;
        report.permission_check = "failed";
        report.reason = Some("protection");
        let (actual, bytes) = read(&path)?;
        report.policy_path = Some(actual);
        report.permission_check = "passed";
        report.reason = Some("policyValidation");
        let policy = parse(&bytes)?;
        report.policy_version = Some(policy.version);
        report.executable = Some(policy.client.path);
        report.negative = Some(policy.probe.path);
        report.phase = "verified";
        report.reason = None;
        Ok(())
    })();
    serde_json::to_writer(output, &report).map_err(|_| Rejected)?;
    result
}

#[cfg(test)]
mod verification_tests {
    use super::*;
    #[test]
    fn successful_projection_contains_only_validated_candidates() {
        let artifact = Artifact {
            path: "/installed/client".into(),
            sha256: "a".repeat(64),
            cdhash: None,
        };
        let mut bytes = Vec::new();
        write_candidate(
            &mut bytes,
            Ok("/installed/policy".into()),
            |p| Ok((p.into(), Vec::new())),
            |_| {
                Ok(Policy {
                    version: 1,
                    installation: "id".into(),
                    build: "build".into(),
                    platform: "fixture".into(),
                    service_subject: "secret-subject".into(),
                    allowed_users: vec!["secret-user".into()],
                    client: artifact.clone(),
                    probe: artifact.clone(),
                    service: artifact,
                })
            },
        )
        .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["phase"], "verified");
        assert_eq!(report["permissionCheck"], "passed");
        assert_eq!(report["executable"], "/installed/client");
        assert!(report.get("reason").is_none());
        assert!(!String::from_utf8(bytes).unwrap().contains("secret"));
    }
    #[test]
    fn policy_read_is_bounded_before_allocating_the_input() {
        use std::io::{Seek, SeekFrom, Write};
        let path = std::env::temp_dir().join(format!("rss-policy-size-{}", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(b"{}").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(bounded_read(file.try_clone().unwrap()).unwrap(), b"{}");
        file.set_len(MAX_FRAME as u64 + 1).unwrap();
        assert!(bounded_read(file.try_clone().unwrap()).is_err());
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn source_rejection_never_parses_policy_or_releases_candidates() {
        let mut bytes = Vec::new();
        assert!(write_candidate(
            &mut bytes,
            Ok("/fixture/policy".into()),
            |_| Err(Rejected),
            |_| panic!("untrusted policy parsed")
        )
        .is_err());
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["phase"], "rejected");
        assert_eq!(report["policyPath"], "/fixture/policy");
        assert_eq!(report["permissionCheck"], "failed");
        assert!(report.get("executable").is_none());
    }
    #[test]
    fn missing_known_folder_never_reads_and_has_no_fallback_path() {
        let mut bytes = Vec::new();
        assert!(write_candidate(
            &mut bytes,
            Err(Rejected),
            |_| panic!("unexpected read"),
            |_| panic!("unexpected parse")
        )
        .is_err());
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(report["policyPath"].is_null());
        assert_eq!(report["reason"], "policyPath");
    }
    #[test]
    fn bad_policy_after_trusted_read_never_releases_candidates() {
        let mut bytes = Vec::new();
        assert!(write_candidate(
            &mut bytes,
            Ok("/fixture/policy".into()),
            |path| Ok((path.into(), b"invalid".to_vec())),
            Policy::parse
        )
        .is_err());
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["permissionCheck"], "passed");
        assert_eq!(report["reason"], "policyValidation");
        assert!(report.get("executable").is_none());
    }
}
