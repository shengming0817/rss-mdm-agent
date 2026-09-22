//! Local-only named pipe with explicit ACL and bidirectional kernel peer facts.
use super::*;
use std::{
    ffi::c_void,
    os::windows::{ffi::OsStrExt, io::AsRawHandle},
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, ServerOptions},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::{Com::CoTaskMemFree, Pipes::*, Services::*, Threading::*},
    UI::Shell::{FOLDERID_ProgramData, SHGetKnownFolderPath},
};
const PIPE: &str = r"\\.\pipe\rss-mdm-agent-status-v1";
const NAME: &str = "RssMdmAgentStatus";
static POLICY: OnceLock<Policy> = OnceLock::new();
static STOP: AtomicBool = AtomicBool::new(false);

fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn sid_string(sid: PSID) -> Result<String, Rejected> {
    unsafe {
        let mut value = null_mut();
        if ConvertSidToStringSidW(sid, &mut value) == 0 {
            return Err(Rejected);
        }
        let _value = Local(value.cast());
        let mut len = 0;
        while *value.add(len) != 0 && len < 256 {
            len += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(value, len)).map_err(|_| Rejected)
    }
}
pub fn policy_path() -> Result<PathBuf, Rejected> {
    unsafe {
        let mut value = null_mut();
        if SHGetKnownFolderPath(&FOLDERID_ProgramData, 0, null_mut(), &mut value) < 0 {
            return Err(Rejected);
        }
        let mut len = 0;
        while *value.add(len) != 0 && len < 32768 {
            len += 1;
        }
        let result =
            String::from_utf16(std::slice::from_raw_parts(value, len)).map_err(|_| Rejected);
        CoTaskMemFree(value.cast());
        Ok(PathBuf::from(result?)
            .join("RSS MDM Agent")
            .join("service")
            .join("policy.json"))
    }
}
pub fn protected(path: &Path) -> Result<(), Rejected> {
    // Administrator-created install root and descendants are immutable to users.
    // Ancestors are checked for reparse points; common OS roots need not use our ACL.
    let mut in_product = true;
    for entry in path.ancestors() {
        if entry.as_os_str().is_empty() {
            continue;
        }
        let name = wide(entry);
        unsafe {
            let attrs = GetFileAttributesW(name.as_ptr());
            if attrs == INVALID_FILE_ATTRIBUTES || attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(Rejected);
            }
            if in_product {
                let mut owner = null_mut();
                let mut acl = null_mut();
                let mut descriptor = null_mut();
                if GetNamedSecurityInfoW(
                    name.as_ptr(),
                    SE_FILE_OBJECT,
                    OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                    &mut owner,
                    null_mut(),
                    &mut acl,
                    null_mut(),
                    &mut descriptor,
                ) != 0
                {
                    return Err(Rejected);
                }
                let _descriptor = Local(descriptor);
                let subject = sid_string(owner)?;
                if !matches!(subject.as_str(), "S-1-5-18" | "S-1-5-32-544") || acl.is_null() {
                    return Err(Rejected);
                }
                for index in 0..(*acl).AceCount as u32 {
                    let mut ace = null_mut();
                    if GetAce(acl, index, &mut ace) == 0 {
                        return Err(Rejected);
                    }
                    let header = &*(ace as *const ACE_HEADER);
                    if header.AceType == 1 {
                        continue;
                    }
                    if header.AceType != 0 {
                        return Err(Rejected);
                    }
                    let allowed = &*(ace as *const ACCESS_ALLOWED_ACE);
                    let writer = allowed.Mask
                        & (FILE_WRITE_DATA
                            | FILE_APPEND_DATA
                            | FILE_WRITE_EA
                            | FILE_WRITE_ATTRIBUTES
                            | DELETE
                            | WRITE_DAC
                            | WRITE_OWNER
                            | GENERIC_ALL
                            | GENERIC_WRITE)
                        != 0;
                    if writer {
                        let sid = sid_string((&allowed.SidStart as *const u32).cast_mut().cast())?;
                        if !matches!(sid.as_str(), "S-1-5-18" | "S-1-5-32-544") {
                            return Err(Rejected);
                        }
                    }
                }
            }
        }
        if entry
            .file_name()
            .is_some_and(|name| name == "RSS MDM Agent")
        {
            in_product = false;
        }
    }
    if in_product {
        return Err(Rejected);
    }
    Ok(())
}

