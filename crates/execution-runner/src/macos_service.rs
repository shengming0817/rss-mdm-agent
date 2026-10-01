use crate::diagnostics::{record, Stage};
use crate::host::{self, Handler, Peer, PeerPolicy};
use execution_app::Error;
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::Duration,
};
static HANDLER: OnceLock<Mutex<Box<dyn Handler>>> = OnceLock::new();
static STOP: OnceLock<&'static AtomicBool> = OnceLock::new();
static REQUIREMENT: OnceLock<std::ffi::CString> = OnceLock::new();
extern "C" {
    fn rss_execution_listen(requirement: *const std::ffi::c_char) -> i32;
    fn proc_pidpath(pid: i32, buffer: *mut c_void, size: u32) -> i32;
    fn rss_execution_query(
        request: *const u8,
        length: usize,
        system: i32,
        output: *mut u8,
        capacity: *mut usize,
        requirement: *const std::ffi::c_char,
        expected_uid: u32,
    ) -> i32;
}
pub(crate) fn run(handler: Box<dyn Handler>, stop: &'static AtomicBool) -> Result<(), Error> {
    let result = run_inner(handler, stop);
    if let Err(error) = result {
        record(Stage::Startup, error);
    }
    result
}
fn run_inner(handler: Box<dyn Handler>, stop: &'static AtomicBool) -> Result<(), Error> {
    let requirement = handler.peer_policy().ok_or(Error::Unbound)?.requirement()?;
    REQUIREMENT.set(requirement).map_err(|_| Error::Conflict)?;
    HANDLER
        .set(Mutex::new(handler))
        .map_err(|_| Error::Conflict)?;
    STOP.set(stop).map_err(|_| Error::Conflict)?;
    let worker = std::thread::spawn(move || {
        let mut previous = None;
        while !stop.load(Ordering::Acquire) {
            let mut h = HANDLER.get().unwrap().lock().map_err(|_| {
                stop.store(true, Ordering::Release);
                record(Stage::Ingress, Error::Unavailable);
                Error::Unavailable
            })?;
            {
                match h.tick() {
                    Ok(()) => previous = None,
                    Err(error) => {
                        if previous != Some(error) {
                            record(Stage::Reconcile, error);
                            previous = Some(error);
                        }
                    }
                }
            }
            drop(h);
            std::thread::sleep(Duration::from_millis(100));
        }
        HANDLER
            .get()
            .unwrap()
            .lock()
            .map_err(|_| Error::Unavailable)?
            .stop()
            .inspect_err(|&error| {
                record(Stage::Shutdown, error);
            })?;
        Ok::<(), Error>(())
    });
    let result =
        unsafe { rss_execution_listen(REQUIREMENT.get().map_or(std::ptr::null(), |r| r.as_ptr())) };
    stop.store(true, Ordering::Release);
    worker.join().map_err(|_| Error::Unavailable)??;
    if result == 0 {
        Ok(())
    } else {
        Err(Error::Unavailable)
    }
}
#[no_mangle]
extern "C" fn rss_execution_stopping() -> i32 {
    i32::from(STOP.get().is_none_or(|s| s.load(Ordering::Acquire)))
}
#[no_mangle]
unsafe extern "C" fn rss_execution_call(
    connection: *mut c_void,
    pid: u32,
    uid: u32,
    session: u32,
    data: *const u8,
    size: usize,
    output: *mut u8,
    capacity: usize,
) -> isize {
    // The native stack strongly retains the NSXPCConnection for the entire synchronous call.
    if connection.is_null()
        || data.is_null()
        || output.is_null()
        || size > host::FRAME_LIMIT
        || rss_execution_stopping() != 0
    {
        return -1;
    }
    let Some(handler) = HANDLER.get() else {
        return -1;
    };
    let Some(stop) = STOP.get() else { return -1 };
    let peer = Peer {
        pid,
        uid: Some(uid),
        session,
        native: connection as usize,
    };
    let reply = handle_call(handler, stop, &peer, unsafe {
        std::slice::from_raw_parts(data, size)
    });
    match reply {
        Ok(reply) if reply.len() <= capacity => {
            unsafe {
                std::ptr::copy_nonoverlapping(reply.as_ptr(), output, reply.len());
            }
            reply.len() as isize
        }
        _ => -1,
    }
}
#[no_mangle]
unsafe extern "C" fn rss_execution_allow_helper(
    connection: *mut c_void,
    pid: u32,
    uid: u32,
    session: u32,
) -> i32 {
    if connection.is_null() || rss_execution_stopping() != 0 {
        return 0;
    }
    let Some(handler) = HANDLER.get() else {
        return 0;
    };
    let peer = Peer {
        pid,
        uid: Some(uid),
        session,
        native: connection as usize,
    };
    i32::from(
        handler
            .lock()
            .is_ok_and(|mut handler| handler.register_helper(&peer).is_ok()),
    )
}
#[no_mangle]
extern "C" fn rss_execution_helper_active() -> i32 {
    i32::from(
        HANDLER
            .get()
            .is_some_and(|handler| handler.lock().is_ok_and(|handler| handler.helper_active())),
    )
}
/// Native registered session identity; endpoint contents never pass through request JSON.
pub fn helper_session(uid: u32) -> Result<u32, Error> {
    extern "C" {
        fn rss_execution_helper_session(uid: u32) -> i64;
    }
    u32::try_from(unsafe { rss_execution_helper_session(uid) }).map_err(|_| Error::Unbound)
}
/// Query the endpoint registered by the exact OS user/session and recheck dynamic peer identity.
pub fn query_helper(
    bytes: &[u8],
    uid: u32,
    session: u32,
    policy: &PeerPolicy,
) -> Result<Vec<u8>, Error> {
    extern "C" {
        fn rss_execution_helper_query(
            request: *const u8,
            length: usize,
            uid: u32,
            session: u32,
            output: *mut u8,
            capacity: *mut usize,
            requirement: *const std::ffi::c_char,
        ) -> i32;
    }
    if bytes.len() > host::FRAME_LIMIT || policy.subjects != [uid.to_string()] {
        return Err(Error::Denied);
    }
    let requirement = policy.requirement()?;
    let mut output = vec![0; host::FRAME_LIMIT];
    let mut size = output.len();
    if unsafe {
        rss_execution_helper_query(
            bytes.as_ptr(),
            bytes.len(),
            uid,
            session,
            output.as_mut_ptr(),
            &mut size,
            requirement.as_ptr(),
        )
    } != 0
        || size > output.len()
    {
        return Err(Error::Unavailable);
    }
    output.truncate(size);
    Ok(output)
}

