//! Closed performance recipes execute inside the existing supervised native query worker.
//! ref: GuillaumeGomez/sysinfo v0.37.2 src/unix/apple/cpu.rs (monotonic CPU interval).
use super::values::{Row, Status};
use crate::telemetry::performance as native;
use agent_client::Error;
use rss_mdm_agent_wire::runtime::{PerformanceSection, PERFORMANCE_CPU_SAMPLE_MILLIS};
use std::time::{Duration, Instant};

fn status(error: Error) -> Status {
    match error {
        Error::Unsupported => Status::Unsupported,
        Error::Capacity => Status::Truncated,
        // A failed native API or inconsistent counter is absent evidence, never zero.
        _ => Status::Missing,
    }
}
fn check(until: Instant) -> Result<(), Status> {
    if Instant::now() >= until {
        Err(Status::Timeout)
    } else {
        Ok(())
    }
}
fn bytes(fields: &[(&str, u64)]) -> Row {
    fields
        .iter()
        .map(|(key, value)| ((*key).into(), value.to_string().into()))
        .collect()
}
pub(super) fn read(
    section: PerformanceSection,
    until: Instant,
    mut emit: impl FnMut(Row) -> Result<(), Status>,
) -> Result<(), Status> {
    check(until)?;
    match section {
        PerformanceSection::Cpu => {
            let first = native::read_cpu().map_err(status)?;
            // Start the interval after the first read so it cannot be shortened by API latency.
            let interval = Duration::from_millis(PERFORMANCE_CPU_SAMPLE_MILLIS);
            let began = Instant::now();
            if until.saturating_duration_since(began) <= interval {
                return Err(Status::Timeout);
            }
            while began.elapsed() < interval {
                std::thread::sleep(interval.saturating_sub(began.elapsed()));
            }
            check(until)?;
            let second = native::read_cpu().map_err(status)?;
            let value = native::cpu_percent(&first, &second).ok_or(Status::Missing)?;
            emit([("busy_percent".into(), value.into())].into())?;
        }
        PerformanceSection::Memory => {
            let (total, available) = native::read_memory().map_err(status)?;
            if total == 0 || available > total {
                return Err(Status::Missing);
            }
            emit(bytes(&[
                ("total_bytes", total),
                ("available_bytes", available),
            ]))?;
        }
        PerformanceSection::FileSystems => {
            for (id, space) in native::read_volumes().map_err(status)? {
                check(until)?;
                let mut row = bytes(&[
                    ("total_bytes", space.total_bytes),
                    ("free_bytes", space.free_bytes),
                ]);
                row.insert("id".into(), id.into());
                emit(row)?;
            }
        }
        PerformanceSection::Network => {
            for (id, traffic) in native::read_interfaces().map_err(status)? {
                check(until)?;
                let mut row = bytes(&[
                    ("received_bytes", traffic.received_bytes),
                    ("sent_bytes", traffic.sent_bytes),
                ]);
                row.insert("id".into(), id.into());
                emit(row)?;
            }
        }
    }
    check(until)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uint64_projection_is_exact_decimal_text() {
        let row = bytes(&[("received_bytes", u64::MAX), ("sent_bytes", 0)]);
        assert_eq!(row["received_bytes"], u64::MAX.to_string());
        assert_eq!(row["sent_bytes"], "0");
        let mut row = row;
        row.insert("id".into(), "native-interface".into());
        let definition = rss_mdm_agent_wire::runtime::NativeQueryDefinition::MacosPerformance {
            section: PerformanceSection::Network,
        };
        assert!(definition.accepts_row(&row));
    }
    #[test]
    fn performance_enumeration_rejects_duplicate_native_identity() {
        use rss_mdm_agent_wire::runtime as w;
        let definition = w::NativeQueryDefinition::MacosPerformance {
            section: PerformanceSection::Network,
        };
        let query = w::NativeQuery {
            version: definition.engine().contract_version(),
            definition,
            cardinality: w::NativeCardinality::List,
            instance_keys: vec!["id".into()],
            budget: w::NativeBudget {
                timeout_seconds: 1,
                output_bytes: 4096,
                max_rows: 4,
            },
        };
        let mut rows = super::super::values::Rows::new(&query, 1);
        let mut row = bytes(&[("received_bytes", 1), ("sent_bytes", 2)]);
        row.insert("id".into(), "native-interface".into());
        rows.push(row.clone()).unwrap();
        assert_eq!(rows.push(row), Err(Status::Invalid));
        let result = rows.finish(Err(Status::Invalid));
        assert!(!result.enumeration_complete);
        assert_eq!(result.status, Status::Invalid);
    }
    #[test]
    fn exhausted_budget_never_produces_rows() {
        for section in [
            PerformanceSection::Cpu,
            PerformanceSection::Memory,
            PerformanceSection::FileSystems,
            PerformanceSection::Network,
        ] {
            let result = read(section, Instant::now(), |_| {
                panic!("expired query emitted a row")
            });
            assert_eq!(result, Err(Status::Timeout));
        }
    }
}
