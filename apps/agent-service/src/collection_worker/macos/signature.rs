//! Read-only Security.framework inspection; never launches the application.
//! ref: apple-oss-distributions/Security OSX/libsecurity_codesigning/lib/SecStaticCode.h.
use super::*;
use std::ffi::{c_char, c_void};
use std::os::unix::ffi::OsStrExt;
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
    static kSecCodeInfoIdentifier: Ref;
    static kSecGuestAttributePid: Ref;
    fn SecCodeCopyGuestWithAttributes(
        host: Ref,
        attributes: Ref,
        flags: u32,
        guest: *mut Ref,
    ) -> i32;
    fn SecCodeCheckValidity(code: Ref, flags: u32, requirement: Ref) -> i32;
}
struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}
pub(super) struct Evidence {
    pub status: &'static str,
    pub team: Option<String>,
    pub identifier: Option<String>,
}
pub(super) fn inspect(path: &Path) -> Result<Evidence> {
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
        return Ok(Evidence {
            status: "unknown",
            team: None,
            identifier: None,
        });
    }
    let code = Owned(code);
    let status = match unsafe { SecStaticCodeCheckValidity(code.0, 0, std::ptr::null()) } {
        0 => "signed",
        -67062 => "unsigned",
        _ => "invalid",
    };
    let mut info = std::ptr::null();
    if unsafe { SecCodeCopySigningInformation(code.0, 2, &mut info) } != 0 {
        return Ok(Evidence {
            status,
            team: None,
            identifier: None,
        });
    }
    let info = Owned(info);
    Ok(Evidence {
        status,
        team: string(info.0, unsafe { kSecCodeInfoTeamIdentifier })?,
        identifier: string(info.0, unsafe { kSecCodeInfoIdentifier })?,
    })
}
fn string(info: Ref, key: Ref) -> Result<Option<String>> {
    let value = unsafe { CFDictionaryGetValue(info, key) };
    let mut buffer = [0i8; 1024];
    if value.is_null() {
        return Ok(None);
    }
    if !unsafe {
        CFGetTypeID(value) == CFStringGetTypeID()
            && CFStringGetCString(
                value,
                buffer.as_mut_ptr(),
                buffer.len() as isize,
                0x08000100,
            ) != 0
    } {
        return Err(Status::Invalid.into());
    }
    Ok(Some(
        unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
            .to_str()
            .map_err(|_| Status::Invalid)?
            .into(),
    ))
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFNumberCreate(allocator: Ref, kind: i32, value: *const c_void) -> Ref;
    fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        key_callbacks: Ref,
        value_callbacks: Ref,
    ) -> Ref;
}
pub(super) fn process_matches(pid: i32, bundle: &str, team: &str) -> Result<bool> {
    let number = Owned(unsafe { CFNumberCreate(std::ptr::null(), 3, (&pid as *const i32).cast()) });
    if number.0.is_null() {
        return Err(Status::Failed.into());
    }
    let key = unsafe { kSecGuestAttributePid };
    // Both key and value outlive the dictionary; no retain callbacks needed.
    let attributes = Owned(unsafe {
        CFDictionaryCreate(
            std::ptr::null(),
            &key,
            &number.0,
            1,
            std::ptr::null(),
            std::ptr::null(),
        )
    });
    if attributes.0.is_null() {
        return Err(Status::Failed.into());
    }
    let mut code = std::ptr::null();
    if unsafe { SecCodeCopyGuestWithAttributes(std::ptr::null(), attributes.0, 0, &mut code) } != 0
    {
        return Ok(false);
    }
    let code = Owned(code);
    if unsafe { SecCodeCheckValidity(code.0, 0, std::ptr::null()) } != 0 {
        return Ok(false);
    }
    let mut info = std::ptr::null();
    if unsafe { SecCodeCopySigningInformation(code.0, 2, &mut info) } != 0 {
        return Ok(false);
    }
    let info = Owned(info);
    Ok(
        string(info.0, unsafe { kSecCodeInfoIdentifier })?.as_deref() == Some(bundle)
            && string(info.0, unsafe { kSecCodeInfoTeamIdentifier })?.as_deref() == Some(team),
    )
}
