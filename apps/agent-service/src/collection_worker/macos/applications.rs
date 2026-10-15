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
        // A directory alias is not an application bundle inside this approved root.
        // In particular Safari's Cryptex alias must not broaden /Applications into Preboot.
        if kind.is_symlink() {
            continue;
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
    application_inspecting(c, dir, path, owner, rows, signature::inspect)
}
fn application_inspecting(
    c: &Context<'_>,
    dir: &File,
    path: &Path,
    owner: &str,
    rows: &mut Rows<'_>,
    inspect: impl FnOnce(&Path) -> Result<(&'static str, Option<String>)>,
) -> Result<()> {
    let app_before = dir.metadata()?;
    let contents = openat(dir, std::ffi::OsStr::new("Contents"), true)?;
    let contents_before = contents.metadata()?;
    let file = openat(&contents, std::ffi::OsStr::new("Info.plist"), false)?;
    let before = file.metadata()?;
    let bytes = c.read_bounded(&file, usize::MAX)?;
    let value =
        plist::Value::from_reader(std::io::Cursor::new(bytes)).map_err(|_| Status::Invalid)?;
    let d = object(&value)?;
    let location = path.to_str().ok_or(Status::Invalid)?;
    let mut r = decode(d, location, owner)?;
    let (status, team) = inspect(path)?;
    r.insert("signature_status".into(), status.into());
    if let Some(team) = team {
        r.insert("team_id".into(), team.into());
    }
    // A raced path/signature observation cannot turn into complete presence evidence.
    let reopened = openat(dir, std::ffi::OsStr::new("Contents"), true)?;
    let current = openat(&reopened, std::ffi::OsStr::new("Info.plist"), false)?.metadata()?;
    let named = std::fs::symlink_metadata(path)?;
    if !same_snapshot(&app_before, &dir.metadata()?)
        || !same_snapshot(&app_before, &named)
        || !same_snapshot(&contents_before, &reopened.metadata()?)
        || !same_snapshot(&before, &current)
    {
        return Err(Status::Invalid.into());
    }
    rows.push(r)?;
    Ok(())
}
fn same_snapshot(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
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
    fn real_directory_scan_retains_copies_and_excludes_symlink_targets() {
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
        let app = root.join("A.app");
        let app_dir = open_root(&app).unwrap();
        let mut changed_rows = Rows::new(&q, now().unwrap());
        let changed = application_inspecting(&c, &app_dir, &app, "", &mut changed_rows, |path| {
            std::fs::rename(path.join("Contents"), path.join("old-contents"))?;
            std::fs::create_dir(path.join("Contents"))?;
            std::fs::copy(
                path.join("old-contents/Info.plist"),
                path.join("Contents/Info.plist"),
            )?;
            Ok(("signed", None))
        });
        assert!(matches!(changed, Err(Fault::Query(Status::Invalid))));
        assert!(changed_rows.finish(Ok(())).rows.is_empty());
        std::fs::remove_dir_all(app.join("Contents")).unwrap();
        std::fs::rename(app.join("old-contents"), app.join("Contents")).unwrap();
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
        let outside = root.with_file_name(format!("rss-outside-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(outside.join("External.app/Contents")).unwrap();
        let _outside_cleanup = Cleanup(outside.clone());
        std::fs::write(
            outside.join("External.app/Contents/Info.plist"),
            b"invalid plist",
        )
        .unwrap();
        for (target, name) in [
            (outside.join("External.app"), "Escape.app"),
            (outside.clone(), "outside-directory"),
            (root.join("A.app"), "Alias.app"),
            (outside.join("missing"), "Dangling.app"),
        ] {
            std::os::unix::fs::symlink(target, root.join(name)).unwrap();
        }
        assert!(open_root(&root.join("Alias.app")).is_err());
        let mut rows = Rows::new(&q, now().unwrap());
        walk(&c, &dir, &root, "", 2, &mut rows).unwrap();
        let result = rows.finish(Ok(()));
        assert!(result.complete_for(&q, now().unwrap()));
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0]["location"],
            root.join("A.app").to_str().unwrap()
        );
    }
}
