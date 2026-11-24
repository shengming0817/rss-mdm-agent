//! Shared native AppKit password dialog. Only the native caller receives the secret.
use std::ffi::{c_char, CString};
use zeroize::Zeroizing;

pub(super) fn password(title: &str, label: &str) -> Option<Zeroizing<String>> {
    unsafe extern "C" {
        fn rss_secure_password(
            title: *const c_char,
            label: *const c_char,
            output: *mut u8,
            capacity: usize,
        ) -> i32;
    }
    let title = CString::new(title).ok()?;
    let label = CString::new(label).ok()?;
    let mut bytes = Zeroizing::new(vec![0u8; 1025]);
    // SAFETY: the foreground dialog writes at most capacity bytes before returning.
    let n = unsafe {
        rss_secure_password(
            title.as_ptr(),
            label.as_ptr(),
            bytes.as_mut_ptr(),
            bytes.len(),
        )
    };
    if !(1..=1024).contains(&n) {
        return None;
    }
    Some(Zeroizing::new(
        std::str::from_utf8(&bytes[..n as usize]).ok()?.to_owned(),
    ))
}
