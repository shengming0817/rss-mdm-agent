//! Frozen native scope evidence; resource closure never invents an installation outcome.
use execution_app::Error;
use execution_contract::{Id, ProcessScope};
use serde::{Deserialize, Serialize};

#[cfg(windows)]
pub(crate) const WINDOWS_JOB_ENV: &str = "RSS_NATIVE_JOB";
/// The original kernel boot and process scope, captured by the physical worker before effects.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeActivity {
    boot: Id,
    scope: ProcessScope,
}
impl NativeActivity {
    /// Capture the current runner-owned scope. Windows verifies actual kernel Job membership.
    pub fn current() -> Result<Self, Error> {
        #[cfg(target_os = "macos")]
        let scope = ProcessScope::ProcessGroup {
            owner: std::process::id(),
            group: unsafe { libc::getpgrp() } as u32,
        };
        #[cfg(windows)]
        let scope = crate::windows::current_activity()?;
        #[cfg(not(any(target_os = "macos", windows)))]
        let scope = return Err(Error::Unsupported);
        Ok(Self {
            boot: execution_ipc::host::boot_generation()?,
            scope,
        })
    }
    /// Read-only proof that the original process scope ended, including after kernel restart.
    /// Missing permission or unknown scope remains an error, never proof of closure.
    pub fn closed(&self) -> Result<bool, Error> {
        if self.boot != execution_ipc::host::boot_generation()? {
            return Ok(true);
        }
        match &self.scope {
            #[cfg(target_os = "macos")]
            ProcessScope::ProcessGroup { group, .. } => {
                let group = i32::try_from(*group).map_err(|_| Error::Unbound)?;
                if group <= 1 || group == unsafe { libc::getpgrp() } {
                    return Ok(false);
                }
                if unsafe { libc::kill(-group, 0) } == 0 {
                    return Ok(false);
                }
                match std::io::Error::last_os_error().raw_os_error() {
                    Some(libc::ESRCH) => Ok(true),
                    _ => Err(Error::Unavailable),
                }
            }
            #[cfg(windows)]
            ProcessScope::JobObject { name, session } => {
                crate::windows::activity_closed(name, *session)
            }
            _ => Err(Error::Unbound),
        }
    }
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn native_activity_retains_kernel_generation_and_refuses_a_live_group() {
        let activity = NativeActivity::current().unwrap();
        assert!(!activity.closed().unwrap());
        let bytes = serde_json::to_vec(&activity).unwrap();
        let mut recovered: NativeActivity = serde_json::from_slice(&bytes).unwrap();
        assert!(!recovered.closed().unwrap());
        recovered.boot = Id::new("previous-kernel-boot").unwrap();
        assert!(recovered.closed().unwrap());
        let unknown = NativeActivity {
            boot: execution_ipc::host::boot_generation().unwrap(),
            scope: ProcessScope::Preparing {},
        };
        assert!(unknown.closed().is_err());
    }
}
