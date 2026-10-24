use std::mem::size_of;
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Security::{Authentication::Identity::*, Credentials::*},
        System::Com::*,
        UI::Shell::*,
    },
};
use zeroize::Zeroizing;

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
pub unsafe fn text(value: PCWSTR, limit: usize) -> Result<String> {
    if value.is_null() {
        return Err(E_INVALIDARG.into());
    }
    let mut length = 0;
    while length < limit && unsafe { *value.0.add(length) } != 0 {
        length += 1;
    }
    if length == limit {
        return Err(E_INVALIDARG.into());
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(value.0, length) })
        .map_err(|_| E_INVALIDARG.into())
}
pub fn copy(value: &str) -> Result<PWSTR> {
    let wide = wide(value);
    let allocation = unsafe { CoTaskMemAlloc(wide.len() * 2) }.cast::<u16>();
    if allocation.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    unsafe {
        allocation.copy_from_nonoverlapping(wide.as_ptr(), wide.len());
    }
    Ok(PWSTR(allocation))
}
/// Uses the exact persisted native identity; display names never choose a binding.
pub fn account(authority: &device_access::Authority) -> Result<(String, String)> {
    device_access::windows::account::verify_bound(authority)
        .map_err(|_| Error::from(E_ACCESSDENIED))?;
    let (domain, user) = authority
        .login
        .split_once('\\')
        .ok_or_else(|| Error::from(E_ACCESSDENIED))?;
    if domain.is_empty() || user.is_empty() {
        return Err(E_ACCESSDENIED.into());
    }
    Ok((domain.into(), user.into()))
}

fn package() -> Result<u32> {
    unsafe {
        let mut handle = HANDLE::default();
        if LsaConnectUntrusted(&mut handle).0 < 0 {
            return Err(E_ACCESSDENIED.into());
        }
        let mut name = *b"Negotiate\0";
        let name = LSA_STRING {
            Length: 9,
            MaximumLength: 10,
            Buffer: PSTR(name.as_mut_ptr()),
        };
        let mut package = 0;
        let status = LsaLookupAuthenticationPackage(handle, &name, &mut package);
        let _ = LsaDeregisterLogonProcess(handle);
        if status.0 < 0 {
            Err(E_ACCESSDENIED.into())
        } else {
            Ok(package)
        }
    }
}
/// Called after durable factor consumption. LSA alone verifies the Windows password.
pub fn pack(
    authority: &device_access::Authority,
    qualified_login: &str,
    password: &str,
    scenario: CREDENTIAL_PROVIDER_USAGE_SCENARIO,
) -> Result<CREDENTIAL_PROVIDER_CREDENTIAL_SERIALIZATION> {
    if scenario != CPUS_LOGON && scenario != CPUS_UNLOCK_WORKSTATION {
        return Err(E_INVALIDARG.into());
    }
    let (domain, user) = account(authority)?;
    if qualified_login.is_empty()
        || qualified_login.len() > 512
        || qualified_login.chars().any(char::is_control)
    {
        return Err(E_INVALIDARG.into());
    }
    let (domain, user) = if matches!(
        authority.source,
        rss_mdm_agent_wire::runtime::AccessWindowsAuthority::Local
    ) {
        let current = device_access::windows::account::inspect(&authority.sid)
            .map_err(|_| Error::from(E_ACCESSDENIED))?;
        (current.domain, current.name)
    } else {
        qualified_login
            .split_once('\\')
            .map(|(d, u)| (d.to_owned(), u.to_owned()))
            .unwrap_or((domain, user))
    };
    if matches!(
        authority.source,
        rss_mdm_agent_wire::runtime::AccessWindowsAuthority::Entra { .. }
    ) {
        return pack_cloud(qualified_login, password);
    }
    let package = package()?;
    let clear = Zeroizing::new(wide(password));
    let mut protected = Zeroizing::new(Vec::<u16>::new());
    if !password.is_empty() {
        let mut count = 0;
        let first = unsafe { CredProtectW(false, &clear, PWSTR::null(), &mut count, None) };
        if first.is_ok() || count == 0 || count > 8192 {
            return Err(E_ACCESSDENIED.into());
        }
        protected.resize(count as usize, 0);
        unsafe {
            CredProtectW(
                false,
                &clear,
                PWSTR(protected.as_mut_ptr()),
                &mut count,
                None,
            )?;
        }
        if count as usize > protected.len() || count == 0 {
            return Err(E_UNEXPECTED.into());
        }
        protected.truncate(count as usize);
        if protected.pop() != Some(0) {
            return Err(E_UNEXPECTED.into());
        }
    }
    let domain: Vec<u16> = domain.encode_utf16().collect();
    let user: Vec<u16> = user.encode_utf16().collect();
    let total = size_of::<KERB_INTERACTIVE_UNLOCK_LOGON>()
        + 2 * (domain.len() + user.len() + protected.len());
    if total > 32768 {
        return Err(E_INVALIDARG.into());
    }
    let buffer = unsafe { CoTaskMemAlloc(total) }.cast::<u8>();
    if buffer.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    // No fallible operation after allocation; ownership passes to Windows on return.
    unsafe {
        buffer.write_bytes(0, total);
        let packed = &mut *buffer.cast::<KERB_INTERACTIVE_UNLOCK_LOGON>();
        packed.Logon.MessageType = if scenario == CPUS_LOGON {
            KerbInteractiveLogon
        } else {
            KerbWorkstationUnlockLogon
        };
        let mut offset = size_of::<KERB_INTERACTIVE_UNLOCK_LOGON>();
        for (source, target) in [
            (&domain[..], &mut packed.Logon.LogonDomainName),
            (&user[..], &mut packed.Logon.UserName),
            (&protected[..], &mut packed.Logon.Password),
        ] {
            let bytes = source.len() * 2;
            buffer
                .add(offset)
                .copy_from_nonoverlapping(source.as_ptr().cast(), bytes);
            *target = LSA_UNICODE_STRING {
                Length: bytes as u16,
                MaximumLength: bytes as u16,
                Buffer: PWSTR(offset as *mut u16),
            };
            offset += bytes;
        }
    }
    Ok(CREDENTIAL_PROVIDER_CREDENTIAL_SERIALIZATION {
        ulAuthenticationPackage: package,
        clsidCredentialProvider: super::provider::CLSID,
        cbSerialization: total as u32,
        rgbSerialization: buffer,
    })
}

