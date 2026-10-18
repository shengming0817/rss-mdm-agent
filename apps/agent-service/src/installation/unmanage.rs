//! Fixed recoverable installation retirement; no network-selected paths or commands.
use super::*;
use agent_client::UnmanagePermit;
use rss_mdm_agent_wire::runtime::UnmanageOutcome;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Prepared,
    Stopped,
    PayloadRemoved,
    CredentialsRemoved,
    StateRemoved,
    Finished,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recovery {
    deployment: Deployment,
    candidate: Candidate,
    permit: UnmanagePermit,
    package_managed: bool,
    phase: Phase,
    outcome: Option<UnmanageOutcome>,
    acknowledged: bool,
}
fn root() -> Result<PathBuf, Error> {
    Ok(Deployment::default_path()?
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-operations"))
}
fn path(task: uuid::Uuid) -> Result<PathBuf, Error> {
    Ok(root()?.join(format!("{task}.retirement-work.json")))
}
fn persist(record: &Recovery) -> Result<(), Error> {
    publish(&path(record.permit.task().task_id)?, &json(record)?)
}
fn load(task: uuid::Uuid) -> Result<Recovery, Error> {
    let bytes = installation_security::read_protected(&path(task)?).map_err(|_| Error::Identity)?;
    let record: Recovery = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
    validate(&record, task)?;
    Ok(record)
}
fn validate(r: &Recovery, task: uuid::Uuid) -> Result<(), Error> {
    r.permit
        .verify_handoff(&r.deployment.network()?, r.deployment.installation_id)?;
    if r.permit.task().task_id != task
        || r.candidate.service.path != r.deployment.service.path
        || r.candidate.root.parent() != Some(platform::payload_parent()?.as_path())
        || r.candidate.root.as_path()
            != r.deployment
                .service
                .path
                .parent()
                .ok_or(Error::Configuration)?
        || [
            &r.deployment.state_root,
            &r.deployment.execution.material_root,
            &r.deployment.execution.work_root,
        ]
        .iter()
        .any(|p| {
            p.starts_with(&r.candidate.root) || root().is_ok_and(|recovery| recovery.starts_with(p))
        })
    {
        return Err(Error::Identity);
    }
    Ok(())
}
pub(crate) fn supported(d: &Deployment) -> bool {
    Deployment::default_path()
        .and_then(|p| Candidate::retained(&p, d))
        .and_then(|c| c.format())
        .is_ok_and(|f| matches!(f.as_str(), "macos_pkg" | "nsis_exe" | "windows_msi"))
}
fn prepare(d: &Deployment, permit: &UnmanagePermit, package_managed: bool) -> Result<(), Error> {
    if path(permit.task().task_id)?.exists() {
        let old = load(permit.task().task_id)?;
        if old.deployment.installation_id != d.installation_id
            || old.package_managed != package_managed
        {
            return Err(Error::Conflict);
        }
        return Ok(());
    }
    if !supported(d) {
        return Err(Error::Unsupported);
    }
    let current = Deployment::metadata(&Deployment::default_path()?)?;
    lifecycle::preserved(d, &current)?;
    if current.service.path != d.service.path
        || current.installation_artifact != d.installation_artifact
    {
        return Err(Error::Identity);
    }
    let checked = agent_client::inspect_unmanage(
        &d.state_root.join("communication"),
        &d.network()?,
        d.installation_id,
        permit.task().task_id,
    )?;
    let candidate = Candidate::retained(&Deployment::default_path()?, d)?;
    if package_managed && candidate.format()? != "windows_msi" {
        return Err(Error::Denied);
    }
    let record = Recovery {
        deployment: d.clone(),
        candidate,
        permit: checked,
        package_managed,
        phase: Phase::Prepared,
        outcome: None,
        acknowledged: false,
    };
    validate(&record, permit.task().task_id)?;
    platform::public_directory(&root()?)?;
    d.service
        .verify(&d.service.path)
        .map_err(|_| Error::Identity)?;
    let image = platform::unmanage_binary(d, permit.task().task_id)?;
    match std::fs::symlink_metadata(&image) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            publish_new(&image, &read_image(&d.service.path, &d.service.sha256)?)?
        }
        Err(e) => return Err(e.into()),
        Ok(_) => {
            read_image(&image, &d.service.sha256)?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o755))?;
    }
    Artifact {
        path: image.clone(),
        ..d.service.clone()
    }
    .verify(&image)
    .map_err(|_| Error::Identity)?;
    publish_new(&path(permit.task().task_id)?, &json(&record)?)
}
pub(crate) fn launch(d: &Deployment, permit: &UnmanagePermit) -> Result<(), Error> {
    prepare(d, permit, false)?;
    if load(permit.task().task_id)?.phase != Phase::Finished {
        platform::launch_unmanage(d, permit.task().task_id)?;
    }
    Ok(())
}
/// The package manager owns file removal, while authorization, credentials and terminal
/// uncertainty still pass through exactly the same retirement owner.
pub(super) fn package_remove(config: &Path, binary: &Path, task: uuid::Uuid) -> Result<(), Error> {
    let d = Deployment::metadata(config)?;
    if config != Deployment::default_path()? || d.service.path != binary {
        return Err(Error::Identity);
    }
    let permit = agent_client::inspect_unmanage(
        &d.state_root.join("communication"),
        &d.network()?,
        d.installation_id,
        task,
    )?;
    prepare(&d, &permit, true)?;
    // Register the independent recovery carrier before the package process can stop us.
    platform::launch_unmanage(&d, task)?;
    execute(task, true)
}
fn remove_tree(path: &Path) -> Result<(), Error> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(_) => {
            platform_private_storage::validate(path)?;
            std::fs::remove_dir_all(path)?;
            Ok(())
        }
    }
}
fn remove_file(path: &Path) -> Result<(), Error> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(_) => {
            installation_security::protected(path).map_err(|_| Error::Identity)?;
            std::fs::remove_file(path)?;
            Ok(())
        }
    }
}
fn advance(record: &mut Recovery, next: Phase) -> Result<(), Error> {
    record.phase = next;
    persist(record)
}
/// Fixed installation phases. Each effect may be replayed after a lost checkpoint;
/// an effect or checkpoint error prevents every later phase and terminal success.
fn cleanup(
    mut phase: Phase,
    mut effect: impl FnMut(Phase) -> Result<Option<UnmanageOutcome>, Error>,
    mut checkpoint: impl FnMut(Phase, Option<UnmanageOutcome>) -> Result<(), Error>,
) -> Result<(), Error> {
    loop {
        let next = match phase {
            Phase::Stopped => Phase::PayloadRemoved,
            Phase::PayloadRemoved => Phase::CredentialsRemoved,
            Phase::CredentialsRemoved => Phase::StateRemoved,
            Phase::StateRemoved => Phase::Finished,
            Phase::Finished => return Ok(()),
            Phase::Prepared => return Err(Error::Conflict),
        };
        let observation = effect(phase)?;
        checkpoint(next, observation)?;
        phase = next;
    }
}
fn execute(task: uuid::Uuid, package_entry: bool) -> Result<(), Error> {
    let mut r = load(task)?;
    if r.phase == Phase::Finished {
        return Ok(());
    }
    if package_entry && !r.package_managed {
        return Err(Error::Identity);
    }
    let d = &r.deployment;
    let image = if package_entry {
        d.service.path.clone()
    } else {
        platform::unmanage_binary(d, task)?
    };
    Artifact {
        path: image,
        ..d.service.clone()
    }
    .verify(&std::env::current_exe()?)
    .map_err(|_| Error::Identity)?;
    let config = Deployment::default_path()?;
    // The installation lock is held by every caller. Old authority never owns a
    // replacement installation that happens to reuse the same filesystem paths.
    match std::fs::symlink_metadata(&config) {
        Ok(_) => {
            let current = Deployment::metadata(&config)?;
            lifecycle::preserved(&r.deployment, &current)?;
            if current.service.path != r.deployment.service.path
                || current.installation_artifact != r.deployment.installation_artifact
            {
                return Err(Error::Identity);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    if r.phase == Phase::Prepared {
        // A blocked desktop is a retryable precondition, not a completed worker.
        platform::quiescent(d, &config)?;
        platform::stop(true, &d.service.path, &config)?;
        advance(&mut r, Phase::Stopped)?;
    }
    // Transport may be unavailable after a crash following key deletion. Cleanup never
    // depends on reviving those keys; absent transport leaves the server unconfirmed.
    let transport = (|| {
        let d = &r.deployment;
        let provider = crate::DeviceSecrets::open(&d.state_root.join("secrets"), &d.namespace()?)?;
        let client = agent_client::Client::open(
            &d.state_root.join("communication"),
            d.network()?,
            agent_client::OpenMode::Existing,
            provider,
            SystemClock::new()?,
        )?;
        client.unmanage_transport(&r.permit)
    })()
    .ok();
    let deployment = r.deployment.clone();
    let candidate = r.candidate.clone();
    let permit = r.permit.clone();
    let package_managed = r.package_managed;
    cleanup(
        r.phase,
        |phase| {
            match phase {
                Phase::Stopped => {
                    platform::remove(true, &deployment.service.path, &config)?;
                    if !package_managed {
                        platform::uninstall_payload(&candidate)?;
                    }
                }
                Phase::PayloadRemoved => {
                    let secrets = crate::DeviceSecrets::open(
                        &deployment.state_root.join("secrets"),
                        &deployment.namespace()?,
                    )?;
                    if !secrets.0.retire(permit.credentials())? {
                        return Err(Error::Conflict);
                    }
                }
                Phase::CredentialsRemoved => {
                    let mut roots = vec![
                        &deployment.state_root,
                        &deployment.execution.material_root,
                        &deployment.execution.work_root,
                    ];
                    roots.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
                    roots.dedup();
                    for root in roots {
                        remove_tree(root)?;
                    }
                    remove_file(&config)?;
                }
                Phase::StateRemoved => return Ok(Some(UnmanageOutcome::Unknown)),
                _ => return Err(Error::Conflict),
            }
            Ok(None)
        },
        |next, observation| {
            if let Some(observation) = observation {
                r.outcome = Some(observation);
            }
            advance(&mut r, next)
        },
    )?;
    // All recoverable product cleanup is durably terminal Unknown before relinquishing
    // the recovery carrier. A failure here is terminal uncertainty, never a queued retry
    // without an executable. Only positively verified reclamation upgrades the result.
    let gone = platform::finish_unmanage(&deployment, task).unwrap_or(false);
    if gone && !package_managed {
        r.outcome = Some(UnmanageOutcome::Completed);
        persist(&r)?;
    }
    if let Some(transport) = transport {
        r.acknowledged = crate::service::owner_runtime()?
            .block_on(transport.report(r.outcome.ok_or(Error::Conflict)?))
            .is_ok();
        persist(&r)?;
    }
    Ok(())
}
pub(super) fn run(task: uuid::Uuid, binary: &Path) -> Result<(), Error> {
    if !path(task)?.exists() {
        let d = Deployment::metadata(&Deployment::default_path()?)?;
        if d.service.path != binary {
            return Err(Error::Identity);
        }
        let permit = agent_client::inspect_unmanage(
            &d.state_root.join("communication"),
            &d.network()?,
            d.installation_id,
            task,
        )?;
        prepare(&d, &permit, false)?;
    }
    let recovery = load(task)?;
    if recovery.deployment.service.path != binary {
        return Err(Error::Identity);
    }
    if std::env::current_exe()? == recovery.deployment.service.path {
        // Authorized product uninstall hands off to the same independent carrier;
        // it never attempts to delete its own active payload inline.
        return launch(&recovery.deployment, &recovery.permit);
    }
    let result = execute(task, false);
    if let Err(error) = result {
        outcome::failed(error);
    } else {
        outcome::succeeded();
    }
    outcome::terminal();
    publish(
        &root()?.join(format!("{task}.retirement.json")),
        &json(&outcome::snapshot())?,
    )?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn retirement_cleanup_replays_lost_checkpoints_and_never_skips_failed_effects() {
        let phases = [
            Phase::Stopped,
            Phase::PayloadRemoved,
            Phase::CredentialsRemoved,
            Phase::StateRemoved,
        ];
        for failed in phases {
            for checkpoint_failure in [false, true] {
                let durable = Cell::new(Phase::Stopped);
                let observed = Cell::new(None);
                let mut calls = Vec::new();
                let run = cleanup(
                    durable.get(),
                    |phase| {
                        calls.push(phase);
                        if phase == failed && !checkpoint_failure {
                            return Err(Error::Unavailable);
                        }
                        Ok(match phase {
                            Phase::StateRemoved => Some(UnmanageOutcome::Unknown),
                            _ => None,
                        })
                    },
                    |next, outcome| {
                        if checkpoint_failure && durable.get() == failed {
                            return Err(Error::Unavailable);
                        }
                        durable.set(next);
                        if outcome.is_some() {
                            observed.set(outcome);
                        }
                        Ok(())
                    },
                );
                assert!(run.is_err());
                assert_eq!(calls.last(), Some(&failed));
                assert_ne!(observed.get(), Some(UnmanageOutcome::Completed));
                let mut replay = Vec::new();
                cleanup(
                    durable.get(),
                    |phase| {
                        replay.push(phase);
                        Ok(None)
                    },
                    |next, _| {
                        durable.set(next);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(replay.first(), Some(&failed));
                assert_eq!(durable.get(), Phase::Finished);
            }
        }
    }
    #[test]
    fn retirement_state_cleanup_is_repeatable_and_rejects_symlink_escape() {
        let root = std::env::temp_dir().join(format!("retirement-{}", uuid::Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let owned = root.join("owned");
        platform_private_storage::directory(&owned).unwrap();
        std::fs::write(owned.join("state"), b"old installation").unwrap();
        remove_tree(&owned).unwrap();
        remove_tree(&owned).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, &owned).unwrap();
            assert!(remove_tree(&owned).is_err());
            assert!(root.is_dir());
            std::fs::remove_file(&owned).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
}
