//! Current-login diagnostic only. No keychain items or password operations are exposed.
//! ref: Apple Security/SecKeychain.h, SecKeychainGetStatus.
use execution_ipc::device_access::KeychainStatus;
use std::ffi::{c_void, CStr, CString};
#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecKeychainOpen(path: *const libc::c_char, keychain: *mut *const c_void) -> i32;
    fn SecKeychainGetStatus(keychain: *const c_void, status: *mut u32) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: *const c_void);
}

pub(super) fn observe() -> KeychainStatus {
    // SAFETY: getpwuid_r writes only to owned bounded storage. The returned home pointer is
    // copied before that storage is dropped; no environment-selected path is consumed.
    let path = unsafe {
        let uid = libc::geteuid();
        if uid < 501 {
            return KeychainStatus::Unavailable;
        }
        let mut pwd = std::mem::zeroed::<libc::passwd>();
        let mut found = std::ptr::null_mut();
        let mut buffer = [0u8; 16384];
        if libc::getpwuid_r(
            uid,
            &mut pwd,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut found,
        ) != 0
            || found.is_null()
            || pwd.pw_dir.is_null()
        {
            return KeychainStatus::Unavailable;
        }
        let mut bytes = CStr::from_ptr(pwd.pw_dir).to_bytes().to_vec();
        bytes.extend_from_slice(b"/Library/Keychains/login.keychain-db");
        match CString::new(bytes) {
            Ok(v) => v,
            Err(_) => return KeychainStatus::Unavailable,
        }
    };
    // SAFETY: the keychain reference is released exactly once. GetStatus reads accessibility
    // flags and does not request access to any keychain item or unlock the keychain.
    unsafe {
        let mut keychain = std::ptr::null();
        let opened = SecKeychainOpen(path.as_ptr(), &mut keychain);
        if opened == -25294 {
            return KeychainStatus::Missing;
        }
        if opened != 0 || keychain.is_null() {
            return KeychainStatus::Unavailable;
        }
        let mut status = 0;
        let result = SecKeychainGetStatus(keychain, &mut status);
        CFRelease(keychain);
        match result {
            0 if status & 1 != 0 => KeychainStatus::Unlocked,
            0 => KeychainStatus::Locked,
            -25294 => KeychainStatus::Missing,
            _ => KeychainStatus::Unavailable,
        }
    }
}
