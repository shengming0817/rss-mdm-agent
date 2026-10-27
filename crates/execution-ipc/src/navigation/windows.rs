// ref: Microsoft named-pipe-security-and-access-rights (logon SID isolation).
use super::*;
use crate::windows_identity as native;
use std::{
    os::windows::io::{AsRawHandle, OwnedHandle},
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, ServerOptions},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    System::{Pipes::*, Threading::*},
};

/// Owns this real Windows logon incarnation's navigation endpoint.
pub struct NavigationInstance {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    _mutex: OwnedHandle,
}
impl Drop for NavigationInstance {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn peer(pid: u32) -> Result<(), Error> {
    let process = unsafe { native::own(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)) }?;
    let mut token = null_mut();
    if unsafe { OpenProcessToken(native::raw(&process), TOKEN_QUERY, &mut token) } == 0 {
        return Err(Error::Denied);
    }
    let token = unsafe { native::own(token) }?;
    if native::token_subject(native::raw(&token))? != native::token_identity()?
        || native::token_session_binding(native::raw(&token))?
            != crate::host::current_session_binding()?
    {
        return Err(Error::Denied);
    }
    Ok(())
}
fn descriptor() -> Result<native::Local, Error> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(Error::Denied);
    }
    let token = unsafe { native::own(token) }?;
    let mut bytes = vec![0usize; 256];
    let mut size = 0;
    if unsafe {
        GetTokenInformation(
            native::raw(&token),
            TokenLogonSid,
            bytes.as_mut_ptr().cast(),
            (bytes.len() * std::mem::size_of::<usize>()) as u32,
            &mut size,
        )
    } == 0
    {
        return Err(Error::Denied);
    }
    let groups = unsafe { &*(bytes.as_ptr() as *const TOKEN_GROUPS) };
    if groups.GroupCount != 1 {
        return Err(Error::Denied);
    }
    let sid = unsafe { native::sid(groups.Groups[0].Sid) }?;
    native::security(&format!("D:P(A;;GA;;;{sid})"))
}
impl NavigationInstance {
    /// Elect this logon's receiver or forward one strictly parsed navigation and await ACK.
    pub fn claim(
        initial: Option<&str>,
        receive: impl Fn(String) -> bool + Send + 'static,
    ) -> Result<Option<Self>, Error> {
        if let Some(uri) = initial {
            EntryIntent::parse(uri).map_err(|_| Error::InvalidInput)?;
        }
        let name = name()?;
        let mutex_name = native::wide(format!("Local\\RSSMDM.Nav.{name}"));
        let security = descriptor()?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security.as_ptr(),
            bInheritHandle: 0,
        };
        let handle = unsafe { CreateMutexW(&attributes, 0, mutex_name.as_ptr()) };
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let mutex = unsafe { native::own(handle) }?;
        let pipe_name = format!("\\\\.\\pipe\\RSSMDM.Nav.{name}");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?;
        if existed {
            let body = initial.unwrap_or("").as_bytes();
            runtime.block_on(async {
                let started = Instant::now();
                let mut client = loop {
                    match ClientOptions::new().open(&pipe_name) {
                        Ok(v) => break v,
                        Err(_) if started.elapsed() < Duration::from_secs(2) => {
                            tokio::time::sleep(Duration::from_millis(20)).await
                        }
                        Err(_) => return Err(Error::Unavailable),
                    }
                };
                let mut pid = 0;
                if unsafe { GetNamedPipeServerProcessId(client.as_raw_handle(), &mut pid) } == 0 {
                    return Err(Error::Denied);
                }
                peer(pid)?;
                tokio::time::timeout(Duration::from_secs(2), async {
                    client.write_all(&(body.len() as u32).to_be_bytes()).await?;
                    client.write_all(body).await?;
                    let mut ack = [0];
                    client.read_exact(&mut ack).await?;
                    if ack != [1] {
                        return Err(std::io::Error::other("navigation queue rejected"));
                    }
                    Ok::<_, std::io::Error>(())
                })
                .await
                .map_err(|_| Error::Unavailable)?
                .map_err(|_| Error::Unavailable)
            })?;
            return Ok(None);
        }
        let server = {
            let _guard = runtime.enter();
            unsafe {
                ServerOptions::new()
                    .first_pipe_instance(true)
                    .reject_remote_clients(true)
                    .create_with_security_attributes_raw(
                        &pipe_name,
                        (&attributes as *const SECURITY_ATTRIBUTES)
                            .cast_mut()
                            .cast(),
                    )
            }
            .map_err(|_| Error::Unavailable)?
        };
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let thread = std::thread::spawn(move || {
            runtime.block_on(async move {
                let mut server = server;
                while !cancelled.load(Ordering::Acquire) {
                    match tokio::time::timeout(Duration::from_millis(50), server.connect()).await {
                        Err(_) => continue,
                        Ok(Err(_)) => break,
                        Ok(Ok(())) => {}
                    }
                    let result = tokio::time::timeout(Duration::from_secs(2), async {
                        let mut pid = 0;
                        if unsafe { GetNamedPipeClientProcessId(server.as_raw_handle(), &mut pid) }
                            == 0
                        {
                            return Err(Error::Denied);
                        }
                        peer(pid)?;
                        let mut header = [0; 4];
                        server
                            .read_exact(&mut header)
                            .await
                            .map_err(|_| Error::Unavailable)?;
                        let len = u32::from_be_bytes(header) as usize;
                        if len > execution_contract::ENTRY_URI_BYTES {
                            return Err(Error::InvalidInput);
                        }
                        let mut body = vec![0; len];
                        server
                            .read_exact(&mut body)
                            .await
                            .map_err(|_| Error::Unavailable)?;
                        let uri = String::from_utf8(body).map_err(|_| Error::InvalidInput)?;
                        if !uri.is_empty() {
                            EntryIntent::parse(&uri).map_err(|_| Error::InvalidInput)?;
                        }
                        Ok(receive(uri))
                    })
                    .await;
                    let ack = u8::from(matches!(result, Ok(Ok(true))));
                    let _ = tokio::time::timeout(Duration::from_secs(2), server.write_all(&[ack]))
                        .await;
                    let _ = server.disconnect();
                }
            })
        });
        Ok(Some(Self {
            stop,
            thread: Some(thread),
            _mutex: mutex,
        }))
    }
}
