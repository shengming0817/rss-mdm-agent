//! Reconstruct physical material coordinates from the original journal input, never a new Start.
use crate::{ExecutionConfig, UserResources};
use agent_client::Error;
use execution_contract::*;
use execution_runner::{Artifacts, SoftwareStepArtifacts};
use std::{path::PathBuf, sync::Arc};

pub(crate) fn content(config: &ExecutionConfig, run_as: &RunAs, digest: &Digest) -> PathBuf {
    use sha2::{Digest as _, Sha256};
    let namespace = match run_as {
        RunAs::User { account } => {
            format!("{:x}", Sha256::digest(account.subject.as_str().as_bytes()))
        }
        _ => "system".into(),
    };
    config.material_root.join(namespace).join(digest.as_str())
}
fn recipe(
    config: &ExecutionConfig,
    helpers: &UserResources,
    launch: &LaunchSpec,
    run_as: &RunAs,
    session: &SessionRequirement,
) -> Result<Artifacts, Error> {
    let interpreter = if [
        "native-software-worker",
        "native-local-software-worker",
        "native-settings-worker",
        "native-network-check-worker",
        "native-collection-worker",
        "native-enrollment-worker",
        "native-credential-worker",
        "native-device-access-worker",
    ]
    .contains(&launch.interpreter.profile.id.as_str())
    {
        let image = std::path::PathBuf::from(crate::backend::plan::worker_image(config)?.path);
        execution_runner::staging::verify_retained(&image, &launch.interpreter.artifact.sha256)
            .map_err(crate::error::app_error)?;
        image
    } else {
        config
            .interpreters
            .iter()
            .find(|p| p.image.sha256 == launch.interpreter.artifact.sha256.as_str())
            .ok_or(Error::Configuration)?
            .image
            .path
            .clone()
    };
    let delegate = match (run_as, session) {
        (RunAs::User { account }, SessionRequirement::ActiveUser { session, .. }) => {
            let expected_binding = session.clone();
            let session = session
                .as_str()
                .split('/')
                .nth(1)
                .ok_or(Error::Protocol)?
                .parse::<u32>()
                .map_err(|_| Error::Protocol)?;
            let connection = execution_ipc::helper::Connection::connect(
                execution_ipc::host::PeerPolicy {
                    images: vec![helpers.image.clone()],
                    subjects: vec![account.subject.as_str().into()],
                    interactive: true,
                },
                account.subject.as_str().into(),
                session,
            )
            .map_err(crate::error::app_error)?;
            if connection.context().binding != expected_binding {
                return Err(Error::Untrusted);
            }
            if helpers
                .work_roots
                .get(account.subject.as_str())
                .is_none_or(|p| p != &connection.context().work_root)
            {
                return Err(Error::Untrusted);
            }
            Some(Arc::new(connection))
        }
        (RunAs::System { .. }, SessionRequirement::NotRequired {}) => None,
        _ => return Err(Error::Untrusted),
    };
    let work_root = delegate.as_ref().map_or_else(
        || config.work_root.clone(),
        |h| h.context().work_root.clone(),
    );
    Ok(Artifacts {
        program: vec![],
        delegate,
        interpreter,
        content: content(config, run_as, &launch.artifact.sha256),
        work_root,
        controlled_input: None,
    })
}
pub(crate) fn materials(
    plan: &FrozenExecution,
    config: &ExecutionConfig,
    helpers: &UserResources,
) -> Result<Artifacts, Error> {
    let p = plan.spec();
    let Some(program) = p.execution.software_program() else {
        return recipe(
            config,
            helpers,
            &p.launch,
            &p.run_as,
            &p.session_requirement,
        );
    };
    let mut sources = Vec::new();
    let mut delegate = None;
    for (index, step) in program.steps.iter().enumerate() {
        let original_recipe = |command: &SoftwareInvocation| -> Result<Box<Artifacts>, Error> {
            let mut source = recipe(config, helpers, &command.launch, &command.run_as, &command.session_requirement)?;
            if let Some((manifest, _)) = step.format.bundle() {
                if let Some((member, _)) = manifest.entries.iter().find(|(_, file)| {
                    crate::backend::plan::hex(&file.sha256) == command.launch.artifact.sha256.as_str()
                }) {
                    source.content = PathBuf::from(&command.launch.cwd).join(member);
                }
            }
            Ok(Box::new(source))
        };
        let detection = program.invocation(index, SoftwarePhase::Before).map(original_recipe).transpose()?;
        // Reuse every original protected coordinate. No current compiler decision, filename
        // convention, or filesystem availability can select a replacement on recovery.
        let files = step
            .materials
            .iter()
            .map(|m| (PathBuf::from(&m.path), m.artifact.clone()))
            .collect();
        let mut mutations = std::collections::BTreeMap::new();
        for (&phase, command) in &step.invocations {
            mutations.insert(phase, original_recipe(command)?);
        }
        if delegate.is_none() {
            delegate = mutations
                .values()
                .find_map(|source| source.delegate.clone())
                .or_else(|| {
                    detection
                        .as_ref()
                        .and_then(|source| source.delegate.clone())
                });
            if delegate.is_none() {
                if let (RunAs::User { account }, SessionRequirement::ActiveUser { session, .. }) =
                    step.detection.context()
                {
                    delegate = Some(
                        helpers.connect_peer(
                            account.subject.as_str(),
                            session
                                .as_str()
                                .split('/')
                                .nth(1)
                                .ok_or(Error::Identity)?
                                .parse()
                                .map_err(|_| Error::Identity)?,
                            session,
                        )?,
                    );
                }
            }
        }
        sources.push(SoftwareStepArtifacts {
            mutations,
            detection,
            files,
        });
    }
    Ok(Artifacts {
        program: sources,
        delegate,
        interpreter: PathBuf::new(),
        content: PathBuf::new(),
        work_root: config.work_root.clone(),
        controlled_input: None,
    })
}

