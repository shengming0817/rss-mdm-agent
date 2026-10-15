//! Fixed native collection worker. The service alone owns authorization and persistence.
//! ref: microsoft/windows-rs@0.61.0 Win32/System/Wmi and Management/Deployment.
use agent_client::Error;
use execution_contract::{Digest, Platform, RunAs, SessionRequirement};
use rss_mdm_agent_wire::runtime as w;
use serde::{Deserialize, Serialize};
#[cfg(any(windows, test))]
use std::collections::BTreeMap;
#[cfg(windows)]
use std::io::Write;
use std::path::Path;
#[cfg(windows)]
mod supervisor;
#[cfg(test)]
mod tests;
#[cfg(any(windows, test))]
mod values;
#[cfg(windows)]
mod windows;

pub(crate) const PROFILE: &str = "native-collection-worker";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub task: w::CollectionTaskSpec,
    pub finish_by_unix_ms: u64,
    pub image_sha256: Digest,
}
pub(crate) fn engines() -> w::NativeEngines {
    #[cfg(windows)]
    {
        use w::NativeEngine::*;
        [
            WindowsWmi,
            WindowsRegistry,
            WindowsCpu,
            WindowsSoftware,
            WindowsNetwork,
            WindowsBattery,
        ]
        .into_iter()
        .map(|e| (e, 1))
        .collect()
    }
    #[cfg(not(windows))]
    {
        Default::default()
    }
}
fn now() -> Result<i64, Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Clock)?
        .as_secs()
        .try_into()
        .map_err(|_| Error::Clock)
}
pub(crate) fn failure(
    q: &w::NativeQuery,
    status: w::NativeQueryStatus,
    time: i64,
) -> w::NativeQueryResult {
    w::NativeQueryResult {
        query_digest: q.digest(),
        collected_at: time,
        status,
        enumeration_complete: false,
        row_count: 0,
        rows: vec![],
        users: None,
    }
}
fn diagnostic_limit(p: &w::NativeCollectionPlan) -> usize {
    p.spec().queries.len() * 128
}
fn empty_result(p: &w::NativeCollectionPlan, time: i64) -> w::NativeCollectionResult {
    w::NativeCollectionResult {
        plan_digest: p.digest(),
        collected_at: time,
        queries: p
            .spec()
            .queries
            .iter()
            .map(|(id, q)| (id.clone(), failure(q, w::NativeQueryStatus::Timeout, time)))
            .collect(),
    }
}
/// Reserve enough room for every query's bounded failure evidence before accepting Start.
pub(crate) fn preflight(p: &w::NativeCollectionPlan) -> Result<(), Error> {
    let result = empty_result(p, i64::MAX);
    // Unsupported is the longest status name; row_count can grow to five digits.
    let mut reserve = result;
    for r in reserve.queries.values_mut() {
        r.status = w::NativeQueryStatus::Unsupported;
    }
    if serde_json::to_vec(&reserve)
        .map_err(|_| Error::Protocol)?
        .len()
        .saturating_add(diagnostic_limit(p))
        > p.spec().budget.output_bytes as usize
    {
        return Err(Error::Capacity);
    }
    for (id, q) in &p.spec().queries {
        if serde_json::to_vec(&reserve.queries[id])
            .map_err(|_| Error::Protocol)?
            .len()
            > q.budget.output_bytes as usize
        {
            return Err(Error::Capacity);
        }
    }
    Ok(())
}
/// Internal entry, never a network-selected executable or a second execution authority.
pub fn run(path: &Path, query: Option<&str>) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 256 * 1024)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    input.task.validate()?;
    preflight(&input.task.plan)?;
    if !input.task.plan.supported_by(&engines()) {
        return Err(Error::Unsupported);
    }
    execution_runner::staging::verify_worker_identity(
        &RunAs::System {
            platform: Platform::Windows,
        },
        &SessionRequirement::NotRequired {},
    )
    .map_err(crate::error::app_error)?;
    let image = std::env::current_exe()?;
    let _lease = execution_runner::staging::verify_retained(&image, &input.image_sha256)
        .map_err(crate::error::app_error)?;
    if input.task.permit != w::TaskPermit::Start || now()? >= input.task.deadline {
        return Err(Error::Expired);
    }
    #[cfg(windows)]
    let output = if let Some(id) = query {
        let q = input
            .task
            .plan
            .spec()
            .queries
            .get(id)
            .ok_or(Error::Untrusted)?;
        let result = windows::execute(q, input.task.deadline);
        serde_json::to_vec(&result).map_err(|_| Error::Protocol)?
    } else {
        let result = collect(
            &input.task.plan,
            input.task.deadline,
            input.finish_by_unix_ms,
            |id, q, remaining| supervisor::query(path, &input, id, q, remaining),
        )?;
        serde_json::to_vec(&result).map_err(|_| Error::Protocol)?
    };
    #[cfg(not(windows))]
    {
        let _ = query;
        Err(Error::Unsupported)
    }
    #[cfg(windows)]
    {
        std::io::stdout().lock().write_all(&output)?;
        Ok(0)
    }
}
#[cfg(any(windows, test))]
fn collect(
    plan: &w::NativeCollectionPlan,
    deadline: i64,
    finish_by_unix_ms: u64,
    mut execute: impl FnMut(
        &str,
        &w::NativeQuery,
        std::time::Duration,
    ) -> Result<w::NativeQueryResult, Error>,
) -> Result<w::NativeCollectionResult, Error> {
    preflight(plan)?;
    // Compilation precedes runner dispatch. This conservative common cutoff includes
    // staging, process startup and worker initialization without extending authorization.
    let wall_ms = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Clock)?
            .as_millis(),
    )
    .map_err(|_| Error::Clock)?;
    let available = std::time::Duration::from_millis(
        finish_by_unix_ms
            .saturating_sub(wall_ms)
            .saturating_sub(250),
    )
    .min(std::time::Duration::from_secs(u64::from(
        plan.spec().budget.timeout_seconds,
    )));
    let started = std::time::Instant::now();
    let mut result = empty_result(plan, now()?);
    let mut cache: BTreeMap<[u8; 32], w::NativeQueryResult> = BTreeMap::new();
    for (id, q) in &plan.spec().queries {
        let time = now()?;
        if time >= deadline || started.elapsed() >= available {
            break;
        }
        let remaining = available.saturating_sub(started.elapsed());
        let mut value = if let Some(value) = cache.get(&q.digest()) {
            value.clone()
        } else {
            let value = execute(id, q, remaining)?;
            value
        };
        if value.query_digest != q.digest() || value.collected_at < 1 || value.collected_at > now()?
        {
            value = failure(q, w::NativeQueryStatus::Invalid, time);
        }
        if value.status == w::NativeQueryStatus::Complete && !value.complete_for(q, now()?) {
            value.status = w::NativeQueryStatus::Invalid;
            value.enumeration_complete = false;
        }
        if serde_json::to_vec(&value)
            .map_err(|_| Error::Protocol)?
            .len()
            > q.budget.output_bytes as usize
        {
            value = failure(q, w::NativeQueryStatus::Truncated, time);
        }
        result.collected_at = now()?;
        result.queries.insert(id.clone(), value);
        let rows: usize = result.queries.values().map(|r| r.rows.len()).sum();
        if rows > usize::from(plan.spec().budget.max_rows)
            || serde_json::to_vec(&result)
                .map_err(|_| Error::Protocol)?
                .len()
                > (plan.spec().budget.output_bytes as usize).saturating_sub(diagnostic_limit(plan))
        {
            result.queries.insert(
                id.clone(),
                failure(q, w::NativeQueryStatus::Truncated, time),
            );
        }
        // Retain only accepted, bounded evidence: cached providers share the task output budget.
        cache.insert(q.digest(), result.queries[id].clone());
    }
    result.collected_at = now()?;
    result.validate_for(plan, result.collected_at)?;
    Ok(result)
}

#[cfg(any(windows, test))]
fn decode_query(q: &w::NativeQuery, bytes: &[u8], time: i64) -> w::NativeQueryResult {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|_| failure(q, w::NativeQueryStatus::Invalid, time))
}
