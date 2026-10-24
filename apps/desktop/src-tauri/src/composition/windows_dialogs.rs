//! Native file selection; enterprise passwords use the product login form.
// ref: tauri-apps/plugins-workspace plugins/dialog/src/desktop.rs (v2).
use std::{io, os::windows::ffi::OsStrExt, path::PathBuf};
use windows_sys::Win32::UI::Controls::Dialogs::*;
fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
pub fn save_dialog() -> io::Result<Option<PathBuf>> {
    let mut file = vec![0u16; 32768];
    let name = wide("rss-diagnostics.json");
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

pub fn configuration_dialog() -> io::Result<Option<PathBuf>> {
    let mut file = vec![0u16; 32768];
    let filter = wide("JSON configuration\0*.json\0\0");
    let mut request: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    request.lStructSize = size_of_val(&request) as u32;
    request.lpstrFile = file.as_mut_ptr();
    request.nMaxFile = file.len() as u32;
    request.lpstrFilter = filter.as_ptr();
    request.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    if unsafe { GetOpenFileNameW(&mut request) } == 0 {
        return if unsafe { CommDlgExtendedError() } == 0 {
            Ok(None)
        } else {
            Err(io::Error::other("open dialog"))
        };
    }
    let len = file.iter().position(|v| *v == 0).unwrap_or(file.len());
    Ok(Some(PathBuf::from(
        String::from_utf16(&file[..len]).map_err(io::Error::other)?,
    )))
}
