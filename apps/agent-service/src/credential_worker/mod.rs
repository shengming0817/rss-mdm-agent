//! Fixed system worker. The existing runner owns admission, cancellation and process lifetime.
//! ref: microsoft/windows-rs@0.61.0 Win32/System/Wmi/mod.rs.
use agent_client::Error;
use execution_contract::{Digest, RunAs, SessionRequirement};
use rss_mdm_agent_wire::runtime as w;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
#[cfg(any(windows, test))]
use zeroize::Zeroizing;
#[cfg(any(windows, test))]
mod engine;
pub(crate) mod envelope;
#[cfg(any(windows, test))]
mod rss;
pub(crate) mod state;
#[cfg(test)]
mod tests;
#[cfg(windows)]
mod windows;
pub(crate) const PROFILE: &str = "native-credential-worker";
pub(crate) const OUTPUT_BYTES: u64 = 262144;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub task: w::CredentialTaskSpec,
    pub root: PathBuf,
    pub image_sha256: Digest,
    pub run_as: RunAs,
    pub session: SessionRequirement,
}
#[cfg(any(windows, test))]
pub(crate) struct Material {
    pub subject: w::CredentialSubject,
    pub secret: Zeroizing<String>,
}
#[cfg(any(windows, test))]
pub(crate) struct Inventory {
    pub materials: Vec<Material>,
    pub failure: Option<w::CredentialFailure>,
}
#[cfg(any(windows, test))]
pub(crate) trait Native {
    fn collect(&mut self) -> Result<Inventory, w::CredentialFailure>;
    fn bitlocker(
        &mut self,
        volume: &str,
        protector: uuid::Uuid,
    ) -> Result<Material, w::CredentialFailure>;
    fn apply(
        &mut self,
        operation: &w::CredentialOperation,
        secret: &str,
    ) -> Result<(), w::CredentialFailure>;
    fn locate(
        &mut self,
        operation: &w::CredentialOperation,
        secret: &str,
    ) -> Result<Option<w::CredentialSubject>, w::CredentialFailure>;
    fn remove(&mut self, volume: &str, protector: uuid::Uuid) -> Result<(), w::CredentialFailure>;
    fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>, Error>;
    fn unprotect(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error>;
}
pub(crate) fn failed(
    task: &w::CredentialTaskSpec,
    failure: w::CredentialFailure,
) -> w::CredentialResult {
    w::CredentialResult {
        definition_digest: task.definition_digest,
        recovery: None,
        failure: Some(failure),
        complete: false,
        items: vec![],
    }
}
/// Entry accepts only the runner's retained, protected input and OS identity.
pub fn run(path: &Path) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 262144)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    input.task.validate()?;
    if input.task.permit != w::TaskPermit::Start
        || !matches!(
            input.run_as,
            RunAs::System {
                platform: execution_contract::Platform::Windows
            }
        )
        || !matches!(input.session, SessionRequirement::NotRequired {})
    {
        return Err(Error::Denied);
    }
    let _image =
        execution_runner::staging::verify_retained(&std::env::current_exe()?, &input.image_sha256)
            .map_err(crate::error::app_error)?;
    execution_runner::staging::verify_worker_identity(&input.run_as, &input.session)
        .map_err(crate::error::app_error)?;
    #[cfg(windows)]
    let result = {
        let mut native = windows::NativeWindows::open().map_err(|_| Error::Unavailable)?;
        engine::result(&input, &mut native)?
    };
    #[cfg(not(windows))]
    let result = failed(&input.task, w::CredentialFailure::Unsupported);
    std::io::stdout().write_all(&serde_json::to_vec(&result).map_err(|_| Error::Protocol)?)?;
    Ok(0)
}
pub(crate) fn root(config: &crate::ExecutionConfig) -> PathBuf {
    config.material_root.join("credentials")
}
pub(crate) fn prepare_root(config: &crate::ExecutionConfig) -> Result<PathBuf, Error> {
    let root = root(config);
    platform_private_storage::PrivateDirectory::create(&root)?;
    Ok(root)
}
/// Read ciphertext only. Worker output and execution history never carry escrow payloads.
pub(crate) fn pending(
    root: &Path,
    task: &w::CredentialTaskSpec,
) -> Result<Option<w::CredentialEscrow>, Error> {
    state::read_optional(root, &state::name(task, "escrow"))
}
pub(crate) fn acknowledge(
    root: &Path,
    task: &w::CredentialTaskSpec,
    escrow: &w::CredentialEscrow,
    receipts: &[w::CredentialReceipt],
) -> Result<(), Error> {
    state::write(root, &state::name(task, "receipts"), &receipts)?;
    state::write(root, &state::name(task, "ack"), &envelope::digest(escrow)?)
}

