use super::*;
use std::{
    ffi::c_void,
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::PathBuf,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, Credentials::*, Cryptography::*, *},
    Storage::FileSystem::*,
    System::{Com::CoTaskMemFree, Threading::*},
    UI::{
        Controls::Dialogs::*,
        Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath},
    },
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
    if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
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
            if header.AceFlags & 0x08 != 0 || header.AceType == 1 {
                continue;
            }
            if header.AceType != 0 {
                return Err(io::Error::other("unsupported private ACE"));
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
pub fn validate(path: &Path) -> io::Result<()> {
    let handle = OpenOptions::new()
        .read(true)
        .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    file(&handle)
}
pub fn directory(path: &Path) -> io::Result<()> {
    if path.exists() {
        return validate(path);
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("parent required"))?;
    if !parent.exists() {
        directory(parent)?;
    }
    validate(parent)?;
    let descriptor = descriptor()?;
    let attrs = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    if unsafe { CreateDirectoryW(wide(path).as_ptr(), &attrs) } == 0 {
        return Err(io::Error::last_os_error());
    }
    validate(path)
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
pub fn key_path() -> io::Result<PathBuf> {
    unsafe {
        let mut path = null_mut();
        if SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, null_mut(), &mut path) < 0 {
            return Err(io::Error::other("local app data"));
        }
        let mut len = 0;
        while *path.add(len) != 0 && len < 32768 {
            len += 1;
        }
        let text = String::from_utf16(std::slice::from_raw_parts(path, len));
        CoTaskMemFree(path.cast());
        let root = PathBuf::from(text.map_err(io::Error::other)?)
            .join("RSS MDM Agent")
            .join("private");
        directory(&root)?;
        Ok(root.join("connection-master-key.dpapi"))
    }
}
fn crypt(bytes: &[u8], protect: bool) -> io::Result<Vec<u8>> {
    if bytes.len() > 65536 {
        return Err(io::Error::other("key size"));
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let result = unsafe {
        if protect {
            CryptProtectData(
                &input,
                null(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                null_mut(),
                null(),
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    let _output = Local(output.pbData.cast());
    let value =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
    }
    Ok(value)
}
pub fn protect_key(bytes: &[u8]) -> io::Result<Vec<u8>> {
    crypt(bytes, true)
}
pub fn unprotect_key(bytes: &[u8]) -> io::Result<Vec<u8>> {
    crypt(bytes, false)
}
pub fn random_key() -> io::Result<Vec<u8>> {
    let mut key = vec![0; 32];
    if unsafe {
        BCryptGenRandom(
            null_mut(),
            key.as_mut_ptr(),
            32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } != 0
    {
        return Err(io::Error::other("system RNG"));
    }
    Ok(key)
}
pub fn enter_secret() -> io::Result<Option<String>> {
    let caption = wide("RSS API 密钥");
    let message = wide("输入当前连接的 API 密钥。取消不会保存。");
    let mut info: CREDUI_INFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of_val(&info) as u32;
    info.pszCaptionText = caption.as_ptr();
    info.pszMessageText = message.as_ptr();
    let mut user = vec![0u16; 256];
    user[..7].copy_from_slice(&wide("API key")[..7]);
    let mut password = vec![0u16; 1024];
    let mut save = 0;
    let result = unsafe {
        CredUIPromptForCredentialsW(
            &info,
            wide("RSS custom API").as_ptr(),
            null(),
            0,
            user.as_mut_ptr(),
            user.len() as u32,
            password.as_mut_ptr(),
            password.len() as u32,
            &mut save,
            CREDUI_FLAGS_GENERIC_CREDENTIALS
                | CREDUI_FLAGS_ALWAYS_SHOW_UI
                | CREDUI_FLAGS_DO_NOT_PERSIST
                | CREDUI_FLAGS_EXCLUDE_CERTIFICATES
                | CREDUI_FLAGS_KEEP_USERNAME,
        )
    };
    let value = if result == ERROR_CANCELLED {
        Ok(None)
    } else if result != 0 {
        Err(io::Error::from_raw_os_error(result as i32))
    } else {
        let len = password
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(password.len());
        String::from_utf16(&password[..len])
            .map(Some)
            .map_err(io::Error::other)
    };
    password.fill(0);
    value
}
pub fn save_dialog() -> io::Result<Option<PathBuf>> {
    let mut file = vec![0u16; 32768];
    let name = wide("rss-ai-diagnostics.json");
    file[..name.len()].copy_from_slice(&name);
    let mut request: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    request.lStructSize = size_of_val(&request) as u32;
    request.lpstrFile = file.as_mut_ptr();
    request.nMaxFile = file.len() as u32;
    request.Flags = OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;
    if unsafe { GetSaveFileNameW(&mut request) } == 0 {
        return if unsafe { CommDlgExtendedError() } == 0 {
            Ok(None)
        } else {
            Err(io::Error::other("save dialog"))
        };
    }
    let len = file.iter().position(|c| *c == 0).unwrap_or(file.len());
    Ok(Some(PathBuf::from(
        String::from_utf16(&file[..len]).map_err(io::Error::other)?,
    )))
}
