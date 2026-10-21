//! Cached local evidence; Unknown never authorizes bulk backfill.
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Default)]
pub(super) struct Evidence {
    pub collect: bool,
    pub capacity: bool,
    pub boost: bool,
    pub upload: bool,
}
pub(super) struct Environment {
    #[cfg(target_os = "macos")]
    monitor: usize,
    next: Instant,
    evidence: Evidence,
}
impl Environment {
    #[cfg(test)]
    pub(super) fn force(&mut self, collect: bool, upload: bool) {
        self.evidence = Evidence {
            collect,
            upload,
            capacity: false,
            boost: true,
        };
        self.next = Instant::now() + Duration::from_secs(120);
    }
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            monitor: unsafe { rss_telemetry_environment_new() } as usize,
            next: Instant::now(),
            evidence: Evidence::default(),
        }
    }
    pub fn observe(&mut self, root: &Path) -> Evidence {
        if Instant::now() >= self.next {
            self.next = Instant::now() + Duration::from_secs(10);
            #[cfg(target_os = "macos")]
            {
                self.evidence = observe(self.monitor);
            }
            #[cfg(not(target_os = "macos"))]
            {
                self.evidence = observe();
            }
            // Reserve both existing journals' maximum 1 GiB allocations and additional headroom.
            self.evidence.capacity = !fs2::available_space(root)
                .is_ok_and(|n| n >= 2 * 1024 * 1024 * 1024 + 256 * 1024 * 1024);
            self.evidence.collect &= !self.evidence.capacity;
        }
        self.evidence
    }
}
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn rss_telemetry_environment_new() -> *mut std::ffi::c_void;
    fn rss_telemetry_environment(handle: *mut std::ffi::c_void) -> u32;
    fn rss_telemetry_environment_stop(handle: *mut std::ffi::c_void);
}
#[cfg(target_os = "macos")]
fn observe(handle: usize) -> Evidence {
    // SAFETY: process-local monitor is accessed only by the single service owner.
    let flags = unsafe { rss_telemetry_environment(handle as *mut _) };
    Evidence {
        capacity: false,
        collect: flags & 32 != 0 && flags & 16 == 0,
        boost: flags & 4 == 0,
        upload: flags & 3 == 3,
    }
}
#[cfg(windows)]
fn observe() -> Evidence {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = unsafe { std::mem::zeroed::<SYSTEM_POWER_STATUS>() };
    // SAFETY: writable fixed-size OS structure; no pointers retained by the API.
    let known = unsafe { GetSystemPowerStatus(&mut status) } != 0;
    // Windows cost observation is explicit; failure leaves uploads paused.
    let upload = windows_cost().unwrap_or(false);
    Evidence {
        capacity: false,
        collect: known
            && status.BatteryFlag != 255
            && (status.ACLineStatus == 1
                || status.BatteryLifePercent > 10 && status.BatteryLifePercent != 255),
        boost: known && status.SystemStatusFlag == 0,
        upload,
    }
}
#[cfg(windows)]
fn windows_cost() -> Option<bool> {
    use windows::Win32::{Networking::NetworkListManager::*, System::Com::*};
    // A dedicated COM apartment avoids changing the service thread's existing COM model.
    // At most one outstanding cost query process-wide; COM stalls cannot block reconciliation
    // or accumulate threads. Unknown cost keeps backfill paused.
    static QUERY: std::sync::Mutex<Option<std::sync::mpsc::Receiver<Option<bool>>>> =
        std::sync::Mutex::new(None);
    let mut query = QUERY.lock().ok()?;
    if let Some(rx) = query.as_ref() {
        return match rx.try_recv() {
            Ok(value) => {
                *query = None;
                value
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(_) => {
                *query = None;
                None
            }
        };
    }
    let (tx, rx) = std::sync::mpsc::channel();
    *query = Some(rx);
    std::thread::spawn(move || {
        let value = (|| unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()?;
            struct Apartment;
            impl Drop for Apartment {
                fn drop(&mut self) {
                    unsafe {
                        CoUninitialize();
                    }
                }
            }
            let _apartment = Apartment;
            let manager: INetworkListManager =
                CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).ok()?;
            use windows::core::Interface;
            if !manager.IsConnectedToInternet().ok()?.as_bool() {
                return Some(false);
            }
            let cost: INetworkCostManager = manager.cast().ok()?;
            let mut value = 0;
            cost.GetCost(&mut value, None).ok()?;
            Some(value == NLM_CONNECTION_COST_UNRESTRICTED.0 as u32)
        })();
        let _ = tx.send(value);
    });
    None
}
#[cfg(not(any(windows, target_os = "macos")))]
fn observe() -> Evidence {
    Evidence::default()
}
impl Drop for Environment {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        unsafe {
            rss_telemetry_environment_stop(self.monitor as *mut _);
        }
    }
}