// Once escrow has been published, every failure must retain its subjects. The service may
// have committed it even when the worker did not observe the local acknowledgement.
pub(crate) fn failure_with_materials(
    root: &Path,
    task: &w::CredentialTaskSpec,
    failure: w::CredentialFailure,
) -> Result<w::CredentialResult, Error> {
    let mut result = failed(task, failure);
    if let Some(escrow) = pending(root, task)? {
        if escrow.definition_digest != task.definition_digest {
            return Err(Error::Untrusted);
        }
        result.items = escrow
            .materials
            .into_iter()
            .map(|m| w::CredentialItemResult {
                subject: m.subject,
                outcome: w::CredentialOutcome::Unknown,
            })
            .collect();
    }
    Ok(result)
}
pub(crate) fn plan_root(plan: &execution_contract::FrozenExecution) -> Result<PathBuf, Error> {
    match plan.spec().request.parameters.get("credential-root") {
        Some(execution_contract::InputValue::Literal { value }) => {
            serde_json::from_value(value.clone()).map_err(|_| Error::Untrusted)
        }
        _ => Err(Error::Untrusted),
    }
}

pub(crate) fn stored_result(
    root: &Path,
    task: &w::CredentialTaskSpec,
) -> Result<Option<w::CredentialResult>, Error> {
    let value: Option<w::CredentialResult> =
        state::read_optional(root, &state::name(task, "result"))?;
    if let Some(value) = &value {
        value.validate()?;
        if value.definition_digest != task.definition_digest {
            return Err(Error::Untrusted);
        }
    }
    Ok(value)
}
pub(crate) fn assessment(result: &w::CredentialResult) -> execution_lifecycle::EffectAssessment {
    use execution_lifecycle::EffectAssessment as A;
    if result.complete && result.failure.is_none() {
        if result.recovery == Some(w::CredentialRecoveryOutcome::NotAttempted)
            && result.items.is_empty()
        {
            return A::NoEffect;
        }
        if !result.items.is_empty()
            && result
                .items
                .iter()
                .all(|i| i.outcome == w::CredentialOutcome::Verified)
        {
            return A::Satisfied;
        }
    }
    if result.items.is_empty()
        && matches!(
            result.failure,
            Some(
                w::CredentialFailure::NoMaterials
                    | w::CredentialFailure::Unsupported
                    | w::CredentialFailure::PermissionDenied
                    | w::CredentialFailure::OwnerConflict
                    | w::CredentialFailure::TargetMissing
            )
        )
    {
        return A::NoEffect;
    }
    A::Unknown
}

/// Called only after the execution is quiescent and its terminal result durably acknowledged.
pub(crate) fn retire(root: &Path, task: &w::CredentialTaskSpec) -> Result<(), Error> {
    let _lock = state::lock(root)?;
    let result = stored_result(root, task)?;
    let effect = result.as_ref().map(assessment);
    let success = effect == Some(execution_lifecycle::EffectAssessment::Satisfied)
        || result.as_ref().is_some_and(|r| {
            r.complete && r.recovery == Some(w::CredentialRecoveryOutcome::NotAttempted)
        });
    let no_effect = effect == Some(execution_lifecycle::EffectAssessment::NoEffect);
    // Uncertain mutations retain their protected original, for a future signed recovery.
    // Current account material is independently retained in the registration-bound .account.
    if success || no_effect {
        state::remove(root, &state::name(task, "state"))?;
        if let Some(reference) = task.recovery.as_ref().filter(|_| success) {
            let mut original = task.clone();
            original.task_id = reference.task_id;
            original.attempt_id = reference.attempt_id;
            for suffix in ["state", "escrow", "ack", "receipts", "result"] {
                state::remove(root, &state::name(&original, suffix))?;
            }
        }
    }
    for suffix in ["escrow", "ack", "receipts", "result"] {
        // RSS recovery must prove the exact material/version committed by SQLite.
        if suffix == "receipts"
            && !success
            && !no_effect
            && matches!(
                task.operation,
                w::CredentialOperation::RotateRssLogin { .. }
            )
        {
            continue;
        }
        state::remove(root, &state::name(task, suffix))?;
    }
    // A held owner lock proves no worker is staging a write; the service owner is sequential.
    for entry in std::fs::read_dir(root)?.take(32769) {
        let entry = entry?;
        let name = entry.file_name();
        if let Some(name) = name.to_str() {
            if name
                .strip_suffix(".tmp")
                .is_some_and(|stem| uuid::Uuid::parse_str(stem).is_ok())
            {
                state::remove(root, name)?;
            }
        }
    }
    platform_private_storage::PrivateDirectory::open(root)?.sync()?;
    Ok(())
}
