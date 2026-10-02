//! Reconstruct physical material coordinates from the original journal input, never a new Start.
use crate::{ExecutionConfig, UserResources};
use agent_client::Error;
use execution_contract::*;
use execution_runner::{Artifacts, SoftwareStepArtifacts};
use std::{path::PathBuf, sync::Arc};

fn content(config: &ExecutionConfig, run_as: &RunAs, digest: &Digest) -> PathBuf {
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
    let image = config
        .interpreters
        .iter()
        .find(|p| p.image.sha256 == launch.interpreter.artifact.sha256.as_str())
        .ok_or(Error::Configuration)?;
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
            let connection = execution_runner::helper::Connection::connect(
                execution_runner::host::PeerPolicy {
                    images: vec![helpers.image.clone()],
                    subjects: vec![account.subject.as_str().into()],
                    interactive: true,
                },
                account.subject.as_str().into(),
                session,
            )?;
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
        interpreter: image.image.path.clone(),
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
        let mut mutation = program
            .invocation(index, SoftwarePhase::Mutation)
            .map(|c| {
                recipe(
                    config,
                    helpers,
                    &c.launch,
                    &c.run_as,
                    &c.session_requirement,
                )
                .map(Box::new)
            })
            .transpose()?;
        let mut detection = program
            .invocation(index, SoftwarePhase::Before)
            .map(|c| {
                recipe(
                    config,
                    helpers,
                    &c.launch,
                    &c.run_as,
                    &c.session_requirement,
                )
                .map(Box::new)
            })
            .transpose()?;
        for (source, phase) in [
            (&mut mutation, SoftwarePhase::Mutation),
            (&mut detection, SoftwarePhase::Before),
        ] {
            if let (Some(source), Some(invocation), Some(bundle)) = (
                source.as_mut(),
                program.invocation(index, phase),
                step.format.bundle().map(|(manifest, _)| manifest),
            ) {
                if let Some((member, _)) = bundle.entries.iter().find(|(_, f)| {
                    crate::plan::hex(&f.sha256) == invocation.launch.artifact.sha256.as_str()
                }) {
                    source.content = PathBuf::from(&invocation.launch.cwd).join(member);
                }
            }
        }
        // Reuse every original protected coordinate. No current compiler decision, filename
        // convention, or filesystem availability can select a replacement on recovery.
        let files = step
            .materials
            .iter()
            .map(|m| (PathBuf::from(&m.path), m.artifact.clone()))
            .collect();
        let mut mutations = std::collections::BTreeMap::new();
        for phase in [
            SoftwarePhase::Mutation,
            SoftwarePhase::Upgrade,
            SoftwarePhase::Removal,
            SoftwarePhase::Attach,
            SoftwarePhase::Stage,
            SoftwarePhase::Cleanup,
        ] {
            if let Some(command) = program.invocation(index, phase) {
                mutations.insert(
                    phase,
                    Box::new(recipe(
                        config,
                        helpers,
                        &command.launch,
                        &command.run_as,
                        &command.session_requirement,
                    )?),
                );
            }
        }
        if let Some(original) = mutation {
            mutations.insert(SoftwarePhase::Mutation, original);
        }
        if delegate.is_none() && matches!(step.install.run_as, RunAs::User { .. }) {
            delegate = recipe(
                config,
                helpers,
                &step.install.launch,
                &step.install.run_as,
                &step.install.session_requirement,
            )?
            .delegate;
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
