//! Optional Windows 11 public AC/DC mode API, resolved from the system DLL only.
//! ref: Microsoft powrprof.h Power{Get,Set}UserConfigured{AC,DC}PowerMode.
use super::*;
use windows_sys::Win32::{Foundation::FreeLibrary, System::LibraryLoader::*};
struct Library(windows_sys::Win32::Foundation::HMODULE);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.0);
        }
    }
}
fn library() -> Result<Library, Error> {
    let value = unsafe {
        LoadLibraryExW(
            wide("powrprof.dll").as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if value.is_null() {
        Err(Error::Unsupported)
    } else {
        Ok(Library(value))
    }
}
fn guid(mode: &str) -> Option<windows_sys::core::GUID> {
    Some(windows_sys::core::GUID::from_u128(match mode {
        "efficiency" => 0x961cc777_2547_4f9d_8174_7d86181b8a7a,
        "balanced" => 0,
        "performance" => 0xded574b5_45a0_4f42_8737_46345c09c238,
        _ => return None,
    }))
}
fn same(a: &windows_sys::core::GUID, b: &windows_sys::core::GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}
pub(super) fn read(ac: bool) -> Result<String, Error> {
    let library = library()?;
    let name = if ac {
        c"PowerGetUserConfiguredACPowerMode"
    } else {
        c"PowerGetUserConfiguredDCPowerMode"
    };
    let address =
        unsafe { GetProcAddress(library.0, name.as_ptr().cast()) }.ok_or(Error::Unsupported)?;
    // SAFETY: the fixed export's signature is defined in Microsoft's powrprof.h.
    let get: unsafe extern "system" fn(*mut windows_sys::core::GUID) -> u32 =
        unsafe { std::mem::transmute(address) };
    let mut current = windows_sys::core::GUID::from_u128(0);
    if unsafe { get(&mut current) } != 0 {
        return Err(Error::Unsupported);
    }
    ["efficiency", "balanced", "performance"]
        .into_iter()
        .find(|id| same(&guid(id).expect("constant"), &current))
        .map(str::to_owned)
        .ok_or(Error::Unsupported)
}
pub(super) fn set(ac: bool, mode: &str) -> Result<(), Error> {
    read(ac)?;
    let guid = guid(mode).ok_or(Error::Unsupported)?;
    let library = library()?;
    let name = if ac {
        c"PowerSetUserConfiguredACPowerMode"
    } else {
        c"PowerSetUserConfiguredDCPowerMode"
    };
    let address =
        unsafe { GetProcAddress(library.0, name.as_ptr().cast()) }.ok_or(Error::Unsupported)?;
    let set: unsafe extern "system" fn(*const windows_sys::core::GUID) -> u32 =
        unsafe { std::mem::transmute(address) };
    if unsafe { set(&guid) } != 0 {
        return Err(Error::Denied);
    }
    Ok(())
}
