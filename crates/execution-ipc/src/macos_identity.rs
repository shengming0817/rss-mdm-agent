use execution_app::Error;
use execution_contract::Id;
pub(crate) fn console_account() -> Result<u32, Error> {
    #[link(name = "SystemConfiguration", kind = "framework")]
    extern "C" {
        fn SCDynamicStoreCopyConsoleUser(
            store: *const std::ffi::c_void,
            uid: *mut u32,
            gid: *mut u32,
        ) -> *const std::ffi::c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(value: *const std::ffi::c_void);
    }
    let mut actual = u32::MAX;
    let mut gid = 0;
    // SAFETY: returned CF object is released exactly once; outputs are valid writable integers.
    unsafe {
        let name = SCDynamicStoreCopyConsoleUser(std::ptr::null(), &mut actual, &mut gid);
        if name.is_null() {
            return Err(Error::Unbound);
        }
        CFRelease(name);
        if actual == 0 || actual == u32::MAX {
            Err(Error::Unbound)
        } else {
            Ok(actual)
        }
    }
}
/// Whether this UID owns the current non-root GUI console login.
pub fn console_user(uid: u32) -> bool {
    console_account() == Ok(uid)
}
// ref: Apple XNU bsd/kern/kern_mib.c, kern.bootsessionuuid (kernel boot generation).
pub(crate) fn boot_generation() -> Result<Id, Error> {
    let mut bytes = [0u8; 128];
    let mut length = bytes.len();
    if unsafe {
        libc::sysctlbyname(
            c"kern.bootsessionuuid".as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || length == 0
        || length > bytes.len()
    {
        return Err(Error::Unavailable);
    }
    let text = std::str::from_utf8(&bytes[..length])
        .map_err(|_| Error::Unavailable)?
        .trim_end_matches('\0');
    Id::new(text).map_err(|_| Error::Unavailable)
}
#[cfg(test)]
mod boot_tests {
    #[test]
    fn same_kernel_boot_is_stable_across_reads() {
        assert_eq!(
            super::boot_generation().unwrap(),
            super::boot_generation().unwrap()
        );
    }
}
