//! Archive writes stay relative to the bound directory, not a re-resolved parent pathname.
use execution_app::Error;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::{MetadataExt, OpenOptionsExt},
        },
    };
    pub(super) struct Tree {
        path: PathBuf,
        root: File,
    }
    impl Tree {
        pub(super) fn open(path: &Path) -> Result<Self, Error> {
            let root = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)
                .map_err(|_| Error::Denied)?;
            Ok(Self {
                path: path.into(),
                root,
            })
        }
        pub(super) fn intact(&self) -> bool {
            let Ok(now) = std::fs::symlink_metadata(&self.path) else {
                return false;
            };
            self.root
                .metadata()
                .is_ok_and(|m| now.is_dir() && m.dev() == now.dev() && m.ino() == now.ino())
        }
        pub(super) fn file(&mut self, name: &str) -> Result<File, Error> {
            if !self.intact() {
                return Err(Error::Denied);
            }
            let parts: Vec<_> = name.split('/').collect();
            let mut dir = self.root.try_clone().map_err(|_| Error::Unavailable)?;
            for part in &parts[..parts.len() - 1] {
                let part = CString::new(*part).map_err(|_| Error::Denied)?;
                let result = unsafe { libc::mkdirat(dir.as_raw_fd(), part.as_ptr(), 0o700) };
                if result != 0
                    && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                {
                    return Err(Error::Unavailable);
                }
                let fd = unsafe {
                    libc::openat(
                        dir.as_raw_fd(),
                        part.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(Error::Denied);
                }
                dir = unsafe { File::from_raw_fd(fd) };
            }
            let leaf =
                CString::new(*parts.last().ok_or(Error::Denied)?).map_err(|_| Error::Denied)?;
            let fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    leaf.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(Error::Denied);
            }
            Ok(unsafe { File::from_raw_fd(fd) })
        }
        pub(super) fn cleanup(self) {
            if self.intact() {
                let path = self.path.clone();
                drop(self);
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }
}
#[cfg(windows)]
mod native {
    use super::*;
    pub(super) struct Tree {
        path: PathBuf,
        leases: Vec<crate::platform::PathLease>,
    }
    impl Tree {
        pub(super) fn open(path: &Path) -> Result<Self, Error> {
            Ok(Self {
                path: path.into(),
                leases: vec![crate::platform::PathLease::source(path, false)?],
            })
        }
        pub(super) fn intact(&self) -> bool {
            true
        } // Retained directory handles deny delete/rename.
        pub(super) fn file(&mut self, name: &str) -> Result<File, Error> {
            let parts: Vec<_> = name.split('/').collect();
            let mut path = self.path.clone();
            for part in &parts[..parts.len() - 1] {
                path.push(part);
                match std::fs::create_dir(&path) {
                    Ok(()) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                    Err(_) => return Err(Error::Unavailable),
                };
                self.leases
                    .push(crate::platform::PathLease::source(&path, false)?);
            }
            path.push(parts.last().ok_or(Error::Denied)?);
            use std::os::windows::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .share_mode(0)
                .open(path)
                .map_err(|_| Error::Denied)
        }
        pub(super) fn cleanup(self) {
            let path = self.path.clone();
            drop(self);
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
#[cfg(not(any(target_os = "macos", windows)))]
mod native {
    use super::*;
    pub(super) struct Tree;
    impl Tree {
        pub(super) fn open(_: &Path) -> Result<Self, Error> {
            Err(Error::Unsupported)
        }
        pub(super) fn intact(&self) -> bool {
            false
        }
        pub(super) fn file(&mut self, _: &str) -> Result<File, Error> {
            Err(Error::Unsupported)
        }
        pub(super) fn cleanup(self) {}
    }
}
pub(super) struct Tree(native::Tree);
impl Tree {
    pub(super) fn open(path: &Path) -> Result<Self, Error> {
        native::Tree::open(path).map(Self)
    }
    pub(super) fn intact(&self) -> bool {
        self.0.intact()
    }
    pub(super) fn file(&mut self, name: &str) -> Result<File, Error> {
        self.0.file(name)
    }
    pub(super) fn cleanup(self) {
        self.0.cleanup()
    }
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn replaced_directory_is_not_followed_or_removed() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir().join(format!("rss-bound-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("root")).unwrap();
        std::fs::create_dir(base.join("other")).unwrap();
        let mut tree = Tree::open(&base.join("root")).unwrap();
        std::fs::rename(base.join("root"), base.join("original")).unwrap();
        symlink(base.join("other"), base.join("root")).unwrap();
        assert!(tree.file("escape").is_err());
        tree.cleanup();
        assert!(!base.join("other/escape").exists());
        assert!(base.join("original").is_dir());
        std::fs::remove_dir_all(base).unwrap();
    }
}
