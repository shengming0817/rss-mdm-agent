//! ABI from the pinned official V1 headers; every returned OS allocation has its matching free.
use super::*;
use std::sync::OnceLock;
use windows_sys::{
    core::GUID,
    Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32},
};
pub const ID: GUID = GUID::from_u128(0xf1fd3a7c_27c1_4f88_bf89_6ac6525c2929);
#[repr(C)]
pub struct Request {
    pub hwnd: HWND,
    pub transaction: GUID,
    pub signature_len: u32,
    pub signature: *const u8,
    pub encoding: u32,
    pub encoded_len: u32,
    pub encoded: *const u8,
}
#[repr(C)]
pub struct Response {
    pub len: u32,
    pub bytes: *mut u8,
}
#[repr(C)]
pub struct Cancel {
    pub transaction: GUID,
    pub signature_len: u32,
    pub signature: *const u8,
}
#[repr(C)]
pub struct Add {
    pub name: *const u16,
    pub clsid: *const GUID,
    pub rp: *const u16,
    pub light: *const u16,
    pub dark: *const u16,
    pub info_len: u32,
    pub info: *const u8,
    pub rp_count: u32,
    pub rps: *const *const u16,
}
#[repr(C)]
pub struct AddResponse {
    pub key_len: u32,
    pub key: *mut u8,
}
#[repr(C)]
pub struct Update {
    pub name: *const u16,
    pub clsid: *const GUID,
    pub new_clsid: *const GUID,
    pub light: *const u16,
    pub dark: *const u16,
    pub info_len: u32,
    pub info: *const u8,
    pub rp_count: u32,
    pub rps: *const *const u16,
}
#[repr(C)]
pub struct Credential {
    pub id_len: u32,
    pub id: *const u8,
    pub rp: *const u16,
    pub rp_name: *const u16,
    pub user_len: u32,
    pub user: *const u8,
    pub name: *const u16,
    pub display: *const u16,
}
#[repr(C)]
pub struct Uv {
    pub hwnd: HWND,
    pub transaction: *const GUID,
    pub username: *const u16,
    pub hint: *const u16,
}
pub struct Api {
    pub state: unsafe extern "system" fn(*const GUID, *mut u32) -> i32,
    pub add: unsafe extern "system" fn(*const Add, *mut *mut AddResponse) -> i32,
    pub free_add: unsafe extern "system" fn(*mut AddResponse),
    pub remove: unsafe extern "system" fn(*const GUID) -> i32,
    pub update: unsafe extern "system" fn(*const Update) -> i32,
    pub operation_key: unsafe extern "system" fn(*const GUID, *mut u32, *mut *mut u8) -> i32,
    pub uv_key: unsafe extern "system" fn(*const GUID, *mut u32, *mut *mut u8) -> i32,
    pub free_key: unsafe extern "system" fn(*mut u8),
    pub uv: unsafe extern "system" fn(*const Uv, *mut u32, *mut *mut u8) -> i32,
    pub free_uv: unsafe extern "system" fn(*mut u8),
    pub clear: unsafe extern "system" fn(*const GUID) -> i32,
    pub cache: unsafe extern "system" fn(*const GUID, u32, *const Credential) -> i32,
}
pub fn hr(value: i32) -> Result<()> {
    if value >= 0 {
        Ok(())
    } else if value as u32 == 0x80090036 {
        Err(Error::Cancelled)
    } else {
        Err(Error::SystemUnavailable)
    }
}
impl Api {
    pub fn get() -> Result<&'static Self> {
        static API: OnceLock<Result<Api>> = OnceLock::new();
        API.get_or_init(|| {
            // SAFETY: load only the system DLL; kept loaded for the process lifetime.
            unsafe {
                let module = LoadLibraryExW(
                    wide("webauthn.dll").as_ptr(),
                    std::ptr::null_mut(),
                    LOAD_LIBRARY_SEARCH_SYSTEM32,
                );
                if module.is_null() {
                    return Err(Error::Unsupported);
                }
                macro_rules! symbol {
                    ($name:literal,$ty:ty) => {{
                        let raw = GetProcAddress(module, concat!($name, "\0").as_ptr())
                            .ok_or(Error::Unsupported)?;
                        std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(raw)
                    }};
                }
                Ok(Api {
                    state: symbol!(
                        "WebAuthNPluginGetAuthenticatorState",
                        unsafe extern "system" fn(*const GUID, *mut u32) -> i32
                    ),
                    add: symbol!(
                        "WebAuthNPluginAddAuthenticator",
                        unsafe extern "system" fn(*const Add, *mut *mut AddResponse) -> i32
                    ),
                    free_add: symbol!(
                        "WebAuthNPluginFreeAddAuthenticatorResponse",
                        unsafe extern "system" fn(*mut AddResponse)
                    ),
                    remove: symbol!(
                        "WebAuthNPluginRemoveAuthenticator",
                        unsafe extern "system" fn(*const GUID) -> i32
                    ),
                    update: symbol!(
                        "WebAuthNPluginUpdateAuthenticatorDetails",
                        unsafe extern "system" fn(*const Update) -> i32
                    ),
                    operation_key: symbol!(
                        "WebAuthNPluginGetOperationSigningPublicKey",
                        unsafe extern "system" fn(*const GUID, *mut u32, *mut *mut u8) -> i32
                    ),
                    uv_key: symbol!(
                        "WebAuthNPluginGetUserVerificationPublicKey",
                        unsafe extern "system" fn(*const GUID, *mut u32, *mut *mut u8) -> i32
                    ),
                    free_key: symbol!(
                        "WebAuthNPluginFreePublicKeyResponse",
                        unsafe extern "system" fn(*mut u8)
                    ),
                    uv: symbol!(
                        "WebAuthNPluginPerformUserVerification",
                        unsafe extern "system" fn(*const Uv, *mut u32, *mut *mut u8) -> i32
                    ),
                    free_uv: symbol!(
                        "WebAuthNPluginFreeUserVerificationResponse",
                        unsafe extern "system" fn(*mut u8)
                    ),
                    clear: symbol!(
                        "WebAuthNPluginAuthenticatorRemoveAllCredentials",
                        unsafe extern "system" fn(*const GUID) -> i32
                    ),
                    cache: symbol!(
                        "WebAuthNPluginAuthenticatorAddCredentials",
                        unsafe extern "system" fn(*const GUID, u32, *const Credential) -> i32
                    ),
                })
            }
        })
        .as_ref()
        .map_err(|e| *e)
    }
    pub fn key(&self, uv: bool) -> Result<Vec<u8>> {
        let (mut len, mut ptr) = (0, std::ptr::null_mut());
        // SAFETY: OS output pointers live; copy before invoking the matching deallocator.
        unsafe {
            let status = if uv {
                (self.uv_key)(&ID, &mut len, &mut ptr)
            } else {
                (self.operation_key)(&ID, &mut len, &mut ptr)
            };
            let result = hr(status).and_then(|_| {
                if ptr.is_null() || !(8..=4096).contains(&len) {
                    Err(Error::Authentication)
                } else {
                    Ok(std::slice::from_raw_parts(ptr, len as usize).to_vec())
                }
            });
            if !ptr.is_null() {
                (self.free_key)(ptr);
            }
            result
        }
    }
    pub fn verify_user(&self, hwnd: HWND, transaction: &GUID, rp: &str) -> Result<Vec<u8>> {
        let hint = wide(&format!("RSS Passkey · {rp}"));
        let name = wide("");
        let request = Uv {
            hwnd,
            transaction,
            username: name.as_ptr(),
            hint: hint.as_ptr(),
        };
        let (mut len, mut ptr) = (0, std::ptr::null_mut());
        // SAFETY: request strings and transaction remain live for the synchronous OS call.
        unsafe {
            let status = (self.uv)(&request, &mut len, &mut ptr);
            let result = hr(status).and_then(|_| {
                if ptr.is_null() || !(1..=1024).contains(&len) {
                    Err(Error::Authentication)
                } else {
                    Ok(std::slice::from_raw_parts(ptr, len as usize).to_vec())
                }
            });
            if !ptr.is_null() {
                (self.free_uv)(ptr);
            }
            result
        }
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
#[test]
fn official_v1_x64_abi_layout() {
    use std::mem::{offset_of, size_of};
    assert_eq!(size_of::<Request>(), 56);
    assert_eq!(offset_of!(Request, encoded), 48);
    assert_eq!(size_of::<Response>(), 16);
    assert_eq!(size_of::<Cancel>(), 32);
    assert_eq!(size_of::<Add>(), 72);
    assert_eq!(size_of::<Update>(), 72);
    assert_eq!(offset_of!(Update, name), 0);
    assert_eq!(offset_of!(Update, clsid), 8);
    assert_eq!(offset_of!(Update, rps), 64);
    assert_eq!(size_of::<Credential>(), 64);
    assert_eq!(size_of::<Uv>(), 32);
}
