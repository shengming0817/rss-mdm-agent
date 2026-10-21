//! Local performance provider; remote native recipes remain backend-owned.
//! ref: sysinfo v0.36.1 src/unix/apple/cpu.rs and src/windows/network.rs.
use agent_client::Error;
use serde::Serialize;
use std::collections::BTreeMap;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Snapshot {
    pub boot: String,
    pub cpu_busy: u64,
    pub cpu_total: u64,
    pub memory_total: u64,
    pub memory_available: u64,
    pub volumes: BTreeMap<String, Space>,
    pub interfaces: BTreeMap<String, Traffic>,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Space {
    pub total_bytes: u64,
    pub free_bytes: u64,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Traffic {
    pub received_bytes: u64,
    pub sent_bytes: u64,
}
pub(crate) fn read() -> Result<Snapshot, Error> {
    #[cfg(target_os = "macos")]
    {
        macos::read()
    }
    #[cfg(windows)]
    {
        windows::read()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err(Error::Unsupported)
    }
}
pub(crate) fn boot() -> Result<String, Error> {
    #[cfg(target_os = "macos")]
    {
        macos::boot()
    }
    #[cfg(windows)]
    {
        windows::boot()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err(Error::Unsupported)
    }
}
fn cpu(previous: &Snapshot, current: &Snapshot) -> Option<f64> {
    if previous.boot != current.boot {
        return None;
    }
    let total = current.cpu_total.checked_sub(previous.cpu_total)?;
    let busy = current.cpu_busy.checked_sub(previous.cpu_busy)?;
    if total == 0 || busy > total {
        return None;
    }
    Some(busy as f64 * 100.0 / total as f64)
}
/// Local diagnostic only. Two reads make warm-up explicit; no unsigned collection is uploaded.
pub(crate) fn probe() -> Result<serde_json::Value, Error> {
    let first = read()?;
    std::thread::sleep(std::time::Duration::from_millis(250));
    let second = read()?;
    Ok(
        serde_json::json!({"remoteNativeRecipe":"not_connected","cpuUsagePercent":cpu(&first,&second),"snapshot":second}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample(boot: &str, busy: u64, total: u64) -> Snapshot {
        Snapshot {
            boot: boot.into(),
            cpu_busy: busy,
            cpu_total: total,
            memory_total: 0,
            memory_available: 0,
            volumes: Default::default(),
            interfaces: Default::default(),
        }
    }
    #[test]
    fn cpu_requires_a_real_compatible_interval() {
        assert_eq!(cpu(&sample("a", 10, 20), &sample("a", 20, 40)), Some(50.0));
        for next in [
            sample("b", 20, 40),
            sample("a", 1, 40),
            sample("a", 20, 20),
            sample("a", 100, 40),
        ] {
            assert_eq!(cpu(&sample("a", 10, 20), &next), None);
        }
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_actual_performance_is_finite_and_bounded() {
        let a = read().unwrap();
        assert!(a.memory_total > 0);
        assert!(a.memory_available <= a.memory_total);
        assert!(!a.volumes.is_empty());
        assert!(a.volumes.values().all(|v| v.free_bytes <= v.total_bytes));
        assert!(a.interfaces.len() <= 128);
        std::thread::sleep(std::time::Duration::from_millis(250));
        let b = read().unwrap();
        assert_eq!(a.boot, b.boot);
        assert!(cpu(&a, &b).is_some_and(|v| (0.0..=100.0).contains(&v)));
    }
}
