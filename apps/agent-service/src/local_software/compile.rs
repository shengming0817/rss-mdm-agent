//! Compile code-owned native software adapters into the existing runner envelope.
use super::*;
use crate::backend::plan;
use execution_runner::Artifacts;
use std::sync::Arc;

pub(crate) const LAUNCH_ACTION: &str = "local-software.launch";
pub(crate) const UNINSTALL_ACTION: &str = "local-software.uninstall";
pub(crate) const DETECT_ACTION: &str = "local-software.detect";

pub(crate) fn metadata(
    choice: &LocalSoftwareChoice,
    item: &InstalledSoftware,
    context: &NativeContextRef,
    target: Option<String>,
) -> Result<BTreeMap<String, InputValue>, Error> {
    [
        ("local-software-choice", serde_json::to_value(choice)),
        ("local-software-context", serde_json::to_value(context)),
        ("local-software-name", serde_json::to_value(&item.name)),
        (
            "local-software-target-version",
            serde_json::to_value(&target),
        ),
    ]
    .into_iter()
    .map(|(k, v)| {
        Ok((
            k.into(),
            InputValue::Literal {
                value: v.map_err(|_| Error::Protocol)?,
            },
        ))
    })
    .collect()
}
/// Compile one fixed native adapter into the same process envelope used by collection.
#[allow(clippy::too_many_arguments)]
pub(crate) fn native(
    target: super::native::Target,
    choice: &LocalSoftwareChoice,
    item: &InstalledSoftware,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    initiator: Initiator,
    context: &NativeContextRef,
    policy: &Policy,
    now: u64,
    config: &crate::ExecutionConfig,
    root: &mut execution_runner::staging::MaterialRoot,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
) -> Result<(FrozenExecution, Artifacts), Error> {
    if !super::native::allowed(&policy.rules, choice.action)
        || !super::native::supports(&target, choice.action)
    {
        return Err(Error::Denied);
    }
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
        if !matches!(item.scope, DataScope::Device) || choice.action == LocalSoftwareAction::Launch
        {
            return Err(Error::Denied);
        }
        (
            RunAs::System { platform },
            SessionRequirement::NotRequired {},
            TargetScope::Device {},
            config.work_root.clone(),
        )
    };
    let worker = super::native::Input {
        target: target.clone(),
        action: choice.action,
        request: choice.request.clone(),
        run_as: run_as.clone(),
        session: session.clone(),
    };
    let bytes = serde_json::to_vec(&worker).map_err(|_| Error::Protocol)?;
    let digest = crate::local_execution::hash(&bytes)?;
    let artifact = ExactArtifactRef {
        resource: plan::reference("local-software-input", digest.as_str())?,
        sha256: digest,
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
    let mut parameters = metadata(choice, item, context, None)?;
    parameters.insert(
        "local-software-native".into(),
        InputValue::Literal {
            value: serde_json::to_value(&target).map_err(|_| Error::Protocol)?,
        },
    );
    parameters.insert(
        "exclusive-resources".into(),
        InputValue::Literal {
            value: serde_json::json!([match target {
                super::native::Target::App { .. } => "manager-macos-applications",
                super::native::Target::Msi { .. } => "manager-windows-installers",
                super::native::Target::Msix { .. } => "manager-windows-package-deployment",
            }]),
        },
    );
    let timeout = 300_000;
    let frozen = FrozenExecution::freeze(
        ExecutionInput {
            schema_version: V1,
            execution: ExecutionSpec::Process {},
            request: ExecutionRequest {
                schema_version: V1,
                request_id: choice.request.clone(),
                authority: binding.authority.clone(),
                actor: actor.clone(),
                initiator,
                delegation: None,
                target: execution_contract::Target {
                    device: binding.device.clone(),
                    platform,
                    scope,
                },
                operation: Operation {
                    action: plan::id(match choice.action {
                        LocalSoftwareAction::Launch => LAUNCH_ACTION,
                        LocalSoftwareAction::Uninstall => UNINSTALL_ACTION,
                        LocalSoftwareAction::Detect => DETECT_ACTION,
                        _ => return Err(Error::Unsupported),
                    })?,
                    resource: plan::reference(&item.native_id, policy.revision.as_str())?,
                },
                parameters,
            },
            launch: LaunchSpec {
                artifact,
                interpreter: InterpreterRef {
                    artifact: image.artifact,
                    profile: plan::reference(super::native::PROFILE, "1")?,
                },
                argv: vec![
                    LaunchArg::Literal {
                        value: "--local-software-worker".into(),
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
                total_timeout_ms: timeout,
                total_output_bytes: 65536,
                max_attempts: 1,
            },
            validity: ValidityWindow {
                not_before_unix_ms: now,
                expires_at_unix_ms: now.checked_add(60_000 + timeout).ok_or(Error::Clock)?,
            },
            policy: plan::reference(
                format!("local-software:{}", policy.revision.as_str()),
                policy.revision.as_str(),
            )?,
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

pub(crate) fn native_materials(
    frozen: &FrozenExecution,
    config: &crate::ExecutionConfig,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
) -> Result<Artifacts, Error> {
    if frozen.spec().launch.interpreter.profile != plan::reference(super::native::PROFILE, "1")? {
        return Err(Error::Untrusted);
    }
    let interpreter = PathBuf::from(plan::worker_image(config)?.path);
    execution_runner::staging::verify_retained(
        &interpreter,
        &frozen.spec().launch.interpreter.artifact.sha256,
    )
    .map_err(crate::error::app_error)?;
    let content = crate::recovery::content(
        config,
        &frozen.spec().run_as,
        &frozen.spec().launch.artifact.sha256,
    );
    execution_runner::staging::verify_retained(&content, &frozen.spec().launch.artifact.sha256)
        .map_err(crate::error::app_error)?;
    let work_root = helper.as_ref().map_or_else(
        || config.work_root.clone(),
        |h| h.context().work_root.clone(),
    );
    Ok(Artifacts {
        program: vec![],
        delegate: helper,
        interpreter,
        content,
        work_root,
        controlled_input: None,
    })
}
