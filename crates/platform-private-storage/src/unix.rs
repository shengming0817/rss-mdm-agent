use std::{
    ffi::{CString, OsStr},
    fs::File,
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path, PathBuf},
};
fn text(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(io::Error::other)
}
fn fd(value: i32) -> io::Result<File> {
    if value < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: every caller passes a newly returned open/openat fd. The negative
        // result was rejected above; this is its only ownership transfer, and File closes it.
        Ok(unsafe { File::from_raw_fd(value) })
    }
}
fn normalized(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|p| !matches!(p, Component::RootDir | Component::Normal(_)))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    #[cfg(target_os = "macos")]
    for alias in ["/var", "/tmp", "/etc"] {
        if let Ok(rest) = path.strip_prefix(alias) {
            return Ok(Path::new("/private")
                .join(alias.trim_start_matches('/'))
                .join(rest));
        }
    }
    Ok(path.into())
}
fn open_at(parent: &File, name: &OsStr, flags: i32) -> io::Result<File> {
    let name = text(name)?;
    // SAFETY: owned directory fd, valid C name, and an explicit mode when creating.
    fd(unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    })
}
fn ancestor(file: &File) -> io::Result<()> {
    let meta = file.metadata()?;
    if !meta.is_dir()
        // SAFETY: geteuid has no pointer arguments or memory preconditions.
        || (meta.uid() != 0 && meta.uid() != unsafe { libc::geteuid() })
        || (meta.mode() & 0o022 != 0
            && !(meta.uid() == 0 && meta.mode() & u32::from(libc::S_ISVTX) != 0))
    {
        return Err(io::Error::other("replaceable private ancestor"));
    }
    #[cfg(target_os = "macos")]
    acl(file, false)?;
    Ok(())
}
pub fn private_handle(file: &File) -> io::Result<()> {
    let meta = file.metadata()?;
    // SAFETY: geteuid only reads the kernel-maintained identity of this process.
    if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(io::Error::other("private ownership required"));
    }
    #[cfg(target_os = "macos")]
    acl(file, true)?;
    Ok(())
}
pub fn sqlite_file(path: &Path) -> io::Result<()> {
    let path = normalized(path)?;
    let _parent = directory(path.parent().ok_or(io::ErrorKind::InvalidInput)?, false)?;
    let before = std::fs::symlink_metadata(&path)?;
    // SAFETY: geteuid reads only the current process kernel identity.
    if !before.is_file() || before.uid() != unsafe { libc::geteuid() } || before.mode() & 0o077 != 0
    {
        return Err(io::Error::other("private SQLite file required"));
    }
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn acl_get_link_np(path: *const libc::c_char, kind: i32) -> *mut std::ffi::c_void;
        }
        let name = text(path.as_os_str())?;
        // ref: macOS SDK sys/acl.h acl_get_link_np. Metadata-only and no symlink following;
        // opening a second data descriptor would destroy the caller's POSIX SQLite locks.
        // SAFETY: name is a live NUL-terminated CString and acl_get_link_np does not
        // retain it. ACL_TYPE_EXTENDED is the SDK constant; acl_value takes ownership
        // of the returned ACL allocation and frees it, including rejection paths.
        let raw = unsafe { acl_get_link_np(name.as_ptr(), 0x100) };
        acl_value(raw, true)?;
    }
    let after = std::fs::symlink_metadata(&path)?;
    if (before.dev(), before.ino(), before.uid(), before.mode())
        != (after.dev(), after.ino(), after.uid(), after.mode())
    {
        return Err(io::Error::other(
            "SQLite metadata changed during validation",
        ));
    }
    Ok(())
}

