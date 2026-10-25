use macos_passkey_provider::{Request, Session};
use passkey_vault::{Error, Result, UnlockMethod, Vault};
use std::{
    ffi::{c_char, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
pub struct Handle {
    session: Session,
    approval: Arc<AtomicBool>,
}
fn error(error: Error) -> i32 {
    match error {
        Error::Cancelled => 1,
        Error::NotFound => 2,
        Error::Locked | Error::Authentication => 3,
        Error::Busy | Error::Conflict => 4,
        _ => 5,
    }
}
fn boundary(f: impl FnOnce() -> Result<()>) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => error(e),
        Err(_) => 5,
    }
}
/// # Safety
/// Input is readable for length bytes, output is writable. Native callers retain the handle.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_begin(
    input: *const u8,
    length: usize,
    output: *mut *mut Handle,
) -> i32 {
    if input.is_null() || output.is_null() || length > 1024 * 1024 {
        return 5;
    }
    *output = std::ptr::null_mut();
    boundary(|| {
        let request = Request::decode(std::slice::from_raw_parts(input, length))?;
        let approval = Arc::new(AtomicBool::new(false));
        let grant = approval.clone();
        let protection = Arc::new(passkey_vault::platform::OsProtection::new(move || {
            if grant.swap(false, Ordering::AcqRel) {
                Ok(())
            } else {
                Err(Error::Authentication)
            }
        }));
        let vault = Arc::new(Vault::open(
            &passkey_vault::platform::default_root()?,
            protection,
        )?);
        let session = Session::new(request, vault)?;
        *output = Box::into_raw(Box::new(Handle { session, approval }));
        Ok(())
    })
}
/// # Safety
/// Handle is live; password is readable for length bytes, only on the native secure-input path.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_unlock(
    handle: *mut Handle,
    verified: i32,
    password: *const u8,
    length: usize,
) -> i32 {
    if handle.is_null() || length > 1024 {
        return 5;
    }
    boundary(|| {
        let handle = &*handle;
        if password.is_null() {
            if verified != 1 {
                return Err(Error::Authentication);
            }
            handle.approval.store(true, Ordering::Release);
            handle.session.unlock(UnlockMethod::System, None)
        } else {
            let bytes =
                zeroize::Zeroizing::new(std::slice::from_raw_parts(password, length).to_vec());
            let text = std::str::from_utf8(&bytes).map_err(|_| Error::Input)?;
            handle.session.unlock(UnlockMethod::Password, Some(text))
        }
    })
}
/// # Safety
/// Live handle and writable output. Caller frees returned JSON using rss_passkey_free_text.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_accounts(
    handle: *mut Handle,
    output: *mut *mut c_char,
) -> i32 {
    if handle.is_null() || output.is_null() {
        return 5;
    }
    *output = std::ptr::null_mut();
    boundary(|| {
        let entries = (*handle).session.accounts()?;
        *output = CString::new(serde_json::to_vec(&entries).map_err(|_| Error::Input)?)
            .map_err(|_| Error::Input)?
            .into_raw();
        Ok(())
    })
}
/// # Safety
/// Live handle, optional readable credential ID, writable output. Exactly one execution per handle.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_execute(
    handle: *mut Handle,
    id: *const u8,
    length: usize,
    output: *mut *mut c_char,
) -> i32 {
    if handle.is_null() || output.is_null() || length > 1023 {
        return 5;
    }
    *output = std::ptr::null_mut();
    boundary(|| {
        let selected = if id.is_null() {
            None
        } else {
            Some(std::slice::from_raw_parts(id, length).to_vec())
        };
        let response = (*handle).session.execute(selected)?;
        // Cache failure cannot undo a credential already committed to Vault.
        let _ = macos_passkey_provider::index::refresh((*handle).session.vault().clone());
        *output = CString::new(serde_json::to_vec(&response).map_err(|_| Error::Input)?)
            .map_err(|_| Error::Input)?
            .into_raw();
        Ok(())
    })
}
/// # Safety
/// The handle must remain live throughout this call; concurrent cancellation is supported.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_check(handle: *mut Handle) -> i32 {
    if handle.is_null() {
        return 5;
    }
    boundary(|| {
        (*handle).session.check()?;
        if !(*handle).session.vault().status()?.unlocked {
            return Err(Error::Cancelled);
        }
        Ok(())
    })
}
/// # Safety
/// Handle is live and retained until all outstanding workers finish.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_cancel(handle: *mut Handle) {
    if let Some(handle) = handle.as_ref() {
        handle.session.cancel();
    }
}
/// # Safety
/// The unique handle is no longer referenced by workers or callbacks.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_free(handle: *mut Handle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}
/// # Safety
/// Pointer was returned by this bridge and has not been freed.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_free_text(text: *mut c_char) {
    if !text.is_null() {
        drop(CString::from_raw(text));
    }
}
// Native ABI probes can validate the real parser without creating or unlocking a Vault.
/// # Safety
/// text is a NUL-terminated readable string of bounded test input.
#[no_mangle]
pub unsafe extern "C" fn rss_passkey_validate(text: *const c_char) -> i32 {
    if text.is_null() {
        return 5;
    }
    boundary(|| Request::decode(CStr::from_ptr(text).to_bytes()).map(|_| ()))
}
