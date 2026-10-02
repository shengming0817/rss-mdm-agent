//! Windows-only OS mechanisms. No production registration, approval or credential bootstrap.
mod files;
mod process;
pub mod service;
use execution_app::Error;
use execution_contract::*;
pub(crate) use files::*;
pub(crate) use process::{spawn, Owner};
use std::{
    ffi::{c_void, OsStr},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, Cryptography::*, *},
    System::{RemoteDesktop::*, Threading::*},
};

pub(crate) fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
pub(crate) fn own(handle: HANDLE) -> Result<OwnedHandle, Error> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        Err(Error::Unavailable)
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
}
pub(crate) fn raw(handle: &OwnedHandle) -> HANDLE {
    handle.as_raw_handle()
}
pub(crate) struct Local(pub *mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
pub(crate) fn sid(sid: PSID) -> Result<String, Error> {
    let mut text = null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(Error::Unavailable);
    }
    let _owner = Local(text.cast());
    let mut len = 0;
    while len < 256 && unsafe { *text.add(len) } != 0 {
        len += 1
    }
    if len == 256 {
        return Err(Error::InvalidInput);
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(text, len) })
        .map_err(|_| Error::InvalidInput)
}
pub(crate) fn token_identity() -> Result<(String, u32), Error> {
    let mut handle = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0 {
        return Err(Error::Unbound);
    }
    let token = own(handle)?;
    token_subject(raw(&token))
}
pub(crate) fn token_subject(token: HANDLE) -> Result<(String, u32), Error> {
    let mut buffer = vec![0usize; 128];
    let mut size = 0;
    if unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * size_of::<usize>()) as u32,
            &mut size,
        )
    } == 0
    {
        return Err(Error::Unbound);
    }
    let subject = sid(unsafe { (*(buffer.as_ptr() as *const TOKEN_USER)).User.Sid })?;
    let mut session = 0u32;
    if unsafe {
        GetTokenInformation(
            token,
            TokenSessionId,
            (&mut session as *mut u32).cast(),
            4,
            &mut size,
        )
    } == 0
    {
        return Err(Error::Unbound);
    }
    Ok((subject, session))
}
pub(crate) fn current_session_binding() -> Result<Id, Error> {
    let mut handle = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0 {
        return Err(Error::Unbound);
    }
    let token = own(handle)?;
    token_session_binding(raw(&token))
}
pub(crate) fn token_session_binding(token: HANDLE) -> Result<Id, Error> {
    let mut statistics: TOKEN_STATISTICS = unsafe { std::mem::zeroed() };
    let mut size = 0u32;
    if unsafe {
        GetTokenInformation(
            token,
            TokenStatistics,
            (&mut statistics as *mut TOKEN_STATISTICS).cast(),
            size_of::<TOKEN_STATISTICS>() as u32,
            &mut size,
        )
    } == 0
    {
        return Err(Error::Unbound);
    }
    let (_, session) = token_subject(token)?;
    Id::new(format!(
        "{}/{}/{:x}-{:x}",
        boot_generation()?.as_str(),
        session,
        statistics.AuthenticationId.HighPart,
        statistics.AuthenticationId.LowPart
    ))
    .map_err(|_| Error::Unbound)
}
pub(crate) fn security(text: &str) -> Result<Local, Error> {
    let mut value = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(text).as_ptr(),
            1,
            &mut value,
            null_mut(),
        )
    } == 0
    {
        return Err(Error::Unavailable);
    }
    Ok(Local(value))
}
pub(crate) fn nonce() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    if unsafe {
        BCryptGenRandom(
            null_mut(),
            bytes.as_mut_ptr(),
            16,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } != 0
    {
        return Err(Error::Unavailable);
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub(crate) fn active_user_session() -> Result<(String, u32), Error> {
    let mut sessions = null_mut();
    let mut count = 0;
    if unsafe { WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut sessions, &mut count) }
        == 0
    {
        return Err(Error::Unavailable);
    }
    if sessions.is_null() || count > 4096 {
        if !sessions.is_null() {
            unsafe {
                WTSFreeMemory(sessions.cast());
            }
        }
        return Err(Error::Unavailable);
    }
    let selected = unsafe { std::slice::from_raw_parts(sessions, count as usize) }
        .iter()
        .filter(|entry| entry.SessionId != 0 && entry.State == WTSActive)
        .map(|entry| entry.SessionId)
        .collect::<Vec<_>>();
    unsafe {
        WTSFreeMemory(sessions.cast());
    }
    if selected.len() != 1 {
        return Err(Error::Unbound);
    }
    let mut token = null_mut();
    if unsafe { WTSQueryUserToken(selected[0], &mut token) } == 0 {
        return Err(Error::Unavailable);
    }
    let token = own(token)?;
    let identity = token_subject(raw(&token))?;
    if identity.1 != selected[0] || identity.0 == "S-1-5-18" {
        return Err(Error::Denied);
    }
    Ok(identity)
}
pub(crate) fn identity(run_as: &RunAs, session: &SessionRequirement) -> Result<(), Error> {
    let (subject, current) = token_identity()?;
    match run_as {
        RunAs::System {
            platform: Platform::Windows,
        } if subject == "S-1-5-18" => {}
        RunAs::User { account }
            if account.platform == Platform::Windows
                && account.subject.as_str() == subject
                && current != 0 => {}
        _ => return Err(Error::Unbound),
    }
    if let SessionRequirement::ActiveUser { account, session } = session {
        if session != &current_session_binding()?
            || account.platform != Platform::Windows
            || account.subject.as_str() != subject
            || current == 0
        {
            return Err(Error::Unbound);
        }
        let mut buffer = null_mut();
        let mut bytes = 0;
        if unsafe {
            WTSQuerySessionInformationW(
                WTS_CURRENT_SERVER_HANDLE,
                current,
                WTSConnectState,
                &mut buffer,
                &mut bytes,
            )
        } == 0
        {
            return Err(Error::Unbound);
        }
        let active =
            bytes >= 4 && unsafe { *(buffer as *const WTS_CONNECTSTATE_CLASS) } == WTSActive;
        unsafe {
            WTSFreeMemory(buffer.cast());
        }
        if !active {
            return Err(Error::Unbound);
        }
    }
    Ok(())
}
pub(crate) fn profile(profile: &VersionedRef) -> Result<(), Error> {
    if profile.revision.as_str() == "1"
        && [
            "native-pwsh7-file",
            "native-osquery-template",
            "native-software-worker",
        ]
        .contains(&profile.id.as_str())
    {
        Ok(())
    } else {
        Err(Error::Unsupported)
    }
}
pub(crate) fn encoding(_: ArtifactEncoding) -> Result<(), Error> {
    Ok(())
}
pub(crate) fn arguments(
    profile: &VersionedRef,
    args: &[String],
    script: &std::path::Path,
) -> Result<(), Error> {
    if profile.id.as_str() == crate::osquery::PROFILE {
        return if crate::osquery::prefix_matches(args) {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    let path = script.to_str().ok_or(Error::InvalidInput)?;
    if profile.id.as_str() == "native-software-worker" {
        return if args == ["--software-worker", path] {
            Ok(())
        } else {
            Err(Error::Denied)
        };
    }
    let prefix: &[&str] = match profile.id.as_str() {
        "native-pwsh7-file" => &["-NoLogo", "-NoProfile", "-NonInteractive", "-File", path],
        _ => return Err(Error::Unsupported),
    };
    if args.len() < prefix.len() || !args.iter().zip(prefix).all(|(a, b)| a == b) {
        return Err(Error::Denied);
    }
    Ok(())
}

// ref: System Informer phnt/ntexapi.h, SYSTEM_BOOT_ENVIRONMENT_INFORMATION (class 90).
// The kernel GUID survives service restarts and is not derived from adjustable wall time.
pub(crate) fn boot_generation() -> Result<Id, Error> {
    #[repr(C)]
    struct BootEnvironment {
        identifier: windows_sys::core::GUID,
        firmware: u32,
        flags: u64,
    }
    let mut info = BootEnvironment {
        identifier: windows_sys::core::GUID::from_u128(0),
        firmware: 0,
        flags: 0,
    };
    let mut returned = 0u32;
    let status = unsafe {
        windows_sys::Wdk::System::SystemInformation::NtQuerySystemInformation(
            90,
            (&mut info as *mut BootEnvironment).cast(),
            std::mem::size_of::<BootEnvironment>() as u32,
            &mut returned,
        )
    };
    if status < 0 || returned < 16 {
        return Err(Error::Unavailable);
    }
    let g = info.identifier;
    Id::new(format!(
        "{:08x}-{:04x}-{:04x}-{}",
        g.data1,
        g.data2,
        g.data3,
        g.data4
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
    .map_err(|_| Error::Unavailable)
}
