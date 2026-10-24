use super::Rejected;
use std::ffi::c_char;
extern "C" {
    fn rss_acl_empty(path: *const c_char) -> i32;
}
pub(crate) fn acl_empty(path: &std::path::Path) -> Result<(), Rejected> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| Rejected)?;
    // SAFETY: NUL-terminated path remains valid for this synchronous ACL query.
    if unsafe { rss_acl_empty(path.as_ptr()) } != 1 {
        return Err(Rejected);
    }
    Ok(())
}
/// /Applications is a shared administrator-writable discovery directory, never an image pin.
pub fn applications_directory() -> Result<(), Rejected> {
    use std::os::unix::fs::MetadataExt;
    let path = std::path::Path::new("/Applications");
    super::protected(path.parent().ok_or(Rejected)?)?;
    acl_empty(path)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| Rejected)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.gid() != 80
        || metadata.mode() & 0o002 != 0
    {
        return Err(Rejected);
    }
    Ok(())
}

/// Verify a real Apple signing team and exact bundle identifier, in addition to protected image pins.
pub fn verify_apple_identity(
    path: &std::path::Path,
    team: &str,
    identifier: &str,
) -> Result<(), Rejected> {
    use std::os::unix::ffi::OsStrExt;
    if team.len() != 10
        || !team
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        || identifier.is_empty()
        || identifier.len() > 255
        || !identifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        return Err(Rejected);
    }
    extern "C" {
        fn rss_signed_identity(path: *const c_char, requirement: *const c_char) -> i32;
    }
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| Rejected)?;
    let requirement = std::ffi::CString::new(format!("anchor apple generic and certificate leaf[subject.OU] = \"{team}\" and identifier \"{identifier}\""))
        .map_err(|_| Rejected)?;
    // SAFETY: both strings are owned, bounded and NUL-terminated for this synchronous call.
    if unsafe { rss_signed_identity(path.as_ptr(), requirement.as_ptr()) } == 1 {
        Ok(())
    } else {
        Err(Rejected)
    }
}

#[cfg(test)]
mod permission_tests {
    #[test]
    fn mode_bits_do_not_hide_an_extended_acl() {
        let path = std::env::temp_dir().join(format!("rss-acl-{}", std::process::id()));
        std::fs::write(&path, b"fixture").unwrap();
        assert!(super::acl_empty(&path).is_ok());
        assert!(std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone allow write"])
            .arg(&path)
            .status()
            .unwrap()
            .success());
        assert!(super::acl_empty(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
