//! Fixed installation retirement. Network input can select no paths or executables.
use super::*;
use agent_client::UnmanagePermit;
use rss_mdm_agent_wire::runtime::UnmanageOutcome;

pub(crate) fn supported(deployment: &Deployment) -> bool {
    Deployment::default_path()
        .and_then(|path| Candidate::retained(&path, deployment))
        .and_then(|c| c.format())
        .is_ok_and(|format| matches!(format.as_str(), "macos_pkg" | "nsis_exe" | "windows_msi"))
}
pub(crate) fn launch(deployment: &Deployment, permit: &UnmanagePermit) -> Result<(), Error> {
    if !supported(deployment)
        || permit.installation() != deployment.installation_id
        || !permit.drained()
    {
        return Err(Error::Denied);
    }
    if permit.outcome().is_some() {
        return Ok(());
    }
    let current = Deployment::metadata(&Deployment::default_path()?)?;
    lifecycle::preserved(deployment, &current)?;
    if current.service.path != deployment.service.path
        || current.installation_artifact != deployment.installation_artifact
    {
        return Err(Error::Identity);
    }
    platform::launch_unmanage(deployment, permit.task().task_id)
}

pub(super) fn authorize(
    deployment: &Deployment,
    task: uuid::Uuid,
) -> Result<UnmanagePermit, Error> {
    agent_client::inspect_unmanage(
        &deployment.state_root.join("communication"),
        &deployment.network()?,
        deployment.installation_id,
        task,
    )
}

fn execute(
    deployment: &Deployment,
    config: &Path,
    candidate: &Candidate,
    task: uuid::Uuid,
) -> Result<(), Error> {
    let permit = authorize(deployment, task)?;
    if permit.outcome().is_some() {
        return Err(Error::Conflict);
    }
    // The persisted drain was produced before launching this independent owner.
    // The CLI cannot manufacture it; no shutdown or filesystem effect precedes this check.
    outcome::phase("uninstall-quiescence")?;
    platform::quiescent(deployment, config)?;
    platform::stop(true, &deployment.service.path, config)?;
    let image = platform::unmanage_binary(deployment, task)?;
    let artifact = Artifact {
        path: image.clone(),
        ..deployment.service.clone()
    };
    artifact
        .verify(&std::env::current_exe()?)
        .map_err(|_| Error::Identity)?;
    deployment.preflight()?;
    let namespace = deployment.namespace()?;
    let provider = crate::DeviceSecrets::open(&deployment.state_root.join("secrets"), &namespace)?;
    provider.verify_storage(&deployment.state_root)?;
    let mut client = agent_client::Client::open(
        &deployment.state_root.join("communication"),
        deployment.network()?,
        agent_client::OpenMode::Existing,
        provider,
        SystemClock::new()?,
    )?;
    let permit = client
        .accepted_unmanage(deployment.installation_id)?
        .ok_or(Error::Denied)?;
    if !permit.drained() || permit.task().task_id != task || permit.outcome().is_some() {
        return Err(Error::Conflict);
    }
    let transport = client.unmanage_transport(&permit)?;
    let references = client.unmanage_credentials(&permit)?;
    let secrets = crate::DeviceSecrets::open(
        &deployment.state_root.join("secrets"),
        &deployment.namespace()?,
    )?;
    let removed = (|| {
        outcome::phase("uninstall-registration")?;
        platform::remove(true, &deployment.service.path, config)?;
        outcome::phase("uninstall-payload")?;
        platform::uninstall_payload(candidate)?;
        outcome::phase("uninstall-credentials")?;
        if !secrets.0.retire(&references)? {
            return Err(Error::Conflict);
        }
        platform::finish_unmanage(deployment, task)
    })();
    let effect = match &removed {
        Ok(true) => UnmanageOutcome::Completed,
        Ok(false) | Err(_) => UnmanageOutcome::Unknown,
    };
    // A lost ACK is retained as unconfirmed; it never causes another uninstall or new identity.
    let reported = crate::service::owner_runtime()?
        .block_on(client.finish_unmanage(&permit, transport, effect));
    drop(client);
    if removed.is_ok() {
        // Preserve installation outcome/audit outside the credential and execution namespace.
        // A later installation must initialize a fresh registration rather than reuse old tasks.
        let mut roots = vec![
            &deployment.state_root,
            &deployment.execution.material_root,
            &deployment.execution.work_root,
        ];
        roots.sort_by_key(|root| std::cmp::Reverse(root.components().count()));
        roots.dedup();
        for root in &roots {
            platform_private_storage::validate(root)?;
        }
        for root in roots {
            std::fs::remove_dir_all(root)?;
        }
        std::fs::remove_file(config)?;
    }
    if !removed? {
        return Err(Error::Unavailable);
    }
    reported
}

pub(super) fn run(
    deployment: &Deployment,
    config: &Path,
    candidate: &Candidate,
    task: uuid::Uuid,
) -> Result<(), Error> {
    authorize(deployment, task)?;
    let root = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-operations");
    platform::public_directory(&root)?;
    let result_path = root.join(format!("{task}.retirement.json"));
    publish(&result_path, &json(&outcome::snapshot())?)?;
    let result = execute(deployment, config, candidate, task);
    if let Err(error) = result {
        outcome::failed(error);
    } else {
        outcome::succeeded();
    }
    outcome::terminal();
    publish(&result_path, &json(&outcome::snapshot())?)?;
    result
}
