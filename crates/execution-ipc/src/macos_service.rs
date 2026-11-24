use crate::diagnostics::{record, Stage};
use crate::host::{self, Handler, Peer, PeerPolicy};
use execution_app::Error;
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
static HANDLER: OnceLock<Mutex<Box<dyn Handler>>> = OnceLock::new();
static FENCE: OnceLock<Option<Arc<dyn host::SessionFence>>> = OnceLock::new();
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
    FENCE
        .set(handler.session_fence())
        .map_err(|_| Error::Conflict)?;
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
    };
    let reply = handle_call(
        handler,
        FENCE.get().and_then(|v| v.as_deref()),
        stop,
        &peer,
        unsafe { std::slice::from_raw_parts(data, size) },
    );
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
    query_helper_until(
        bytes,
        uid,
        session,
        policy,
        std::time::Instant::now() + Duration::from_secs(5),
    )
}
pub(crate) fn query_helper_until(
    bytes: &[u8],
    uid: u32,
    session: u32,
    policy: &PeerPolicy,
    deadline: std::time::Instant,
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
            timeout_ms: u32,
        ) -> i32;
    }
    if bytes.len() > host::FRAME_LIMIT || policy.subjects != [uid.to_string()] {
        return Err(Error::Denied);
    }
    let requirement = policy.requirement()?;
    let mut output = vec![0; host::FRAME_LIMIT];
    let mut size = output.len();
    let remaining = deadline
        .saturating_duration_since(std::time::Instant::now())
        .as_millis()
        .min(5000) as u32;
    if remaining == 0 {
        return Err(Error::Unavailable);
    }
    if unsafe {
        rss_execution_helper_query(
            bytes.as_ptr(),
            bytes.len(),
            uid,
            session,
            output.as_mut_ptr(),
            &mut size,
            requirement.as_ptr(),
            remaining,
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
    let started = std::time::Instant::now();
    // The listener pins the union of desktop/helper images before receiving any message.
    // A specific operation may narrow that set; the live PID path must match its own pins.
    if REQUIREMENT.get().is_none() {
        return Err(Error::Denied);
    }
    let uid = peer.uid().ok_or(Error::Denied)?;
    if !policy.subjects.contains(&uid.to_string())
        || (policy.interactive
            && (uid == 0 || peer.session() == 0 || !crate::macos_identity::console_user(uid)))
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
    let elapsed = started.elapsed().as_millis();
    if elapsed > 200 {
        eprintln!("RSS_IPC_AUTH_TIMING elapsedMs={elapsed}");
    }
    Ok(uid.to_string())
}

/// Query a pinned server; the OS enforces dynamic code identity on reply messages.
pub fn query_trusted(bytes: &[u8], system: bool, server: &PeerPolicy) -> Result<Vec<u8>, Error> {
    if bytes.len() > host::FRAME_LIMIT || server.subjects.len() != 1 {
        return Err(Error::InvalidInput);
    }
    let started = std::time::Instant::now();
    let requirement = server.requirement()?;
    let pins_ms = started.elapsed().as_millis();
    let uid = server.subjects[0]
        .parse()
        .map_err(|_| Error::InvalidInput)?;
    let mut output = vec![0; host::FRAME_LIMIT];
    let mut size = output.len();
    let status = unsafe {
        rss_execution_query(
            bytes.as_ptr(),
            bytes.len(),
            i32::from(system),
            output.as_mut_ptr(),
            &mut size,
            requirement.as_ptr(),
            uid,
        )
    };
    if status != 0 || started.elapsed().as_millis() > 1500 {
        eprintln!(
            "RSS_IPC_TIMING pinsMs={pins_ms} totalMs={} transport={status}",
            started.elapsed().as_millis()
        );
    }
    if status == -2 {
        return Err(Error::Denied);
    }
    if status != 0 || size == 0 || size > output.len() {
        return Err(Error::Unavailable);
    }
    output.truncate(size);
    Ok(output)
}

fn handle_call(
    handler: &Mutex<Box<dyn Handler>>,
    fence: Option<&dyn host::SessionFence>,
    stop: &AtomicBool,
    peer: &Peer,
    bytes: &[u8],
) -> Result<Vec<u8>, Error> {
    if stop.load(Ordering::Acquire) {
        return Err(Error::Unavailable);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(reply) = fence.and_then(|f| host::dispatch_fence(f, peer, bytes)) {
            return Ok(reply);
        }
        let mut h = handler.lock().map_err(|_| Error::Unavailable)?;
        if let Some(wait) = h.deferred_reply(peer, bytes) {
            drop(h);
            return Ok(host::encode_reply(wait()));
        }
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
    fn deferred_native_reply_releases_handler_for_other_calls() {
        struct Slow {
            started: std::sync::mpsc::Sender<()>,
            finish: Option<std::sync::mpsc::Receiver<()>>,
        }
        impl Handler for Slow {
            fn deferred_reply(&mut self, _: &Peer, bytes: &[u8]) -> Option<host::DeferredReply> {
                if bytes != b"slow" {
                    return None;
                }
                let started = self.started.clone();
                let release = self.finish.take().unwrap();
                Some(Box::new(move || {
                    started.send(()).unwrap();
                    release.recv_timeout(Duration::from_secs(2)).unwrap();
                    host::Reply::Unavailable
                }))
            }
            fn handle(&mut self, _: &Peer, _: host::Request) -> host::Reply {
                host::Reply::Rejected
            }
            fn tick(&mut self) -> Result<(), Error> {
                Ok(())
            }
            fn stop(&mut self) -> Result<(), Error> {
                Ok(())
            }
        }
        let (started, ready) = std::sync::mpsc::channel();
        let (done, finish) = std::sync::mpsc::channel();
        let handler = Mutex::new(Box::new(Slow {
            started,
            finish: Some(finish),
        }) as Box<dyn Handler>);
        let stop = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let peer = || Peer {
                pid: 1,
                uid: Some(1),
                session: 1,
            };
            scope.spawn(|| {
                handle_call(
                    &handler,
                    None,
                    &stop,
                    &Peer {
                        pid: 1,
                        uid: Some(1),
                        session: 1,
                    },
                    b"slow",
                )
                .unwrap()
            });
            ready.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(handler.try_lock().is_ok());
            assert!(handle_call(&handler, None, &stop, &peer(), b"{}").is_ok());
            done.send(()).unwrap();
        });
    }
    #[test]
    fn poisoned_handler_stops_the_service_instead_of_silently_skipping_work() {
        let handler: Mutex<Box<dyn Handler>> = Mutex::new(Box::new(Panicking));
        let stop = AtomicBool::new(false);
        let peer = Peer {
            pid: 1,
            uid: Some(1),
            session: 1,
        };
        assert!(handle_call(
            &handler,
            None,
            &stop,
            &peer,
            &serde_json::to_vec(&serde_json::json!({"version": crate::host::IPC_VERSION, "request":{"method":"offers"}})).unwrap()
        )
        .is_err());
        assert!(stop.load(Ordering::Acquire));
        assert!(handler.is_poisoned());
    }
    #[test]
    fn revocation_completes_while_the_business_handler_is_busy() {
        struct Fence(AtomicBool);
        impl host::SessionFence for Fence {
            fn revoke(&self, _: &Peer) -> host::Reply {
                self.0.store(true, Ordering::Release);
                host::Reply::ContextRevoked
            }
        }
        let handler: Mutex<Box<dyn Handler>> = Mutex::new(Box::new(Panicking));
        let held = handler.lock().unwrap();
        let fence = Fence(AtomicBool::new(false));
        let stop = AtomicBool::new(false);
        let peer = Peer {
            pid: 1,
            uid: Some(1),
            session: 1,
        };
        let bytes = host::encode(host::Request::RevokeContext {}).unwrap();
        let reply = handle_call(&handler, Some(&fence), &stop, &peer, &bytes).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&reply).unwrap()["reply"]["kind"],
            "contextRevoked"
        );
        assert!(fence.0.load(Ordering::Acquire));
        for bytes in [b"{}".to_vec(), host::encode(host::Request::Offers {}).unwrap(), serde_json::to_vec(&serde_json::json!({"version": host::IPC_VERSION - 1, "request": {"method":"employeeSession", "value":null}})).unwrap()] {
            assert!(host::dispatch_fence(&fence, &peer, &bytes).is_none());
        }
        drop(held);
    }
    #[test]
    #[ignore = "requires an isolated macOS login with no execution service registration"]
    fn absent_service_transport_is_unavailable_without_claiming_identity_rejection() {
        use sha2::{Digest as _, Sha256};
        let status = std::process::Command::new("/bin/launchctl")
            .args(["print", "system/com.rss-mdm.agent.execution"])
            .output()
            .unwrap();
        assert_eq!(status.status.code(), Some(113));
        let signature = std::process::Command::new("/usr/bin/codesign")
            .args(["-d", "--verbose=4", "/bin/sh"])
            .output()
            .unwrap();
        assert!(signature.status.success());
        let description = String::from_utf8(signature.stderr).unwrap();
        let cdhash = description
            .lines()
            .find_map(|line| line.strip_prefix("CDHash="))
            .unwrap();
        let policy = PeerPolicy {
            images: vec![installation_security::Artifact {
                path: "/bin/sh".into(),
                sha256: format!("{:x}", Sha256::digest(std::fs::read("/bin/sh").unwrap())),
                cdhash: Some(cdhash.into()),
            }],
            subjects: vec!["0".into()],
            interactive: false,
        };
        let request = host::encode(host::Request::ServiceStatus {}).unwrap();
        assert_eq!(
            query_trusted(&request, true, &policy),
            Err(Error::Unavailable)
        );
    }
}

/// Read this GUI helper's current unlocked-session generation.
pub fn presence_epoch() -> Result<uuid::Uuid, Error> {
    unsafe extern "C" {
        fn rss_execution_presence_epoch(out: *mut u8) -> i32;
    }
    let mut bytes = [0u8; 16];
    // SAFETY: native code writes the sixteen UUID bytes synchronously into this buffer.
    if unsafe { rss_execution_presence_epoch(bytes.as_mut_ptr()) } != 1 {
        return Err(Error::Unbound);
    }
    Ok(uuid::Uuid::from_bytes(bytes))
}
