//! Fixed native collection worker. The service alone owns authorization and persistence.
//! ref: microsoft/windows-rs@0.61.0 Win32/System/Wmi and Management/Deployment.
use agent_client::Error;
use execution_contract::{
    CollectionCompletion, Digest, RunAs, SessionRequirement, SoftwareMaterial,
};
use rss_mdm_agent_wire::runtime as w;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_supervisor;
#[cfg(windows)]
mod supervisor;
#[cfg(any(windows, test))]
mod system_values;
#[cfg(test)]
mod tests;
#[cfg(any(windows, target_os = "macos", test))]
mod values;
#[cfg(windows)]
mod windows;

pub(crate) const PROFILE: &str = "native-collection-worker";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub read: ReadPlan,
    pub finish_by_unix_ms: u64,
    pub image_sha256: Digest,
    pub run_as: RunAs,
    pub session: SessionRequirement,
    pub tools: BTreeMap<String, SoftwareMaterial>,
    #[serde(skip)]
    pub observed: std::sync::Arc<std::sync::atomic::AtomicU64>,
}
/// Physical read instructions shared by verified commands and signed periodic leases.
/// Remote authorization is verified by the service before publishing this protected artifact.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadPlan {
    pub plan: w::NativeCollectionPlan,
    pub execution: w::NativeExecutionBinding,
    pub platform: w::TaskPlatform,
    pub deadline: i64,
}
impl From<w::CollectionTaskSpec> for ReadPlan {
    fn from(task: w::CollectionTaskSpec) -> Self {
        Self {
            plan: task.plan,
            execution: task.execution,
            platform: task.platform,
            deadline: task.deadline,
        }
    }
}
pub(crate) fn engines() -> w::NativeEngines {
    #[cfg(windows)]
    {
        use w::NativeEngine::*;
        [
            WindowsSecurity,
            WindowsAccounts,
            WindowsCertificates,
            WindowsUpdates,
            WindowsWmi,
            WindowsRegistry,
            WindowsCpu,
            WindowsSoftware,
            WindowsNetwork,
            WindowsBattery,
        ]
        .into_iter()
        .map(|e| (e, e.contract_version()))
        .collect()
    }
    #[cfg(target_os = "macos")]
    {
        [
            w::NativeEngine::MacosSecurity,
            w::NativeEngine::MacosAccounts,
            w::NativeEngine::MacosCertificates,
            w::NativeEngine::MacosUpdates,
            w::NativeEngine::MacosHardware,
            w::NativeEngine::MacosApplications,
            w::NativeEngine::MacosPkgReceipts,
            w::NativeEngine::MacosBrew,
        ]
        .into_iter()
        .map(|e| (e, e.contract_version()))
        .collect()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
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
        scan: match &q.definition {
            w::NativeQueryDefinition::System { query } => {
                query.scan_mode().map(|mode| w::NativeScanEvidence {
                    mode,
                    source: None,
                    started_at: None,
                    finished_at: None,
                })
            }
            _ => None,
        },
    }
}
/// Worst-case failure envelope used only to reserve capacity, never as observed evidence.
fn failure_reserve(q: &w::NativeQuery, time: i64) -> w::NativeQueryResult {
    let mut result = failure(q, w::NativeQueryStatus::SourceUnavailable, time);
    if let Some(scan) = &mut result.scan {
        scan.source = Some("s".repeat(255));
        scan.started_at = Some(time);
        scan.finished_at = Some(time);
    }
    result
}
#[cfg(any(windows, target_os = "macos", test))]
fn truncated(q: &w::NativeQuery, original: &w::NativeQueryResult) -> w::NativeQueryResult {
    let mut result = failure(q, w::NativeQueryStatus::Truncated, original.collected_at);
    if original.evidence_matches(q) {
        result.scan = original.scan.clone();
    }
    result
}
fn diagnostic_limit(p: &w::NativeCollectionPlan) -> usize {
    512 + p.spec().queries.len() * 128
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
    let mut reserve = result;
    for (id, q) in &p.spec().queries {
        reserve
            .queries
            .insert(id.clone(), failure_reserve(q, i64::MAX));
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
    #[cfg(target_os = "macos")]
    if query.is_some() {
        macos_supervisor::confine_query()?;
    }
    let bytes = execution_runner::staging::read_worker_input(path, 256 * 1024)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    input.read.execution.validate(&input.read.plan)?;
    preflight(&input.read.plan)?;
    if !input.read.plan.supported_by(&engines()) {
        return Err(Error::Unsupported);
    }
    verify_identity(&input)?;
    let image = std::env::current_exe()?;
    let _lease = execution_runner::staging::verify_retained(&image, &input.image_sha256)
        .map_err(crate::error::app_error)?;
    if now()? >= input.read.deadline {
        return Err(Error::Expired);
    }
    #[cfg(any(windows, target_os = "macos"))]
    {
        let output = if let Some(id) = query {
            let q = input
                .read
                .plan
                .spec()
                .queries
                .get(id)
                .ok_or(Error::Untrusted)?;
            #[cfg(windows)]
            let result = windows::execute(&input, q, input.read.deadline);
            #[cfg(target_os = "macos")]
            let result = macos::execute(&input, q)?;
            serde_json::to_vec(&result).map_err(|_| Error::Protocol)?
        } else {
            let result = collect(
                &input.read.plan,
                input.read.deadline,
                input.finish_by_unix_ms,
                &input.observed,
                |id, q, remaining, raw_allowance| {
                    #[cfg(windows)]
                    {
                        let _ = raw_allowance;
                        supervisor::query(path, &input, id, q, remaining)
                    }
                    #[cfg(target_os = "macos")]
                    {
                        macos_supervisor::query(&bytes, &input, id, q, remaining, raw_allowance)
                    }
                },
            )?;
            serde_json::to_vec(&result).map_err(|_| Error::Protocol)?
        };
        verify_identity(&input)?;
        let completion = CollectionCompletion {
            input_sha256: hex(&bytes),
            output_sha256: hex(&output),
            closed: true,
            native_output_bytes: input.observed.load(std::sync::atomic::Ordering::Relaxed),
        };
        let control = serde_json::to_vec(&completion).map_err(|_| Error::Protocol)?;
        if query.is_none()
            && output
                .len()
                .saturating_add(control.len())
                .saturating_add(completion.native_output_bytes as usize)
                > input.read.plan.spec().budget.output_bytes as usize
        {
            return Err(Error::Capacity);
        }
        std::io::stdout().lock().write_all(&output)?;
        std::io::stderr().lock().write_all(&control)?;
        Ok(0)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = query;
        Err(Error::Unsupported)
    }
}
fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn verify_identity(input: &Input) -> Result<(), Error> {
    verify_identity_binding(input)?;
    execution_runner::staging::verify_worker_identity(&input.run_as, &input.session)
        .map_err(crate::error::app_error)
}
fn verify_identity_binding(input: &Input) -> Result<(), Error> {
    input.read.execution.validate(&input.read.plan)?;
    let valid = match (&input.read.execution.target, &input.run_as, &input.session) {
        (
            w::SoftwareExecutionTarget::Device,
            RunAs::System { platform },
            SessionRequirement::NotRequired {},
        ) => matches!(
            (input.read.platform, platform),
            (w::TaskPlatform::Macos, execution_contract::Platform::Macos)
                | (
                    w::TaskPlatform::Windows,
                    execution_contract::Platform::Windows
                )
        ),
        (
            w::SoftwareExecutionTarget::User { identity, .. },
            RunAs::User { account },
            SessionRequirement::ActiveUser {
                account: active, ..
            },
        ) => {
            account == active
                && account.subject.as_str() == identity
                && match (input.read.platform, account.platform) {
                    (w::TaskPlatform::Windows, execution_contract::Platform::Windows) => {
                        !matches!(identity.as_str(), "S-1-5-18" | "S-1-5-19" | "S-1-5-20")
                    }
                    (w::TaskPlatform::Macos, execution_contract::Platform::Macos) => {
                        identity != "0"
                            && crate::account_identity::generated_uid(identity)
                                .ok()
                                .flatten()
                                == input.read.execution.generated_uid
                    }
                    _ => false,
                }
        }
        _ => false,
    };
    if !valid {
        return Err(Error::Untrusted);
    }
    Ok(())
}
pub(crate) fn tools(
    config: &crate::ExecutionConfig,
    plan: &w::NativeCollectionPlan,
) -> Result<BTreeMap<String, SoftwareMaterial>, Error> {
    #[cfg(target_os = "macos")]
    {
        macos::tools(config, plan)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (config, plan);
        Ok(BTreeMap::new())
    }
}

#[cfg(any(windows, target_os = "macos", test))]
fn collect(
    plan: &w::NativeCollectionPlan,
    deadline: i64,
    finish_by_unix_ms: u64,
    observed: &std::sync::atomic::AtomicU64,
    mut execute: impl FnMut(
        &str,
        &w::NativeQuery,
        std::time::Duration,
        usize,
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
        // Reserve this query's final bounded failure before lending bytes to its provider.
        result.queries.insert(
            id.clone(),
            failure(q, w::NativeQueryStatus::Truncated, time),
        );
        let used = usize::try_from(observed.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(usize::MAX);
        let mut reserve = result.clone();
        for (pending_id, pending_query) in plan.spec().queries.range(id.clone()..) {
            reserve
                .queries
                .insert(pending_id.clone(), failure_reserve(pending_query, i64::MAX));
        }
        let raw_allowance = (plan.spec().budget.output_bytes as usize)
            .saturating_sub(
                serde_json::to_vec(&reserve)
                    .map_err(|_| Error::Protocol)?
                    .len(),
            )
            .saturating_sub(diagnostic_limit(plan))
            .saturating_sub(used);
        let mut value = if let Some(value) = cache.get(&q.digest()) {
            value.clone()
        } else if raw_allowance == 0 {
            failure(q, w::NativeQueryStatus::Truncated, time)
        } else {
            execute(id, q, remaining, raw_allowance)?
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
            value = truncated(q, &value);
        }
        result.collected_at = now()?;
        result.queries.insert(id.clone(), value);
        let rows: usize = result.queries.values().map(|r| r.rows.len()).sum();
        if rows > usize::from(plan.spec().budget.max_rows)
            || serde_json::to_vec(&result)
                .map_err(|_| Error::Protocol)?
                .len()
                .saturating_add(
                    usize::try_from(observed.load(std::sync::atomic::Ordering::Relaxed))
                        .unwrap_or(usize::MAX),
                )
                .saturating_add(diagnostic_limit(plan))
                > plan.spec().budget.output_bytes as usize
        {
            let failure = truncated(q, &result.queries[id]);
            result.queries.insert(id.clone(), failure);
        }
        // Retain only accepted, bounded evidence: cached providers share the task output budget.
        cache.insert(q.digest(), result.queries[id].clone());
    }
    result.collected_at = now()?;
    result.validate_for(plan, result.collected_at)?;
    Ok(result)
}

#[cfg(any(windows, target_os = "macos", test))]
fn decode_query(q: &w::NativeQuery, bytes: &[u8], time: i64) -> w::NativeQueryResult {
    serde_json::from_slice(bytes)
        .unwrap_or_else(|_| failure(q, w::NativeQueryStatus::Invalid, time))
}
