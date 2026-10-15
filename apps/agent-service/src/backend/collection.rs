//! Compile a verified native plan into the sole runner's fixed service worker.
use super::plan::{self, hex, id, reference};
use agent_client::{Error, Materials, Offer};
use execution_contract::*;
use execution_runner::{staging::MaterialRoot, Artifacts};
use rss_mdm_agent_wire::runtime as w;
use sha2::{Digest as _, Sha256};

pub(crate) fn compile(
    offer: &Offer,
    materials: &Materials,
    payload: &w::CollectionTaskSpec,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    config: &crate::ExecutionConfig,
    root: &mut MaterialRoot,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let w::TaskPayload::Collection(original) = offer.payload() else {
        return Err(Error::Untrusted);
    };
    let mut compared = payload.clone();
    compared.permit = original.permit;
    compared.expires_at = original.expires_at;
    if compared != **original
        || payload.device_id != binding.device.as_str()
        || !materials.files().is_empty()
    {
        return Err(Error::Untrusted);
    }
    materials.validate_offer(offer)?;
    payload.validate()?;
    if plan::platform()? != Platform::Windows
        || !payload
            .plan
            .supported_by(&crate::collection_worker::engines())
    {
        return Err(Error::Unsupported);
    }
    crate::collection_worker::preflight(&payload.plan)?;
    // The runner derives Start expiry as validity end minus execution budget.
    let seconds = u64::from(payload.plan.spec().budget.timeout_seconds)
        .min(u64::try_from(payload.deadline - payload.expires_at).map_err(|_| Error::Clock)?);
    if seconds == 0 {
        return Err(Error::Expired);
    }
    let compiled_at_ms = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Clock)?
            .as_millis(),
    )
    .map_err(|_| Error::Clock)?;
    let finish_by_unix_ms = compiled_at_ms
        .checked_add(seconds * 1000)
        .ok_or(Error::Clock)?;
    let image = plan::pin(&std::env::current_exe()?)?;
    let input = crate::collection_worker::Input {
        task: payload.clone(),
        finish_by_unix_ms,
        image_sha256: image.artifact.sha256.clone(),
    };
    let bytes = serde_json::to_vec(&input).map_err(|_| Error::Protocol)?;
    let hash = hex(&Sha256::digest(&bytes));
    let artifact = ExactArtifactRef {
        resource: reference("native-collection-input", &hash)?,
        sha256: Digest::new(hash).map_err(|_| Error::Protocol)?,
    };
    let content = root
        .publish(
            None,
            std::io::Cursor::new(&bytes),
            &artifact.sha256,
            bytes.len() as u64,
        )
        .map_err(crate::error::app_error)?;
    let frozen = FrozenExecution::freeze(
        ExecutionInput {
            schema_version: V8,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: super::request_id(offer)?,
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
                    platform: Platform::Windows,
                    scope: TargetScope::Device {},
                },
                operation: Operation {
                    action: id("backend-collection")?,
                    resource: reference("backend-resource", hex(&payload.resource_digest))?,
                },
                parameters: Default::default(),
            },
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: image.artifact,
                    profile: reference(crate::collection_worker::PROFILE, "1")?,
                },
                argv: vec![
                    LaunchArg::Literal {
                        value: "--collection-worker".into(),
                    },
                    LaunchArg::ArtifactPath {},
                ],
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: StandardInput::Closed {},
                output: OutputSpec {
                    format: OutputFormat::Json { max_rows: 1 },
                    stdout: TextEncoding::Utf8,
                    stderr: TextEncoding::Utf8,
                },
                cwd: config
                    .work_root
                    .to_str()
                    .ok_or(Error::Configuration)?
                    .into(),
                env: Default::default(),
            },
            run_as: RunAs::System {
                platform: Platform::Windows,
            },
            session_requirement: SessionRequirement::NotRequired {},
            constraints: IsolationPolicy::OsIdentity {},
            budget: ExecutionBudget {
                total_timeout_ms: seconds * 1000,
                total_output_bytes: u64::from(payload.plan.spec().budget.output_bytes),
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: compiled_at_ms,
                expires_at_unix_ms: u64::try_from(payload.expires_at)
                    .map_err(|_| Error::Clock)?
                    .checked_add(seconds)
                    .and_then(|s| s.checked_mul(1000))
                    .ok_or(Error::Clock)?,
            },
            policy: reference(
                format!("backend:{}", payload.task_id),
                payload.attempt_id.to_string(),
            )?,
        },
        &plan::storage_limits().input,
    )
    .map_err(|_| Error::Untrusted)?;
    Ok((
        frozen,
        Artifacts {
            program: vec![],
            delegate: None,
            interpreter: image.path.into(),
            content,
            work_root: config.work_root.clone(),
            controlled_input: None,
        },
    ))
}

pub(crate) fn verify_input(
    plan: &FrozenExecution,
    task: &w::CollectionTaskSpec,
) -> Result<(), Error> {
    let input = crate::collection_worker::Input {
        task: task.clone(),
        finish_by_unix_ms: plan
            .spec()
            .validity
            .not_before_unix_ms
            .checked_add(plan.spec().budget.total_timeout_ms)
            .ok_or(Error::Clock)?,
        image_sha256: plan.spec().launch.interpreter.artifact.sha256.clone(),
    };
    let bytes = serde_json::to_vec(&input).map_err(|_| Error::Protocol)?;
    if hex(&Sha256::digest(&bytes)) != plan.spec().launch.artifact.sha256.as_str() {
        return Err(Error::Untrusted);
    }
    Ok(())
}
