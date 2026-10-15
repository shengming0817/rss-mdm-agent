//! Read-only Security.framework inspection; never launches the application.
//! ref: apple-oss-distributions/Security OSX/libsecurity_codesigning/lib/SecStaticCode.h.
use super::*;
use std::ffi::{c_char, c_void};
type Ref = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: Ref,
        bytes: *const u8,
        len: isize,
        directory: u8,
    ) -> Ref;
    fn CFRelease(value: Ref);
    fn CFDictionaryGetValue(value: Ref, key: Ref) -> Ref;
    fn CFStringGetCString(value: Ref, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFStringGetTypeID() -> usize;
}
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecStaticCodeCreateWithPath(url: Ref, flags: u32, code: *mut Ref) -> i32;
    fn SecStaticCodeCheckValidity(code: Ref, flags: u32, requirement: Ref) -> i32;
    fn SecCodeCopySigningInformation(code: Ref, flags: u32, info: *mut Ref) -> i32;
    static kSecCodeInfoTeamIdentifier: Ref;
}
struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}
pub(super) fn inspect(path: &Path) -> Result<(&'static str, Option<String>)> {
    let bytes = path.as_os_str().as_bytes();
    let url = Owned(unsafe {
        CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            1,
        )
    });
    if url.0.is_null() {
        return Err(Status::Invalid.into());
    }
    let mut code = std::ptr::null();
    let rc = unsafe { SecStaticCodeCreateWithPath(url.0, 0, &mut code) };
    if rc != 0 {
        return Ok(("unknown", None));
    }
    let code = Owned(code);
    let status = match unsafe { SecStaticCodeCheckValidity(code.0, 0, std::ptr::null()) } {
        0 => "signed",
        -67062 => "unsigned",
        _ => "invalid",
    };
    let mut info = std::ptr::null();
    if unsafe { SecCodeCopySigningInformation(code.0, 2, &mut info) } != 0 {
        return Ok((status, None));
    }
    let info = Owned(info);
    let value = unsafe { CFDictionaryGetValue(info.0, kSecCodeInfoTeamIdentifier) };
    let mut buffer = [0i8; 1024];
    let team = if !value.is_null()
        && unsafe {
            CFGetTypeID(value) == CFStringGetTypeID()
                && CFStringGetCString(
                    value,
                    buffer.as_mut_ptr(),
                    buffer.len() as isize,
                    0x08000100,
                ) != 0
        } {
        Some(
            unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
                .to_str()
                .map_err(|_| Status::Invalid)?
                .to_owned(),
        )
    } else {
        None
    };
    Ok((status, team))
}
