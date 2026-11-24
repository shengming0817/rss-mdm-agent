use super::{lock_slot, SharedVault};
use passkey_vault::{Error, Result};
use std::sync::Arc;
use zeroize::Zeroizing;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn rss_personal_vault_verify() -> i32;
    fn rss_personal_vault_cancel();
    fn rss_personal_vault_observe(
        callback: extern "C" fn(*mut std::ffi::c_void),
        context: *mut std::ffi::c_void,
    );
}
pub fn verify() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: Objective-C shim owns bounded UI wait; caller is a blocking worker.
        if unsafe { rss_personal_vault_verify() } == 1 {
            Ok(())
        } else {
            Err(Error::Authentication)
        }
    }
    #[cfg(windows)]
    {
        windows::verify()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err(Error::Unsupported)
    }
}
pub fn password(label: &str) -> Result<Zeroizing<String>> {
    #[cfg(target_os = "macos")]
    {
        super::super::secure_input::password("个人 Passkey Vault", label).ok_or(Error::Cancelled)
    }
    #[cfg(windows)]
    {
        windows::password(label)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = label;
        Err(Error::Unsupported)
    }
}
pub fn cancel() {
    #[cfg(target_os = "macos")]
    // SAFETY: shim dispatches UI cancellation to AppKit's main queue.
    unsafe {
        rss_personal_vault_cancel();
    }
    #[cfg(windows)]
    windows::cancel();
}
pub fn observe<R: tauri::Runtime>(_window: &tauri::WebviewWindow<R>, vault: SharedVault) {
    #[cfg(target_os = "macos")]
    {
        extern "C" fn locked(pointer: *mut std::ffi::c_void) {
            // SAFETY: registration retains one Arc for process lifetime; callback never releases it.
            let slot =
                unsafe { &*pointer.cast::<std::sync::Mutex<Option<Arc<passkey_vault::Vault>>>>() };
            lock_slot(slot);
        }
        // SAFETY: called once at native application setup. Observers and retained Arc live
        // until process exit; no dangling callback can outlive its Vault.
        unsafe {
            rss_personal_vault_observe(locked, Arc::into_raw(vault).cast_mut().cast());
        }
    }
    #[cfg(windows)]
    windows::observe(_window, vault);
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = vault;
}