// ref: Microsoft Windows-classic-samples CSampleCredential::GetSerialization, identity-provider branch.
fn pack_cloud(login: &str, password: &str) -> Result<CREDENTIAL_PROVIDER_CREDENTIAL_SERIALIZATION> {
    let package = package()?;
    let mut login = wide(login);
    let mut password = Zeroizing::new(wide(password));
    let flags = CRED_PACK_PROTECTED_CREDENTIALS | CRED_PACK_ID_PROVIDER_CREDENTIALS;
    let mut count = 0;
    if unsafe {
        CredPackAuthenticationBufferW(
            flags,
            PWSTR(login.as_mut_ptr()),
            PWSTR(password.as_mut_ptr()),
            None,
            &mut count,
        )
    }
    .is_ok()
        || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER
        || count == 0
        || count > 32768
    {
        return Err(E_ACCESSDENIED.into());
    }
    let buffer = unsafe { CoTaskMemAlloc(count as usize) }.cast::<u8>();
    if buffer.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    let capacity = count;
    if let Err(error) = unsafe {
        CredPackAuthenticationBufferW(
            flags,
            PWSTR(login.as_mut_ptr()),
            PWSTR(password.as_mut_ptr()),
            Some(buffer.cast()),
            &mut count,
        )
    } {
        unsafe {
            std::ptr::write_bytes(buffer, 0, capacity as usize);
            CoTaskMemFree(Some(buffer.cast()));
        }
        return Err(error);
    }
    if count > capacity {
        unsafe {
            std::ptr::write_bytes(buffer, 0, capacity as usize);
            CoTaskMemFree(Some(buffer.cast()));
        }
        return Err(E_UNEXPECTED.into());
    }
    Ok(CREDENTIAL_PROVIDER_CREDENTIAL_SERIALIZATION {
        ulAuthenticationPackage: package,
        clsidCredentialProvider: super::provider::CLSID,
        cbSerialization: count,
        rgbSerialization: buffer,
    })
}
