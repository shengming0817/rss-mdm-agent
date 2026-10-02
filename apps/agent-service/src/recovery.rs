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
        // All step files were published into the task's user namespace if any command needs it.
        let account = program
            .steps
            .iter()
            .flat_map(|s| {
                std::iter::once(&s.install)
                    .chain(s.uninstall.iter())
                    .chain(match &s.detection {
                        SoftwareDetector::Script { invocation } => Some(invocation.as_ref()),
                        _ => None,
                    })
            })
            .map(|i| &i.run_as)
            .find(|r| matches!(r, RunAs::User { .. }))
            .unwrap_or(&p.run_as);
        let original = content(config, account, &step.payload.sha256);
        let payload = match crate::software::native_export_name(
            step.format.adapter(),
            step.package.as_str(),
        )? {
            Some(name) => {
                let export = original
                    .parent()
                    .ok_or(Error::Configuration)?
                    .join(format!("export-{}", step.payload.sha256.as_str()))
                    .join(name);
                retained_payload(
                    original,
                    export,
                    std::iter::once(&step.install)
                        .chain(step.uninstall.iter())
                        .flat_map(|c| c.launch.argv.iter()),
                )
            }
            None => original,
        };
        let mut files = vec![(payload, step.payload.clone())];
        for command in std::iter::once(&step.install).chain(step.uninstall.iter()) {
            for manager in &config.managers {
                if command.launch.argv.iter().any(|a| matches!(a, LaunchArg::Literal { value } if Some(value.as_str()) == manager.image.path.to_str())) {
                    files.push((manager.image.path.clone(), ExactArtifactRef { resource: crate::plan::reference("native-manager", &manager.image.sha256)?, sha256: Digest::new(&manager.image.sha256).map_err(|_| Error::Configuration)? }));
                }
            }
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
            mutation,
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

// The protected journal keeps the original invocation coordinates. Current compilation rules
// never rewrite them, and filesystem availability never chooses a replacement payload.
fn retained_payload<'a>(
    original: PathBuf,
    export: PathBuf,
    argv: impl Iterator<Item = &'a LaunchArg>,
) -> PathBuf {
    if argv.into_iter().any(|arg| matches!(arg, LaunchArg::Literal { value } if Some(value.as_str()) == export.to_str())) {
        export
    } else {
        original
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_keeps_frozen_package_coordinates_when_exports_differ() {
        let root =
            std::env::temp_dir().join(format!("rss-retained-payload-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let original = root.join("original-digest");
        std::fs::write(&original, b"retained package").unwrap();
        for leaf in ["package.pkg", "package.msi"] {
            let export = root.join("export-digest").join(leaf);
            let old = [LaunchArg::Literal {
                value: original.to_str().unwrap().into(),
            }];
            let new = [LaunchArg::Literal {
                value: export.to_str().unwrap().into(),
            }];
            assert!(!export.exists());
            assert_eq!(
                retained_payload(original.clone(), export.clone(), old.iter()),
                original
            );
            assert_eq!(
                retained_payload(original.clone(), export.clone(), new.iter()),
                export
            );
            std::fs::create_dir_all(export.parent().unwrap()).unwrap();
            std::fs::hard_link(&original, &export).unwrap();
            // Publication of a new filename does not change an already frozen invocation.
            assert_eq!(
                retained_payload(original.clone(), export, old.iter()),
                original
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
