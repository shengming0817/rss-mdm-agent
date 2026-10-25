//! Read-only fixed Apple escrow carrier; no profile installation, rotation or PRK decryption.
use agent_client::filevault::ReadError;
use std::{
    fs::{File, Metadata, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
const MAX: usize = rss_mdm_agent_wire::filevault::CMS_BYTES;
/// The production entry has no path parameter. Unit/service fixtures never inspect a host PRK.
pub(crate) fn read() -> Result<Vec<u8>, ReadError> {
    #[cfg(test)]
    {
        Err(ReadError::Unavailable)
    }
    #[cfg(not(test))]
    {
        read_file(Path::new("/var/db/FileVaultPRK.dat"), 0)
    }
}
fn io_error(error: std::io::Error) -> ReadError {
    match error.kind() {
        std::io::ErrorKind::NotFound => ReadError::Missing,
        _ if error.raw_os_error() == Some(libc::ELOOP) => ReadError::Unsafe,
        _ => ReadError::Unavailable,
    }
}
fn safe(meta: &Metadata, owner: u32) -> bool {
    meta.is_file()
        && meta.uid() == owner
        && meta.mode() & 0o077 == 0
        && meta.nlink() == 1
        && meta.len() > 0
        && meta.len() <= MAX as u64
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    (
        a.dev(),
        a.ino(),
        a.len(),
        a.mtime(),
        a.mtime_nsec(),
        a.ctime(),
        a.ctime_nsec(),
        a.mode(),
        a.uid(),
        a.nlink(),
    ) == (
        b.dev(),
        b.ino(),
        b.len(),
        b.mtime(),
        b.mtime_nsec(),
        b.ctime(),
        b.ctime_nsec(),
        b.mode(),
        b.uid(),
        b.nlink(),
    )
}
fn read_file(path: &Path, owner: u32) -> Result<Vec<u8>, ReadError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(io_error)?;
    snapshot(file, path, owner)
}
fn snapshot(mut file: File, path: &Path, owner: u32) -> Result<Vec<u8>, ReadError> {
    let before = file.metadata().map_err(io_error)?;
    if !safe(&before, owner) {
        return Err(ReadError::Unsafe);
    }
    platform_private_storage::validate_file_handle(&file).map_err(|_| ReadError::Unsafe)?;
    let mut bytes = Vec::with_capacity(before.len() as usize);
    file.by_ref()
        .take((MAX + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    let after = file.metadata().map_err(io_error)?;
    let current = std::fs::symlink_metadata(path).map_err(io_error)?;
    if bytes.len() != before.len() as usize
        || !safe(&after, owner)
        || !safe(&current, owner)
        || !same(&before, &after)
        || !same(&after, &current)
    {
        return Err(ReadError::Unsafe);
    }
    platform_private_storage::validate_file_handle(&file).map_err(|_| ReadError::Unsafe)?;
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    #[test]
    fn bounded_snapshot_rejects_links_mutable_files_and_replacement() {
        let root = std::env::temp_dir().join(format!("filevault-fixture-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("carrier");
        // SAFETY: geteuid has no pointer arguments and reads only this fixture process's identity.
        let owner = unsafe { libc::geteuid() };
        std::fs::write(&path, vec![42; MAX]).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(read_file(&path, owner).unwrap().len(), MAX);
        let link = root.join("link");
        symlink(&path, &link).unwrap();
        assert_eq!(read_file(&link, owner), Err(ReadError::Unsafe));
        std::fs::remove_file(&link).unwrap();
        std::fs::hard_link(&path, &link).unwrap();
        assert_eq!(read_file(&path, owner), Err(ReadError::Unsafe));
        std::fs::remove_file(&link).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o620)).unwrap();
        assert_eq!(read_file(&path, owner), Err(ReadError::Unsafe));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone allow write"])
            .arg(&path)
            .status()
            .unwrap()
            .success());
        assert_eq!(read_file(&path, owner), Err(ReadError::Unsafe));
        assert!(std::process::Command::new("/bin/chmod")
            .arg("-N")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        assert_eq!(
            read_file(&path, owner.wrapping_add(1)),
            Err(ReadError::Unsafe)
        );
        let old = File::open(&path).unwrap();
        std::fs::rename(&path, &link).unwrap();
        std::fs::write(&path, [0; 10]).unwrap();
        assert_eq!(snapshot(old, &path, owner), Err(ReadError::Unsafe));
        std::fs::write(&path, vec![42; MAX + 1]).unwrap();
        assert_eq!(read_file(&path, owner), Err(ReadError::Unsafe));
        std::fs::remove_dir_all(root).unwrap();
    }
}
