use platform_private_storage::{self as storage, PrivateDirectory};
use std::{fs, io::Write, path::PathBuf};
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "private-files-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        storage::directory(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn bounded_reads_creation_conflicts_and_no_replace_publication() {
    let root = Root::new();
    let target = root.0.join("target");
    storage::write_new(&target, b"original").unwrap();
    assert_eq!(storage::read(&target, 8).unwrap(), b"original");
    assert!(storage::read(&target, 7).is_err());
    assert_eq!(
        storage::create_new(&target).unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    let staged = root.0.join("staged");
    storage::write_new(&staged, b"replacement").unwrap();
    assert!(storage::publish_new(&staged, &target).is_err());
    assert_eq!(storage::read(&target, 32).unwrap(), b"original");
    storage::replace(&staged, &target).unwrap();
    assert_eq!(storage::read(&target, 32).unwrap(), b"replacement");
}
#[cfg(unix)]
#[test]
fn directory_capability_survives_path_replacement_without_following_it() {
    let root = Root::new();
    let path = root.0.join("directory");
    storage::directory(&path).unwrap();
    storage::write_new(&path.join("secret"), b"checked").unwrap();
    let dir = PrivateDirectory::open(&path).unwrap();
    let moved = root.0.join("original");
    fs::rename(&path, &moved).unwrap();
    storage::directory(&path).unwrap();
    storage::write_new(&path.join("secret"), b"substitute").unwrap();
    assert_eq!(
        dir.read(std::path::Path::new("secret"), 32).unwrap(),
        b"checked"
    );
    dir.create_file(std::path::Path::new("created"))
        .unwrap()
        .write_all(b"same directory")
        .unwrap();
    assert!(moved.join("created").exists());
    assert!(!path.join("created").exists());
    assert!(dir.open_file(std::path::Path::new("../secret")).is_err());
}
#[cfg(unix)]
#[test]
fn wide_modes_links_and_special_files_are_rejected() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = Root::new();
    let file = root.0.join("file");
    storage::write_new(&file, b"secret").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(storage::read(&file, 32).is_err());
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&file, root.0.join("link")).unwrap();
    assert!(storage::read(&root.0.join("link"), 32).is_err());
    let fifo = root.0.join("fifo");
    assert!(std::process::Command::new("/usr/bin/mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    fs::set_permissions(&fifo, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(storage::open_existing(&fifo).is_err());
}
#[cfg(target_os = "macos")]
#[test]
fn extended_and_inherited_acl_are_not_hidden_by_private_modes() {
    let root = Root::new();
    let file = root.0.join("secret");
    storage::write_new(&file, b"secret").unwrap();
    assert!(std::process::Command::new("/bin/chmod")
        .args(["+a", "everyone allow read"])
        .arg(&file)
        .status()
        .unwrap()
        .success());
    assert!(storage::read(&file, 32).is_err());
    assert!(storage::validate(&file).is_err());
    assert!(std::process::Command::new("/bin/chmod")
        .args(["+a", "everyone allow read,file_inherit,directory_inherit"])
        .arg(&root.0)
        .status()
        .unwrap()
        .success());
    assert!(PrivateDirectory::open(&root.0).is_err());
    assert!(storage::create_new(&root.0.join("new-secret")).is_err());
    assert!(!root.0.join("new-secret").exists());
}
#[test]
fn helper_bounds_and_failures_do_not_emit_partial_secrets() {
    let root = Root::new();
    let path = root.0.join("secret");
    storage::write_new(&path, b"canary-secret").unwrap();
    for args in [
        vec!["read", path.to_str().unwrap(), "3"],
        vec!["read", path.to_str().unwrap()],
        vec!["read", path.to_str().unwrap(), "65537"],
    ] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_rss-private-storage"))
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert_eq!(result.stderr, b"private_storage_unavailable\n");
    }
}

#[cfg(unix)]
#[test]
fn sqlite_validation_preserves_live_posix_locks() {
    use std::os::fd::AsRawFd;
    let root = Root::new();
    let path = root.0.join("live.sqlite-shm");
    storage::write_new(&path, b"shared memory").unwrap();
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    // SAFETY: a zeroed flock is initialized below with a valid exclusive byte range.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    lock.l_len = 1;
    // SAFETY: file owns a writable fd and lock is a live, fully initialized flock.
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) },
        0
    );
    storage::validate_sqlite_file(&path).unwrap();
    let child = std::process::Command::new("python3")
        .args([
            "-c",
            r#"
import fcntl,sys
with open(sys.argv[1],'r+b') as stream:
    try: fcntl.lockf(stream,fcntl.LOCK_EX|fcntl.LOCK_NB,1)
    except BlockingIOError: sys.exit(0)
    sys.exit(1)
"#,
        ])
        .arg(&path)
        .status()
        .unwrap();
    assert!(
        child.success(),
        "validation released the live SQLite process lock"
    );
}

#[cfg(unix)]
#[test]
fn sqlite_metadata_keeps_private_file_type_mode_and_acl_checks() {
    use std::os::unix::{fs::symlink, fs::PermissionsExt};
    let root = Root::new();
    let path = root.0.join("journal.sqlite");
    storage::write_new(&path, b"journal").unwrap();
    storage::validate_sqlite_file(&path).unwrap();
    let link = root.0.join("alias");
    symlink(&path, &link).unwrap();
    assert!(storage::validate_sqlite_file(&link).is_err());
    assert!(storage::validate_sqlite_file(&root.0).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(storage::validate_sqlite_file(&path).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    #[cfg(target_os = "macos")]
    {
        assert!(std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone allow read"])
            .arg(&path)
            .status()
            .unwrap()
            .success());
        assert!(storage::validate_sqlite_file(&path).is_err());
        assert!(std::process::Command::new("/bin/chmod")
            .arg("-N")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        storage::validate_sqlite_file(&path).unwrap();
    }
}