/// Every invocation retained by the original reachable phase closure.
/// Recovery uses only original frozen inputs.
pub(crate) fn software_invocations(
    step: &SoftwareProgramStep,
) -> impl Iterator<Item = &SoftwareInvocation> {
    step.invocations.values().chain(match &step.detection {
        SoftwareDetector::Script { invocation } => Some(invocation.as_ref()),
        _ => None,
    })
}

/// Pure reference projection of the original frozen input. Never reconnect a helper or
/// compile a replacement recipe while determining which physical objects must be retained.
pub(crate) fn references(
    plan: &FrozenExecution,
    config: &ExecutionConfig,
) -> Result<std::collections::BTreeMap<PathBuf, Digest>, Error> {
    let mut references = std::collections::BTreeMap::new();
    let mut insert = |path: PathBuf, digest: &Digest| -> Result<(), Error> {
        let Ok(relative) = path.strip_prefix(&config.material_root) else {
            return Ok(());
        };
        let namespace = relative.components().next().ok_or(Error::Untrusted)?;
        let source = config.material_root.join(namespace).join(digest.as_str());
        // Publication also owns a digest source when the frozen recipe uses a native alias.
        for path in [path, source] {
            if references
                .insert(path, digest.clone())
                .is_some_and(|old| old != *digest)
            {
                return Err(Error::Untrusted);
            }
        }
        if references.len() > 32_768 {
            return Err(Error::Capacity);
        }
        Ok(())
    };
    let p = plan.spec();
    if p.launch.interpreter.profile.id.as_str() == crate::enrollment_worker::PROFILE {
        if let Some(InputValue::Literal { value }) = p.request.parameters.get("nativeProfile") {
            let digest = Digest::new(value.as_str().ok_or(Error::Untrusted)?)
                .map_err(|_| Error::Untrusted)?;
            let source = content(config, &p.run_as, &digest);
            let native = source
                .parent()
                .ok_or(Error::Untrusted)?
                .join(format!("export-{}", digest.as_str()))
                .join("enrollment.mobileconfig");
            insert(native, &digest)?;
        }
    }
    if let Some(program) = p.execution.software_program() {
        for step in &program.steps {
            for material in &step.materials {
                insert(PathBuf::from(&material.path), &material.artifact.sha256)?;
            }
            for invocation in software_invocations(step) {
                let bundle = step.format.bundle().map(|(manifest, _)| manifest);
                let member = bundle.and_then(|manifest| {
                    manifest.entries.iter().find(|(_, member)| {
                        crate::backend::plan::hex(&member.sha256)
                            == invocation.launch.artifact.sha256.as_str()
                    })
                });
                let path = if let Some((name, _)) = member {
                    PathBuf::from(&invocation.launch.cwd).join(name)
                } else {
                    content(
                        config,
                        &invocation.run_as,
                        &invocation.launch.artifact.sha256,
                    )
                };
                insert(path, &invocation.launch.artifact.sha256)?;
                if let (Some(manifest), Some(_)) = (bundle, member) {
                    for (name, member) in &manifest.entries {
                        let digest = Digest::new(crate::backend::plan::hex(&member.sha256))
                            .map_err(|_| Error::Untrusted)?;
                        insert(PathBuf::from(&invocation.launch.cwd).join(name), &digest)?;
                    }
                }
            }
        }
    } else {
        insert(
            content(config, &p.run_as, &p.launch.artifact.sha256),
            &p.launch.artifact.sha256,
        )?;
    }
    Ok(references)
}
