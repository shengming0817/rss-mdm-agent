//! SCM and per-session helper transport. The transport supplies native facts, never authority.
use crate::diagnostics::{record, Stage};
use crate::host::{self, Handler, Peer};
use crate::windows_identity::*;
use execution_app::Error;
use execution_contract::*;
use std::os::windows::io::{AsRawHandle, IntoRawHandle, OwnedHandle};
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        Mutex, OnceLock,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer},
};
use windows_sys::Win32::{Foundation::*, Security::*, System::Threading::*};
use windows_sys::Win32::{
    Storage::FileSystem::*,
    System::{Console::*, Pipes::*, Services::*},
};
const NAME: &str = "RssExecution";
/// Inspect only the actual current SCM registration; access errors are not absence.
pub fn registration_present() -> Result<bool, Error> {
    unsafe {
        let manager = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
        if manager.is_null() {
            return Err(Error::Unavailable);
        }
        let service = OpenServiceW(manager, wide(NAME).as_ptr(), SERVICE_QUERY_STATUS);
        let error = GetLastError();
        CloseServiceHandle(manager);
        if !service.is_null() {
            CloseServiceHandle(service);
            return Ok(true);
        }
        if error == ERROR_SERVICE_DOES_NOT_EXIST {
            Ok(false)
        } else {
            Err(Error::Unavailable)
        }
    }
}
static STOP: OnceLock<&'static AtomicBool> = OnceLock::new();
static HANDLER: Mutex<Option<Box<dyn Handler>>> = Mutex::new(None);
static STATUS: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static FAILED: AtomicBool = AtomicBool::new(false);
static CLIENT_SUBJECTS: OnceLock<Vec<String>> = OnceLock::new();
fn stopping() -> bool {
    STOP.get().is_none_or(|s| s.load(Ordering::Acquire))
}
fn stop() {
    if let Some(value) = STOP.get() {
        value.store(true, Ordering::Release)
    }
}
fn status(state: u32, code: u32) {
    let handle = STATUS.load(Ordering::Acquire);
    if handle.is_null() {
        return;
    }
    let value = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING {
            SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
        } else {
            0
        },
        dwWin32ExitCode: code,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: if state == SERVICE_STOP_PENDING {
            10000
        } else {
            0
        },
    };
    unsafe {
        SetServiceStatus(handle, &value);
    }
}
unsafe extern "system" fn control(event: u32, _: u32, _: *mut c_void, _: *mut c_void) -> u32 {
    match event {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            status(SERVICE_STOP_PENDING, 0);
            stop();
            0
        }
        SERVICE_CONTROL_INTERROGATE => 0,
        _ => ERROR_CALL_NOT_IMPLEMENTED,
    }
}
unsafe extern "system" fn console(_: u32) -> i32 {
    stop();
    1
}
unsafe extern "system" fn service_main(_: u32, _: *mut *mut u16) {
    let result = std::panic::catch_unwind(|| {
        let handle =
            unsafe { RegisterServiceCtrlHandlerExW(wide(NAME).as_ptr(), Some(control), null()) };
        if handle.is_null() {
            return Err(Error::Unavailable);
        }
        STATUS.store(handle, Ordering::Release);
        status(SERVICE_START_PENDING, 0);
        let handler = HANDLER
            .lock()
            .map_err(|_| Error::Unavailable)?
            .take()
            .ok_or(Error::Unavailable)?;
        drive(handler, true)
    });
    let failed = !matches!(result, Ok(Ok(())));
    FAILED.store(failed, Ordering::Release);
    if failed {
        record(Stage::Startup, Error::Unavailable);
    }
    status(
        SERVICE_STOPPED,
        if failed { ERROR_PROCESS_ABORTED } else { 0 },
    );
}
/// Run a LocalSystem SCM host or the explicitly selected current interactive-session helper.
pub fn run(
    handler: Box<dyn Handler>,
    stop_flag: &'static AtomicBool,
    system: bool,
) -> Result<(), Error> {
    let (subject, session) = token_identity()?;
    if (system && (subject != "S-1-5-18" || session != 0))
        || (!system && (subject == "S-1-5-18" || session == 0))
    {
        return Err(Error::Unbound);
    }
    let policy = handler.peer_policy().ok_or(Error::Unbound)?;
    policy.validate()?;
    CLIENT_SUBJECTS
        .set(policy.subjects)
        .map_err(|_| Error::Conflict)?;
    STOP.set(stop_flag).map_err(|_| Error::Conflict)?;
    if !system {
        if unsafe { SetConsoleCtrlHandler(Some(console), 1) } == 0 {
            return Err(Error::Unavailable);
        }
        return drive(handler, false);
    }
    *HANDLER.lock().map_err(|_| Error::Unavailable)? = Some(handler);
    let mut name = wide(NAME);
    let entries = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: null_mut(),
            lpServiceProc: None,
        },
    ];
    if unsafe { StartServiceCtrlDispatcherW(entries.as_ptr()) } == 0
        || FAILED.load(Ordering::Acquire)
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}
fn endpoint(system: bool) -> Result<(String, String), Error> {
    let (subject, session) = token_identity()?;
    Ok(if system {
        (r"\\.\pipe\rss-mdm-execution-system-v5".into(), {
            let mut sddl = String::from("D:P(A;;GA;;;SY)(A;;GA;;;BA)");
            if let Some(subjects) = CLIENT_SUBJECTS.get() {
                for subject in subjects {
                    if !subject.starts_with("S-1-")
                        || !subject
                            .bytes()
                            .all(|b| b.is_ascii_digit() || b == b'S' || b == b'-')
                    {
                        return Err(Error::Configuration);
                    }
                    sddl.push_str(&format!("(A;;0x0012019b;;;{subject})"));
                }
            }
            sddl
        })
    } else {
        (
            format!(r"\\.\pipe\rss-mdm-execution-{subject}-{session}-v5"),
            format!("D:P(A;;GA;;;{subject})(A;;GA;;;SY)"),
        )
    })
}

