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
    (config, root): (&crate::ExecutionConfig, &mut MaterialRoot),
    delegate: Option<std::sync::Arc<execution_ipc::helper::Connection>>,
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
    compile_read(
        &{
            let mut input = payload.clone();
            input.permit = w::TaskPermit::Offer;
            input.expires_at = input.deadline;
            input.try_into()?
        },
        super::request_id(offer)?,
        Initiator::Backend {
            task: id(payload.task_id.to_string())?,
            attempt: id(payload.attempt_id.to_string())?,
            trigger: BackendTrigger::Automatic {},
        },
        Operation {
            action: id("backend-collection")?,
            resource: reference("backend-resource", hex(&payload.resource_digest))?,
        },
        reference(
            format!("backend:{}", payload.task_id),
            payload.attempt_id.to_string(),
        )?,
        binding,
        actor,
        (config, root),
        delegate,
    )
}

/// Compile physical collection without manufacturing a remote task or Start authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compile_read(
    read: &crate::collection_worker::ReadPlan,
    request: RequestId,
    initiator: Initiator,
    operation: Operation,
    policy: VersionedRef,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    (config, root): (&crate::ExecutionConfig, &mut MaterialRoot),
    delegate: Option<std::sync::Arc<execution_ipc::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    read.validate()?;
    if read.platform
        != match plan::platform()? {
            Platform::Windows => w::TaskPlatform::Windows,
            Platform::Macos => w::TaskPlatform::Macos,
            Platform::Linux => return Err(Error::Unsupported),
        }
        || !read.plan.supported_by(&crate::collection_worker::engines())
    {
        return Err(Error::Unsupported);
    }
    crate::collection_worker::preflight(&read.plan)?;
    let platform = plan::platform()?;
    let (run_as, session, work_root) = match (&read.execution.target, read.execution.identity) {
        (w::SoftwareExecutionTarget::Device, w::ExecutionIdentity::System)
            if delegate.is_none() =>
        {
            (
                RunAs::System { platform },
                SessionRequirement::NotRequired {},
                config.work_root.clone(),
            )
        }
        (
            w::SoftwareExecutionTarget::User {
                identity,
                session_id,
            },
            w::ExecutionIdentity::LoggedInUser,
        ) => {
            let helper = delegate.as_ref().ok_or(Error::Unavailable)?;
            if identity != &helper.context().subject
                || *session_id != crate::service::login_id(helper.context())
                || read.execution.generated_uid != crate::account_identity::generated_uid(identity)?
            {
                return Err(Error::Untrusted);
            }
            let account = OsAccountRef {
                platform,
                subject: id(identity)?,
            };
            (
                RunAs::User {
                    account: account.clone(),
                },
                SessionRequirement::ActiveUser {
                    account,
                    session: helper.context().binding.clone(),
                },
                helper.context().work_root.clone(),
            )
        }
        _ => return Err(Error::Untrusted),
    };
    let tools = crate::collection_worker::tools(config, &read.plan)?;

    let seconds = u64::from(read.plan.spec().budget.timeout_seconds);
    let compiled_at_ms = if matches!(
        read.source,
        crate::collection_worker::ReadSource::Task { .. }
    ) {
        0
    } else {
        u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| Error::Clock)?
                .as_millis(),
        )
        .map_err(|_| Error::Clock)?
    };
    let image = plan::worker_image(config)?;
    let input = crate::collection_worker::Input {
        read: read.clone(),
        image_sha256: image.artifact.sha256.clone(),
        run_as: run_as.clone(),
        session: session.clone(),
        tools: tools.clone(),
        observed: Default::default(),
    };
    let bytes = serde_json::to_vec(&input).map_err(|_| Error::Protocol)?;
    if bytes.len() as u64 > crate::collection_worker::INPUT_BYTES {
        return Err(Error::Capacity);
    }
    let hash = hex(&Sha256::digest(&bytes));
    let artifact = ExactArtifactRef {
        resource: reference("native-collection-input", &hash)?,
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
            schema_version: V1,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: request,
                authority: binding.authority.clone(),
                actor: actor.clone(),
                initiator,
                delegation: None,
                target: Target {
                    device: binding.device.clone(),
                    platform,
                    scope: TargetScope::Device {},
                },
                operation,
                parameters: [
                    (
                        "collection-tools".into(),
                        InputValue::Literal {
                            value: serde_json::to_value(&tools).map_err(|_| Error::Protocol)?,
                        },
                    ),
                    (
                        "collection-read".into(),
                        InputValue::Literal {
                            value: serde_json::to_value(read).map_err(|_| Error::Protocol)?,
                        },
                    ),
                ]
                .into(),
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
                cwd: work_root.to_str().ok_or(Error::Configuration)?.into(),
                env: Default::default(),
            },
            run_as,
            session_requirement: session,
            constraints: IsolationPolicy::OsIdentity {},
            budget: ExecutionBudget {
                total_timeout_ms: seconds * 1000,
                total_output_bytes: u64::from(read.plan.spec().budget.output_bytes),
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: compiled_at_ms,
                expires_at_unix_ms: u64::try_from(read.deadline)
                    .map_err(|_| Error::Clock)?
                    .checked_mul(1000)
                    .ok_or(Error::Clock)?,
            },
            policy,
        },
        &plan::storage_limits().input,
    )
    .map_err(|error| {
        eprintln!("agent_native_plan: {error}");
        Error::Untrusted
    })?;
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
    task: &w::CollectionTaskSpec,
) -> Result<(), Error> {
    let read: crate::collection_worker::ReadPlan =
        match plan.spec().request.parameters.get("collection-read") {
            Some(InputValue::Literal { value }) => {
                serde_json::from_value(value.clone()).map_err(|_| Error::Protocol)?
            }
            _ => return Err(Error::Untrusted),
        };
    read.validate()?;
    let mut normalized = task.clone();
    normalized.permit = w::TaskPermit::Offer;
    normalized.expires_at = normalized.deadline;
    let task = &normalized;
    if !matches!(&read.source, crate::collection_worker::ReadSource::Task { task: original, .. } if original.as_ref() == task)
    {
        return Err(Error::Untrusted);
    }
    let input = crate::collection_worker::Input {
        read,
        image_sha256: plan.spec().launch.interpreter.artifact.sha256.clone(),
        run_as: plan.spec().run_as.clone(),
        session: plan.spec().session_requirement.clone(),
        observed: Default::default(),
        tools: match plan.spec().request.parameters.get("collection-tools") {
            Some(InputValue::Literal { value }) => {
                serde_json::from_value(value.clone()).map_err(|_| Error::Protocol)?
            }
            _ => return Err(Error::Untrusted),
        },
    };
    let bytes = serde_json::to_vec(&input).map_err(|_| Error::Protocol)?;
    if bytes.len() as u64 > crate::collection_worker::INPUT_BYTES {
        return Err(Error::Capacity);
    }
    if hex(&Sha256::digest(&bytes)) != plan.spec().launch.artifact.sha256.as_str() {
        return Err(Error::Untrusted);
    }
    Ok(())
}
