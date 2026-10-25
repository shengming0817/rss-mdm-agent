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
    fn CFStringCreateWithCString(allocator: Ref, text: *const c_char, encoding: u32) -> Ref;
    fn CFDictionaryGetValue(value: Ref, key: Ref) -> Ref;
    fn CFStringGetCString(value: Ref, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDataGetTypeID() -> usize;
    fn CFDataGetLength(data: Ref) -> isize;
    fn CFDataGetBytePtr(data: Ref) -> *const u8;
}
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecRequirementCreateWithString(text: Ref, flags: u32, requirement: *mut Ref) -> i32;
    fn SecStaticCodeCreateWithPath(url: Ref, flags: u32, code: *mut Ref) -> i32;
    fn SecStaticCodeCheckValidity(code: Ref, flags: u32, requirement: Ref) -> i32;
    fn SecCodeCopySigningInformation(code: Ref, flags: u32, info: *mut Ref) -> i32;
    static kSecCodeInfoTeamIdentifier: Ref;
    static kSecCodeInfoIdentifier: Ref;
    static kSecCodeInfoUnique: Ref;
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
    pub cdhash: Option<String>,
}
pub(super) fn inspect(path: &Path, approved: Option<(&str, &str)>) -> Result<Evidence> {
    let requirement = approved
        .map(|(bundle, team)| requirement(bundle, team))
        .transpose()?;
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
            cdhash: None,
        });
    }
    let code = Owned(code);
    let status = match unsafe {
        SecStaticCodeCheckValidity(
            code.0,
            0,
            requirement.as_ref().map_or(std::ptr::null(), |r| r.0),
        )
    } {
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
            cdhash: None,
        });
    }
    let info = Owned(info);
    Ok(Evidence {
        status,
        team: string(info.0, unsafe { kSecCodeInfoTeamIdentifier })?,
        identifier: string(info.0, unsafe { kSecCodeInfoIdentifier })?,
        cdhash: code_hash(info.0)?,
    })
}
fn code_hash(info: Ref) -> Result<Option<String>> {
    let data = unsafe { CFDictionaryGetValue(info, kSecCodeInfoUnique) };
    if data.is_null() {
        return Ok(None);
    }
    if unsafe { CFGetTypeID(data) != CFDataGetTypeID() || CFDataGetLength(data) != 20 } {
        return Err(Status::Invalid.into());
    }
    let bytes = unsafe { CFDataGetBytePtr(data) };
    if bytes.is_null() {
        return Err(Status::Invalid.into());
    }
    Ok(Some(
        unsafe { std::slice::from_raw_parts(bytes, 20) }
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    ))
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
fn process_code(pid: i32) -> Result<Option<Owned>> {
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
        return Ok(None);
    }
    let code = Owned(code);
    Ok(Some(code))
}
pub(super) fn process_matches(pid: i32, bundle: &str, team: &str) -> Result<bool> {
    let requirement = requirement(bundle, team)?;
    let Some(code) = process_code(pid)? else {
        return Ok(false);
    };
    if unsafe { SecCodeCheckValidity(code.0, 0, requirement.0) } != 0 {
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

// The query worker's parent is the supervisor running the same fixed Agent image.
// run() and the supervisor already hold verify_retained leases for the frozen SHA.
// Rehashing here would bypass the query's native-read budget.
// ref: rust-security-framework v3.5.1 security-framework/src/os/macos/code_signing.rs.
pub(super) fn pinned_process_matches(
    pid: i32,
    image: &installation_security::Artifact,
) -> Result<bool> {
    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_pidpath(pid: i32, buffer: *mut c_void, size: u32) -> i32;
    }
    let mut path = [0u8; 4096];
    if unsafe { proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) } <= 0 {
        return Ok(false);
    }
    let path = std::ffi::CStr::from_bytes_until_nul(&path).map_err(|_| Status::Invalid)?;
    let path = Path::new(std::ffi::OsStr::from_bytes(path.to_bytes()));
    if path != image.path {
        return Ok(false);
    }
    let expression = image.requirement().map_err(|_| Status::Invalid)?;
    let requirement = code_requirement(&expression)?;
    let Some(code) = process_code(pid)? else {
        return Ok(false);
    };
    Ok(unsafe { SecCodeCheckValidity(code.0, 0, requirement.0) } == 0)
}

fn requirement(bundle: &str, team: &str) -> Result<Owned> {
    // A closed identity is escaped by construction, never interpolated from an arbitrary requirement.
    if bundle.is_empty()
        || bundle.len() > 255
        || !bundle
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || team.len() != 10
        || !team
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return Err(Status::Invalid.into());
    }
    let expression = CString::new(format!("anchor apple generic and identifier \"{bundle}\" and certificate leaf[subject.OU] = \"{team}\"")).map_err(|_| Status::Invalid)?;
    code_requirement(&expression)
}
fn code_requirement(expression: &std::ffi::CStr) -> Result<Owned> {
    let text = Owned(unsafe {
        CFStringCreateWithCString(std::ptr::null(), expression.as_ptr(), 0x08000100)
    });
    if text.0.is_null() {
        return Err(Status::Failed.into());
    }
    let mut value = std::ptr::null();
    if unsafe { SecRequirementCreateWithString(text.0, 0, &mut value) } != 0 {
        return Err(Status::Invalid.into());
    }
    Ok(Owned(value))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_process_identity_does_not_require_an_apple_team() {
        // A system image without a Team is a real, protected fixture, independent
        // of whether the developer has any signing certificate in their Keychain.
        let path = PathBuf::from("/bin/sleep");
        let evidence = inspect(&path, None).unwrap();
        assert_eq!(evidence.status, "signed");
        assert!(evidence.team.is_none());
        let mut image = installation_security::Artifact {
            sha256: crate::installation::hash(&std::fs::read(&path).unwrap()),
            cdhash: evidence.cdhash,
            path,
        };
        let digest = execution_contract::Digest::new(image.sha256.clone()).unwrap();
        let _lease = execution_runner::staging::verify_retained(&image.path, &digest).unwrap();
        let mut child = std::process::Command::new(&image.path)
            .arg("10")
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        // Wait for exec; a PID alone does not establish the new image identity.
        let until = Instant::now() + Duration::from_secs(3);
        let matched = loop {
            if pinned_process_matches(pid, &image).unwrap() {
                break true;
            }
            if Instant::now() >= until {
                break false;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let wrong_pid = pinned_process_matches(std::process::id() as i32, &image).unwrap();
        image.cdhash = Some("0".repeat(40));
        let wrong_code = pinned_process_matches(pid, &image).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(matched);
        assert!(!wrong_pid);
        assert!(!wrong_code);
    }
    #[test]
    fn untrusted_code_cannot_satisfy_an_approved_apple_team() {
        // The developer test image has no approved Apple team certificate.
        let image = std::env::current_exe().unwrap();
        let value = inspect(&image, Some(("org.example.fixture", "ABCDE12345"))).unwrap();
        assert_ne!(value.status, "signed");
        assert!(!process_matches(
            std::process::id() as i32,
            "org.example.fixture",
            "ABCDE12345"
        )
        .unwrap());
        assert!(requirement("org.example\" or true", "ABCDE12345").is_err());
    }
}
