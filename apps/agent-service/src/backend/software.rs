//! Deterministic V1 compiler. The backend has already selected sources and ordered dependencies.
use crate::{
    backend::plan::{self, hex, id, reference},
    ExecutionConfig, SoftwareManagerKind,
};
use agent_client::{Error, Materials, Offer};
use execution_contract::*;
use execution_runner::{Artifacts, SoftwareStepArtifacts};
use rss_mdm_agent_wire::runtime as wire;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

mod native;

fn package(value: &str) -> Result<PackageValue, Error> {
    PackageValue::new(value).map_err(|_| Error::Protocol)
}
fn artifact(bytes: &[u8; 32]) -> Result<ExactArtifactRef, Error> {
    Ok(ExactArtifactRef {
        resource: reference("backend-material", hex(bytes))?,
        sha256: Digest::new(hex(bytes)).map_err(|_| Error::Protocol)?,
    })
}
struct Compiler<'a> {
    config: &'a ExecutionConfig,
    root: &'a mut execution_runner::staging::MaterialRoot,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
    files: BTreeMap<String, (PathBuf, ExactArtifactRef)>,
    bundle_root: Option<PathBuf>,
}
pub(crate) fn compile(
    offer: &Offer,
    materials: &Materials,
    payload: &wire::SoftwareTaskSpec,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    (config, root): (
        &ExecutionConfig,
        &mut execution_runner::staging::MaterialRoot,
    ),
    helper: Option<Arc<execution_ipc::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let wire::TaskPayload::Software(original) = offer.payload() else {
        return Err(Error::Untrusted);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if &compared != original.as_ref() || payload.device_id != binding.device.as_str() {
        return Err(Error::Untrusted);
    }
    payload.validate().map_err(|_| Error::Protocol)?;
    let platform = plan::platform()?;
    let architecture = package(match payload.architecture {
        wire::TaskArchitecture::Aarch64 => "aarch64",
        wire::TaskArchitecture::X86_64 => "x86_64",
    })?;
    let intent = match payload.intent {
        wire::SoftwareTaskIntent::Install => SoftwareOperation::Install,
        wire::SoftwareTaskIntent::Uninstall => SoftwareOperation::Uninstall,
        wire::SoftwareTaskIntent::Detect => SoftwareOperation::Detect,
    };
    let mut steps = Vec::new();
    let mut sources = Vec::new();
    for (index, step) in payload.steps.iter().enumerate() {
        match (&step.target, helper.as_ref()) {
            (wire::SoftwareExecutionTarget::Device, _) => (),
            (
                wire::SoftwareExecutionTarget::User {
                    identity,
                    session_id,
                },
                Some(h),
            ) if identity == &h.context().subject
                && *session_id == crate::service::login_id(h.context()) => {}
            _ => return Err(Error::Identity),
        }

        let (compiled, source) = native::compile(
            (step, index),
            payload,
            materials,
            (config, root),
            helper.clone(),
            intent,
            (&architecture, plan::worker_image(config)?),
        )?;
        steps.push(compiled);
        sources.push(source);
    }
    let (timeout, output) = crate::backend::commands::software_budget(payload)?;
    let mut controller = steps.first().ok_or(Error::Protocol)?.install.launch.clone();
    controller.interpreter.profile = reference("native-software-sequence", "1")?;
    let program = SoftwareProgram {
        definition_digest: Digest::new(hex(&payload.definition_digest))
            .map_err(|_| Error::Protocol)?,
        intent,
        steps,
    };
    let input = ExecutionInput {
        schema_version: V1,
        request: ExecutionRequest {
            schema_version: V1,
            request_id: crate::backend::request_id(offer)?,
            authority: binding.authority.clone(),
            actor: actor.clone(),
            initiator: Initiator::Backend {
                task: id(payload.task_id.to_string())?,
                attempt: id(payload.attempt_id.to_string())?,
                trigger: BackendTrigger::Automatic {},
            },
            delegation: None,
            target: Target {
                device: binding.device.clone(),
                platform,
                scope: TargetScope::Device {},
            },
            operation: Operation {
                action: id(match intent {
                    SoftwareOperation::Install => "software.install",
                    SoftwareOperation::Uninstall => "software.uninstall",
                    SoftwareOperation::Detect => "software.detect",
                })?,
                resource: reference("backend-software", hex(&payload.definition_digest))?,
            },
            parameters: BTreeMap::new(),
        },
        execution: ExecutionSpec::SoftwareProgram {
            program: Box::new(program),
        },
        launch: controller,
        run_as: RunAs::System { platform },
        session_requirement: SessionRequirement::NotRequired {},
        constraints: IsolationPolicy::OsIdentity {},
        budget: ExecutionBudget {
            total_timeout_ms: timeout,
            total_output_bytes: output,
            max_attempts: 1,
        },
        validity: ValidityWindow {
            not_before_unix_ms: 0,
            expires_at_unix_ms: u64::try_from(payload.expires_at)
                .map_err(|_| Error::Clock)?
                .checked_mul(1000)
                .and_then(|v| v.checked_add(timeout))
                .ok_or(Error::Clock)?,
        },
        policy: reference(
            format!("backend:{}", payload.task_id),
            payload.attempt_id.to_string(),
        )?,
    };
    let frozen = FrozenExecution::freeze(input, &plan::storage_limits().input)
        .map_err(|_| Error::Untrusted)?;
    Ok((
        frozen,
        Artifacts {
            program: sources,
            delegate: helper,
            interpreter: PathBuf::new(),
            content: PathBuf::new(),
            work_root: config.work_root.clone(),
            controlled_input: None,
        },
    ))
}
