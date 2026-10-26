//! Freeze one finite user network check into the existing runner envelope.
use super::*;
use crate::backend::plan;
use execution_app::ServiceBinding;
use execution_runner::Artifacts;
use std::sync::Arc;
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    target: NetworkTarget,
    request: RequestId,
    binding: &ServiceBinding,
    actor: &ActorId,
    initiator: Initiator,
    now: u64,
    config: &crate::ExecutionConfig,
    root: &mut execution_runner::staging::MaterialRoot,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let platform = plan::platform()?;
    let (run_as, session, scope, work_root) = if let Some(helper) = &helper {
        let account = OsAccountRef {
            platform,
            subject: plan::id(&helper.context().subject)?,
        };
        (
            RunAs::User {
                account: account.clone(),
            },
            SessionRequirement::ActiveUser {
                account: account.clone(),
                session: helper.context().binding.clone(),
            },
            TargetScope::User { account },
            helper.context().work_root.clone(),
        )
    } else {
        (
            RunAs::System { platform },
            SessionRequirement::NotRequired {},
            TargetScope::Device {},
            config.work_root.clone(),
        )
    };
    let timeout = 30_000;
    let worker = Input {
        target: target.clone(),
        run_as: run_as.clone(),
        session: session.clone(),
        finish_by_unix_ms: now.checked_add(timeout).ok_or(Error::Clock)?,
    };
    let bytes = serde_json::to_vec(&worker).map_err(|_| Error::Protocol)?;
    if bytes.len() > 65536 {
        return Err(Error::Capacity);
    }
    let artifact = ExactArtifactRef {
        resource: plan::reference("network-check-input", "1")?,
        sha256: hash(&bytes)?,
    };
    let content = root
        .publish(
            helper.as_ref().map(|h| h.context().subject.as_str()),
            std::io::Cursor::new(&bytes),
            &artifact.sha256,
            bytes.len() as u64,
        )
        .map_err(crate::error::app_error)?;
    let image = plan::worker_image(config)?;
    let mut parameters: std::collections::BTreeMap<String, InputValue> = [
        (
            "network-check-target".into(),
            InputValue::Literal {
                value: serde_json::to_value(&target).map_err(|_| Error::Protocol)?,
            },
        ),
        (
            "local-owner".into(),
            crate::account_identity::local_owner(&initiator)?,
        ),
    ]
    .into();
    parameters.insert(
        "exclusive-resources".into(),
        InputValue::Literal {
            value: serde_json::json!([resource(&initiator, &target)?]),
        },
    );
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
                    scope,
                },
                operation: Operation {
                    action: plan::id(ACTION)?,
                    resource: plan::reference("network-check", "1")?,
                },
                parameters,
            },
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: image.artifact,
                    profile: plan::reference(PROFILE, "1")?,
                },
                argv: vec![
                    LaunchArg::Literal {
                        value: "--network-check-worker".into(),
                    },
                    LaunchArg::ArtifactPath {},
                ],
                artifact_encoding: ArtifactEncoding::Utf8,
                stdin: StandardInput::Closed {},
                output: OutputSpec {
                    format: OutputFormat::Text {},
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
                total_timeout_ms: timeout,
                total_output_bytes: 65536,
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: now,
                expires_at_unix_ms: now.checked_add(timeout).ok_or(Error::Clock)?,
            },
            policy: plan::reference("local-network-check", "1")?,
        },
        &plan::storage_limits().input,
    )
    .map_err(|_| Error::Configuration)?;
    Ok((
        frozen,
        Artifacts {
            program: vec![],
            delegate: helper,
            interpreter: image.path.into(),
            content,
            work_root,
            controlled_input: None,
        },
    ))
}
