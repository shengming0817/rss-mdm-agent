//! Root-relative no-follow traversal; directory handles, not canonical-path checks, own access.
//! ref: osquery/osquery@5.23.1 osquery/tables/system/darwin/apps.mm.
use super::*;
use std::{
    fs::File,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
};
mod signature;
pub(super) fn collect(
    c: &Context<'_>,
    root: w::MacosApplicationRoot,
    depth: u8,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let (path, owner) = match root {
        w::MacosApplicationRoot::LocalApplications => {
            (PathBuf::from("/Applications"), String::new())
        }
        w::MacosApplicationRoot::SystemApplications => {
            (PathBuf::from("/System/Applications"), String::new())
        }
        w::MacosApplicationRoot::UserApplications => {
            let w::SoftwareExecutionTarget::User { identity, .. } = &c.input.task.execution.target
            else {
                return Err(Status::Denied.into());
            };
            let uid = identity.parse::<u32>().map_err(|_| Status::Denied)?;
            (home(uid)?.join("Applications"), identity.clone())
        }
    };
    let dir = match open_root(&path) {
        Ok(v) => v,
        Err(Fault::Query(Status::Missing)) => return Ok(()),
        Err(e) => return Err(e),
    };
    walk(c, &dir, &path, &owner, depth, rows)
}
fn open_root(path: &Path) -> Result<File> {
    let mut dir = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")?;
    for part in path.components() {
        match part {
            std::path::Component::RootDir => {}
            std::path::Component::Normal(name) => dir = openat(&dir, name, true)?,
            _ => return Err(Status::Denied.into()),
        }
    }
    Ok(dir)
}
fn openat(parent: &File, name: &std::ffi::OsStr, directory: bool) -> Result<File> {
    let name = CString::new(name.as_bytes()).map_err(|_| Status::Invalid)?;
    let flags = libc::O_RDONLY
        | libc::O_NOFOLLOW
        | libc::O_CLOEXEC
        | libc::O_NONBLOCK
        | if directory { libc::O_DIRECTORY } else { 0 };
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    if !directory && !file.metadata()?.is_file() {
        return Err(Status::Invalid.into());
    }
    Ok(file)
}
fn walk(
    c: &Context<'_>,
    dir: &File,
    path: &Path,
    owner: &str,
    depth: u8,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let before = dir.metadata()?;
    // read_dir opens the held descriptor, never a mutable ancestor path.
    let entries = std::fs::read_dir(format!("/dev/fd/{}", dir.as_raw_fd()))?;
    for entry in entries {
        c.check()?;
        let entry = entry?;
        let name = entry.file_name();
        let kind = entry.file_type()?;
        c.consume(name.as_bytes().len())?;
        if kind.is_symlink() {
            return Err(Status::Denied.into());
        }
        if !kind.is_dir() {
            continue;
        }
        let child = openat(dir, &name, true)?;
        let child_path = path.join(&name);
        if child_path.extension().is_some_and(|e| e == "app") {
            application(c, &child, &child_path, owner, rows)?;
        } else {
            if depth <= 1 {
                return Err(Status::Truncated.into());
            }
            walk(c, &child, &child_path, owner, depth - 1, rows)?;
        }
    }
    let after = dir.metadata()?;
    if before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(Status::Invalid.into());
    }
    Ok(())
}
fn application(
    c: &Context<'_>,
    dir: &File,
    path: &Path,
    owner: &str,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let contents = openat(dir, std::ffi::OsStr::new("Contents"), true)?;
    let file = openat(&contents, std::ffi::OsStr::new("Info.plist"), false)?;
    let before = file.metadata()?;
    let mut bytes = Vec::new();
    (&file)
        .take(c.cap.saturating_sub(c.bytes.get()) as u64 + 1)
        .read_to_end(&mut bytes)?;
    c.consume(bytes.len())?;
    let value =
        plist::Value::from_reader(std::io::Cursor::new(bytes)).map_err(|_| Status::Invalid)?;
    let d = object(&value)?;
    let location = path.to_str().ok_or(Status::Invalid)?;
    let mut r = decode(d, location, owner)?;
    let (status, team) = signature::inspect(path)?;
    r.insert("signature_status".into(), status.into());
    if let Some(team) = team {
        r.insert("team_id".into(), team.into());
    }
    // A raced path/signature observation cannot turn into complete presence evidence.
    let current = openat(&contents, std::ffi::OsStr::new("Info.plist"), false)?.metadata()?;
    let named = std::fs::symlink_metadata(path)?;
    let held = dir.metadata()?;
    if current.ino() != before.ino()
        || current.dev() != before.dev()
        || current.mtime_nsec() != before.mtime_nsec()
        || current.mtime() != before.mtime()
        || named.ino() != held.ino()
        || named.dev() != held.dev()
    {
        return Err(Status::Invalid.into());
    }
    rows.push(r)?;
    Ok(())
}
pub(super) fn decode(d: &plist::Dictionary, location: &str, owner: &str) -> Result<Row> {
    let native = text(d, "CFBundleIdentifier")?;
    let name = d
        .get("CFBundleDisplayName")
        .or_else(|| d.get("CFBundleName"))
        .and_then(plist::Value::as_string)
        .unwrap_or(native);
    Ok(row(&[
        (
            "id",
            stable(&["application", owner, location, native]).into(),
        ),
        ("native_id", native.into()),
        ("name", name.into()),
        ("version", text(d, "CFBundleShortVersionString")?.into()),
        ("build_version", text(d, "CFBundleVersion")?.into()),
        (
            "scope",
            if owner.is_empty() { "device" } else { "user" }.into(),
        ),
        ("owner", owner.into()),
        ("origin", "application".into()),
        ("location", location.into()),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_directory_scan_retains_copies_and_detects_delete_depth_and_symlink() {
        let root = std::env::temp_dir().join(format!("rss-collection-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let root = root.canonicalize().unwrap();
        let q = super::super::tests::query(w::NativeQueryDefinition::MacosApplications {
            root: w::MacosApplicationRoot::LocalApplications,
            max_depth: 2,
        });
        let input = super::super::tests::input(q.clone());
        let c = Context {
            input: &input,
            until: Instant::now() + Duration::from_secs(10),
            bytes: Cell::new(0),
            cap: 1_000_000,
        };
        let dict = plist::Dictionary::from_iter::<[(&str, plist::Value); 3]>([
            ("CFBundleIdentifier", "org.example.fixture".into()),
            ("CFBundleShortVersionString", "1".into()),
            ("CFBundleVersion", "12".into()),
        ]);
        for name in ["A.app", "B.app"] {
            let path = root.join(name).join("Contents");
            std::fs::create_dir_all(&path).unwrap();
            plist::Value::Dictionary(dict.clone())
                .to_file_xml(path.join("Info.plist"))
                .unwrap();
        }
        let dir = open_root(&root).unwrap();
        let mut rows = Rows::new(&q, now().unwrap());
        walk(&c, &dir, &root, "", 2, &mut rows).unwrap();
        let result = rows.finish(Ok(()));
        assert!(result.complete_for(&q, now().unwrap()));
        assert_eq!(result.rows.len(), 2);
        assert_ne!(result.rows[0]["id"], result.rows[1]["id"]);
        std::fs::remove_dir_all(root.join("B.app")).unwrap();
        let mut rows = Rows::new(&q, now().unwrap());
        walk(&c, &dir, &root, "", 2, &mut rows).unwrap();
        assert_eq!(rows.finish(Ok(())).rows.len(), 1);
        std::fs::create_dir(root.join("nested")).unwrap();
        let mut rows = Rows::new(&q, now().unwrap());
        assert!(matches!(
            walk(&c, &dir, &root, "", 1, &mut rows),
            Err(Fault::Query(Status::Truncated))
        ));
        std::os::unix::fs::symlink("/", root.join("escape")).unwrap();
        let mut rows = Rows::new(&q, now().unwrap());
        assert!(matches!(
            walk(&c, &dir, &root, "", 2, &mut rows),
            Err(Fault::Query(Status::Denied))
        ));
    }
}
