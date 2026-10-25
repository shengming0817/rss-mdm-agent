//! Native storage only. The desktop/Provider owns the system authentication UI.
use crate::{Error, Result, SystemProtection};
use zeroize::Zeroizing;

pub struct OsProtection {
    verify: Box<dyn Fn() -> Result<()> + Send + Sync>,
}
impl OsProtection {
    pub fn new(verify: impl Fn() -> Result<()> + Send + Sync + 'static) -> Self {
        Self {
            verify: Box::new(verify),
        }
    }
}
impl SystemProtection for OsProtection {
    fn verify(&self) -> Result<()> {
        (self.verify)()
    }
    fn protect(&self, reference: &str, key: &[u8]) -> Result<Vec<u8>> {
        if uuid::Uuid::parse_str(reference).is_err() || key.len() != 32 {
            return Err(Error::Input);
        }
        #[cfg(target_os = "macos")]
        {
            match self.unprotect(reference, &[]) {
                Ok(existing) if existing.as_slice() == key => Ok(vec![]),
                Ok(_) => Err(Error::Conflict),
                Err(Error::NotFound) => platform_credentials::create_user_password(
                    "RSS Personal Passkeys",
                    reference,
                    key,
                )
                .map(|_| vec![])
                .map_err(|_| Error::SystemUnavailable),
                Err(error) => Err(error),
            }
        }
        #[cfg(windows)]
        {
            platform_credentials::protect_key(key).map_err(|_| Error::SystemUnavailable)
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            Err(Error::Unsupported)
        }
    }
    fn unprotect(&self, reference: &str, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        if uuid::Uuid::parse_str(reference).is_err() {
            return Err(Error::Input);
        }
        #[cfg(target_os = "macos")]
        {
            if !protected.is_empty() {
                return Err(Error::Corrupt);
            }
            platform_credentials::read_user_password("RSS Personal Passkeys", reference)
                .map(Zeroizing::new)
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        Error::NotFound
                    } else {
                        Error::SystemUnavailable
                    }
                })
        }
        #[cfg(windows)]
        {
            if protected.is_empty() {
                return Err(Error::NotFound);
            }
            platform_credentials::unprotect_key(protected)
                .map(Zeroizing::new)
                .map_err(|_| Error::SystemUnavailable)
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = protected;
            Err(Error::Unsupported)
        }
    }
}

pub fn owner() -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn mbr_uid_to_uuid(uid: libc::uid_t, uuid: *mut u8) -> libc::c_int;
        }
        // SAFETY: geteuid reads kernel identity; UUID points to a writable 16-byte buffer.
        let uid = unsafe { libc::geteuid() };
        if uid == 0 {
            return Err(Error::Owner);
        }
        let mut id = [0u8; 16];
        if unsafe { mbr_uid_to_uuid(uid, id.as_mut_ptr()) } != 0 {
            return Err(Error::Owner);
        }
        Ok(uuid::Uuid::from_bytes(id).to_string())
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, LocalFree},
            Security::{Authorization::ConvertSidToStringSidW, *},
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        };
        let mut token = std::ptr::null_mut();
        // SAFETY: current process handle and writable out handle are valid.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(Error::Owner);
        }
        let mut size = 0;
        // SAFETY: first call obtains required byte size only.
        unsafe {
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size);
        }
        let mut bytes = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        // SAFETY: word-aligned buffer has the size requested by Windows.
        let ok = unsafe {
            GetTokenInformation(token, TokenUser, bytes.as_mut_ptr().cast(), size, &mut size)
        };
        unsafe {
            CloseHandle(token);
        }
        if ok == 0 || (size as usize) < size_of::<TOKEN_USER>() {
            return Err(Error::Owner);
        }
        let mut text = std::ptr::null_mut();
        // SAFETY: successful TokenUser query returned a valid SID in the live buffer.
        if unsafe {
            ConvertSidToStringSidW((*(bytes.as_ptr().cast::<TOKEN_USER>())).User.Sid, &mut text)
        } == 0
        {
            return Err(Error::Owner);
        }
        let mut len = 0;
        // SAFETY: OS allocated a NUL-terminated SID string; released after conversion.
        let value = unsafe {
            while *text.add(len) != 0 {
                len += 1;
            }
            let value = String::from_utf16(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            value
        }
        .map_err(|_| Error::Owner)?;
        if value == "S-1-5-18" {
            return Err(Error::Owner);
        }
        Ok(value)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(Error::Unsupported)
    }
}

/// The existing desktop location, independent of package family and enterprise identity.
#[cfg(windows)]
pub fn default_root() -> Result<std::path::PathBuf> {
    use windows_sys::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath},
    };
    let mut value = std::ptr::null_mut();
    // SAFETY: Windows allocates a NUL-terminated path for the current user.
    unsafe {
        if SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, std::ptr::null_mut(), &mut value) < 0 {
            return Err(Error::Owner);
        }
        let mut length = 0;
        while *value.add(length) != 0 {
            length += 1;
        }
        let result =
            String::from_utf16(std::slice::from_raw_parts(value, length)).map_err(|_| Error::Owner);
        CoTaskMemFree(value.cast());
        Ok(std::path::PathBuf::from(result?)
            .join("com.rss.mdmagent")
            .join("personal-passkeys"))
    }
}
/// Notify any active Provider; no waiting behind native UI or a store transaction.
#[cfg(windows)]
pub(crate) fn notify_lock() {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenEventW, SetEvent, EVENT_MODIFY_STATE},
    };
    if let Ok(owner) = owner() {
        let name: Vec<u16> = format!("Local\\RSS.PersonalPasskeys.Cancel.{owner}")
            .encode_utf16()
            .chain([0])
            .collect();
        // SAFETY: name is live and terminated; an absent Provider needs no notification.
        unsafe {
            let event = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
            if !event.is_null() {
                SetEvent(event);
                CloseHandle(event);
            }
        }
    }
}

#[cfg(windows)]
pub fn notify_management_change() {
    notify_lock();
}
