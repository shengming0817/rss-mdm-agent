//! Local-only named pipe with explicit ACL and bidirectional kernel peer facts.
use super::*;
use std::{
    ffi::c_void,
    os::windows::io::AsRawHandle,
    path::{Path, PathBuf},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_ProgramData, SHGetKnownFolderPath},
};
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
pub fn deployment_path() -> Result<PathBuf, Rejected> {
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
            .join("execution.json"))
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
