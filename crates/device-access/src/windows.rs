#[path = "windows_account.rs"]
pub mod account;
// Native inputs shared by the service and in-process LogonUI consumer.
// ref: execution-ipc/src/windows/identity.rs (boot identity and token queries).
use crate::{Clock, Error};
use std::{mem::size_of, ptr::null_mut};
use windows_sys::Win32::{Foundation::CloseHandle, Security::*, System::Threading::*};

pub fn require_system() -> Result<(), Error> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(Error::Invalid);
    }
    let mut buffer = [0usize; 128];
    let mut length = 0;
    let queried = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size_of::<[usize; 128]>() as u32,
            &mut length,
        )
    };
    let system = queried != 0
        && unsafe {
            IsWellKnownSid(
                (*(buffer.as_ptr().cast::<TOKEN_USER>())).User.Sid,
                WinLocalSystemSid,
            )
        } != 0;
    unsafe {
        CloseHandle(token);
    }
    if system {
        Ok(())
    } else {
        Err(Error::Invalid)
    }
}
pub fn clock() -> Result<Clock, Error> {
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
    let mut returned = 0;
    let status = unsafe {
        windows_sys::Wdk::System::SystemInformation::NtQuerySystemInformation(
            90,
            (&mut info as *mut BootEnvironment).cast(),
            size_of::<BootEnvironment>() as u32,
            &mut returned,
        )
    };
    if status < 0 || returned < 16 {
        return Err(Error::Invalid);
    }
    let g = info.identifier;
    let boot = uuid::Uuid::from_fields(g.data1, g.data2, g.data3, &g.data4);
    let utc = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Invalid)?
            .as_secs(),
    )
    .map_err(|_| Error::Invalid)?;
    let monotonic =
        unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() } / 1000;
    let clock = Clock {
        utc,
        monotonic,
        boot,
    };
    clock.validate()?;
    Ok(clock)
}

pub const PROVIDER_CLSID: &str = "{C61C6B95-9162-42A2-B80B-61EBCA730510}";
pub const CONFIG_KEY: &str = "SOFTWARE\\RSS\\DeviceAccess";
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    pub root: std::path::PathBuf,
    pub id: uuid::Uuid,
}
/// HKLM configuration is written by the independent Agent component command.
pub fn installation() -> Result<Installation, Error> {
    use windows_sys::Win32::System::Registry::*;
    let key: Vec<u16> = CONFIG_KEY.encode_utf16().chain([0]).collect();
    let value: Vec<u16> = "Installation".encode_utf16().chain([0]).collect();
    let mut bytes = 0;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            null_mut(),
            null_mut(),
            &mut bytes,
        )
    };
    if status != 0 || !(2..=8192).contains(&bytes) || bytes % 2 != 0 {
        return Err(Error::Invalid);
    }
    let mut text = vec![0u16; bytes as usize / 2];
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            null_mut(),
            text.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status != 0 || bytes as usize > text.len() * 2 || bytes < 2 || bytes % 2 != 0 {
        return Err(Error::Invalid);
    }
    text.truncate(bytes as usize / 2);
    if text.pop() != Some(0) || text.contains(&0) {
        return Err(Error::Invalid);
    }
    let text = String::from_utf16(&text).map_err(|_| Error::Invalid)?;
    serde_json::from_str(&text).map_err(|_| Error::Invalid)
}