fn peer_token(peer: &crate::host::Peer) -> Result<std::os::windows::io::OwnedHandle, Error> {
    use windows_sys::Win32::Security::*;
    if unsafe { ImpersonateNamedPipeClient(peer.native_handle() as HANDLE) } == 0 {
        return Err(Error::Denied);
    }
    struct Revert;
    impl Drop for Revert {
        fn drop(&mut self) {
            unsafe {
                RevertToSelf();
            }
        }
    }
    let _revert = Revert;
    let mut handle = null_mut();
    if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut handle) } == 0 {
        return Err(Error::Denied);
    }
    // SAFETY: this OS call returns a fresh owned handle; this is its only owner.
    unsafe { own(handle) }
}
pub(crate) fn session_binding(peer: &crate::host::Peer) -> Result<Id, Error> {
    crate::windows_identity::token_session_binding(raw(&peer_token(peer)?))
}
pub(crate) fn authenticate(
    peer: &crate::host::Peer,
    policy: &crate::host::PeerPolicy,
) -> Result<String, Error> {
    let token = peer_token(peer)?;
    let (subject, session) = crate::windows_identity::token_subject(raw(&token))?;
    if !policy.subjects.contains(&subject)
        || session != peer.session()
        || (policy.interactive && session == 0)
    {
        return Err(Error::Denied);
    }
    // SAFETY: this OS call returns a fresh owned handle; this is its only owner.
    let process = unsafe {
        own(OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            peer.pid(),
        ))
    }?;
    let mut path = vec![0u16; 32768];
    let mut size = path.len() as u32;
    if unsafe { QueryFullProcessImageNameW(raw(&process), 0, path.as_mut_ptr(), &mut size) } == 0 {
        return Err(Error::Denied);
    }
    let path = std::path::PathBuf::from(
        String::from_utf16(&path[..size as usize]).map_err(|_| Error::Denied)?,
    );
    if !policy
        .images
        .iter()
        .any(|image| image.verify(&path).is_ok())
    {
        return Err(Error::Denied);
    }
    let mut exit = 0;
    if unsafe { GetExitCodeProcess(raw(&process), &mut exit) } == 0 || exit != 259 {
        return Err(Error::Denied);
    }
    Ok(subject)
}
fn listener(name: &str, sddl: &str) -> Result<NamedPipeServer, Error> {
    let descriptor = security(sddl)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.as_ptr(),
        bInheritHandle: 0,
    };
    // SAFETY: this OS call returns a fresh owned handle; this is its only owner.
    let handle = unsafe {
        own(CreateNamedPipeW(
            wide(name).as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            65536,
            65536,
            5000,
            &attributes,
        ))
    }?;
    unsafe { NamedPipeServer::from_raw_handle(handle.into_raw_handle()) }
        .map_err(|_| Error::Unavailable)
}
struct Call {
    pid: u32,
    session: u32,
    connection: OwnedHandle,
    _process: OwnedHandle,
    bytes: Vec<u8>,
    reply: tokio::sync::oneshot::Sender<Vec<u8>>,
}
struct OwnerThread {
    sender: std::sync::mpsc::SyncSender<Call>,
    stopping: std::sync::Arc<AtomicBool>,
    progress: std::sync::Arc<Mutex<std::time::Instant>>,
    thread: Option<std::thread::JoinHandle<Result<(), Error>>>,
}
impl OwnerThread {
    fn new(mut handler: Box<dyn Handler>) -> Result<Self, Error> {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Call>(1);
        let stopping = std::sync::Arc::new(AtomicBool::new(false));
        let stopped = stopping.clone();
        let progress = std::sync::Arc::new(Mutex::new(std::time::Instant::now()));
        let clock = progress.clone();
        let thread = std::thread::Builder::new()
            .name("execution-owner".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut previous_error = None;
                    while !stopped.load(Ordering::Acquire) {
                        match receiver.recv_timeout(Duration::from_millis(100)) {
                            Ok(call) => {
                                // If the connection expired before dequeue, no mutation starts.
                                if !call.reply.is_closed() {
                                    let peer = Peer {
                                        pid: call.pid,
                                        session: call.session,
                                        uid: None,
                                        native: raw(&call.connection) as usize,
                                    };
                                    let _ = call.reply.send(host::dispatch(
                                        handler.as_mut(),
                                        &peer,
                                        &call.bytes,
                                    ));
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                        let error = handler.tick().err();
                        if error != previous_error {
                            if let Some(error) = error {
                                record(Stage::Reconcile, error)
                            }
                            previous_error = error;
                        }
                        *clock.lock().map_err(|_| Error::Unavailable)? = std::time::Instant::now();
                    }
                    Ok(())
                }))
                .unwrap_or(Err(Error::Unavailable));
                let stopped =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler.stop()))
                        .unwrap_or(Err(Error::Unavailable));
                if let Err(error) = stopped {
                    record(Stage::Shutdown, error);
                }
                outcome.and(stopped)
            })
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            sender,
            stopping,
            progress,
            thread: Some(thread),
        })
    }
    fn healthy(&self) -> bool {
        !self.thread.as_ref().is_none_or(|t| t.is_finished())
            && self
                .progress
                .lock()
                .is_ok_and(|p| p.elapsed() < Duration::from_secs(5))
    }
    fn finish(&mut self) -> Result<(), Error> {
        self.stopping.store(true, Ordering::Release);
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        let until = std::time::Instant::now() + Duration::from_secs(3);
        while !thread.is_finished() && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !thread.is_finished() {
            // Do not detach an owner which may still mutate/hold jobs. End this dedicated host;
            // the kernel closes all job handles and durable intents recover as Unknown.
            status(SERVICE_STOPPED, ERROR_TIMEOUT);
            std::process::exit(ERROR_TIMEOUT as i32);
        }
        thread.join().unwrap_or(Err(Error::Unavailable))
    }
}
impl Drop for OwnerThread {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
async fn call(pipe: &mut NamedPipeServer, owner: &OwnerThread) -> Result<(), Error> {
    let mut pid = 0;
    let mut session = 0;
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut pid) } == 0
        || unsafe { GetNamedPipeClientSessionId(pipe.as_raw_handle(), &mut session) } == 0
    {
        return Err(Error::Denied);
    }
    // SAFETY: this OS call returns a fresh owned handle; this is its only owner.
    let process = unsafe { own(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)) }?;
    let mut connection = null_mut();
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            pipe.as_raw_handle(),
            GetCurrentProcess(),
            &mut connection,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(Error::Unavailable);
    }
    // SAFETY: this OS call returns a fresh owned handle; this is its only owner.
    let connection = unsafe { own(connection) }?;
    let size = pipe.read_u32_le().await.map_err(|_| Error::Unavailable)? as usize;
    if size == 0 || size > host::FRAME_LIMIT {
        return Err(Error::InvalidInput);
    }
    let mut bytes = vec![0; size];
    pipe.read_exact(&mut bytes)
        .await
        .map_err(|_| Error::Unavailable)?;
    let (reply, received) = tokio::sync::oneshot::channel();
    owner
        .sender
        .try_send(Call {
            pid,
            session,
            connection,
            _process: process,
            bytes,
            reply,
        })
        .map_err(|_| Error::Capacity)?;
    let reply = received.await.map_err(|_| Error::Unavailable)?;
    if reply.len() > host::FRAME_LIMIT {
        return Err(Error::Unavailable);
    }
    pipe.write_u32_le(reply.len() as u32)
        .await
        .map_err(|_| Error::Unavailable)?;
    pipe.write_all(&reply)
        .await
        .map_err(|_| Error::Unavailable)?;
    Ok(())
}
// A timeout is a terminal host outcome, not permission to reuse a pipe whose old
// handler may still hold its duplicate. Normal completion proves the callback returned.
async fn exchange(
    pipe: &mut NamedPipeServer,
    owner: &OwnerThread,
    limit: Duration,
) -> Result<(), Error> {
    let request = call(pipe, owner);
    tokio::pin!(request);
    let deadline = tokio::time::sleep(limit);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            result=&mut request=>return result,
            _=&mut deadline=>return Err(Error::Unavailable),
            _=tokio::time::sleep(Duration::from_millis(100))=>{
                if stopping(){return Ok(())}
                if !owner.healthy(){return Err(Error::Unavailable)}
            }
        }
    }
}
fn drive(handler: Box<dyn Handler>, system: bool) -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::Unavailable)?;
    let mut owner = OwnerThread::new(handler)?;
    let outcome = runtime.block_on(async {
        let (name, sddl) = endpoint(system)?;
        let mut pipe = listener(&name, &sddl)?;
        if system {
            status(SERVICE_RUNNING, 0)
        }
        while !stopping() {
            if !owner.healthy() {
                return Err(Error::Unavailable);
            }
            tokio::select! {
                connected=pipe.connect()=>{
                    connected.map_err(|_|Error::Unavailable)?;
                    let result=exchange(&mut pipe,&owner,Duration::from_secs(5)).await;
                    pipe.disconnect().map_err(|_|Error::Unavailable)?;
                    // Sticky failure: never reconnect this instance after its callback timed out.
                    result?;
                },
                _=tokio::time::sleep(Duration::from_millis(100))=>{}
            }
        }
        Ok(())
    });
    if let Err(error) = outcome {
        record(Stage::Ingress, error);
    }
    outcome.and(owner.finish())
}
/// Read-only mechanism probe. No production admission context is constructed.
pub fn query_trusted(
    bytes: &[u8],
    system: bool,
    policy: &crate::host::PeerPolicy,
) -> Result<Vec<u8>, Error> {
    let (name, _) = endpoint(system)?;
    query_at(bytes, system.then_some(0), name, policy)
}
/// Address a helper by an OS-derived subject/session; verify its live token and installed image.
pub fn query_session(
    bytes: &[u8],
    subject: &str,
    session: u32,
    policy: &crate::host::PeerPolicy,
) -> Result<Vec<u8>, Error> {
    if token_identity()? != ("S-1-5-18".into(), 0)
        || session == 0
        || policy.subjects != [subject]
        || !subject.starts_with("S-1-")
        || !subject
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'S' || b == b'-')
    {
        return Err(Error::Denied);
    }
    query_at(
        bytes,
        Some(session),
        format!(r"\\.\pipe\rss-mdm-execution-{subject}-{session}-v5"),
        policy,
    )
}
fn query_at(
    bytes: &[u8],
    expected_session: Option<u32>,
    name: String,
    policy: &crate::host::PeerPolicy,
) -> Result<Vec<u8>, Error> {
    policy.validate()?;
    if bytes.len() > host::FRAME_LIMIT {
        return Err(Error::InvalidInput);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::Unavailable)?;
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut client = ClientOptions::new()
                .security_qos_flags(SECURITY_IDENTIFICATION)
                .open(name)
                .map_err(|_| Error::Unavailable)?;
            let pipe = client.as_raw_handle();
            let mut pid = 0;
            if unsafe { GetNamedPipeServerProcessId(pipe, &mut pid) } == 0 {
                return Err(Error::Denied);
            }
            // SAFETY: this OS call returns a fresh owned handle; this is its only owner.
            let process = unsafe { own(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)) }?;
            let mut handle = null_mut();
            if unsafe { OpenProcessToken(raw(&process), TOKEN_QUERY, &mut handle) } == 0 {
                return Err(Error::Denied);
            }
            // SAFETY: this OS call returns a fresh owned handle; this is its only owner.
            let token = unsafe { own(handle) }?;
            let (subject, session) = crate::windows_identity::token_subject(raw(&token))?;
            if !policy.subjects.contains(&subject)
                || expected_session.is_some_and(|expected| session != expected)
            {
                return Err(Error::Denied);
            }
            let mut path = vec![0u16; 32768];
            let mut len = path.len() as u32;
            if unsafe { QueryFullProcessImageNameW(raw(&process), 0, path.as_mut_ptr(), &mut len) }
                == 0
            {
                return Err(Error::Denied);
            }
            let path = std::path::PathBuf::from(
                String::from_utf16(&path[..len as usize]).map_err(|_| Error::Denied)?,
            );
            if !policy
                .images
                .iter()
                .any(|image| image.verify(&path).is_ok())
            {
                return Err(Error::Denied);
            }

            client
                .write_u32_le(bytes.len() as u32)
                .await
                .map_err(|_| Error::Unavailable)?;
            client
                .write_all(bytes)
                .await
                .map_err(|_| Error::Unavailable)?;
            let size = client.read_u32_le().await.map_err(|_| Error::Unavailable)? as usize;
            if size > host::FRAME_LIMIT {
                return Err(Error::InvalidInput);
            }
            let mut reply = vec![0; size];
            client
                .read_exact(&mut reply)
                .await
                .map_err(|_| Error::Unavailable)?;
            Ok(reply)
        })
        .await
        .map_err(|_| Error::Unavailable)?
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn named_pipe_frames_are_bounded_and_unbound_calls_are_rejected() {
        let (subject, _) = token_identity().unwrap();
        let name = format!(r"\\.\pipe\rss-execution-test-{}", nonce().unwrap());
        let mut pipe = listener(&name, &format!("D:P(A;;GA;;;{subject})")).unwrap();
        assert!(
            listener(&name, &format!("D:P(A;;GA;;;{subject})")).is_err(),
            "first instance forbids squatting/duplicate hosts"
        );
        let mut client = ClientOptions::new().open(&name).unwrap();
        pipe.connect().await.unwrap();
        let frame = br#"{"version":4,"request":{"method":"status","request":"probe"}}"#;
        client.write_u32_le(frame.len() as u32).await.unwrap();
        client.write_all(frame).await.unwrap();
        let mut owner = OwnerThread::new(Box::new(host::Unbound)).unwrap();
        call(&mut pipe, &owner).await.unwrap();
        let size = client.read_u32_le().await.unwrap();
        let mut reply = vec![0; size as usize];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply, br#"{"version":7,"reply":{"kind":"rejected"}}"#);
        client.write_u32_le(65537).await.unwrap();
        assert!(matches!(
            call(&mut pipe, &owner).await,
            Err(Error::InvalidInput)
        ));
        owner.finish().unwrap();
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::*;
    struct Slow(Duration);
    impl Handler for Slow {
        fn handle(&mut self, _: &Peer, _: host::Request) -> host::Reply {
            std::thread::sleep(self.0);
            host::Reply::Rejected
        }
        fn tick(&mut self) -> Result<(), Error> {
            Ok(())
        }
        fn stop(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }
    #[tokio::test]
    async fn synchronous_handler_cannot_block_connection_deadline() {
        let (subject, _) = token_identity().unwrap();
        let name = format!(r"\\.\pipe\rss-deadline-test-{}", nonce().unwrap());
        let mut pipe = listener(&name, &format!("D:P(A;;GA;;;{subject})")).unwrap();
        let mut client = ClientOptions::new().open(&name).unwrap();
        pipe.connect().await.unwrap();
        let bytes = br#"{"version":4,"request":{"method":"status","request":"probe"}}"#;
        client.write_u32_le(bytes.len() as u32).await.unwrap();
        client.write_all(bytes).await.unwrap();
        let mut owner = OwnerThread::new(Box::new(Slow(Duration::from_millis(300)))).unwrap();
        let before = std::time::Instant::now();
        assert!(exchange(&mut pipe, &owner, Duration::from_millis(30))
            .await
            .is_err());
        assert!(before.elapsed() < Duration::from_millis(200));
        pipe.disconnect().unwrap();
        owner.finish().unwrap();
    }
    #[tokio::test]
    async fn incomplete_frame_disconnect_is_a_terminal_exchange_error() {
        let (subject, _) = token_identity().unwrap();
        let name = format!(r"\\.\pipe\rss-disconnect-test-{}", nonce().unwrap());
        let mut pipe = listener(&name, &format!("D:P(A;;GA;;;{subject})")).unwrap();
        let mut client = ClientOptions::new().open(&name).unwrap();
        pipe.connect().await.unwrap();
        client.write_u32_le(128).await.unwrap();
        client.write_all(b"partial").await.unwrap();
        drop(client);
        let mut owner = OwnerThread::new(Box::new(host::Unbound)).unwrap();
        assert!(exchange(&mut pipe, &owner, Duration::from_secs(1))
            .await
            .is_err());
        owner.finish().unwrap();
    }
    #[test]
    #[ignore = "subprocess-only blocked shutdown fixture"]
    fn blocked_owner_fixture() {
        struct Blocked;
        impl Handler for Blocked {
            fn handle(&mut self, _: &Peer, _: host::Request) -> host::Reply {
                host::Reply::Rejected
            }
            fn tick(&mut self) -> Result<(), Error> {
                Ok(())
            }
            fn stop(&mut self) -> Result<(), Error> {
                std::thread::sleep(Duration::from_secs(30));
                Ok(())
            }
        }
        OwnerThread::new(Box::new(Blocked))
            .unwrap()
            .finish()
            .unwrap();
        panic!("blocked owner must terminate the dedicated host instead of detaching");
    }
    #[test]
    fn blocked_owner_ends_the_host_with_a_bounded_error() {
        let start = std::time::Instant::now();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows_service::deadline_tests::blocked_owner_fixture",
                "--ignored",
            ])
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(ERROR_TIMEOUT as i32));
        assert!(start.elapsed() < Duration::from_secs(6));
    }
}
