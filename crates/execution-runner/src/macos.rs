use execution_app::Error;
use execution_contract::*;
use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
// ref: Darwin openat(2), fstat(2), acl_get_fd_np(3). Never validate a pathname
// and subsequently resolve it again for cwd: every component is relative to its retained parent.
fn check_fd(file: &File, immutable: bool, directory: Option<bool>) -> Result<(), Error> {
    use std::os::fd::AsRawFd;
    extern "C" {
        fn rss_execution_fd_acl_restrictive(fd: i32) -> i32;
    }
    let meta = file.metadata().map_err(|_| Error::Unavailable)?;
    if unsafe { rss_execution_fd_acl_restrictive(file.as_raw_fd()) } != 1
        || meta.mode() & 0o022 != 0
        || (meta.uid() != 0 && (immutable || meta.uid() != unsafe { libc::geteuid() }))
        || (!meta.is_dir() && !meta.is_file())
        || directory.is_some_and(|dir| if dir { !meta.is_dir() } else { !meta.is_file() })
    {
        return Err(Error::Denied);
    }
    Ok(())
}
fn walk(
    path: &Path,
    directory: Option<bool>,
    immutable: bool,
    mut opened: impl FnMut(&Path),
) -> Result<Vec<File>, Error> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
        path::Component,
    };
    if !path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(Error::Denied);
    }
    let root = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")
        .map_err(|_| Error::Unavailable)?;
    check_fd(&root, true, Some(true))?;
    let mut files = vec![root];
    let mut prefix = std::path::PathBuf::from("/");
    let parts: Vec<_> = path
        .components()
        .filter_map(|c| {
            if let Component::Normal(c) = c {
                Some(c)
            } else {
                None
            }
        })
        .collect();
    for (i, part) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        let name = CString::new(part.as_bytes()).map_err(|_| Error::Denied)?;
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if !last || directory == Some(true) {
                libc::O_DIRECTORY
            } else {
                0
            };
        let fd = unsafe { libc::openat(files.last().unwrap().as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(Error::Denied);
        }
        let file = unsafe { File::from_raw_fd(fd) };
        check_fd(&file, immutable, if last { directory } else { Some(true) })?;
        files.push(file);
        prefix.push(part);
        opened(&prefix);
    }
    check_fd(files.last().unwrap(), immutable, directory)?;
    Ok(files)
}
fn bound_file(path: &Path, directory: bool, immutable: bool) -> Result<File, Error> {
    walk(path, Some(directory), immutable, |_| {}).map(|mut files| files.pop().unwrap())
}
pub(crate) fn protected_path(path: &Path, directory: bool) -> Result<(), Error> {
    bound_file(path, directory, false).map(|_| ())
}
pub(crate) fn open_directory(path: &Path) -> Result<File, Error> {
    bound_file(path, true, false)
}
pub(crate) fn open_file(path: &Path) -> Result<File, Error> {
    bound_file(path, false, false)
}
pub(crate) fn identity(run_as: &RunAs, session: &SessionRequirement) -> Result<(), Error> {
    let uid = unsafe { libc::geteuid() };
    match run_as {
        RunAs::System {
            platform: Platform::Macos,
        } if uid == 0 => {}
        RunAs::User { account }
            if account.platform == Platform::Macos
                && account.subject.as_str() == uid.to_string() => {}
        _ => return Err(Error::Unbound),
    }
    if let SessionRequirement::ActiveUser { account } = session {
        if account.platform != Platform::Macos
            || account.subject.as_str() != uid.to_string()
            || !console_user(uid)
        {
            return Err(Error::Unbound);
        }
    }
    Ok(())
}
fn console_user(uid: u32) -> bool {
    #[link(name = "SystemConfiguration", kind = "framework")]
    extern "C" {
        fn SCDynamicStoreCopyConsoleUser(
            store: *const std::ffi::c_void,
            uid: *mut u32,
            gid: *mut u32,
        ) -> *const std::ffi::c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(value: *const std::ffi::c_void);
    }
    let mut actual = u32::MAX;
    let mut gid = 0;
    // SAFETY: returned CF object is released exactly once; outputs are valid writable integers.
    unsafe {
        let name = SCDynamicStoreCopyConsoleUser(std::ptr::null(), &mut actual, &mut gid);
        if name.is_null() {
            return false;
        }
        CFRelease(name);
        actual == uid && uid != 0
    }
}
pub(crate) fn profile(profile: &VersionedRef) -> Result<(), Error> {
    if profile.revision.as_str() != "1"
        || ![
            "native-posix-sh-file",
            "native-bash-file",
            "native-osquery-info-v1",
        ]
        .contains(&profile.id.as_str())
    {
        return Err(Error::Unsupported);
    }
    Ok(())
}
pub(crate) fn arguments(
    profile: &VersionedRef,
    args: &[String],
    script: &Path,
) -> Result<(), Error> {
    let path = script.to_str().ok_or(Error::InvalidInput)?;
    let prefix: &[&str] = match profile.id.as_str() {
        "native-posix-sh-file" => &[path],
        "native-bash-file" => &["--noprofile", "--norc", path],
        "native-osquery-info-v1" => &["--json", "SELECT version FROM osquery_info;"],
        _ => return Err(Error::Unsupported),
    };
    if args.len() < prefix.len()
        || !args.iter().zip(prefix).all(|(a, b)| a == b)
        || (profile.id.as_str() == "native-osquery-info-v1" && args.len() != prefix.len())
    {
        return Err(Error::Denied);
    }
    Ok(())
}
pub(crate) struct Owner {
    group: native_process::AttemptGroup,
}
impl Owner {
    pub(crate) fn prepare(command: &mut tokio::process::Command) -> Result<Self, Error> {
        let group = native_process::AttemptGroup::new().map_err(|_| Error::Unavailable)?;
        group.configure(command.as_std_mut());
        // No shell or inherited terminal; user switching belongs to the launchd user helper.
        Ok(Self { group })
    }
    pub(crate) fn scope(&self) -> ProcessScope {
        ProcessScope::ProcessGroup {
            owner: std::process::id(),
            group: self.group.group(),
        }
    }
    pub(crate) fn stop(&self) {
        self.group.request_stop()
    }
    pub(crate) fn terminate(&mut self) {
        self.group.terminate()
    }
    pub(crate) fn quiescent(&self) -> bool {
        false
    }
}

