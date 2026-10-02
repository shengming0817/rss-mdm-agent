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
