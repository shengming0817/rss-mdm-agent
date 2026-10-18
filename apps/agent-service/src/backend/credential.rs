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
    payload: &w::CredentialTaskSpec,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    (config, root): (&crate::ExecutionConfig, &mut MaterialRoot),
    delegate: Option<std::sync::Arc<execution_ipc::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let w::TaskPayload::Credential(original) = offer.payload() else {
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
    if payload.platform
        != match plan::platform()? {
            Platform::Windows => w::TaskPlatform::Windows,
            Platform::Macos => w::TaskPlatform::Macos,
            Platform::Linux => return Err(Error::Unsupported),
        }
    {
        return Err(Error::Unsupported);
    }
    if delegate.is_some() {
        return Err(Error::Denied);
    }
    let platform = plan::platform()?;
    let run_as = RunAs::System { platform };
    let session = SessionRequirement::NotRequired {};
    let work_root = config.work_root.clone();
    let state_root = crate::credential_worker::prepare_root(config)?;

    // The runner derives Start expiry as validity end minus execution budget.
    let seconds = 3600u64
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
    let image = plan::pin(&std::env::current_exe()?)?;
    let input = crate::credential_worker::Input {
        task: payload.clone(),
        root: state_root.clone(),
        image_sha256: image.artifact.sha256.clone(),
        run_as: run_as.clone(),
        session: session.clone(),
    };
    let bytes = serde_json::to_vec(&input).map_err(|_| Error::Protocol)?;
    let hash = hex(&Sha256::digest(&bytes));
    let artifact = ExactArtifactRef {
        resource: reference("native-credential-input", &hash)?,
        sha256: Digest::new(hash).map_err(|_| Error::Protocol)?,
    };
    let content = root
        .publish(
            delegate.as_ref().map(|h| h.context().subject.as_str()),
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
                    platform,
                    scope: TargetScope::Device {},
                },
                operation: Operation {
                    action: id("backend-credential")?,
                    resource: reference("backend-resource", hex(&payload.definition_digest))?,
                },
                parameters: [(
                    "credential-root".into(),
                    InputValue::Literal {
                        value: serde_json::to_value(&state_root).map_err(|_| Error::Protocol)?,
                    },
                )]
                .into(),
            },
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: image.artifact,
                    profile: reference(crate::credential_worker::PROFILE, "1")?,
                },
                argv: vec![
                    LaunchArg::Literal {
                        value: "--credential-worker".into(),
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
                cwd: work_root.to_str().ok_or(Error::Configuration)?.into(),
                env: Default::default(),
            },
            run_as,
            session_requirement: session,
            constraints: IsolationPolicy::OsIdentity {},
            budget: ExecutionBudget {
                total_timeout_ms: seconds * 1000,
                total_output_bytes: crate::credential_worker::OUTPUT_BYTES,
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
            delegate,
            interpreter: image.path.into(),
            content,
            work_root,
            controlled_input: None,
        },
    ))
}

pub(crate) fn verify_input(
    plan: &FrozenExecution,
    task: &w::CredentialTaskSpec,
) -> Result<(), Error> {
    let root = match plan.spec().request.parameters.get("credential-root") {
        Some(InputValue::Literal { value }) => {
            serde_json::from_value(value.clone()).map_err(|_| Error::Protocol)?
        }
        _ => return Err(Error::Untrusted),
    };
    let input = crate::credential_worker::Input {
        task: task.clone(),
        root,
        image_sha256: plan.spec().launch.interpreter.artifact.sha256.clone(),
        run_as: plan.spec().run_as.clone(),
        session: plan.spec().session_requirement.clone(),
    };
    if hex(&Sha256::digest(
        serde_json::to_vec(&input).map_err(|_| Error::Protocol)?,
    )) != plan.spec().launch.artifact.sha256.as_str()
    {
        return Err(Error::Untrusted);
    }
    Ok(())
}
