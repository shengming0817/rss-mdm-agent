//! Device Access runs through the same fixed worker, protected state and task event transport.
use crate::credential_worker::state;
use agent_client::Error;
use execution_contract::{Digest, RunAs, SessionRequirement};
use rss_mdm_agent_wire::runtime as w;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
#[cfg(any(windows, test))]
mod engine;
#[cfg(any(windows, test))]
pub(crate) mod hello;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(test)]
mod tests;
#[cfg(windows)]
pub(crate) mod windows;
/// Both activation and the health CLI require a usable component, not merely a readable status.
#[cfg(any(windows, test))]
pub(crate) fn component_health(
    state: execution_ipc::device_access::WindowsComponentState,
) -> Result<(), Error> {
    if state == execution_ipc::device_access::WindowsComponentState::Ready {
        Ok(())
    } else {
        Err(Error::Unavailable)
    }
}
pub(crate) const PROFILE: &str = "native-device-access-worker";
pub(crate) const OUTPUT_BYTES: u64 = 16384;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub task: w::AccessTaskSpec,
    pub root: PathBuf,
    pub installation: uuid::Uuid,
    pub image_sha256: Digest,
    pub run_as: RunAs,
    pub session: SessionRequirement,
}
pub(crate) fn name(task: &w::AccessTaskSpec, suffix: &str) -> String {
    format!(
        "{}-{}-{}-{}.access-{suffix}",
        task.registration_id, task.generation, task.task_id, task.attempt_id
    )
}
pub(crate) fn root(config: &crate::ExecutionConfig) -> PathBuf {
    config.material_root.join("device-access")
}
pub(crate) fn plan_root(plan: &execution_contract::FrozenExecution) -> Result<PathBuf, Error> {
    match plan.spec().request.parameters.get("access-root") {
        Some(execution_contract::InputValue::Literal { value }) => {
            serde_json::from_value(value.clone()).map_err(|_| Error::Untrusted)
        }
        _ => Err(Error::Untrusted),
    }
}
#[cfg(any(windows, test))]
fn authority(
    task: &w::AccessTaskSpec,
    sid: &str,
    native: &impl engine::Native,
) -> Result<device_access::Authority, Error> {
    let (source, login, join) = native.identity(sid).map_err(|_| Error::Identity)?;
    let actual = w::AccessAccount::Windows {
        sid: sid.into(),
        authority: source.clone(),
    };
    match &task.action {
        w::AccessAction::Apply { account, .. } | w::AccessAction::Unbind { account, .. }
            if account != &actual =>
        {
            return Err(Error::Identity)
        }
        w::AccessAction::Create { .. } if source != w::AccessWindowsAuthority::Local => {
            return Err(Error::Identity)
        }
        _ => (),
    }
    Ok(device_access::Authority {
        instance: task.instance_id,
        principal: task.principal_id,
        source,
        login,
        join,
        tenant: task.tenant_id,
        device: task.device_id.clone(),
        registration: task.registration_id,
        registration_generation: task.generation,
        binding: task.binding_id,
        binding_generation: task.binding_generation,
        factor_generation: task.factor_generation,
        sid: sid.into(),
    })
}
/// Verify the consumer registration against the task's frozen storage identity.
// ref: rustls verify.rs@2976d90 (verification receives the caller's expected identity).
#[cfg(any(windows, test))]
pub(crate) fn verify_installation(
    expected_root: &Path,
    expected_id: uuid::Uuid,
    actual_root: &Path,
    actual_id: uuid::Uuid,
) -> Result<(), Error> {
    if expected_root != actual_root || expected_id != actual_id {
        return Err(Error::Identity);
    }
    Ok(())
}
pub(crate) fn report(task: &w::AccessTaskSpec, effect: w::AccessEffect) -> w::AccessResult {
    w::AccessResult {
        binding_id: task.binding_id,
        binding_generation: task.binding_generation,
        revision: task.revision,
        definition_digest: task.definition_digest,
        effect,
    }
}
pub(crate) fn prepared(
    root: &Path,
    task: &w::AccessTaskSpec,
) -> Result<Option<w::AccessPrepared>, Error> {
    state::read_optional(root, &name(task, "prepared"))
}
pub(crate) fn pending(
    root: &Path,
    task: &w::AccessTaskSpec,
) -> Result<Option<w::CredentialEscrow>, Error> {
    state::read_optional(root, &name(task, "escrow"))
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Ack {
    digest: String,
    receipts: Vec<w::CredentialReceipt>,
}
pub(crate) fn acknowledge(
    root: &Path,
    task: &w::AccessTaskSpec,
    escrow: &w::CredentialEscrow,
    receipts: Vec<w::CredentialReceipt>,
) -> Result<(), Error> {
    use sha2::{Digest, Sha256};
    if receipts.len() != 1
        || escrow.materials.len() != 1
        || receipts[0].subject != escrow.materials[0].subject
    {
        return Err(Error::Untrusted);
    }
    receipts[0].validate()?;
    state::write(
        root,
        &name(task, "ack"),
        &Ack {
            digest: format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(escrow).map_err(|_| Error::Protocol)?)
            ),
            receipts,
        },
    )
}
pub(crate) fn stored_result(
    root: &Path,
    task: &w::AccessTaskSpec,
) -> Result<Option<w::AccessResult>, Error> {
    let value: Option<w::AccessResult> = state::read_optional(root, &name(task, "result"))?;
    if let Some(value) = &value {
        value.validate_for(task.platform)?;
        if value.binding_id != task.binding_id
            || value.binding_generation != task.binding_generation
            || value.revision != task.revision
            || value.definition_digest != task.definition_digest
        {
            return Err(Error::Untrusted);
        }
    }
    Ok(value)
}
/// Only a runner-retained input and verified SYSTEM image can execute a started task.
pub fn run(path: &Path) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 262144)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    input.task.validate()?;
    if input.task.permit != w::TaskPermit::Start
        || input.task.platform
            != if cfg!(target_os = "macos") {
                w::TaskPlatform::Macos
            } else {
                w::TaskPlatform::Windows
            }
        || !matches!(
            input.run_as,
            RunAs::System { platform } if platform == if cfg!(target_os="macos") { execution_contract::Platform::Macos } else { execution_contract::Platform::Windows }
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
    let result = engine::execute(
        &input,
        &mut windows::NativeWindows {
            root: input.root.clone(),
            installation: input.installation,
        },
    )?;
    #[cfg(target_os = "macos")]
    let result = macos::execute(&input)?;
    #[cfg(not(any(windows, target_os = "macos")))]
    let result = report(
        &input.task,
        w::AccessEffect::Failed {
            reason: w::AccessFailure::Unsupported,
        },
    );
    std::io::stdout().write_all(&serde_json::to_vec(&result).map_err(|_| Error::Protocol)?)?;
    Ok(0)
}

/// Caller proves quiescence and a durably acknowledged terminal result first.
pub(crate) fn retire(
    plan: &execution_contract::FrozenExecution,
    task: &w::AccessTaskSpec,
) -> Result<(), Error> {
    let root = plan_root(plan)?;
    let installation: uuid::Uuid = match plan.spec().request.parameters.get("access-installation") {
        Some(execution_contract::InputValue::Literal { value }) => {
            serde_json::from_value(value.clone()).map_err(|_| Error::Untrusted)?
        }
        _ => return Err(Error::Untrusted),
    };
    let _lock = state::lock(&root)?;
    if stored_result(&root, task)?.is_some_and(|r| matches!(r.effect, w::AccessEffect::Unknown)) {
        return Err(Error::Conflict);
    }
    #[cfg(windows)]
    device_access::Store::open(&root, installation)
        .map_err(|_| Error::Storage)?
        .retire_candidate(task.task_id)
        .map_err(|_| Error::Storage)?;
    #[cfg(not(windows))]
    let _ = installation;
    for suffix in ["prepared", "escrow", "ack", "creation", "result"] {
        state::remove(&root, &name(task, suffix))?;
    }
    Ok(())
}

pub(crate) use agent_client::device_access_complete as complete;
