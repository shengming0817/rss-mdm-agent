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
    if let SessionRequirement::ActiveUser { account, session } = session {
        if session != &crate::host::current_session_binding()?
            || account.platform != Platform::Macos
            || account.subject.as_str() != uid.to_string()
            || !console_user(uid)
            || !crate::host::active_login()
        {
            return Err(Error::Unbound);
        }
    }
    Ok(())
}
pub(crate) fn console_account() -> Result<u32, Error> {
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
            return Err(Error::Unbound);
        }
        CFRelease(name);
        if actual == 0 || actual == u32::MAX {
            Err(Error::Unbound)
        } else {
            Ok(actual)
        }
    }
}
pub(crate) fn console_user(uid: u32) -> bool {
    console_account() == Ok(uid)
}
pub(crate) fn profile(profile: &VersionedRef) -> Result<(), Error> {
    if profile.revision.as_str() != "1"
        || ![
            "native-posix-sh-file",
            "native-bash-file",
            "native-osquery-template",
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
    if profile.id.as_str() == crate::osquery::PROFILE {
        return if crate::osquery::prefix_matches(args) {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    let path = script.to_str().ok_or(Error::InvalidInput)?;
    let prefix: &[&str] = match profile.id.as_str() {
        "native-posix-sh-file" => &[path],
        "native-bash-file" => &["--noprofile", "--norc", path],
        _ => return Err(Error::Unsupported),
    };
    if args.len() < prefix.len() || !args.iter().zip(prefix).all(|(a, b)| a == b) {
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
    mut file: File,
    bytes: &[u8],
    root: &Path,
    _: &AttemptId,
    _: &VersionedRef,
) -> Result<crate::materialize::Payload, Error> {
    use std::os::fd::AsRawFd;
    let metadata = file.metadata().map_err(|_| Error::Unavailable)?;
    let uid = unsafe { libc::geteuid() };
    if uid != 0 && (metadata.uid() != uid || metadata.mode() & 0o400 == 0) {
        // /dev/fd does not preserve an ACL read grant when the interpreter reopens a root
        // artifact. Snapshot the already verified bytes into this physical owner's anonymous
        // file, keeping the protected source unchanged and exposing only a read-only handle.
        file = anonymous_payload(bytes, root)?;
    }
    Ok(crate::materialize::Payload {
        path: format!("/dev/fd/{}", file.as_raw_fd()).into(),
        file: Some(file),
        directory: None,
    })
}
fn anonymous_payload(bytes: &[u8], root: &Path) -> Result<File, Error> {
    use std::{
        io::Write,
        os::fd::{AsRawFd, FromRawFd},
    };
    let directory = open_directory(root)?;
    let mut nonce = [0u8; 16];
    // SAFETY: arc4random_buf initializes the writable nonce buffer.
    unsafe { libc::arc4random_buf(nonce.as_mut_ptr().cast(), nonce.len()) };
    let name = std::ffi::CString::new(format!(
        ".script-{}",
        nonce.iter().map(|v| format!("{v:02x}")).collect::<String>()
    ))
    .map_err(|_| Error::Unavailable)?;
    // Open the reader while the file is still empty, then remove its only name BEFORE writing.
    // /dev/fd duplicates access mode, so reopening a writable anonymous fd cannot downgrade it.
    // ref: Apple Libc stdio/FreeBSD/tmpfile.c; XNU bsd/miscfs/devfs/devfs_vnops.c.
    let raw = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o400,
        )
    };
    if raw < 0 {
        return Err(Error::Unavailable);
    }
    // SAFETY: openat returned a uniquely owned descriptor.
    let mut writer = unsafe { File::from_raw_fd(raw) };
    let raw_reader = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    let removed = unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } == 0;
    if raw_reader < 0 {
        return Err(Error::Unavailable);
    }
    // SAFETY: the second openat returned another uniquely owned descriptor.
    let mut reader = unsafe { File::from_raw_fd(raw_reader) };
    let original = writer.metadata().map_err(|_| Error::Unavailable)?;
    let retained = reader.metadata().map_err(|_| Error::Unavailable)?;
    if !removed
        || !retained.is_file()
        || retained.nlink() != 0
        || original.nlink() != 0
        || retained.uid() != unsafe { libc::geteuid() }
        || retained.dev() != original.dev()
        || retained.ino() != original.ino()
        || retained.mode() & 0o777 != 0o400
    {
        return Err(Error::Denied);
    }
    writer.write_all(bytes).map_err(|_| Error::Unavailable)?;
    std::io::Seek::rewind(&mut reader).map_err(|_| Error::Unavailable)?;
    Ok(reader)
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
    fn acl_readable_payload_uses_an_anonymous_read_only_snapshot_for_the_interpreter() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-anonymous-script-{}", std::process::id()));
        native_process::private_storage::directory(&root).unwrap();
        let source = root.join("input");
        let bytes = b"printf 'exact-snapshot\\n'\n";
        let mut original = File::create(&source).unwrap();
        original.write_all(bytes).unwrap();
        // An already open ACL-readable file can lack the POSIX read bit used by /dev/fd.
        original
            .set_permissions(std::fs::Permissions::from_mode(0o000))
            .unwrap();
        let profile = VersionedRef {
            id: Id::new("native-posix-sh-file").unwrap(),
            revision: Id::new("1").unwrap(),
        };
        let captured = payload(
            original,
            bytes,
            &root,
            &AttemptId::new("snapshot").unwrap(),
            &profile,
        )
        .unwrap();
        let descriptor = captured.file.as_ref().unwrap();
        assert_eq!(descriptor.metadata().unwrap().nlink(), 0);
        let mut command = std::process::Command::new("/bin/sh");
        command.arg(&captured.path);
        let cwd = WorkingDirectory::open(&root).unwrap();
        cwd.configure(&mut command, descriptor).unwrap();
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"exact-snapshot\n");
        assert_eq!(std::fs::metadata(&source).unwrap().mode() & 0o777, 0);
        use std::os::fd::AsRawFd;
        assert_eq!(
            unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFL) } & libc::O_ACCMODE,
            libc::O_RDONLY
        );
        drop(captured);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn read_only_acl_is_allowed_but_mutation_acl_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-read-acl-{}", std::process::id()));
        native_process::private_storage::directory(&root).unwrap();
        let path = root.join("artifact");
        std::fs::write(&path, b"immutable input").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(std::process::Command::new("/bin/chmod")
            .args([
                "+a",
                "everyone allow read,readattr,readextattr,readsecurity"
            ])
            .arg(&path)
            .status()
            .unwrap()
            .success());
        assert!(PathLease::source(&path, false).is_ok());
        assert!(std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone allow write,append,delete"])
            .arg(&path)
            .status()
            .unwrap()
            .success());
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o022,
            0
        );
        assert!(matches!(
            PathLease::source(&path, false),
            Err(Error::Denied)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
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

pub(crate) fn grant_read(path: &Path, subject: &str) -> Result<(), Error> {
    use std::os::fd::AsRawFd;
    extern "C" {
        fn rss_execution_grant_read(fd: i32, uid: u32) -> i32;
    }
    let uid: u32 = subject.parse().map_err(|_| Error::InvalidInput)?;
    if uid == 0 || unsafe { libc::geteuid() } != 0 {
        return Err(Error::Denied);
    }
    let directory = path.is_dir();
    let file = bound_file(path, directory, true)?;
    if unsafe { rss_execution_grant_read(file.as_raw_fd(), uid) } != 0 {
        return Err(Error::Denied);
    }
    Ok(())
}
