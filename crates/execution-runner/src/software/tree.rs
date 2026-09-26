//! Archive writes stay relative to the bound directory, not a re-resolved parent pathname.
use execution_app::Error;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
#[cfg(target_os = "macos")]
mod native {
    use super::*;
    #[cfg(test)]
    use std::os::unix::fs::OpenOptionsExt;
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::MetadataExt,
        },
    };
    pub(super) struct Tree {
        path: PathBuf,
        root: File,
    }
    impl Tree {
        pub(super) fn create(path: &Path) -> Result<Self, Error> {
            let parent = crate::platform::open_directory(path.parent().ok_or(Error::Denied)?)?;
            let name = CString::new(path.file_name().ok_or(Error::Denied)?.as_encoded_bytes())
                .map_err(|_| Error::Denied)?;
            if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
                return Err(Error::Conflict);
            }
            let fd = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(Error::Denied);
            }
            let root = unsafe { File::from_raw_fd(fd) };
            if unsafe { libc::fchmod(root.as_raw_fd(), 0o700) } != 0 {
                return Err(Error::Denied);
            }
            let value = Self {
                path: path.into(),
                root,
            };
            if !value.intact() {
                return Err(Error::Denied);
            }
            Ok(value)
        }
        #[cfg(test)]
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
        pub(super) fn identity(&self) -> Result<execution_contract::Id, Error> {
            crate::platform::file_identity(&self.root)
        }
        pub(super) fn reopen(
            path: &Path,
            expected: &execution_contract::Id,
        ) -> Result<Self, Error> {
            let root = crate::platform::open_directory(path)?;
            if crate::platform::file_identity(&root)? != *expected {
                return Err(Error::Conflict);
            }
            Ok(Self {
                path: path.into(),
                root,
            })
        }
        pub(super) fn cleanup(
            self,
            control: &super::super::PreparationControl,
        ) -> Result<(), Error> {
            if !self.intact() {
                return Err(Error::Conflict);
            }
            let parent = crate::platform::open_directory(self.path.parent().ok_or(Error::Denied)?)?;
            let name = CString::new(
                self.path
                    .file_name()
                    .ok_or(Error::Denied)?
                    .as_encoded_bytes(),
            )
            .map_err(|_| Error::Denied)?;
            super::empty(&self.root, control, 0)?;
            if !self.intact() {
                return Err(Error::Conflict);
            }
            if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0
            {
                return Err(Error::Unavailable);
            }
            Ok(())
        }
    }
}
#[cfg(target_os = "macos")]
fn empty(root: &File, control: &super::PreparationControl, depth: u32) -> Result<(), Error> {
    use std::{
        ffi::CStr,
        os::fd::{AsRawFd, FromRawFd},
    };
    if depth > 64 {
        return Err(Error::Capacity);
    }
    let fd = unsafe { libc::fcntl(root.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if fd < 0 {
        return Err(Error::Unavailable);
    }
    let dir = unsafe { libc::fdopendir(fd) };
    if dir.is_null() {
        unsafe { libc::close(fd) };
        return Err(Error::Unavailable);
    }
    struct Directory(*mut libc::DIR);
    impl Drop for Directory {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.0) };
        }
    }
    let _dir = Directory(dir);
    loop {
        control.check()?;
        unsafe {
            *libc::__error() = 0;
        }
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            if std::io::Error::last_os_error().raw_os_error() != Some(0) {
                return Err(Error::Unavailable);
            }
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }
        let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
        if unsafe {
            libc::fstatat(
                root.as_raw_fd(),
                name.as_ptr(),
                &mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(Error::Unavailable);
        }
        let flags = if stat.st_mode & libc::S_IFMT == libc::S_IFDIR {
            let fd = unsafe {
                libc::openat(
                    root.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(Error::Conflict);
            }
            let child = unsafe { File::from_raw_fd(fd) };
            use std::os::unix::fs::MetadataExt;
            let opened = child.metadata().map_err(|_| Error::Unavailable)?;
            if opened.dev() != stat.st_dev as u64 || opened.ino() != stat.st_ino {
                return Err(Error::Conflict);
            }
            empty(&child, control, depth + 1)?;
            let mut now = unsafe { std::mem::zeroed::<libc::stat>() };
            if unsafe {
                libc::fstatat(
                    root.as_raw_fd(),
                    name.as_ptr(),
                    &mut now,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } != 0
                || now.st_dev != stat.st_dev
                || now.st_ino != stat.st_ino
            {
                return Err(Error::Conflict);
            }
            libc::AT_REMOVEDIR
        } else {
            0
        };
        if unsafe { libc::unlinkat(root.as_raw_fd(), name.as_ptr(), flags) } != 0 {
            return Err(Error::Unavailable);
        }
    }
    Ok(())
}
#[cfg(windows)]
mod native {
    use super::*;
    pub(super) struct Tree {
        path: PathBuf,
        leases: Vec<crate::platform::PathLease>,
    }
    impl Tree {
        pub(super) fn create(path: &Path) -> Result<Self, Error> {
            let _parent =
                crate::platform::PathLease::source(path.parent().ok_or(Error::Denied)?, false)?;
            use std::os::windows::ffi::OsStrExt;
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            // Atomic create-new; inheriting a protected parent never opens an existing staging tree.
            if unsafe {
                windows_sys::Win32::Storage::FileSystem::CreateDirectoryW(
                    wide.as_ptr(),
                    std::ptr::null(),
                )
            } == 0
            {
                return Err(Error::Conflict);
            }
            native_process::private_storage::directory(path).map_err(|_| Error::Denied)?;
            Self::open(path)
        }
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
        pub(super) fn identity(&self) -> Result<execution_contract::Id, Error> {
            self.leases.first().ok_or(Error::Unavailable)?.identity()
        }
        pub(super) fn reopen(
            path: &Path,
            expected: &execution_contract::Id,
        ) -> Result<Self, Error> {
            let tree = Self::open(path)?;
            if tree.identity()? != *expected {
                return Err(Error::Conflict);
            }
            Ok(tree)
        }
        pub(super) fn cleanup(
            self,
            control: &super::super::PreparationControl,
        ) -> Result<(), Error> {
            let expected = self.identity()?;
            let path = self.path.clone();
            let _parent =
                crate::platform::PathLease::source(path.parent().ok_or(Error::Denied)?, false)?;
            drop(self);
            let handle = super::delete_handle(&path)?;
            if crate::platform::file_identity(&handle)? != expected {
                return Err(Error::Conflict);
            }
            super::delete_tree(handle, &path, control, 0)
        }
    }
}
#[cfg(windows)]
fn delete_handle(path: &Path) -> Result<File, Error> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::*;
    std::fs::OpenOptions::new()
        .access_mode(DELETE | FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|_| Error::Unavailable)
}
#[cfg(windows)]
fn delete_tree(
    handle: File,
    path: &Path,
    control: &super::PreparationControl,
    depth: u32,
) -> Result<(), Error> {
    use std::os::windows::{fs::MetadataExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::*;
    control.check()?;
    if depth > 64 {
        return Err(Error::Capacity);
    }
    let meta = handle.metadata().map_err(|_| Error::Unavailable)?;
    if meta.is_dir() && meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
        for entry in std::fs::read_dir(path).map_err(|_| Error::Unavailable)? {
            control.check()?;
            let path = entry.map_err(|_| Error::Unavailable)?.path();
            delete_tree(delete_handle(&path)?, &path, control, depth + 1)?;
        }
    }
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    if unsafe {
        SetFileInformationByHandle(
            handle.as_raw_handle(),
            FileDispositionInfo,
            (&disposition as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}
#[cfg(not(any(target_os = "macos", windows)))]
mod native {
    use super::*;
    pub(super) struct Tree;
    impl Tree {
        pub(super) fn create(_: &Path) -> Result<Self, Error> {
            Err(Error::Unsupported)
        }
        pub(super) fn open(_: &Path) -> Result<Self, Error> {
            Err(Error::Unsupported)
        }
        pub(super) fn intact(&self) -> bool {
            false
        }
        pub(super) fn file(&mut self, _: &str) -> Result<File, Error> {
            Err(Error::Unsupported)
        }
        pub(super) fn identity(&self) -> Result<execution_contract::Id, Error> {
            Err(Error::Unsupported)
        }
        pub(super) fn reopen(_: &Path, _: &execution_contract::Id) -> Result<Self, Error> {
            Err(Error::Unsupported)
        }
        pub(super) fn cleanup(self, _: &super::super::PreparationControl) -> Result<(), Error> {
            Err(Error::Unsupported)
        }
    }
}
pub(super) struct Tree(native::Tree);
impl Tree {
    pub(super) fn create(path: &Path) -> Result<Self, Error> {
        native::Tree::create(path).map(Self)
    }
    #[cfg(all(test, target_os = "macos"))]
    pub(super) fn open(path: &Path) -> Result<Self, Error> {
        native::Tree::open(path).map(Self)
    }
    pub(super) fn intact(&self) -> bool {
        self.0.intact()
    }
    pub(super) fn file(&mut self, name: &str) -> Result<File, Error> {
        self.0.file(name)
    }
    pub(super) fn identity(&self) -> Result<execution_contract::Id, Error> {
        self.0.identity()
    }
    pub(super) fn reopen(path: &Path, expected: &execution_contract::Id) -> Result<Self, Error> {
        native::Tree::reopen(path, expected).map(Self)
    }
    pub(super) fn cleanup(self, control: &super::PreparationControl) -> Result<(), Error> {
        self.0.cleanup(control)
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
        assert!(tree
            .cleanup(&super::super::PreparationControl::test())
            .is_err());
        assert!(!base.join("other/escape").exists());
        assert!(base.join("original").is_dir());
        std::fs::remove_dir_all(base).unwrap();
    }
}
