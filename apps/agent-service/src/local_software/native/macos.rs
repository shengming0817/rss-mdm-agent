//! Bundle removal isolates the exact inode before deletion; no link-following recursive remover.
//! ref: rust-lang/rust library/std/src/sys/fs/unix.rs@1.96.0 (remove_dir_all/openat/unlinkat).
use super::*;
use std::{
    ffi::{CString, OsStr},
    fs::{self, File},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
        },
    },
    path::Component,
};

fn open_at(parent: &File, name: &OsStr, directory: bool) -> Result<File, Error> {
    let name = CString::new(name.as_bytes()).map_err(|_| Error::Untrusted)?;
    let flags = libc::O_RDONLY
        | libc::O_CLOEXEC
        | libc::O_NOFOLLOW
        | libc::O_NONBLOCK
        | if directory { libc::O_DIRECTORY } else { 0 };
    // SAFETY: live owned descriptor and terminated name; ownership transfers only on success.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn open_directory(path: &Path) -> Result<File, Error> {
    if !path.is_absolute() {
        return Err(Error::Untrusted);
    }
    let mut dir = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open("/")?;
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => dir = open_at(&dir, name, true)?,
            _ => return Err(Error::Untrusted),
        }
    }
    Ok(dir)
}
fn identity(path: &Path, bundle: &str, version: &str, team: &str) -> Result<(u64, u64), Error> {
    let app = open_directory(path)?;
    let contents = open_at(&app, OsStr::new("Contents"), true)?;
    let info = open_at(&contents, OsStr::new("Info.plist"), false)?;
    if !info.metadata()?.is_file() || info.metadata()?.len() > 1024 * 1024 {
        return Err(Error::Untrusted);
    }
    let mut bytes = Vec::new();
    info.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    let plist =
        plist::Value::from_reader(std::io::Cursor::new(bytes)).map_err(|_| Error::Untrusted)?;
    let d = plist.as_dictionary().ok_or(Error::Untrusted)?;
    if d.get("CFBundleIdentifier")
        .and_then(plist::Value::as_string)
        != Some(bundle)
        || d.get("CFBundleShortVersionString")
            .and_then(plist::Value::as_string)
            != Some(version)
    {
        return Err(Error::Conflict);
    }
    crate::collection_worker::macos::local_app_identity(path, bundle, team)?;
    let meta = app.metadata()?;
    let named = open_directory(path)?.metadata()?;
    if meta.dev() != named.dev() || meta.ino() != named.ino() {
        return Err(Error::Conflict);
    }
    Ok((meta.dev(), meta.ino()))
}
pub(super) fn select(item: &InstalledSoftware, row: &serde_json::Value) -> Result<Target, Error> {
    if item.kind != InstalledSoftwareKind::Application {
        return Err(Error::Unsupported);
    }
    let path = PathBuf::from(
        row.get("location")
            .and_then(serde_json::Value::as_str)
            .ok_or(Error::Untrusted)?,
    );
    let root = match &item.scope {
        DataScope::Device
            if action != LocalSoftwareAction::Uninstall
                && path.starts_with("/System/Applications") =>
        {
            PathBuf::from("/System/Applications")
        }
        DataScope::Device => PathBuf::from("/Applications"),
        DataScope::User { subject } => {
            crate::collection_worker::macos::local_user_app_root(subject.as_str())?
        }
    };
    if !path.starts_with(&root) || path == root || path.extension().is_none_or(|e| e != "app") {
        return Err(Error::Denied);
    }
    if matches!(item.scope, DataScope::Device) && action == LocalSoftwareAction::Uninstall {
        installation_security::applications_directory().map_err(|_| Error::Denied)?;
        // Nested administrator/user-writable discovery directories need an explicit recipe.
        if path.parent() != Some(root.as_path()) {
            return Err(Error::Unsupported);
        }
    }
    let version = item.version.clone().ok_or(Error::Untrusted)?;
    let team = item.publisher.clone().ok_or(Error::Untrusted)?;
    let (device, inode) = identity(&path, &item.native_id, &version, &team)?;
    Ok(Target::App {
        path,
        device,
        inode,
        bundle: item.native_id.clone(),
        version,
        team,
    })
}
fn rename_exclusive(source: &File, name: &OsStr, target: &File, slot: &OsStr) -> Result<(), Error> {
    let name = CString::new(name.as_bytes()).map_err(|_| Error::Untrusted)?;
    let slot = CString::new(slot.as_bytes()).map_err(|_| Error::Untrusted)?;
    // SAFETY: live directory descriptors and terminated names; replacement is forbidden.
    if unsafe {
        libc::renameatx_np(
            source.as_raw_fd(),
            name.as_ptr(),
            target.as_raw_fd(),
            slot.as_ptr(),
            libc::RENAME_EXCL,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    source.sync_all()?;
    target.sync_all()?;
    Ok(())
}
pub(super) fn execute(input: &Input) -> Result<Option<SoftwareState>, Error> {
    let Target::App {
        path,
        device,
        inode,
        bundle,
        version,
        team,
    } = &input.target
    else {
        return Err(Error::Unsupported);
    };
    // The protected worker input freezes scope and object, even after a login or source changes.
    if identity(path, bundle, version, team)? != (*device, *inode) {
        return Err(Error::Conflict);
    }
    match input.action {
        LocalSoftwareAction::Detect => Ok(Some(SoftwareState::Present {
            version: PackageValue::new(version.clone()).map_err(|_| Error::Untrusted)?,
        })),
        LocalSoftwareAction::Launch => {
            if !matches!(input.run_as, RunAs::User { .. }) {
                return Err(Error::Denied);
            }
            let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Untrusted)?;
            unsafe extern "C" {
                fn rss_open_local_application(path: *const libc::c_char) -> i32;
            }
            // SAFETY: native function copies this terminated path before its async call.
            if unsafe { rss_open_local_application(path.as_ptr()) } != 0 {
                return Err(Error::Unavailable);
            }
            Ok(None)
        }
        LocalSoftwareAction::Uninstall => {
            let parent_path = path.parent().ok_or(Error::Untrusted)?;
            let parent = open_directory(parent_path)?;
            // Same-volume exclusive private namespace; no privileged writes into caller-selected trash.
            let private_path =
                parent_path.join(format!(".rss-removing-{}", input.request.as_str()));
            fs::DirBuilder::new().mode(0o700).create(&private_path)?;
            let private = open_directory(&private_path)?;
            let name = path.file_name().ok_or(Error::Untrusted)?;
            rename_exclusive(&parent, name, &private, OsStr::new("application"))?;
            let isolated = private_path.join("application");
            let actual = open_directory(&isolated)?.metadata()?;
            if (actual.dev(), actual.ino()) != (*device, *inode) {
                // Never overwrite a concurrently recreated application; leave retained evidence on failure.
                rename_exclusive(&private, OsStr::new("application"), &parent, name)?;
                fs::remove_dir(&private_path)?;
                return Err(Error::Conflict);
            }
            crate::collection_worker::macos::local_app_identity(&isolated, bundle, team)?;
            // std's macOS implementation uses descriptor-relative no-follow traversal, including races.
            fs::remove_dir_all(&isolated)?;
            fs::remove_dir(&private_path)?;
            match fs::symlink_metadata(path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    Ok(Some(SoftwareState::Absent {}))
                }
                _ => Err(Error::Conflict),
            }
        }
        _ => Err(Error::Unsupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolation_never_overwrites_a_replacement_and_recursive_removal_never_follows_links() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-local-removal-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let approved = root.join("Approved.app");
        let private = root.join("private");
        let outside = root.join("outside");
        fs::create_dir(&approved).unwrap();
        fs::create_dir(&private).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("preserve"), b"outside bundle").unwrap();
        std::os::unix::fs::symlink(&outside, approved.join("link")).unwrap();
        let original = open_directory(&approved).unwrap().metadata().unwrap();
        rename_exclusive(
            &open_directory(&root).unwrap(),
            OsStr::new("Approved.app"),
            &open_directory(&private).unwrap(),
            OsStr::new("application"),
        )
        .unwrap();
        let retained = open_directory(&private.join("application"))
            .unwrap()
            .metadata()
            .unwrap();
        assert_eq!(
            (original.dev(), original.ino()),
            (retained.dev(), retained.ino())
        );
        fs::create_dir(&approved).unwrap();
        fs::write(approved.join("replacement"), b"preserve").unwrap();
        assert!(rename_exclusive(
            &open_directory(&private).unwrap(),
            OsStr::new("application"),
            &open_directory(&root).unwrap(),
            OsStr::new("Approved.app")
        )
        .is_err());
        fs::remove_dir_all(private.join("application")).unwrap();
        assert_eq!(
            fs::read(outside.join("preserve")).unwrap(),
            b"outside bundle"
        );
        assert_eq!(fs::read(approved.join("replacement")).unwrap(), b"preserve");
        assert!(open_directory(&root.join("outside/../Approved.app")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
