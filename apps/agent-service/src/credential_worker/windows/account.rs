use super::*;
use ::windows::{
    core::{w, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL},
        NetworkManagement::NetManagement::*,
        Security::{Authorization::*, *},
        System::Registry::*,
    },
};
fn wide(s: &str) -> Zeroizing<Vec<u16>> {
    Zeroizing::new(s.encode_utf16().chain([0]).collect())
}
struct Buffer(*mut u8);
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            let _ = NetApiBufferFree(Some(self.0.cast()));
        }
    }
}
fn code(c: u32) -> Result<(), F> {
    match c {
        0 => Ok(()),
        5 => Err(F::PermissionDenied),
        2221 => Err(F::TargetMissing),
        _ => Err(F::NativeFailure),
    }
}
unsafe fn text(ptr: *const u16) -> Result<String, F> {
    if ptr.is_null() {
        return Err(F::NativeFailure);
    }
    let mut len = 0;
    while len < 256 && unsafe { *ptr.add(len) } != 0 {
        len += 1
    }
    if len == 256 {
        return Err(F::NativeFailure);
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(ptr, len) })
        .map_err(|_| F::NativeFailure)
}
fn sid(value: PSID) -> Result<String, F> {
    unsafe {
        let mut ptr = PWSTR::null();
        ConvertSidToStringSidW(value, &mut ptr).map_err(error)?;
        let result = text(ptr.0);
        LocalFree(Some(HLOCAL(ptr.0.cast())));
        result
    }
}
fn ownership() -> Result<(), F> {
    // Any configured native LAPS owner is exclusive, including disabled/partial policies.
    // Access failures are not evidence that the competing owner is absent.
    for path in [
        "SOFTWARE\\Microsoft\\Policies\\LAPS",
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\LAPS",
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\LAPS\\Config",
        "SOFTWARE\\Policies\\Microsoft Services\\AdmPwd",
        "SOFTWARE\\RSS\\PasswordManagementOwner",
    ] {
        let mut key = HKEY::default();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(wide(path).as_ptr()),
                None,
                KEY_READ | KEY_WOW64_64KEY,
                &mut key,
            )
        };
        match status.0 {
            2 | 3 => (),
            0 => {
                unsafe {
                    let _ = RegCloseKey(key);
                }
                return Err(F::OwnerConflict);
            }
            5 => return Err(F::PermissionDenied),
            _ => return Err(F::OwnerConflict),
        }
    }
    Ok(())
}
fn name_for_sid(expected: &str) -> Result<String, F> {
    unsafe {
        let mut resume = 0;
        let mut pages = 0;
        loop {
            pages += 1;
            if pages > 128 {
                return Err(F::NativeFailure);
            }
            let mut buffer = Buffer(std::ptr::null_mut());
            let (mut count, mut total) = (0, 0);
            let status = NetUserEnum(
                PCWSTR::null(),
                0,
                NET_USER_ENUM_FILTER_FLAGS(0),
                &mut buffer.0,
                65536,
                &mut count,
                &mut total,
                Some(&mut resume),
            );
            if status != 234 {
                code(status)?
            }
            if count > 65536 || (count > 0 && buffer.0.is_null()) {
                return Err(F::NativeFailure);
            }
            for i in 0..count as usize {
                let name = (*buffer.0.cast::<USER_INFO_0>().add(i)).usri0_name;
                let mut detail = Buffer(std::ptr::null_mut());
                code(NetUserGetInfo(
                    PCWSTR::null(),
                    PCWSTR(name.0),
                    23,
                    &mut detail.0,
                ))?;
                if detail.0.is_null() {
                    return Err(F::NativeFailure);
                }
                let info = &*detail.0.cast::<USER_INFO_23>();
                if sid(info.usri23_user_sid)? == expected {
                    if info.usri23_flags.0 & UF_ACCOUNTDISABLE.0 != 0 {
                        return Err(F::PermissionDenied);
                    }
                    return text(info.usri23_name.0);
                }
            }
            if status == 0 {
                break;
            }
            if count == 0 {
                return Err(F::NativeFailure);
            }
        }
        Err(F::TargetMissing)
    }
}
fn administrator(expected: &str) -> Result<(), F> {
    unsafe {
        let mut admin = PSID::default();
        ConvertStringSidToSidW(w!("S-1-5-32-544"), &mut admin).map_err(error)?;
        let mut name = [0u16; 256];
        let mut domain = [0u16; 256];
        let (mut nc, mut dc) = (256, 256);
        let mut kind = SID_NAME_USE::default();
        let result = LookupAccountSidW(
            PCWSTR::null(),
            admin,
            Some(PWSTR(name.as_mut_ptr())),
            &mut nc,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut dc,
            &mut kind,
        );
        LocalFree(Some(HLOCAL(admin.0)));
        result.map_err(error)?;
        let mut resume = 0;
        for _ in 0..128 {
            let mut buffer = Buffer(std::ptr::null_mut());
            let (mut count, mut total) = (0, 0);
            let status = NetLocalGroupGetMembers(
                PCWSTR::null(),
                PCWSTR(name.as_ptr()),
                0,
                &mut buffer.0,
                65536,
                &mut count,
                &mut total,
                Some(&mut resume),
            );
            if status != 234 {
                code(status)?
            }
            if count > 65536 || (count > 0 && buffer.0.is_null()) {
                return Err(F::NativeFailure);
            }
            for i in 0..count as usize {
                if sid((*buffer.0.cast::<LOCALGROUP_MEMBERS_INFO_0>().add(i)).lgrmi0_sid)?
                    == expected
                {
                    return Ok(());
                }
            }
            if status == 0 {
                break;
            }
        }
        Err(F::PermissionDenied)
    }
}
pub(super) fn check(expected: &str) -> Result<String, F> {
    ownership()?;
    if !expected.starts_with("S-1-5-21-") {
        return Err(F::TargetMissing);
    }
    let name = name_for_sid(expected)?;
    administrator(expected)?;
    Ok(name)
}
pub(super) fn reset(expected: &str, secret: &str) -> Result<(), F> {
    let name = check(expected)?;
    let password = wide(secret);
    let info = USER_INFO_1003 {
        usri1003_password: PWSTR(password.as_ptr().cast_mut()),
    };
    // Resolve immediately before the local SAM call; never accept a remote server or domain.
    if name_for_sid(expected)? != name {
        return Err(F::TargetMissing);
    }
    code(unsafe {
        NetUserSetInfo(
            PCWSTR::null(),
            PCWSTR(wide(&name).as_ptr()),
            1003,
            (&info as *const USER_INFO_1003).cast(),
            None,
        )
    })?;
    if name_for_sid(expected)? != name {
        return Err(F::NativeFailure);
    }
    Ok(())
}
pub(super) fn verify(expected: &str, secret: &str) -> Result<bool, F> {
    let name = check(expected)?;
    let mut token = HANDLE::default();
    let result = unsafe {
        LogonUserW(
            PCWSTR(wide(&name).as_ptr()),
            w!("."),
            PCWSTR(wide(secret).as_ptr()),
            LOGON32_LOGON_NETWORK,
            LOGON32_PROVIDER_DEFAULT,
            &mut token,
        )
    };
    if let Err(e) = result {
        if e.code().0 as u32 == 0x8007052e {
            return Ok(false);
        }
        return Err(error(e));
    }
    // Authenticate the returned token's SID, not only a mutable SAM account name.
    let verified = (|| unsafe {
        let mut length = 0;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut length);
        if length == 0 || length > 65536 {
            return Err(F::NativeFailure);
        }
        let mut bytes = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
        GetTokenInformation(
            token,
            TokenUser,
            Some(bytes.as_mut_ptr().cast()),
            length,
            &mut length,
        )
        .map_err(error)?;
        Ok(sid((*(bytes.as_ptr().cast::<TOKEN_USER>())).User.Sid)? == expected)
    })();
    unsafe {
        let _ = CloseHandle(token);
    }
    verified
}