pub(crate) fn authenticate(peer: &Peer, policy: &PeerPolicy) -> Result<String, Error> {
    // The listener pins the union of desktop/helper images before receiving any message.
    // A specific operation may narrow that set; the live PID path must match its own pins.
    if REQUIREMENT.get().is_none() {
        return Err(Error::Denied);
    }
    let uid = peer.uid().ok_or(Error::Denied)?;
    if !policy.subjects.contains(&uid.to_string())
        || (policy.interactive
            && (uid == 0 || peer.session() == 0 || !crate::macos::console_user(uid)))
    {
        return Err(Error::Denied);
    }
    let mut bytes = [0u8; 4096];
    if unsafe {
        proc_pidpath(
            peer.pid() as i32,
            bytes.as_mut_ptr().cast(),
            bytes.len() as u32,
        )
    } <= 0
    {
        return Err(Error::Denied);
    }
    let end = bytes.iter().position(|b| *b == 0).ok_or(Error::Denied)?;
    let path = std::path::Path::new(std::str::from_utf8(&bytes[..end]).map_err(|_| Error::Denied)?);
    if !policy.images.iter().any(|image| image.verify(path).is_ok()) {
        return Err(Error::Denied);
    }
    Ok(uid.to_string())
}

/// Query a pinned server; the OS enforces dynamic code identity on reply messages.
pub fn query_trusted(bytes: &[u8], system: bool, server: &PeerPolicy) -> Result<Vec<u8>, Error> {
    if bytes.len() > host::FRAME_LIMIT || server.subjects.len() != 1 {
        return Err(Error::InvalidInput);
    }
    let requirement = server.requirement()?;
    let uid = server.subjects[0]
        .parse()
        .map_err(|_| Error::InvalidInput)?;
    let mut output = vec![0; host::FRAME_LIMIT];
    let mut size = output.len();
    if unsafe {
        rss_execution_query(
            bytes.as_ptr(),
            bytes.len(),
            i32::from(system),
            output.as_mut_ptr(),
            &mut size,
            requirement.as_ptr(),
            uid,
        )
    } != 0
        || size > output.len()
    {
        return Err(Error::Denied);
    }
    output.truncate(size);
    Ok(output)
}

fn handle_call(
    handler: &Mutex<Box<dyn Handler>>,
    stop: &AtomicBool,
    peer: &Peer,
    bytes: &[u8],
) -> Result<Vec<u8>, Error> {
    if stop.load(Ordering::Acquire) {
        return Err(Error::Unavailable);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut h = handler.lock().map_err(|_| Error::Unavailable)?;
        Ok(host::dispatch(h.as_mut(), peer, bytes))
    }))
    .unwrap_or(Err(Error::Unavailable));
    if result.is_err() && !stop.swap(true, Ordering::AcqRel) {
        record(Stage::Ingress, Error::Unavailable);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Panicking;
    impl Handler for Panicking {
        fn handle(&mut self, _: &Peer, _: host::Request) -> host::Reply {
            panic!("injected handler fault")
        }
        fn tick(&mut self) -> Result<(), Error> {
            Ok(())
        }
        fn stop(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }
    #[test]
    fn poisoned_handler_stops_the_service_instead_of_silently_skipping_work() {
        let handler: Mutex<Box<dyn Handler>> = Mutex::new(Box::new(Panicking));
        let stop = AtomicBool::new(false);
        let peer = Peer {
            pid: 1,
            uid: Some(1),
            session: 1,
            native: 0,
        };
        assert!(handle_call(
            &handler,
            &stop,
            &peer,
            br#"{"version":6,"request":{"method":"status","request":"r"}}"#
        )
        .is_err());
        assert!(stop.load(Ordering::Acquire));
        assert!(handler.is_poisoned());
    }
}
