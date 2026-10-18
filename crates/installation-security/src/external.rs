//! Untrusted, bounded input. This does not confer installation or enrollment authority.
//! ref: rust-lang/rust library/std/src/os/unix/fs.rs@1159e78c4747b02ef996e55082b704c09b970588.
use std::{
    fs::File,
    io::{self, Read},
    path::{Component, Path},
};

/// Read external JSON through retained directory/file handles, refusing links and special files.
/// Callers must authenticate the returned contents independently before acting on them.
pub fn read_external(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || limit == 0
        || limit > 65536
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let (mut file, _parents) = open(path)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() > limit as u64 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut bytes = Vec::new();
    (&mut file).take(limit as u64 + 1).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() > limit
        || before.len() != after.len()
        || before.modified()? != after.modified()?
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}
#[cfg(unix)]
fn open(path: &Path) -> io::Result<(File, Vec<File>)> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::OpenOptionsExt},
        },
    };
    let mut path = path.to_path_buf();
    // macOS system aliases only; no caller-controlled canonicalize/symlink traversal.
    #[cfg(target_os = "macos")]
    for alias in ["/var", "/tmp", "/etc"] {
        if let Ok(rest) = path.strip_prefix(alias) {
            path = Path::new("/private")
                .join(alias.trim_start_matches('/'))
                .join(rest);
            break;
        }
    }
    let mut parent = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")?;
    let names: Vec<_> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(v) => Some(v),
            _ => None,
        })
        .collect();
    let mut retained = vec![];
    for (i, name) in names.iter().enumerate() {
        let name = CString::new(name.as_bytes()).map_err(io::Error::other)?;
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if i + 1 < names.len() {
                libc::O_DIRECTORY
            } else {
                0
            };
        // SAFETY: live parent descriptor and NUL-terminated single component; successful fd has one owner.
        let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let next = unsafe { File::from_raw_fd(fd) };
        retained.push(parent);
        parent = next;
    }
    Ok((parent, retained))
}
#[cfg(windows)]
fn open(path: &Path) -> io::Result<(File, Vec<File>)> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_READ,
    };
    let mut current = std::path::PathBuf::new();
    let mut handles = vec![];
    for part in path.components() {
        match part {
            Component::Prefix(p) => {
                if !matches!(
                    p.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) {
                    return Err(io::ErrorKind::InvalidInput.into());
                }
                current.push(part.as_os_str());
                continue;
            }
            Component::RootDir | Component::Normal(_) => current.push(part.as_os_str()),
            _ => return Err(io::ErrorKind::InvalidInput.into()),
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(&current)?;
        if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        handles.push(file);
    }
    let file = handles.pop().ok_or(io::ErrorKind::InvalidInput)?;
    Ok((file, handles))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    #[test]
    fn external_input_is_bounded_and_rejects_links_ancestors_and_special_files() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("external-input-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let result = std::panic::catch_unwind(|| {
            let file = root.join("file.json");
            std::fs::write(&file, b"signed").unwrap();
            assert_eq!(read_external(&file, 6).unwrap(), b"signed");
            assert!(read_external(&file, 5).is_err());
            assert!(read_external(&root, 64).is_err());
            symlink(&file, root.join("link.json")).unwrap();
            assert!(read_external(&root.join("link.json"), 64).is_err());
            symlink(&root, root.join("alias")).unwrap();
            assert!(read_external(&root.join("alias/file.json"), 64).is_err());
            let fifo = root.join("fifo");
            let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
            // SAFETY: temporary test path is a valid NUL-terminated string.
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            assert!(read_external(&fifo, 64).is_err());
        });
        std::fs::remove_dir_all(root).unwrap();
        result.unwrap();
    }
}