fn peer(pipe: HANDLE, client: bool, policy: &Policy) -> Result<Handle, Rejected> {
    unsafe {
        let mut pid = 0;
        let result = if client {
            GetNamedPipeClientProcessId(pipe, &mut pid)
        } else {
            GetNamedPipeServerProcessId(pipe, &mut pid)
        };
        if result == 0 {
            return Err(Rejected);
        }
        let process = Handle(OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE,
            0,
            pid,
        ));
        if process.0.is_null() || WaitForSingleObject(process.0, 0) != WAIT_TIMEOUT {
            return Err(Rejected);
        }
        let mut token = null_mut();
        if OpenProcessToken(process.0, TOKEN_QUERY, &mut token) == 0 {
            return Err(Rejected);
        }
        let token = Handle(token);
        let mut bytes = vec![0usize; 512];
        let mut needed = 0;
        if GetTokenInformation(
            token.0,
            TokenUser,
            bytes.as_mut_ptr().cast(),
            (bytes.len() * size_of::<usize>()) as u32,
            &mut needed,
        ) == 0
        {
            return Err(Rejected);
        }
        let user = &*(bytes.as_ptr() as *const TOKEN_USER);
        let sid = sid_string(user.User.Sid)?;
        let mut session = 0u32;
        if GetTokenInformation(
            token.0,
            TokenSessionId,
            (&mut session as *mut u32).cast(),
            4,
            &mut needed,
        ) == 0
        {
            return Err(Rejected);
        }
        if client {
            let mut pipe_session = 0;
            if GetNamedPipeClientSessionId(pipe, &mut pipe_session) == 0
                || session == 0
                || session != pipe_session
                || !policy.allowed_users.contains(&sid)
            {
                return Err(Rejected);
            }
        } else if sid != policy.service_subject || session != 0 {
            return Err(Rejected);
        }
        let mut image = vec![0u16; 32768];
        let mut len = image.len() as u32;
        if QueryFullProcessImageNameW(process.0, 0, image.as_mut_ptr(), &mut len) == 0 {
            return Err(Rejected);
        }
        let path = PathBuf::from(String::from_utf16(&image[..len as usize]).map_err(|_| Rejected)?);
        let artifact = if client {
            &policy.client
        } else {
            &policy.service
        };
        artifact.verify(&path)?;
        // Retain the process handle through the complete exchange, never trust a
        // later process that happens to reuse this PID.
        Ok(process)
    }
}
async fn read(stream: &mut (impl tokio::io::AsyncRead + Unpin)) -> Result<Vec<u8>, Rejected> {
    let size = stream.read_u32().await.map_err(|_| Rejected)? as usize;
    if size == 0 || size > MAX_FRAME {
        return Err(Rejected);
    }
    let mut data = vec![0; size];
    stream.read_exact(&mut data).await.map_err(|_| Rejected)?;
    Ok(data)
}
async fn write(
    stream: &mut (impl tokio::io::AsyncWrite + Unpin),
    data: &[u8],
) -> Result<(), Rejected> {
    stream
        .write_u32(data.len() as u32)
        .await
        .map_err(|_| Rejected)?;
    stream.write_all(data).await.map_err(|_| Rejected)
}
fn runtime() -> Result<tokio::runtime::Runtime, Rejected> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| Rejected)
}
pub fn query(policy: &Policy) -> Result<Status, Rejected> {
    runtime()?.block_on(async {
        tokio::time::timeout(DEADLINE, async {
            let mut pipe = ClientOptions::new()
                .security_qos_flags(SECURITY_IMPERSONATION)
                .open(PIPE)
                .map_err(|_| Rejected)?;
            let owner = peer(pipe.as_raw_handle(), false, policy)?;
            pipe.write_all(b"RSSSTATUS1").await.map_err(|_| Rejected)?;
            let greeting = read(&mut pipe).await?;
            let (challenge, request) = client_request(&greeting)?;
            write(&mut pipe, &request).await?;
            let response = client_reply(&read(&mut pipe).await?, &challenge)?;
            if response != policy.status()
                || unsafe { WaitForSingleObject(owner.0, 0) } != WAIT_TIMEOUT
            {
                return Err(Rejected);
            }
            Ok(response)
        })
        .await
        .map_err(|_| Rejected)?
    })
}
async fn listen(policy: &Policy) -> Result<(), Rejected> {
    let mut sddl = String::from("D:P(A;;FA;;;SY)(A;;FA;;;BA)");
    for user in &policy.allowed_users {
        // SID text only, no arbitrary SDDL from configuration.
        if !user.starts_with("S-1-")
            || !user
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'S' || b == b'-')
        {
            return Err(Rejected);
        }
        sddl.push_str(&format!("(A;;0x0012019b;;;{user})"));
    }
    if !policy.service_subject.starts_with("S-1-")
        || !policy
            .service_subject
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'S' || b == b'-')
    {
        return Err(Rejected);
    }
    sddl.push_str(&format!("(A;;FA;;;{})", policy.service_subject));
    let mut descriptor = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(sddl).as_ptr(),
            1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(Rejected);
    }
    let _descriptor = Local(descriptor);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let make = |first| unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                PIPE,
                (&attributes as *const SECURITY_ATTRIBUTES)
                    .cast_mut()
                    .cast(),
            )
            .map_err(|_| Rejected)
    };
    let mut pending = make(true)?;
    let mut peers = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            connected = pending.connect() => {
                connected.map_err(|_| Rejected)?;
                let mut pipe = std::mem::replace(&mut pending, make(false)?);
                if peers.len() >= 64 { continue; }
                let policy = policy.clone();
                peers.spawn_local(async move {
                    let _ = tokio::time::timeout(DEADLINE, async {
                        let mut preamble = [0u8; 10];
                        pipe.read_exact(&mut preamble).await.map_err(|_| Rejected)?;
                        if &preamble != b"RSSSTATUS1" { return Err(Rejected); }
                        let owner = {
                            if unsafe { ImpersonateNamedPipeClient(pipe.as_raw_handle()) } == 0 { return Err(Rejected); }
                            let owner = peer(pipe.as_raw_handle(), true, &policy);
                            // No async suspension or business work under the client's token.
                            if unsafe { RevertToSelf() } == 0 { std::process::abort(); }
                            owner?
                        };
                        let mut connection = Connection::new(random_challenge()?, Instant::now(), policy.status());
                        write(&mut pipe, &connection.greeting()?).await?;
                        let request = read(&mut pipe).await?;
                        if unsafe { WaitForSingleObject(owner.0, 0) } != WAIT_TIMEOUT { return Err(Rejected); }
                        let response = connection.accept(&request, Instant::now())?;
                        write(&mut pipe, &response).await?;
                        Ok::<(), Rejected>(())
                    }).await;
                    // Drop disconnects this one-use instance, including buffered replays.
                });
            }
            _ = peers.join_next(), if !peers.is_empty() => {},
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if STOP.load(Ordering::Acquire) { peers.abort_all(); return Ok(()); }
            }
        }
    }
}
unsafe extern "system" fn control(code: u32, _: u32, _: *mut c_void, _: *mut c_void) -> u32 {
    if code == SERVICE_CONTROL_STOP || code == SERVICE_CONTROL_SHUTDOWN {
        STOP.store(true, Ordering::Release);
    }
    NO_ERROR
}
unsafe extern "system" fn service_main(_: u32, _: *mut *mut u16) {
    let name = wide(NAME);
    let handle = unsafe { RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control), null_mut()) };
    if handle.is_null() {
        return;
    }
    let mut status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_RUNNING,
        dwControlsAccepted: SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN,
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 0,
    };
    unsafe {
        SetServiceStatus(handle, &status);
    }
    let result = runtime().and_then(|runtime| {
        tokio::task::LocalSet::new().block_on(&runtime, listen(POLICY.get().ok_or(Rejected)?))
    });
    status.dwCurrentState = SERVICE_STOPPED;
    status.dwControlsAccepted = 0;
    if result.is_err() {
        status.dwWin32ExitCode = ERROR_SERVICE_SPECIFIC_ERROR;
        status.dwServiceSpecificExitCode = 1;
    }
    unsafe {
        SetServiceStatus(handle, &status);
    }
}
pub fn run(policy: Policy) -> Result<(), Rejected> {
    POLICY.set(policy).map_err(|_| Rejected)?;
    let mut name = wide(NAME);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: null_mut(),
            lpServiceProc: None,
        },
    ];
    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
        return Err(Rejected);
    }
    Ok(())
}