pub fn directory(path: &Path, create: bool) -> io::Result<File> {
    let path = normalized(path)?;
    // SAFETY: the root pathname is a static NUL-terminated string; open returns a new owned fd.
    let mut directory = fd(unsafe {
        libc::open(
            c"/".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    })?;
    let names: Vec<_> = path
        .components()
        .filter_map(|p| {
            if let Component::Normal(n) = p {
                Some(n)
            } else {
                None
            }
        })
        .collect();
    for (index, name) in names.iter().enumerate() {
        ancestor(&directory)?;
        let next = match open_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY) {
            Ok(file) => file,
            Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
                // The empty new directory is checked before any child data is created.
                let name_c = text(name)?;
                // SAFETY: the borrowed directory fd remains open, name_c is NUL-terminated
                // and live for the call, and mkdirat does not retain either argument.
                if unsafe { libc::mkdirat(directory.as_raw_fd(), name_c.as_ptr(), 0o700) } != 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::AlreadyExists {
                        return Err(error);
                    }
                }
                let file = open_at(&directory, name, libc::O_RDONLY | libc::O_DIRECTORY)?;
                private_handle(&file)?;
                directory.sync_all()?;
                file
            }
            Err(e) => return Err(e),
        };
        if index + 1 == names.len() {
            private_handle(&next)?;
        }
        directory = next;
    }
    private_handle(&directory)?;
    Ok(directory)
}
pub fn open_file(dir: &File, name: &OsStr, create: bool) -> io::Result<File> {
    private_handle(dir)?;
    open_at(
        dir,
        name,
        if create {
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL
        } else {
            libc::O_RDONLY
        },
    )
}
pub fn open_path(path: &Path) -> io::Result<File> {
    // Directory roots may have public ancestors. Files must have a private immediate parent.
    match directory(path, false) {
        Ok(dir) => Ok(dir),
        Err(e) if e.raw_os_error() == Some(libc::ENOTDIR) => {
            let parent = directory(path.parent().ok_or(io::ErrorKind::InvalidInput)?, false)?;
            open_file(
                &parent,
                path.file_name().ok_or(io::ErrorKind::InvalidInput)?,
                false,
            )
        }
        Err(e) => Err(e),
    }
}
pub fn replace(dir: &File, staged: &Path, target: &Path) -> io::Result<()> {
    let staged = text(staged.as_os_str())?;
    let target = text(target.as_os_str())?;
    // SAFETY: both directory fd borrows remain valid for the call; both C strings
    // remain live and NUL-terminated. renameat consumes neither fd nor pointer.
    if unsafe {
        libc::renameat(
            dir.as_raw_fd(),
            staged.as_ptr(),
            dir.as_raw_fd(),
            target.as_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub fn publish_new(dir: &File, staged: &Path, target: &Path) -> io::Result<()> {
    let staged = text(staged.as_os_str())?;
    let target = text(target.as_os_str())?;
    // SAFETY: the same live directory fd anchors both names, and the C strings
    // outlive this synchronous call. No ownership of either fd or string is transferred.
    if unsafe {
        libc::linkat(
            dir.as_raw_fd(),
            staged.as_ptr(),
            dir.as_raw_fd(),
            target.as_ptr(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn acl(file: &File, private: bool) -> io::Result<()> {
    unsafe extern "C" {
        fn acl_get_fd_np(fd: i32, kind: i32) -> *mut std::ffi::c_void;
    }
    // SAFETY: file remains owned and live; the returned ACL has independent ownership.
    acl_value(unsafe { acl_get_fd_np(file.as_raw_fd(), 0x100) }, private)
}
#[cfg(target_os = "macos")]
fn acl_value(raw: *mut std::ffi::c_void, private: bool) -> io::Result<()> {
    use std::ffi::c_void;
    unsafe extern "C" {
        fn acl_get_entry(acl: *mut c_void, which: i32, entry: *mut *mut c_void) -> i32;
        fn acl_get_tag_type(entry: *mut c_void, tag: *mut i32) -> i32;
        fn acl_get_permset(entry: *mut c_void, set: *mut *mut c_void) -> i32;
        fn acl_get_perm_np(set: *mut c_void, permission: u32) -> i32;
        fn acl_get_qualifier(entry: *mut c_void) -> *mut c_void;
        fn acl_free(value: *mut c_void) -> i32;
        fn mbr_uuid_to_id(uuid: *const u8, id: *mut u32, kind: *mut i32) -> i32;
    }
    struct Acl(*mut c_void);
    impl Drop for Acl {
        fn drop(&mut self) {
            // SAFETY: Acl is constructed only from non-null allocations returned by
            // acl_get_fd_np/acl_get_link_np/acl_get_qualifier; this is their sole owner.
            unsafe {
                acl_free(self.0);
            }
        }
    }
    if raw.is_null() {
        let e = io::Error::last_os_error();
        // Darwin uses ENOENT for an absent extended ACL on an already-open object.
        return if e.raw_os_error() == Some(libc::ENOENT) {
            Ok(())
        } else {
            Err(e)
        };
    }
    let _acl = Acl(raw);
    let mut which = 0;
    loop {
        let mut entry = std::ptr::null_mut();
        // SAFETY: _acl owns raw until function exit. entry is a writable stack output;
        // returned entries borrow raw and are used only while that owner is alive.
        let result = unsafe { acl_get_entry(raw, which, &mut entry) };
        if result != 0 {
            let error = io::Error::last_os_error();
            return if result == -1 && error.raw_os_error() == Some(libc::EINVAL) {
                Ok(())
            } else {
                Err(error)
            };
        }
        if private {
            return Err(io::Error::other("extended private ACL"));
        }
        which = -1;
        let mut tag = 0;
        // SAFETY: entry was obtained successfully from the live ACL; tag is a writable i32.
        if unsafe { acl_get_tag_type(entry, &mut tag) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if tag == 2 {
            continue;
        } // Deny entries cannot grant mutation.
        if tag != 1 {
            return Err(io::Error::other("unsupported ancestor ACL"));
        }
        let mut set = std::ptr::null_mut();
        // SAFETY: entry still borrows the live ACL; set is a writable output, and
        // the returned permset is borrowed, never independently freed.
        if unsafe { acl_get_permset(entry, &mut set) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut mutates = false;
        for permission in [1 << 2, 1 << 4, 1 << 6, 1 << 8, 1 << 10, 1 << 12, 1 << 13] {
            // SAFETY: set borrows the still-live ACL entry; permissions are valid
            // Darwin ACL permission bits, and the query neither frees nor retains set.
            let has = unsafe { acl_get_perm_np(set, permission) };
            if has < 0 {
                return Err(io::Error::last_os_error());
            }
            mutates |= has == 1;
        }
        if mutates {
            // SAFETY: entry is a live extended-allow entry with a UUID qualifier.
            // The returned copy is separately allocated and released by _qualifier using acl_free.
            let qualifier = unsafe { acl_get_qualifier(entry) };
            if qualifier.is_null() {
                return Err(io::Error::last_os_error());
            }
            let _qualifier = Acl(qualifier);
            let mut id = 0;
            let mut kind = 0;
            // SAFETY: _qualifier owns a non-null 16-byte Darwin UUID; id/kind are
            // writable u32/i32 outputs whose lifetimes cover the synchronous membership query.
            if unsafe { mbr_uuid_to_id(qualifier.cast(), &mut id, &mut kind) } != 0
                || kind != 0
                // SAFETY: geteuid has no pointer arguments and does not alter identity.
                || (id != 0 && id != unsafe { libc::geteuid() })
            {
                return Err(io::Error::other("writable ancestor ACL"));
            }
        }
    }
}
