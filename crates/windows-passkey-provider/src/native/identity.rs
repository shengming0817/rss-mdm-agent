use super::*;
use windows_sys::{
    core::GUID,
    Win32::{
        Security::{Authorization::ConvertSidToStringSidW, *},
        System::{LibraryLoader::*, Registry::*},
    },
};
pub fn guid_bytes(id: &GUID) -> [u8; 16] {
    let mut out = [0; 16];
    out[..4].copy_from_slice(&id.data1.to_le_bytes());
    out[4..6].copy_from_slice(&id.data2.to_le_bytes());
    out[6..8].copy_from_slice(&id.data3.to_le_bytes());
    out[8..].copy_from_slice(&id.data4);
    out
}
pub fn platform() -> Result<()> {
    // Read the installed build and UBR, then probe all required exports; no older-OS fallback.
    fn number(name: &str) -> Result<u32> {
        let mut value = 0u32;
        let mut size = 4u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion").as_ptr(),
                wide(name).as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        if result != 0 || size != 4 {
            Err(Error::Unsupported)
        } else {
            Ok(value)
        }
    }
    #[repr(C)]
    struct Version {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service: [u16; 128],
    }
    let mut version = Version {
        size: std::mem::size_of::<Version>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        service: [0; 128],
    };
    unsafe {
        let module = GetModuleHandleW(wide("ntdll.dll").as_ptr());
        let function =
            GetProcAddress(module, c"RtlGetVersion".as_ptr().cast()).ok_or(Error::Unsupported)?;
        let version_fn: unsafe extern "system" fn(*mut Version) -> i32 =
            std::mem::transmute(function);
        if version_fn(&mut version) < 0
            || version.major < 10
            || version.build < crate::MINIMUM_BUILD
        {
            return Err(Error::Unsupported);
        }
    }
    if matches!(version.build, 26100 | 26200) && number("UBR")? < crate::MINIMUM_REVISION {
        return Err(Error::Unsupported);
    }
    api::Api::get()?;
    Ok(())
}
fn token_owner(token: HANDLE) -> Result<String> {
    let mut count = 0;
    unsafe {
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut count);
    }
    if count == 0 || count > 65536 {
        return Err(Error::Owner);
    }
    let mut buffer = vec![0usize; (count as usize).div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        if GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            count,
            &mut count,
        ) == 0
        {
            return Err(Error::Owner);
        }
        let mut text = std::ptr::null_mut();
        if ConvertSidToStringSidW(
            (*(buffer.as_ptr().cast::<TOKEN_USER>())).User.Sid,
            &mut text,
        ) == 0
        {
            return Err(Error::Owner);
        }
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        let result =
            String::from_utf16(std::slice::from_raw_parts(text, len)).map_err(|_| Error::Owner);
        LocalFree(text.cast());
        result
    }
}
pub fn caller(hwnd: HWND, owner: &str) -> Result<()> {
    if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 {
        return Err(Error::Owner);
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    let (mut session, mut ours) = (0, 0);
    unsafe {
        if ProcessIdToSessionId(pid, &mut session) == 0
            || ProcessIdToSessionId(GetCurrentProcessId(), &mut ours) == 0
            || session != ours
            || ours == 0
        {
            return Err(Error::Owner);
        }
        let process = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
        if process.0.is_null() {
            return Err(Error::Owner);
        }
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(process.0, TOKEN_QUERY, &mut token) == 0 {
            return Err(Error::Owner);
        }
        let token = Handle(token);
        if token_owner(token.0)? != owner {
            return Err(Error::Owner);
        }
    }
    Ok(())
}