pub(crate) fn encoding(encoding: ArtifactEncoding) -> Result<(), Error> {
    if encoding == ArtifactEncoding::Utf8 {
        Ok(())
    } else {
        Err(Error::Unsupported)
    }
}
pub(crate) fn payload(
    file: File,
    _: &[u8],
    _: &Path,
    _: &AttemptId,
    _: &VersionedRef,
) -> Result<crate::materialize::Payload, Error> {
    use std::os::fd::AsRawFd;
    Ok(crate::materialize::Payload {
        path: format!("/dev/fd/{}", file.as_raw_fd()).into(),
        file: Some(file),
        directory: None,
    })
}
pub(crate) struct WorkingDirectory(File);
impl WorkingDirectory {
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        bound_file(path, true, false).map(Self)
    }
    pub(crate) fn configure(
        &self,
        command: &mut std::process::Command,
        script: &File,
    ) -> Result<(), Error> {
        use std::os::{fd::AsRawFd, unix::process::CommandExt};
        let cwd = self.0.as_raw_fd();
        let script = script.as_raw_fd();
        // SAFETY: pre_exec performs only async-signal-safe syscalls on retained open descriptors.
        unsafe {
            command.pre_exec(move || {
                if libc::fchdir(cwd) != 0 || libc::fcntl(script, libc::F_SETFD, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        Ok(())
    }
}

pub(crate) async fn spawn(
    command: &mut tokio::process::Command,
    _: &mut Owner,
    cancel: &std::sync::atomic::AtomicBool,
    deadline: std::time::Instant,
) -> std::io::Result<tokio::process::Child> {
    if cancel.load(std::sync::atomic::Ordering::Acquire) || std::time::Instant::now() >= deadline {
        return Err(std::io::ErrorKind::TimedOut.into());
    }
    command.spawn()
}

pub(crate) struct PathLease {
    _handles: Vec<File>,
}
impl PathLease {
    pub(crate) fn source(path: &Path, immutable: bool) -> Result<Self, Error> {
        Ok(Self {
            _handles: walk(path, None, immutable, |_| {})?,
        })
    }
}
#[cfg(test)]
mod path_tests {
    use super::*;
    #[test]
    fn replacing_an_ancestor_with_a_symlink_does_not_redirect_the_opened_cwd() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../.cache/fd-walk-{}", std::process::id()));
        std::fs::create_dir_all(root.join("ancestor/child")).unwrap();
        std::fs::create_dir_all(root.join("other/child")).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let root = root.canonicalize().unwrap();
        let expected = std::fs::metadata(root.join("ancestor/child"))
            .unwrap()
            .ino();
        let files = walk(&root.join("ancestor/child"), Some(true), false, |prefix| {
            if prefix == root.join("ancestor") {
                std::fs::rename(prefix, root.join("saved")).unwrap();
                symlink(root.join("other"), prefix).unwrap();
            }
        })
        .unwrap();
        assert_eq!(files.last().unwrap().metadata().unwrap().ino(), expected);
        assert!(WorkingDirectory::open(&root.join("ancestor/child")).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

// ref: Apple XNU bsd/kern/kern_mib.c, kern.bootsessionuuid (kernel boot generation).
pub(crate) fn boot_generation() -> Result<Id, Error> {
    let mut bytes = [0u8; 128];
    let mut length = bytes.len();
    if unsafe {
        libc::sysctlbyname(
            c"kern.bootsessionuuid".as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || length == 0
        || length > bytes.len()
    {
        return Err(Error::Unavailable);
    }
    let text = std::str::from_utf8(&bytes[..length])
        .map_err(|_| Error::Unavailable)?
        .trim_end_matches('\0');
    Id::new(text).map_err(|_| Error::Unavailable)
}
#[cfg(test)]
mod boot_tests {
    #[test]
    fn same_kernel_boot_is_stable_across_reads() {
        assert_eq!(
            super::boot_generation().unwrap(),
            super::boot_generation().unwrap()
        );
    }
}
