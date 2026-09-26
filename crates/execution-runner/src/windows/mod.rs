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
    let mut buffer = vec![0usize; 128];
    let mut size = 0;
    if unsafe {
        GetTokenInformation(
            raw(&token),
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
            raw(&token),
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
    if let SessionRequirement::ActiveUser { account } = session {
        if account.platform != Platform::Windows
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
        && ["native-pwsh7-file", "native-osquery-info-v1"].contains(&profile.id.as_str())
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
    let path = script.to_str().ok_or(Error::InvalidInput)?;
    let prefix: &[&str] = match profile.id.as_str() {
        "native-pwsh7-file" => &["-NoLogo", "-NoProfile", "-NonInteractive", "-File", path],
        "native-osquery-info-v1" => &["--json", "SELECT version FROM osquery_info;"],
        _ => return Err(Error::Unsupported),
    };
    if args.len() < prefix.len()
        || !args.iter().zip(prefix).all(|(a, b)| a == b)
        || (profile.id.as_str() == "native-osquery-info-v1" && args.len() != prefix.len())
    {
        return Err(Error::Denied);
    }
    Ok(())
}
