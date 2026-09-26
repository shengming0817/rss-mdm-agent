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
// ref: rust-lang/rust library/std/src/sys/fs/windows.rs@ac68faa20c58cbccd01ee7208bf3b6e93a7d7f96
// ref: microsoft/windows-rs crates/libs/sys/src/Windows/Win32/Security/Authorization/mod.rs@32c3144490c016fe496a0aed769bce60987a2e9d
pub fn protected(path: &Path) -> Result<(), Rejected> {
    open_protected(path, false).map(|_| ())
}

fn final_path(file: &std::fs::File) -> Result<PathBuf, Rejected> {
    use std::os::windows::ffi::OsStringExt;
    let mut buffer = vec![0u16; 32768];
    // SAFETY: the live file handle and writable buffer are valid for this call.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            0,
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(Rejected);
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..length],
    )))
}
fn same_path(actual: &Path, expected: &Path) -> bool {
    fn normalized(path: &Path) -> Option<String> {
        let value = path.to_str()?;
        Some(
            value
                .strip_prefix(r"\\?\")
                .unwrap_or(value)
                .trim_end_matches('\\')
                .to_ascii_lowercase(),
        )
    }
    match (normalized(actual), normalized(expected)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn handle_protected(file: &std::fs::File, product: bool) -> Result<(), Rejected> {
    use std::os::windows::fs::MetadataExt;
    let metadata = file.metadata().map_err(|_| Rejected)?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(Rejected);
    }
    // SAFETY: GetSecurityInfo returns a LocalFree-owned descriptor; ACEs/SIDs
    // remain borrowed from it until all checks below have completed.
    unsafe {
        let mut owner = null_mut();
        let mut acl = null_mut();
        let mut descriptor = null_mut();
        if GetSecurityInfo(
            file.as_raw_handle(),
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
        if owner.is_null()
            || acl.is_null()
            || IsValidSid(owner) == 0
            || IsValidAcl(acl) == 0
            || !super::policy_acl::trusted(&sid_string(owner)?, product)
        {
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
            } // A deny never widens the accepted policy.
            if header.AceType != 0 || (header.AceSize as usize) < size_of::<ACCESS_ALLOWED_ACE>() {
                return Err(Rejected);
            }
            let allowed = &*(ace as *const ACCESS_ALLOWED_ACE);
            let subject = (&allowed.SidStart as *const u32).cast_mut().cast();
            if IsValidSid(subject) == 0
                || !super::policy_acl::grant_allowed(
                    &sid_string(subject)?,
                    allowed.Mask,
                    header.AceFlags,
                    metadata.is_dir(),
                    product,
                )
            {
                return Err(Rejected);
            }
        }
    }
    Ok(())
}

/// Keep the complete chain open without write/delete sharing until the read ends.
/// Product ACLs reject writers; OS ancestors reject replacement of existing children.
pub(crate) fn open_protected(
    path: &Path,
    read: bool,
) -> Result<(std::fs::File, Vec<std::fs::File>, PathBuf), Rejected> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::{Component, Prefix};
    if !path.is_absolute()
        || !matches!(path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(Rejected);
    }
    let chain: Vec<_> = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    let product_root = chain
        .iter()
        .position(|p| p.file_name().is_some_and(|n| n == "RSS MDM Agent"))
        .ok_or(Rejected)?;
    let mut handles = Vec::with_capacity(chain.len());
    let mut actual = PathBuf::new();
    for (index, entry) in chain.iter().enumerate().rev() {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .access_mode(
                READ_CONTROL
                    | FILE_READ_ATTRIBUTES
                    | if index == 0 && read {
                        FILE_READ_DATA
                    } else {
                        0
                    },
            )
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(entry)
            .map_err(|_| Rejected)?;
        handle_protected(&file, index <= product_root)?;
        actual = final_path(&file)?;
        if !same_path(&actual, entry) {
            return Err(Rejected);
        }
        let metadata = file.metadata().map_err(|_| Rejected)?;
        if (index == 0 && !metadata.is_file()) || (index != 0 && !metadata.is_dir()) {
            return Err(Rejected);
        }
        handles.push(file);
    }
    let file = handles.pop().ok_or(Rejected)?;
    Ok((file, handles, actual))
}

fn image_peer(pipe: HANDLE, client: bool, policy: &Policy) -> Result<Handle, Rejected> {
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
        Ok(process)
    }
}

