use super::*;
use std::ffi::{c_char, c_void};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
static POLICY: OnceLock<Policy> = OnceLock::new();

extern "C" {
    fn rss_acl_empty(path: *const c_char) -> i32;
    fn rss_service_run(requirement: *const c_char) -> i32;
    fn rss_service_query(
        requirement: *const c_char,
        uid: u32,
        output: *mut u8,
        size: *mut usize,
    ) -> i32;
    fn proc_pidpath(pid: i32, buffer: *mut c_void, size: u32) -> i32;
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

pub fn run(policy: Policy) -> Result<(), Rejected> {
    let uid: u32 = policy.service_subject.parse().map_err(|_| Rejected)?;
    // SAFETY: geteuid takes no arguments.
    if uid == 0 || unsafe { libc::geteuid() } != uid {
        return Err(Rejected);
    }
    let requirement = policy.client.requirement()?;
    POLICY.set(policy).map_err(|_| Rejected)?;
    // SAFETY: requirement outlives the synchronous service run loop.
    if unsafe { rss_service_run(requirement.as_ptr()) } != 0 {
        return Err(Rejected);
    }
    Ok(())
}
pub fn query(policy: &Policy) -> Result<Status, Rejected> {
    let requirement = policy.service.requirement()?;
    let uid: u32 = policy.service_subject.parse().map_err(|_| Rejected)?;
    if uid == 0 {
        return Err(Rejected);
    }
    let mut bytes = vec![0u8; MAX_FRAME];
    let mut len = bytes.len();
    // SAFETY: native bridge writes only within the supplied buffer before returning.
    if unsafe { rss_service_query(requirement.as_ptr(), uid, bytes.as_mut_ptr(), &mut len) } != 0
        || len > bytes.len()
    {
        return Err(Rejected);
    }
    let status: Status = serde_json::from_slice(&bytes[..len]).map_err(|_| Rejected)?;
    if status != policy.status() {
        return Err(Rejected);
    }
    Ok(status)
}

// Only the XPC bridge calls these entry points after enforcing the peer code
// requirement. The returned context is owned by that one XPC connection.
#[no_mangle]
extern "C" fn rss_peer_new(uid: u32, session: u32, pid: i32) -> *mut c_void {
    let Some(policy) = POLICY.get() else {
        return std::ptr::null_mut();
    };
    if session == 0 || !policy.allowed_users.contains(&uid.to_string()) {
        return std::ptr::null_mut();
    }
    let mut path = [0u8; 4096];
    // SAFETY: writable buffer and fixed capacity; PID comes from NSXPCConnection.
    let count = unsafe { proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
    if count <= 0 {
        return std::ptr::null_mut();
    }
    let end = path.iter().position(|b| *b == 0).unwrap_or(path.len());
    let Ok(path) = std::str::from_utf8(&path[..end]) else {
        return std::ptr::null_mut();
    };
    if policy.client.verify(&PathBuf::from(path)).is_err() {
        return std::ptr::null_mut();
    }
    let Ok(challenge) = random_challenge() else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(Mutex::new(Connection::new(
        challenge,
        Instant::now(),
        policy.status(),
    ))))
    .cast()
}
#[no_mangle]
unsafe extern "C" fn rss_peer_free(peer: *mut c_void) {
    if !peer.is_null() {
        // SAFETY: bridge releases the unique context only after all callbacks finish.
        unsafe {
            drop(Box::from_raw(peer.cast::<Mutex<Connection>>()));
        }
    }
}
fn copy(bytes: Result<Vec<u8>, Rejected>, output: *mut u8, size: usize) -> isize {
    let Ok(bytes) = bytes else {
        return -1;
    };
    if bytes.len() > size || output.is_null() {
        return -1;
    }
    // SAFETY: bridge supplied output capacity is checked before copying.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
    }
    bytes.len() as isize
}
#[no_mangle]
unsafe extern "C" fn rss_peer_greeting(peer: *mut c_void, output: *mut u8, size: usize) -> isize {
    // SAFETY: context lifetime is retained by the Objective-C peer during callbacks.
    let Ok(peer) = (unsafe { &*peer.cast::<Mutex<Connection>>() }).lock() else {
        return -1;
    };
    copy(peer.greeting(), output, size)
}
#[no_mangle]
unsafe extern "C" fn rss_peer_request(
    peer: *mut c_void,
    data: *const u8,
    size: usize,
    output: *mut u8,
    capacity: usize,
) -> isize {
    if size > MAX_FRAME || data.is_null() {
        return -1;
    }
    // SAFETY: both pointers are retained by the bridge for this synchronous call.
    let Ok(mut peer) = (unsafe { &*peer.cast::<Mutex<Connection>>() }).lock() else {
        return -1;
    };
    let bytes = unsafe { std::slice::from_raw_parts(data, size) };
    copy(peer.accept(bytes, Instant::now()), output, capacity)
}
#[no_mangle]
unsafe extern "C" fn rss_query_request(
    data: *const u8,
    size: usize,
    output: *mut u8,
    capacity: usize,
) -> isize {
    if size > MAX_FRAME || data.is_null() {
        return -1;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, size) };
    copy(
        client_request(bytes).map(|(_, request)| request),
        output,
        capacity,
    )
}
#[no_mangle]
unsafe extern "C" fn rss_query_reply(
    greeting: *const u8,
    greeting_size: usize,
    data: *const u8,
    size: usize,
    output: *mut u8,
    capacity: usize,
) -> isize {
    if greeting_size > MAX_FRAME || size > MAX_FRAME || greeting.is_null() || data.is_null() {
        return -1;
    }
    let greeting = unsafe { std::slice::from_raw_parts(greeting, greeting_size) };
    let bytes = unsafe { std::slice::from_raw_parts(data, size) };
    copy(
        client_request(greeting)
            .and_then(|(challenge, _)| client_reply(bytes, &challenge))
            .and_then(|status| serde_json::to_vec(&status).map_err(|_| Rejected)),
        output,
        capacity,
    )
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
