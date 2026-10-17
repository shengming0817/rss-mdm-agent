//! Local read-only Windows providers. No script, remote query, or provider installation.
use super::{
    values::{Row, Rows, Status},
    *,
};
use std::time::{Duration, Instant};
mod accounts;
mod certificates;
mod registry;
mod security;
mod software;
mod standard;
mod updates;
mod wmi;
struct Budget {
    start: Instant,
    timeout: Duration,
    output: usize,
}
impl Budget {
    fn check(&self) -> Result<(), Status> {
        if self.start.elapsed() >= self.timeout {
            Err(Status::Timeout)
        } else {
            Ok(())
        }
    }
}
pub(super) fn execute(input: &Input, q: &w::NativeQuery, deadline: i64) -> w::NativeQueryResult {
    let time = now().unwrap_or(1);
    let budget = Budget {
        output: q.budget.output_bytes as usize,
        start: Instant::now(),
        timeout: Duration::from_secs(
            u64::from(q.budget.timeout_seconds)
                .min(u64::try_from(deadline.saturating_sub(time)).unwrap_or(0)),
        ),
    };
    let mut rows = Rows::new(q, time);
    let status = (|| {
        q.validate().map_err(|_| Status::Invalid)?;
        budget.check()?;
        match &q.definition {
            w::NativeQueryDefinition::System { query } => match query {
                w::NativeSystemQuery::WindowsAccounts { .. }
                | w::NativeSystemQuery::WindowsJoin => accounts::read(query, &mut rows, &budget),
                w::NativeSystemQuery::WindowsCertificates { .. } => {
                    certificates::read(input, query, &mut rows, &budget)
                }
                w::NativeSystemQuery::WindowsUpdates { .. } => {
                    updates::read(query, &mut rows, &budget)
                }
                _ => security::read(query, &mut rows, &budget),
            },
            w::NativeQueryDefinition::WindowsWmi { .. } => {
                wmi::read(&q.definition, &budget, |row| rows.push(row))
            }
            w::NativeQueryDefinition::WindowsRegistry { .. } => {
                registry::read(&q.definition, &budget, |row| rows.push(row))
            }
            w::NativeQueryDefinition::WindowsSoftware => software::read(&mut rows, &budget),
            _ => standard::read(&q.definition, &budget, |row| rows.push(row)),
        }?;
        budget.check()
    })();
    rows.finish(status)
}
fn error(e: ::windows::core::Error) -> Status {
    eprintln!("native-provider HRESULT={:#010x}", e.code().0 as u32);
    match e.code().0 as u32 {
        0x80070005 | 0x80041003 => Status::Denied,
        0x80041002 => Status::Missing,
        0x8004100e | 0x80041010 | 0x80040154 => Status::Unsupported,
        0x80043001 | 0x800705b4 => Status::Timeout,
        _ => Status::Invalid,
    }
}
