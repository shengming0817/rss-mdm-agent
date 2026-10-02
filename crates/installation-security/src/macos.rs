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
