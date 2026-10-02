use super::*;
use std::{
    ffi::c_void,
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::Threading::*,
};
fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn sid(sid: PSID) -> io::Result<String> {
    unsafe {
        let mut value = null_mut();
        if ConvertSidToStringSidW(sid, &mut value) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _owner = Local(value.cast());
        let mut len = 0;
        while *value.add(len) != 0 && len < 256 {
            len += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(value, len)).map_err(io::Error::other)
    }
}
fn current_sid() -> io::Result<String> {
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0usize; 512];
        let mut needed = 0;
        let result = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * size_of::<usize>()) as u32,
            &mut needed,
        );
        CloseHandle(token);
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        sid((*(buffer.as_ptr() as *const TOKEN_USER)).User.Sid)
    }
}
fn descriptor() -> io::Result<Local> {
    let user = current_sid()?;
    let text = wide(format!(
        "O:{user}D:P(A;OICI;FA;;;{user})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)"
    ));
    let mut descriptor = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(Local(descriptor))
}
pub fn file(file: &File) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other("reparse private path"));
    }
    let user = current_sid()?;
    unsafe {
        let mut owner = null_mut();
        let mut acl = null_mut();
        let mut security = null_mut();
        let result = GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut security,
        );
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        let _security = Local(security);
        if acl.is_null() || sid(owner)? != user {
            return Err(io::Error::other("private owner/ACL"));
        }
        for index in 0..(*acl).AceCount as u32 {
            let mut ace = null_mut();
            if GetAce(acl, index, &mut ace) == 0 {
                return Err(io::Error::last_os_error());
            }
            let header = &*(ace as *const ACE_HEADER);
            if header.AceType == 1 {
                continue;
            }
            if header.AceType != 0 {
                return Err(io::Error::other("unsupported private ACE"));
            }
            let applies_to_object = header.AceFlags & INHERIT_ONLY_ACE as u8 == 0;
            let propagates_to_children = metadata.is_dir()
                && header.AceFlags & (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8 != 0;
            if !applies_to_object && !propagates_to_children {
                continue;
            }
            let allowed = &*(ace as *const ACCESS_ALLOWED_ACE);
            let subject = sid((&allowed.SidStart as *const u32).cast_mut().cast())?;
            if allowed.Mask != 0
                && subject != user
                && !matches!(subject.as_str(), "S-1-5-18" | "S-1-5-32-544")
            {
                return Err(io::Error::other("private ACL grants another subject"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn inherit_only_world_access_on_private_directory_is_rejected() {
        let root = std::env::temp_dir().join(format!("rss-private-acl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let text = wide(format!(
            "O:{}D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICIIO;FA;;;WD)",
            current_sid().unwrap()
        ));
        let mut security = null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    text.as_ptr(),
                    1,
                    &mut security,
                    null_mut(),
                )
            },
            0
        );
        let _security = Local(security);
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security,
            bInheritHandle: 0,
        };
        assert_ne!(
            unsafe { CreateDirectoryW(wide(&root).as_ptr(), &attributes) },
            0
        );
        assert!(open_directory(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
pub fn directory(path: &Path) -> io::Result<()> {
    if path.exists() {
        return open_directory(path).map(|_| ());
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("parent required"))?;
    if !parent.exists() {
        directory(parent)?;
    }
    let mut pins = Vec::new();
    let mut chain: Vec<_> = parent
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    chain.reverse();
    for entry in chain {
        let handle = OpenOptions::new()
            .read(true)
            .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(entry)?;
        ancestor(&handle)?;
        pins.push(handle);
    }
    let descriptor = descriptor()?;
    let attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    if unsafe { CreateDirectoryW(wide(path).as_ptr(), &attrs) } == 0 {
        return Err(io::Error::last_os_error());
    }
    open_directory(path).map(|_| ())
}
pub fn create_new(path: &Path) -> io::Result<File> {
    let descriptor = descriptor()?;
    let attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let handle = unsafe {
        CreateFileW(
            wide(path).as_ptr(),
            GENERIC_WRITE | READ_CONTROL | FILE_READ_ATTRIBUTES,
            0,
            &attrs,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_handle(handle) };
    self::file(&file)?;
    Ok(file)
}
pub fn replace(staged: &Path, path: &Path) -> io::Result<()> {
    if unsafe {
        MoveFileExW(
            wide(staged).as_ptr(),
            wide(path).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn ancestor(handle: &File) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    if !handle.metadata()?.is_dir()
        || handle.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(io::Error::other("private ancestor type"));
    }
    let user = current_sid()?;
    unsafe {
        let mut owner = null_mut();
        let mut acl = null_mut();
        let mut security = null_mut();
        let status = GetSecurityInfo(
            handle.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut security,
        );
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let _security = Local(security);
        let trusted =
            |subject: &str| subject == user || matches!(subject, "S-1-5-18" | "S-1-5-32-544");
        if acl.is_null() || !trusted(&sid(owner)?) {
            return Err(io::Error::other("untrusted ancestor owner"));
        }
        for index in 0..(*acl).AceCount as u32 {
            let mut ace = null_mut();
            if GetAce(acl, index, &mut ace) == 0 {
                return Err(io::Error::last_os_error());
            }
            let header = &*(ace as *const ACE_HEADER);
            if header.AceType == 1 || header.AceFlags & INHERIT_ONLY_ACE as u8 != 0 {
                continue;
            }
            if header.AceType != 0 {
                return Err(io::Error::other("unsupported ancestor ACE"));
            }
            let allowed = &*(ace as *const ACCESS_ALLOWED_ACE);
            let subject = sid((&allowed.SidStart as *const u32).cast_mut().cast())?;
            // Public traversal/child creation does not authorize replacing a pinned directory.
            if !trusted(&subject)
                && allowed.Mask
                    & (GENERIC_ALL
                        | GENERIC_WRITE
                        | DELETE
                        | WRITE_DAC
                        | WRITE_OWNER
                        | FILE_DELETE_CHILD)
                    != 0
            {
                return Err(io::Error::other("replaceable private ancestor"));
            }
        }
    }
    Ok(())
}
pub fn open_directory(path: &Path) -> io::Result<(File, Vec<File>)> {
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut ancestors = Vec::new();
    let mut chain: Vec<_> = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    chain.reverse();
    for entry in chain {
        let handle = OpenOptions::new()
            .read(true)
            .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(entry)?;
        ancestor(&handle)?;
        ancestors.push(handle);
    }
    let handle = ancestors.pop().ok_or(io::ErrorKind::InvalidInput)?;
    file(&handle)?;
    Ok((handle, ancestors))
}
pub fn open_file(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    self::file(&file)?;
    Ok(file)
}

pub fn open_writable(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    self::file(&file)?;
    Ok(file)
}