fn peer(pipe: HANDLE, client: bool, policy: &Policy) -> Result<Handle, Rejected> {
    let process = image_peer(pipe, client, policy)?;
    unsafe {
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
async fn listen(policy: &Policy, status_handle: SERVICE_STATUS_HANDLE) -> Result<(), Rejected> {
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
    report(status_handle, SERVICE_RUNNING, 0)?;
    let mut peers = tokio::task::JoinSet::new();
    loop {
        if STOP.load(Ordering::Acquire) {
            peers.abort_all();
            return Ok(());
        }
        tokio::select! {
            connected = pending.connect() => {
                connected.map_err(|_| Rejected)?;
                let mut pipe = std::mem::replace(&mut pending, make(false)?);
                // Reject a different image before it can occupy a waiting slot.
                // This uses kernel process identity and immutable installation bytes,
                // not a caller-provided preamble. Full token checks still follow.
                let Ok(candidate) = image_peer(pipe.as_raw_handle(), true, policy) else { continue; };
                if peers.len() >= 64 { continue; }
                let policy = policy.clone();
                peers.spawn_local(async move {
                    let _candidate = candidate;
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
        let _ = report(
            STATUS_HANDLE.load(Ordering::Acquire),
            SERVICE_STOP_PENDING,
            0,
        );
    }
    NO_ERROR
}
unsafe extern "system" fn service_main(_: u32, _: *mut *mut u16) {
    let name = wide(NAME);
    let handle = unsafe { RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control), null_mut()) };
    if handle.is_null() {
        return;
    }
    STATUS_HANDLE.store(handle, Ordering::Release);
    if report(handle, SERVICE_START_PENDING, 0).is_err() {
        return;
    }
    let result = runtime().and_then(|runtime| {
        tokio::task::LocalSet::new()
            .block_on(&runtime, listen(POLICY.get().ok_or(Rejected)?, handle))
    });
    let _ = report(
        handle,
        SERVICE_STOPPED,
        if result.is_err() {
            ERROR_SERVICE_SPECIFIC_ERROR
        } else {
            0
        },
    );
    STATUS_HANDLE.store(null_mut(), Ordering::Release);
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

static STATUS_HANDLE: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(null_mut());
fn report(handle: SERVICE_STATUS_HANDLE, state: u32, error: u32) -> Result<(), Rejected> {
    let pending = state == SERVICE_START_PENDING || state == SERVICE_STOP_PENDING;
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING {
            SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
        } else {
            0
        },
        dwWin32ExitCode: error,
        dwServiceSpecificExitCode: u32::from(error != 0),
        dwCheckPoint: u32::from(pending),
        dwWaitHint: if pending { 5000 } else { 0 },
    };
    if handle.is_null() || unsafe { SetServiceStatus(handle, &status) } == 0 {
        return Err(Rejected);
    }
    Ok(())
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    #[test]
    fn windows_rights_match_the_installation_policy() {
        let user = "S-1-5-21-1-2-3-1001";
        for right in [
            DELETE,
            FILE_DELETE_CHILD,
            WRITE_DAC,
            WRITE_OWNER,
            GENERIC_ALL,
        ] {
            assert!(!crate::policy_acl::grant_allowed(
                user, right, 0, true, false
            ));
        }
        assert!(crate::policy_acl::grant_allowed(
            user,
            FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_WRITE_EA | FILE_WRITE_ATTRIBUTES,
            0,
            true,
            false
        ));
        assert!(!crate::policy_acl::grant_allowed(
            user,
            FILE_WRITE_DATA,
            (INHERIT_ONLY_ACE | CONTAINER_INHERIT_ACE) as u8,
            true,
            true
        ));
    }
    #[test]
    fn a_world_writable_file_is_not_an_installation_policy() {
        let root =
            std::env::temp_dir().join(format!("rss-policy-untrusted-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("policy.json");
        use std::os::windows::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .access_mode(READ_CONTROL | WRITE_DAC | FILE_WRITE_DATA)
            .open(&path)
            .unwrap();
        // SAFETY: all pointers below refer to owned descriptors or live handles.
        unsafe {
            let mut descriptor = null_mut();
            assert_ne!(
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    wide("D:P(A;;FA;;;WD)").as_ptr(),
                    1,
                    &mut descriptor,
                    null_mut()
                ),
                0
            );
            let _descriptor = Local(descriptor);
            let mut present = 0;
            let mut defaulted = 0;
            let mut acl = null_mut();
            assert_ne!(
                GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted),
                0
            );
            assert_eq!(
                SetSecurityInfo(
                    file.as_raw_handle(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    acl,
                    null_mut()
                ),
                0
            );
        }
        assert!(handle_protected(&file, true).is_err());
        drop(file);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn final_paths_compare_verbatim_prefixes_without_accepting_another_chain() {
        assert!(same_path(
            Path::new(r"\\?\C:\ProgramData\RSS MDM Agent\service\policy.json"),
            Path::new(r"C:\ProgramData\RSS MDM Agent\service\policy.json")
        ));
        assert!(!same_path(
            Path::new(r"C:\attacker\policy.json"),
            Path::new(r"C:\ProgramData\RSS MDM Agent\service\policy.json")
        ));
    }
}
