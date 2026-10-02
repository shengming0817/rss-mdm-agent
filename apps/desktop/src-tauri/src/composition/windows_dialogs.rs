use std::{io, os::windows::ffi::OsStrExt, path::PathBuf, ptr::null};
use windows_sys::Win32::{Foundation::*, Security::Credentials::*, UI::Controls::Dialogs::*};
fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
pub fn enter_enterprise_password(target: &str) -> io::Result<Option<String>> {
    let caption = wide("RSS 企业登录");
    let message = wide(target);
    let mut info: CREDUI_INFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of_val(&info) as u32;
    info.pszCaptionText = caption.as_ptr();
    info.pszMessageText = message.as_ptr();
    let mut user = vec![0u16; 256];
    user[..6].copy_from_slice(&wide("Secret")[..6]);
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
