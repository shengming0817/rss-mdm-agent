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

pub fn deployment_path() -> Result<PathBuf, Rejected> {
    #[cfg(target_os = "macos")]
    {
        Ok(PathBuf::from(
            "/Library/Application Support/RSS MDM Agent/execution.json",
        ))
    }
    #[cfg(windows)]
    {
        super::windows::deployment_path()
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

fn read_installation(path: &Path) -> Result<(PathBuf, Vec<u8>), Rejected> {
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
    read_installation(path).map(|(_, bytes)| bytes)
}

// ref: rust-lang/rust library/std/src/sys/fs/windows.rs (FileType::is_dir/is_file).
#[cfg(any(windows, test))]
pub(crate) fn path_type_allowed(leaf: bool, read: bool, kind: std::fs::FileType) -> bool {
    if leaf {
        kind.is_file() || (!read && kind.is_dir())
    } else {
        kind.is_dir()
    }
}

#[cfg(test)]
mod path_type_tests {
    use super::*;
    #[test]
    fn protected_directory_is_not_a_readable_file_and_ancestors_must_be_directories() {
        let root = std::env::temp_dir().join(format!("installation-leaf-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let file = root.join("execution.json");
        std::fs::write(&file, b"{}").unwrap();
        let directory_type = root.metadata().unwrap().file_type();
        let file_type = file.metadata().unwrap().file_type();
        assert!(path_type_allowed(true, false, directory_type));
        assert!(!path_type_allowed(true, true, directory_type));
        assert!(path_type_allowed(true, true, file_type));
        assert!(path_type_allowed(true, false, file_type));
        assert!(path_type_allowed(false, true, directory_type));
        assert!(!path_type_allowed(false, false, file_type));
        std::fs::remove_dir_all(root).unwrap();
    }
}
