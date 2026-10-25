//! Asynchronous, reconstructible OS projection. No Vault transaction survives dispatch.
use passkey_vault::{Error, IndexSnapshot, Result, Vault};
use std::{
    ffi::{c_char, c_void, CString},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
};
static PUBLISHING: AtomicBool = AtomicBool::new(false);
unsafe extern "C" {
    fn rss_passkey_index_replace(
        json: *const c_char,
        context: *mut c_void,
        complete: extern "C" fn(*mut c_void, i32),
    );
    fn rss_passkey_index_state() -> i32;
}
struct Publication {
    vault: Weak<Vault>,
    snapshot: IndexSnapshot,
}
extern "C" fn complete(context: *mut c_void, success: i32) {
    // SAFETY: native completion invokes exactly once and owns this publication until then.
    let work = unsafe { Box::from_raw(context.cast::<Publication>()) };
    // Do not retain an unlocked Vault (and its root key) for an unbounded OS callback.
    // If its owner has exited, the preexisting dirty notice remains for the next activation.
    if let Some(vault) = work.vault.upgrade() {
        let result = if success == 1 {
            vault.acknowledge_index(&work.snapshot)
        } else {
            Err(Error::SystemUnavailable)
        };
        if result.is_err() {
            let _ = vault.invalidate_index();
        }
    }
    PUBLISHING.store(false, Ordering::Release);
}
pub fn refresh(vault: Arc<Vault>) -> Result<()> {
    if PUBLISHING.swap(true, Ordering::AcqRel) {
        return Err(Error::Busy);
    }
    let prepared = (|| {
        let snapshot = vault.index_snapshot()?;
        let json = CString::new(serde_json::to_vec(&snapshot.entries).map_err(|_| Error::Input)?)
            .map_err(|_| Error::Input)?;
        Ok((snapshot, json))
    })();
    let (snapshot, json) = match prepared {
        Ok(value) => value,
        Err(error) => {
            PUBLISHING.store(false, Ordering::Release);
            return Err(error);
        }
    };
    let context = Box::into_raw(Box::new(Publication {
        vault: Arc::downgrade(&vault),
        snapshot,
    }));
    // SAFETY: native copies JSON before returning and transfers context to exactly one completion.
    unsafe {
        rss_passkey_index_replace(json.as_ptr(), context.cast(), complete);
    }
    Ok(())
}
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Unknown,
    Disabled,
    Enabled,
}
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub availability: Availability,
    pub index_pending: bool,
}
pub fn status(vault: &Vault) -> Result<Status> {
    // SAFETY: native returns a cached state and schedules a nonblocking state refresh.
    let availability = match unsafe { rss_passkey_index_state() } {
        1 => Availability::Enabled,
        2 => Availability::Disabled,
        _ => Availability::Unknown,
    };
    Ok(Status {
        availability,
        index_pending: vault.index_pending()?,
    })
}
