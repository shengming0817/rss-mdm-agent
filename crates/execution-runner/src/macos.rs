use execution_app::Error;
use execution_contract::*;
use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};
pub(crate) fn protected_path(path: &Path, directory: bool) -> Result<(), Error> {
    if !path.is_absolute()
        || path.components().any(|p| {
            matches!(
                p,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(Error::Denied);
    }
    for component in path.ancestors() {
        use std::os::unix::ffi::OsStrExt;
        extern "C" {
            fn rss_execution_acl_restrictive(path: *const std::ffi::c_char) -> i32;
        }
        let name =
            std::ffi::CString::new(component.as_os_str().as_bytes()).map_err(|_| Error::Denied)?;
        if unsafe { rss_execution_acl_restrictive(name.as_ptr()) } != 1 {
            return Err(Error::Denied);
        }
        let meta = std::fs::symlink_metadata(component).map_err(|_| Error::Unavailable)?;
        if meta.file_type().is_symlink()
            || meta.mode() & 0o022 != 0
            || (meta.uid() != 0 && meta.uid() != unsafe { libc::geteuid() })
        {
            return Err(Error::Denied);
        }
    }
    let m = std::fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
    if (directory && !m.is_dir()) || (!directory && !m.is_file()) {
        return Err(Error::Denied);
    }
    Ok(())
}
pub(crate) fn open_file(path: &Path) -> Result<File, Error> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| Error::Unavailable)
}
pub(crate) fn create_private_directory(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|_| Error::Conflict)
}
pub(crate) fn restrict_file(file: &File) -> Result<(), Error> {
    file.set_permissions(std::fs::Permissions::from_mode(0o400))
        .map_err(|_| Error::Unavailable)
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
    pub(crate) fn attach(&self, _pid: u32) -> Result<(), Error> {
        Ok(())
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
