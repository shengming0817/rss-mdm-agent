use super::*;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Component, Path, PathBuf, Prefix},
};
use windows_sys::Win32::Storage::FileSystem::*;
const SYSTEM: &str = "S-1-5-18";
const ADMIN: &str = "S-1-5-32-544";
const INSTALLER: &str = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";
fn trusted(sid: &str, immutable: bool, current: &str) -> bool {
    matches!(sid, SYSTEM | ADMIN | INSTALLER) || (!immutable && sid == current)
}
fn check(file: &File, immutable: bool, leaf: bool, current: &str) -> Result<(), Error> {
    let metadata = file.metadata().map_err(|_| Error::Unavailable)?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(Error::Denied);
    }
    let mut owner = null_mut();
    let mut acl = null_mut();
    let mut descriptor = null_mut();
    if unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut descriptor,
        )
    } != 0
    {
        return Err(Error::Denied);
    }
    let _descriptor = Local(descriptor);
    if owner.is_null()
        || acl.is_null()
        || unsafe { IsValidSid(owner) } == 0
        || unsafe { IsValidAcl(acl) } == 0
        || !trusted(&sid(owner)?, immutable, current)
    {
        return Err(Error::Denied);
    }
    for index in 0..unsafe { (*acl).AceCount } as u32 {
        let mut entry = null_mut();
        if unsafe { GetAce(acl, index, &mut entry) } == 0 {
            return Err(Error::Denied);
        }
        let header = unsafe { &*(entry as *const ACE_HEADER) };
        if header.AceType == 1 {
            continue;
        }
        if header.AceType != 0 || (header.AceSize as usize) < size_of::<ACCESS_ALLOWED_ACE>() {
            return Err(Error::Denied);
        }
        if header.AceFlags & INHERIT_ONLY_ACE as u8 != 0 {
            continue;
        }
        let allowed = unsafe { &*(entry as *const ACCESS_ALLOWED_ACE) };
        let subject = (&allowed.SidStart as *const u32).cast_mut().cast();
        if unsafe { IsValidSid(subject) } == 0 {
            return Err(Error::Denied);
        }
        let mut mask = allowed.Mask;
        for (generic, specific) in [
            (GENERIC_ALL, FILE_ALL_ACCESS),
            (GENERIC_WRITE, FILE_GENERIC_WRITE),
            (GENERIC_READ, FILE_GENERIC_READ),
            (GENERIC_EXECUTE, FILE_GENERIC_EXECUTE),
        ] {
            if mask & generic != 0 {
                mask = (mask & !generic) | specific
            }
        }
        let replace = DELETE | FILE_DELETE_CHILD | WRITE_DAC | WRITE_OWNER;
        let forbidden = if leaf {
            replace | FILE_WRITE_DATA | FILE_APPEND_DATA | FILE_WRITE_EA | FILE_WRITE_ATTRIBUTES
        } else {
            replace
        };
        if mask & forbidden != 0 && !trusted(&sid(subject)?, immutable, current) {
            return Err(Error::Denied);
        }
    }
    Ok(())
}
fn local_path(path: &Path) -> Result<(), Error> {
    if !path.is_absolute()
        || !matches!(path.components().next(),Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)))
        || path
            .components()
            .any(|p| matches!(p, Component::ParentDir | Component::CurDir))
    {
        return Err(Error::Denied);
    }
    Ok(())
}
/// Retain every path component without delete/write sharing for the entire invocation.
pub(crate) struct PathLease(Vec<File>);
impl PathLease {
    pub(crate) fn source(path: &Path, immutable: bool) -> Result<Self, Error> {
        Self::open(path, immutable)
    }
    fn open(path: &Path, immutable: bool) -> Result<Self, Error> {
        local_path(path)?;
        let (current, _) = token_identity()?;
        let mut handles = Vec::new();
        for part in path
            .ancestors()
            .filter(|p| !p.as_os_str().is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let file = OpenOptions::new()
                .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
                .share_mode(FILE_SHARE_READ)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
                .open(part)
                .map_err(|_| Error::Unavailable)?;
            check(&file, immutable, part == path, &current)?;
            handles.push(file);
        }
        Ok(Self(handles))
    }
}
pub(crate) fn protected_path(path: &Path, directory: bool) -> Result<(), Error> {
    let guard = PathLease::open(path, false)?;
    let meta = guard
        .0
        .last()
        .ok_or(Error::Denied)?
        .metadata()
        .map_err(|_| Error::Unavailable)?;
    if (directory && !meta.is_dir()) || (!directory && !meta.is_file()) {
        return Err(Error::Denied);
    }
    Ok(())
}
pub(crate) fn open_file(path: &Path) -> Result<File, Error> {
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| Error::Unavailable)
}

