//! SCM and per-session helper transport. The transport supplies native facts, never authority.
use super::*;
use crate::host::{self, Handler, Peer};
use std::os::windows::io::IntoRawHandle;
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
use windows_sys::Win32::{
    Storage::FileSystem::*,
    System::{Console::*, Pipes::*, Services::*},
};
const NAME: &str = "RssExecution";
static STOP: OnceLock<&'static AtomicBool> = OnceLock::new();
static HANDLER: Mutex<Option<Box<dyn Handler>>> = Mutex::new(None);
static STATUS: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static FAILED: AtomicBool = AtomicBool::new(false);
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
    status(
        SERVICE_STOPPED,
        if failed {
            ERROR_SERVICE_SPECIFIC_ERROR
        } else {
            0
        },
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
        (
            r"\\.\pipe\rss-mdm-execution-system-v2".into(),
            "D:P(A;;GA;;;SY)(A;;GA;;;BA)".into(),
        )
    } else {
        (
            format!(r"\\.\pipe\rss-mdm-execution-{subject}-{session}-v2"),
            format!("D:P(A;;GA;;;{subject})(A;;GA;;;SY)"),
        )
    })
}
fn listener(name: &str, sddl: &str) -> Result<NamedPipeServer, Error> {
    let descriptor = security(sddl)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let handle = own(unsafe {
        CreateNamedPipeW(
            wide(name).as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            65536,
            65536,
            5000,
            &attributes,
        )
    })?;
    unsafe { NamedPipeServer::from_raw_handle(handle.into_raw_handle()) }
        .map_err(|_| Error::Unavailable)
}
async fn call(pipe: &mut NamedPipeServer, handler: &mut dyn Handler) -> Result<(), Error> {
    let mut pid = 0;
    let mut session = 0;
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut pid) } == 0
        || unsafe { GetNamedPipeClientSessionId(pipe.as_raw_handle(), &mut session) } == 0
    {
        return Err(Error::Denied);
    }
    // Keep the peer process alive as an identity object throughout the callback; no DTO PID trust.
    let _process = own(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })?;
    let peer = Peer {
        pid,
        uid: None,
        session,
        native: pipe.as_raw_handle() as usize,
    };
    let size = pipe.read_u32_le().await.map_err(|_| Error::Unavailable)? as usize;
    if size == 0 || size > 65536 {
        return Err(Error::InvalidInput);
    }
    let mut bytes = vec![0; size];
    pipe.read_exact(&mut bytes)
        .await
        .map_err(|_| Error::Unavailable)?;
    let reply = host::dispatch(handler, &peer, &bytes);
    if reply.len() > 65536 {
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
fn drive(mut handler: Box<dyn Handler>, system: bool) -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::Unavailable)?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.block_on(async {
        let(name,sddl)=endpoint(system)?;
        let mut pipe=listener(&name,&sddl)?;
        if system{status(SERVICE_RUNNING,0)}
        let mut previous_error=None;
        while !stopping(){
            tokio::select!{
                connected=pipe.connect()=>{
                    connected.map_err(|_|Error::Unavailable)?;
                    let mut request=Box::pin(call(&mut pipe,handler.as_mut()));
                    // Bounded connection lifetime; cancellation closes a pending read/write.
                    let deadline=tokio::time::sleep(Duration::from_secs(5));tokio::pin!(deadline);
                    loop{tokio::select!{_=&mut request=>break,_=&mut deadline=>break,_=tokio::time::sleep(Duration::from_millis(100))=>{if stopping(){break}}}}
                    drop(request);
                    pipe.disconnect().map_err(|_|Error::Unavailable)?;
                },
                _=tokio::time::sleep(Duration::from_millis(100))=>{}
            }
            let error=handler.tick().err().map(|e|e.to_string());
            if error!=previous_error{if let Some(ref error)=error{eprintln!("execution reconcile: {error}")}previous_error=error;}
        }
        Ok(())
    })
    }));
    // A panicking handler fails the host closed. Never continue accepting requests after panic.
    let result = match outcome {
        Ok(value) => value,
        Err(_) => {
            stop();
            Err(Error::Unavailable)
        }
    };
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler.stop()))
        .unwrap_or(Err(Error::Unavailable));
    result.and(stopped)
}
/// Read-only mechanism probe. No production admission context is constructed.
pub fn query(bytes: &[u8], system: bool) -> Result<Vec<u8>, Error> {
    if bytes.len() > 65536 {
        return Err(Error::InvalidInput);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Error::Unavailable)?;
    runtime.block_on(async {
        let (name, _) = endpoint(system)?;
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut client = ClientOptions::new()
                .open(name)
                .map_err(|_| Error::Unavailable)?;
            client
                .write_u32_le(bytes.len() as u32)
                .await
                .map_err(|_| Error::Unavailable)?;
            client
                .write_all(bytes)
                .await
                .map_err(|_| Error::Unavailable)?;
            let size = client.read_u32_le().await.map_err(|_| Error::Unavailable)? as usize;
            if size > 65536 {
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
        let frame = br#"{"version":2,"request":{"method":"status","request":"probe"}}"#;
        client.write_u32_le(frame.len() as u32).await.unwrap();
        client.write_all(frame).await.unwrap();
        call(&mut pipe, &mut host::Unbound).await.unwrap();
        let size = client.read_u32_le().await.unwrap();
        let mut reply = vec![0; size as usize];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply, br#"{"kind":"rejected"}"#);
        client.write_u32_le(65537).await.unwrap();
        assert!(matches!(
            call(&mut pipe, &mut host::Unbound).await,
            Err(Error::InvalidInput)
        ));
    }
}
