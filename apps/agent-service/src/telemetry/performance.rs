//! Local performance provider; remote native recipes remain backend-owned.
//! ref: GuillaumeGomez/sysinfo v0.37.2 src/unix/apple/cpu.rs and src/windows/network.rs.
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
#[derive(Clone, Debug)]
pub(crate) struct Cpu {
    pub boot: String,
    pub processors: u32,
    pub counters: Vec<u64>,
    pub busy: u64,
    pub total: u64,
}
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(windows)]
use windows as platform;

pub(crate) fn read() -> Result<Snapshot, Error> {
    let cpu = read_cpu()?;
    let (memory_total, memory_available) = read_memory()?;
    Ok(Snapshot {
        boot: cpu.boot,
        cpu_busy: cpu.busy,
        cpu_total: cpu.total,
        memory_total,
        memory_available,
        volumes: read_volumes()?,
        interfaces: read_interfaces()?,
    })
}
macro_rules! reader {
    ($name:ident, $native:ident, $output:ty) => {
        pub(crate) fn $name() -> Result<$output, Error> {
            #[cfg(any(windows, target_os = "macos"))]
            {
                platform::$native()
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                Err(Error::Unsupported)
            }
        }
    };
}
reader!(read_cpu, cpu, Cpu);
reader!(read_memory, memory, (u64, u64));
reader!(read_volumes, volumes, BTreeMap<String, Space>);
reader!(read_interfaces, interfaces, BTreeMap<String, Traffic>);

pub(crate) fn cpu_percent(previous: &Cpu, current: &Cpu) -> Option<f64> {
    if previous.boot != current.boot
        || previous.processors != current.processors
        || current.processors == 0
        || previous.counters.len() != current.counters.len()
        || current.counters.is_empty()
        || previous
            .counters
            .iter()
            .zip(&current.counters)
            .any(|(before, after)| after < before)
    {
        return None;
    }
    let total = current.total.checked_sub(previous.total)?;
    let busy = current.busy.checked_sub(previous.busy)?;
    if total == 0 || busy > total {
        return None;
    }
    Some(busy as f64 * 100.0 / total as f64)
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
/// Local diagnostic only. Two reads make warm-up explicit; no unsigned collection is uploaded.
pub(crate) fn probe() -> Result<serde_json::Value, Error> {
    let first = read_cpu()?;
    std::thread::sleep(std::time::Duration::from_millis(
        rss_mdm_agent_wire::runtime::PERFORMANCE_CPU_SAMPLE_MILLIS,
    ));
    let current = read_cpu()?;
    let second = read()?;
    Ok(
        serde_json::json!({"nativePerformanceEngines":crate::collection_worker::engines(),"cpuUsagePercent":cpu_percent(&first,&current),"snapshot":second}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample(boot: &str, busy: u64, total: u64) -> Cpu {
        Cpu {
            boot: boot.into(),
            processors: 1,
            counters: vec![busy, total],
            busy,
            total,
        }
    }
    #[test]
    fn cpu_requires_a_real_compatible_interval() {
        assert_eq!(
            cpu_percent(&sample("a", 10, 20), &sample("a", 20, 40)),
            Some(50.0)
        );
        for next in [
            sample("b", 20, 40),
            sample("a", 1, 40),
            sample("a", 20, 20),
            sample("a", 100, 40),
        ] {
            assert_eq!(cpu_percent(&sample("a", 10, 20), &next), None);
        }
    }
    #[test]
    fn cpu_rejects_domain_change_and_hidden_component_regression() {
        let previous = sample("a", 10, 20);
        let mut current = sample("a", 20, 40);
        current.processors = 2;
        assert_eq!(cpu_percent(&previous, &current), None);
        current.processors = 1;
        current.counters = vec![9, 40];
        assert_eq!(cpu_percent(&previous, &current), None);
        current.counters = vec![20];
        assert_eq!(cpu_percent(&previous, &current), None);
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
        std::thread::sleep(std::time::Duration::from_millis(
            rss_mdm_agent_wire::runtime::PERFORMANCE_CPU_SAMPLE_MILLIS,
        ));
        let b = read().unwrap();
        assert_eq!(a.boot, b.boot);
        let first = read_cpu().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(
            rss_mdm_agent_wire::runtime::PERFORMANCE_CPU_SAMPLE_MILLIS,
        ));
        let second = read_cpu().unwrap();
        assert!(cpu_percent(&first, &second).is_some_and(|v| (0.0..=100.0).contains(&v)));
    }
}
