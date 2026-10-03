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
        if session != &execution_ipc::host::current_session_binding()?
            || account.platform != Platform::Macos
            || account.subject.as_str() != uid.to_string()
            || !execution_ipc::macos_identity::console_user(uid)
            || !execution_ipc::host::active_login()
        {
            return Err(Error::Unbound);
        }
    }
    Ok(())
}
pub(crate) fn profile(profile: &VersionedRef) -> Result<(), Error> {
    if profile.revision.as_str() != "1"
        || ![
            "native-posix-sh-file",
            "native-bash-file",
            "native-osquery-template",
            "native-software-worker",
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
    if profile.id.as_str() == "native-software-worker" {
        return if args == ["--software-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
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
    file: File,
    bytes: &[u8],
    _: &Path,
    _: &AttemptId,
    _: &VersionedRef,
) -> Result<crate::materialize::Payload, Error> {
    use std::os::fd::{AsRawFd, OwnedFd};
    // A user-owned named snapshot would let another process of that UID retain a write FD.
    // Pass only the verified bytes through an anonymous read-only pipe instead. The existing
    // process input task feeds it after spawn, under the same cancellation and timeout owner.
    // ref: rust-lang/rust library/std/src/io/pipe.rs (anonymous_pipe, stable since 1.87).
    let (file, input) = if unsafe { libc::geteuid() } != 0 {
        let (reader, writer) = std::io::pipe().map_err(|_| Error::Unavailable)?;
        (
            File::from(OwnedFd::from(reader)),
            Some((writer, bytes.to_vec())),
        )
    } else {
        (file, None)
    };
    Ok(crate::materialize::Payload {
        path: format!("/dev/fd/{}", file.as_raw_fd()).into(),
        file: Some(file),
        input,
        directory: None,
    })
}
// ref: tokio@1.53.1 src/net/unix/pipe.rs, Sender::from_owned_fd validates write access
// and registers a nonblocking pipe; dropping the cancelled input future closes its writer.
pub(crate) async fn deliver_payload(
    input: Option<(std::io::PipeWriter, Vec<u8>)>,
) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    if let Some((writer, bytes)) = input {
        let mut sender = tokio::net::unix::pipe::Sender::from_owned_fd(writer.into())?;
        sender.write_all(&bytes).await?;
    }
    Ok(())
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

#[cfg(test)]
mod path_tests {
    use super::*;
    #[tokio::test]
    async fn helper_script_pipe_has_no_name_and_ignores_retained_source_write_fds() {
        use std::io::{Seek, Write};
        use std::os::{
            fd::AsRawFd,
            unix::fs::{FileTypeExt, PermissionsExt},
        };
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-script-pipe-{}", std::process::id()));
        platform_private_storage::directory(&root).unwrap();
        let source = root.join("input");
        // Larger than a pipe buffer: preparation must not preload and block before spawning.
        let bytes = format!(
            "{}printf 'exact-snapshot\\n'\n",
            "# verified comment\n".repeat(100_000)
        );
        let mut original = File::create(&source).unwrap();
        original.write_all(bytes.as_bytes()).unwrap();
        let mut external_writer = original.try_clone().unwrap();
        original
            .set_permissions(std::fs::Permissions::from_mode(0o000))
            .unwrap();
        let profile = VersionedRef {
            id: Id::new("native-posix-sh-file").unwrap(),
            revision: Id::new("1").unwrap(),
        };
        let mut captured = payload(
            original,
            bytes.as_bytes(),
            &root,
            &AttemptId::new("pipe").unwrap(),
            &profile,
        )
        .unwrap();
        let descriptor = captured.file.as_ref().unwrap();
        assert!(descriptor.metadata().unwrap().file_type().is_fifo());
        assert_eq!(
            unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFL) } & libc::O_ACCMODE,
            libc::O_RDONLY
        );
        assert_eq!(
            unsafe { libc::write(descriptor.as_raw_fd(), b"evil".as_ptr().cast(), 4) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
        assert_eq!(
            std::fs::read_dir(&root).unwrap().count(),
            1,
            "no named snapshot may be exposed"
        );
        // Already acquired writable handles cannot change the verified byte stream.
        external_writer.rewind().unwrap();
        external_writer
            .write_all(b"printf 'tampered\\n'\n")
            .unwrap();
        let mut command = tokio::process::Command::new("/bin/sh");
        command.arg(&captured.path);
        let cwd = WorkingDirectory::open(&root).unwrap();
        cwd.configure(command.as_std_mut(), descriptor).unwrap();
        let (output, ()) =
            tokio::try_join!(command.output(), deliver_payload(captured.input.take())).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"exact-snapshot\n");
        assert_eq!(std::fs::metadata(&source).unwrap().mode() & 0o777, 0);
        drop(captured);
        drop(external_writer);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn full_unread_script_pipe_feed_can_be_cancelled_and_dropped() {
        use std::os::fd::OwnedFd;
        let (_reader, writer) = std::io::pipe().unwrap();
        let mut feeding = tokio::spawn(deliver_payload(Some((writer, vec![b'#'; 1024 * 1024]))));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut feeding)
                .await
                .is_err()
        );
        feeding.abort();
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), feeding)
                .await
                .unwrap()
                .unwrap_err()
                .is_cancelled()
        );
        let _reader = File::from(OwnedFd::from(_reader));
    }
    #[test]
    fn read_only_acl_is_allowed_but_mutation_acl_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-read-acl-{}", std::process::id()));
        platform_private_storage::directory(&root).unwrap();
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
