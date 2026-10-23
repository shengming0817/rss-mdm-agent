//! Adapt one protected local recipe into the existing runner and journal envelope.
use super::*;
use crate::backend::plan;
use execution_runner::{Artifacts, SoftwareStepArtifacts};
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
pub(crate) fn recipe(
    recipe: &Recipe,
    choice: &LocalSoftwareChoice,
    item: &InstalledSoftware,
    binding: &execution_app::ServiceBinding,
    actor: &ActorId,
    initiator: Initiator,
    context: &NativeContextRef,
    policy: &Policy,
    now: u64,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
    work_root: PathBuf,
) -> Result<(FrozenExecution, Artifacts), Error> {
    let mut input = recipe.input.clone();
    let platform = plan::platform()?;
    let (scope, run_as, session) = match &item.scope {
        DataScope::Device => (
            TargetScope::Device {},
            RunAs::System { platform },
            SessionRequirement::NotRequired {},
        ),
        DataScope::User { subject } => {
            let h = helper.as_ref().ok_or(Error::Unavailable)?;
            if h.context().subject != subject.as_str() {
                return Err(Error::Denied);
            }
            let account = OsAccountRef {
                platform,
                subject: subject.clone(),
            };
            (
                TargetScope::User {
                    account: account.clone(),
                },
                RunAs::User {
                    account: account.clone(),
                },
                SessionRequirement::ActiveUser {
                    account,
                    session: h.context().binding.clone(),
                },
            )
        }
    };
    input.request = ExecutionRequest {
        schema_version: V1,
        request_id: choice.request.clone(),
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
            action: plan::id(match choice.action {
                LocalSoftwareAction::Update => "software.install",
                LocalSoftwareAction::Uninstall => "software.uninstall",
                LocalSoftwareAction::Detect => "software.detect",
                _ => return Err(Error::Unsupported),
            })?,
            resource: plan::reference(item.native_id.clone(), policy.revision.as_str())?,
        },
        parameters: metadata(choice, item, context, recipe.target_version.clone())?,
    };
    input.run_as = run_as.clone();
    input.session_requirement = session.clone();
    input.policy = plan::reference(
        format!("local-software:{}", policy.revision.as_str()),
        policy.revision.as_str(),
    )?;
    input.validity = ValidityWindow {
        not_before_unix_ms: now,
        expires_at_unix_ms: now
            .checked_add(60_000)
            .and_then(|n| n.checked_add(input.budget.total_timeout_ms))
            .ok_or(Error::Clock)?,
    };
    input.launch.cwd = work_root.to_str().ok_or(Error::Configuration)?.into();
    let ExecutionSpec::SoftwareProgram { program } = &mut input.execution else {
        return Err(Error::Configuration);
    };
    if choice.action == LocalSoftwareAction::Detect {
        program.intent = SoftwareOperation::Detect;
    }
    for step in &mut program.steps {
        rebind(&mut step.install, &run_as, &session, &work_root)?;
        if let Some(uninstall) = &mut step.uninstall {
            rebind(uninstall, &run_as, &session, &work_root)?;
        }
        if let SoftwareUpgrade::InPlace { invocation } = &mut step.upgrade {
            rebind(invocation, &run_as, &session, &work_root)?;
        }
        if let SoftwareDetector::Script { invocation } = &mut step.detection {
            rebind(invocation, &run_as, &session, &work_root)?;
        }
        for invocation in step.auxiliary.values_mut() {
            rebind(invocation, &run_as, &session, &work_root)?;
        }
    }
    let frozen = FrozenExecution::freeze(input, &plan::storage_limits().input)
        .map_err(|_| Error::Configuration)?;
    let artifacts = materials(&frozen, recipe, helper, work_root)?;
    Ok((frozen, artifacts))
}
fn rebind(
    invocation: &mut SoftwareInvocation,
    run_as: &RunAs,
    session: &SessionRequirement,
    root: &std::path::Path,
) -> Result<(), Error> {
    invocation.run_as = run_as.clone();
    invocation.session_requirement = session.clone();
    invocation.launch.cwd = root.to_str().ok_or(Error::Configuration)?.into();
    Ok(())
}
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
            serde_json::to_value(target),
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
fn file(recipe: &Recipe, reference: &ExactArtifactRef) -> Result<PathBuf, Error> {
    let mut paths = recipe
        .files
        .iter()
        .filter(|file| file.artifact == *reference);
    let selected = paths.next().ok_or(Error::Configuration)?;
    if paths.next().is_some() {
        return Err(Error::Configuration);
    }
    let path = PathBuf::from(&selected.path);
    installation_security::protected(&path).map_err(|_| Error::Denied)?;
    execution_runner::staging::verify_retained(&path, &reference.sha256)
        .map_err(crate::error::app_error)?;
    Ok(path)
}
fn physical(
    recipe: &Recipe,
    launch: &LaunchSpec,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
    root: &std::path::Path,
) -> Result<Artifacts, Error> {
    Ok(Artifacts {
        program: vec![],
        delegate: helper,
        interpreter: file(recipe, &launch.interpreter.artifact)?,
        content: file(recipe, &launch.artifact)?,
        work_root: root.into(),
        controlled_input: None,
    })
}
pub(crate) fn materials(
    plan: &FrozenExecution,
    recipe: &Recipe,
    helper: Option<Arc<execution_ipc::helper::Connection>>,
    root: PathBuf,
) -> Result<Artifacts, Error> {
    let mut result = physical(recipe, &plan.spec().launch, helper.clone(), &root)?;
    let ExecutionSpec::SoftwareProgram { program } = &plan.spec().execution else {
        return Err(Error::Configuration);
    };
    for (index, step) in program.steps.iter().enumerate() {
        let mut mutations = BTreeMap::new();
        for phase in [
            SoftwarePhase::Mutation,
            SoftwarePhase::Removal,
            SoftwarePhase::Upgrade,
            SoftwarePhase::Prepare,
            SoftwarePhase::Attach,
            SoftwarePhase::Stage,
            SoftwarePhase::Cleanup,
        ] {
            if let Some(invocation) = program.invocation(index, phase) {
                mutations.insert(
                    phase,
                    Box::new(physical(recipe, &invocation.launch, helper.clone(), &root)?),
                );
            }
        }
        let detection = match &step.detection {
            SoftwareDetector::Script { invocation } => Some(Box::new(physical(
                recipe,
                &invocation.launch,
                helper.clone(),
                &root,
            )?)),
            _ => None,
        };
        let files = recipe
            .files
            .iter()
            .map(|m| Ok((file(recipe, &m.artifact)?, m.artifact.clone())))
            .collect::<Result<Vec<_>, Error>>()?;
        result.program.push(SoftwareStepArtifacts {
            mutations,
            detection,
            files,
        });
    }
    Ok(result)
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
    let digest = crate::collected_data::digest(&bytes)?;
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
    let image = plan::pin(&std::env::current_exe()?)?;
    let mut parameters = metadata(choice, item, context, None)?;
    parameters.insert(
        "local-software-native".into(),
        InputValue::Literal {
            value: serde_json::to_value(target).map_err(|_| Error::Protocol)?,
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
                        LocalSoftwareAction::Launch => "software.launch",
                        LocalSoftwareAction::Uninstall => "software.uninstall",
                        LocalSoftwareAction::Detect => "software.detect",
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
    let interpreter = std::env::current_exe()?;
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