pub(crate) struct WorkingDirectory {
    path: PathBuf,
    _guard: PathLease,
}
impl WorkingDirectory {
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        protected_path(path, true)?;
        Ok(Self {
            path: path.into(),
            _guard: PathLease::open(path, false)?,
        })
    }
    pub(crate) fn configure(
        &self,
        command: &mut std::process::Command,
        _: &File,
    ) -> Result<(), Error> {
        command.current_dir(&self.path);
        Ok(())
    }
}
pub(crate) fn payload(
    source: File,
    content: &[u8],
    root: &Path,
    attempt: &AttemptId,
    profile: &VersionedRef,
) -> Result<crate::materialize::Payload, Error> {
    if profile.id.as_str() == "native-osquery-template" {
        return Ok(crate::materialize::Payload {
            path: PathBuf::new(),
            file: Some(source),
            directory: None,
        });
    }
    let (current, _) = token_identity()?;
    let descriptor = security(&format!(
        "O:{current}D:P(A;OICI;FA;;;{current})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)"
    ))?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    use sha2::{Digest as _, Sha256};
    let directory = root.join(format!("{:x}", Sha256::digest(attempt.as_str().as_bytes())));
    if unsafe { CreateDirectoryW(wide(&directory).as_ptr(), &attributes) } == 0 {
        return Err(Error::Conflict);
    }
    let path = directory.join("payload.ps1");
    let created = (|| {
        let handle = unsafe {
            CreateFileW(
                wide(&path).as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ,
                &attributes,
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        let owned = own(handle)?;
        use std::os::windows::io::IntoRawHandle;
        let mut file = unsafe { File::from_raw_handle(owned.into_raw_handle()) };
        file.write_all(content)
            .and_then(|_| file.sync_all())
            .map_err(|_| Error::Unavailable)?;
        drop(file);
        // Reopen read-only so interpreters using FILE_SHARE_READ can open it. Verify after
        // acquiring the no-write/no-delete lease; a substitution during reopen is rejected.
        let mut file = open_file(&path)?;
        let mut actual = Vec::new();
        use std::io::{Read, Seek};
        Read::by_ref(&mut file)
            .take(content.len() as u64 + 1)
            .read_to_end(&mut actual)
            .map_err(|_| Error::Unavailable)?;
        if actual != content {
            return Err(Error::Denied);
        }
        file.rewind().map_err(|_| Error::Unavailable)?;
        Ok(crate::materialize::Payload {
            path: path.clone(),
            file: Some(file),
            directory: Some(directory.clone()),
        })
    })();
    if created.is_err() {
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&directory);
    }
    created
}

pub(crate) fn executable(path: &Path) -> Result<(), Error> {
    local_path(path)?;
    if !path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
    {
        return Err(Error::Denied);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn extensionless_executable_cannot_select_an_unhashed_exe() {
        assert!(super::executable(std::path::Path::new(r"C:\trusted\pwsh")).is_err());
        assert!(super::executable(std::path::Path::new(r"C:\trusted\pwsh.EXE")).is_ok());
    }
}

pub(crate) fn grant_read(path: &Path, subject: &str) -> Result<(), Error> {
    if token_identity()?.0 != SYSTEM
        || !subject.starts_with("S-1-")
        || !subject
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'S' || b == b'-')
    {
        return Err(Error::Denied);
    }
    let _lease = PathLease::source(path, true)?;
    let descriptor = security(&format!(
        "O:SYD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FRFX;;;{subject})"
    ))?;
    let mut present = 0;
    let mut defaulted = 0;
    let mut acl = null_mut();
    if unsafe { GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, &mut defaulted) }
        == 0
        || present == 0
        || acl.is_null()
    {
        return Err(Error::Denied);
    }
    if unsafe {
        SetNamedSecurityInfoW(
            wide(path).as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null_mut(),
        )
    } != 0
    {
        return Err(Error::Denied);
    }
    Ok(())
}
