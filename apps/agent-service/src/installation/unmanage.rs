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
#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "channel",
    content = "proof",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum RemovalPermit {
    Mdm(Box<UnmanagePermit>),
    #[cfg(target_os = "macos")]
    Da(rss_mdm_agent_wire::da::RevocationReceipt),
}
impl RemovalPermit {
    fn operation(&self) -> uuid::Uuid {
        match self {
            Self::Mdm(p) => p.task().task_id,
            #[cfg(target_os = "macos")]
            Self::Da(r) => r.operation_id,
        }
    }
    fn verify(&self, d: &Deployment) -> Result<(), Error> {
        match self {
            Self::Mdm(p) => p.verify_handoff(&d.network()?, d.installation_id),
            #[cfg(target_os = "macos")]
            Self::Da(r) => {
                let backend = d.modules.da.backend()?;
                if d.modules.mdm.enabled()
                    || r.operation_id.is_nil()
                    || r.credential.is_nil()
                    || (r.tenant, r.service_id) != (backend.tenant, backend.instance)
                {
                    return Err(Error::Identity);
                }
                Ok(())
            }
        }
    }
}
#[cfg(target_os = "macos")]
fn released_da(
    d: &Deployment,
    operation: uuid::Uuid,
) -> Result<rss_mdm_agent_wire::da::RevocationReceipt, Error> {
    if d.modules.mdm.enabled() || !crate::deployment::never_initialized(&d.state_root)? {
        return Err(Error::Denied);
    }
    agent_client::da::retired_receipt(
        &d.da_root()?,
        d.modules.da.backend()?,
        d.installation_id,
        operation,
    )
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recovery {
    version: u8,
    deployment: Deployment,
    candidate: Candidate,
    permit: RemovalPermit,
    package_managed: bool,
    phase: Phase,
    outcome: Option<UnmanageOutcome>,
    acknowledged: bool,
    credentials_complete: Option<bool>,
    carrier_released: bool,
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
    publish(&path(record.permit.operation())?, &json(record)?)
}
fn load(task: uuid::Uuid) -> Result<Recovery, Error> {
    let bytes = installation_security::read_protected(&path(task)?).map_err(|_| Error::Identity)?;
    let record: Recovery = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
    validate(&record, task)?;
    Ok(record)
}
fn validate(r: &Recovery, task: uuid::Uuid) -> Result<(), Error> {
    if r.version != 1 {
        return Err(Error::Configuration);
    }
    r.permit.verify(&r.deployment)?;
    if r.permit.operation() != task
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
    if !d.modules.mdm.enabled() || crate::da_service::has_retained_identity(d).unwrap_or(true) {
        return false;
    }
    Deployment::default_path()
        .and_then(|p| Candidate::retained(&p, d))
        .and_then(|c| c.format())
        .is_ok_and(|f| matches!(f.as_str(), "macos_pkg" | "windows_exe"))
}
fn prepare(d: &Deployment, permit: RemovalPermit, package_managed: bool) -> Result<(), Error> {
    permit.verify(d)?;
    if path(permit.operation())?.exists() {
        let old = load(permit.operation())?;
        if old.deployment.installation_id != d.installation_id
            || old.package_managed != package_managed
        {
            return Err(Error::Conflict);
        }
        return Ok(());
    }
    if crate::da_service::has_retained_identity(d)? {
        return Err(Error::Denied);
    }
    if matches!(permit, RemovalPermit::Mdm(_)) && !package_managed && !supported(d) {
        return Err(Error::Unsupported);
    }
    let current = Deployment::metadata(&Deployment::default_path()?)?;
    lifecycle::preserved(d, &current)?;
    if current.service.path != d.service.path
        || current.installation_artifact != d.installation_artifact
    {
        return Err(Error::Identity);
    }
    let candidate = Candidate::retained(&Deployment::default_path()?, d)?;
    let checked = match permit {
        RemovalPermit::Mdm(ref p) => {
            if package_managed && candidate.format()? != "windows_msi" {
                return Err(Error::Denied);
            }
            RemovalPermit::Mdm(Box::new(agent_client::inspect_unmanage(
                &d.state_root.join("communication"),
                &d.network()?,
                d.installation_id,
                p.task().task_id,
            )?))
        }
        #[cfg(target_os = "macos")]
        RemovalPermit::Da(ref receipt) => {
            if package_managed
                || candidate.format()? != "macos_pkg"
                || &released_da(d, receipt.operation_id)? != receipt
            {
                return Err(Error::Denied);
            }
            crate::service::platform_sso::uninstall_preflight(d)?;
            permit.clone()
        }
    };
    let record = Recovery {
        version: 1,
        deployment: d.clone(),
        candidate,
        permit: checked,
        package_managed,
        phase: Phase::Prepared,
        outcome: None,
        acknowledged: false,
        credentials_complete: None,
        carrier_released: false,
    };
    validate(&record, permit.operation())?;
    platform::public_directory(&root()?)?;
    d.service
        .verify(&d.service.path)
        .map_err(|_| Error::Identity)?;
    let image = platform::unmanage_binary(d, permit.operation())?;
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
    publish_new(&path(permit.operation())?, &json(&record)?)
}
pub(crate) fn launch(d: &Deployment, permit: &UnmanagePermit) -> Result<(), Error> {
    prepare(d, RemovalPermit::Mdm(Box::new(permit.clone())), false)?;
    if !load(permit.task().task_id)?.carrier_released {
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
    prepare(&d, RemovalPermit::Mdm(Box::new(permit)), true)?;
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
#[cfg(target_os = "macos")]
fn verify_da_state(
    deployment: &Deployment,
    phase: Phase,
    receipt: &rss_mdm_agent_wire::da::RevocationReceipt,
) -> Result<(), Error> {
    // After credential removal this operation owns the validated receipt; the DA journal may already be partly deleted.
    if matches!(
        phase,
        Phase::Prepared | Phase::Stopped | Phase::PayloadRemoved
    ) && agent_client::da::retired_receipt(
        &deployment.da_root()?,
        deployment.modules.da.backend()?,
        deployment.installation_id,
        receipt.operation_id,
    )? != *receipt
    {
        return Err(Error::Identity);
    }
    Ok(())
}
fn execute(task: uuid::Uuid, package_entry: bool) -> Result<(), Error> {
    let mut r = load(task)?;
    if r.phase == Phase::Finished && r.carrier_released {
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
            r.permit.verify(&current)?;
            #[cfg(target_os = "macos")]
            if let RemovalPermit::Da(receipt) = &r.permit {
                if !crate::deployment::never_initialized(&current.state_root)? {
                    return Err(Error::Denied);
                }
                verify_da_state(&current, r.phase, receipt)?;
            }
            if matches!(r.permit, RemovalPermit::Mdm(_))
                && crate::da_service::has_retained_identity(&current)?
            {
                return Err(Error::Denied);
            }
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
        #[cfg(target_os = "macos")]
        crate::service::platform_sso::uninstall_preflight(d)?;
        #[cfg(windows)]
        {
            let root = crate::access_worker::root(&d.execution);
            // Preserve the service and durable fences while user/directory cleanup is pending.
            if root.exists()
                && !device_access::Store::open(&root, d.installation_id)
                    .map_err(|_| Error::Identity)?
                    .hello_cleanup_complete()
                    .map_err(|_| Error::Identity)?
            {
                return Err(Error::Conflict);
            }
        }
        outcome::phase("retirement-quiescence")?;
        // A blocked desktop is a retryable precondition, not a completed worker.
        outcome::phase("retirement-stop")?;
        lifecycle::stop_for_maintenance(d, &config)?;
        advance(&mut r, Phase::Stopped)?;
    }
    // Transport may be unavailable after a crash following key deletion. Cleanup never
    // depends on reviving those keys; absent transport leaves the server unconfirmed.
    let transport = (|| {
        let d = &r.deployment;
        let permit = match &r.permit {
            RemovalPermit::Mdm(p) => p,
            #[cfg(target_os = "macos")]
            RemovalPermit::Da(_) => return Err(Error::Unsupported),
        };
        let provider = crate::DeviceSecrets::open(&d.state_root.join("secrets"), &d.namespace()?)?;
        let client = agent_client::Client::open(
            &d.state_root.join("communication"),
            d.network()?,
            agent_client::OpenMode::Existing,
            provider,
            SystemClock::new()?,
        )?;
        client.unmanage_transport(permit)
    })()
    .ok();
    let deployment = r.deployment.clone();
    let candidate = r.candidate.clone();
    let permit = r.permit.clone();
    let package_managed = r.package_managed;
    let complete = std::cell::Cell::new(r.credentials_complete);
    cleanup(
        r.phase,
        |phase| {
            match phase {
                Phase::Stopped => {
                    outcome::phase("retirement-payload")?;
                    platform::remove(true, &deployment.service.path, &config)?;
                    #[cfg(windows)]
                    crate::access_component::remove_for(&deployment)?;
                    if !package_managed {
                        platform::uninstall_payload(&candidate)?;
                    }
                }
                Phase::PayloadRemoved => {
                    outcome::phase("retirement-credentials")?;
                    let done = match &permit {
                        RemovalPermit::Mdm(p) => {
                            let secrets = crate::DeviceSecrets::open(
                                &deployment.state_root.join("secrets"),
                                &deployment.namespace()?,
                            )?;
                            secrets.0.retire(p.credentials())?
                        }
                        #[cfg(target_os = "macos")]
                        RemovalPermit::Da(_) => {
                            crate::da_service::release_installation(&deployment)?
                        }
                    };
                    complete.set(Some(done));
                }
                Phase::CredentialsRemoved => {
                    outcome::phase("retirement-state")?;
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
                    #[cfg(target_os = "macos")]
                    if matches!(permit, RemovalPermit::Da(_)) {
                        remove_tree(&deployment.da_root()?)?;
                    }
                    remove_file(&config)?;
                }
                Phase::StateRemoved => return Ok(Some(UnmanageOutcome::Unknown)),
                _ => return Err(Error::Conflict),
            }
            Ok(None)
        },
        |next, observation| {
            r.credentials_complete = complete.get();
            if let Some(observation) = observation {
                r.outcome = Some(observation);
            }
            advance(&mut r, next)
        },
    )?;
    // Subject cleanup is terminal independently of carrier reclamation. A restart
    // after its checkpoint must still retry the exact carrier, never repeat deletion.
    outcome::phase("retirement-carrier")?;
    let reclaimed = platform::finish_unmanage(&deployment, task);
    #[cfg(target_os = "macos")]
    {
        r.carrier_released = reclaimed.as_ref().is_ok_and(|gone| *gone);
    }
    #[cfg(windows)]
    {
        r.carrier_released = reclaimed.is_ok();
    }
    if reclaimed.as_ref().is_ok_and(|gone| *gone)
        && !package_managed
        && r.credentials_complete == Some(true)
    {
        r.outcome = Some(UnmanageOutcome::Completed);
    }
    persist(&r)?;
    if let Some(transport) = transport {
        if reclaimed.is_ok() {
            outcome::phase("retirement-result")?;
        }
        r.acknowledged = crate::service::owner_runtime()?
            .block_on(transport.report(r.outcome.ok_or(Error::Conflict)?))
            .is_ok();
        persist(&r)?;
    }
    reclaimed.map(|_| ())
}
#[cfg(target_os = "macos")]
fn observed_state(
    phase: Phase,
    gone: bool,
    credentials_complete: Option<bool>,
    package_managed: bool,
) -> &'static str {
    if phase != Phase::Finished || !gone {
        "pending"
    } else if credentials_complete == Some(true) && !package_managed {
        "completed"
    } else {
        "unknown"
    }
}
#[cfg(target_os = "macos")]
pub(super) fn observed_status(
    config: &Path,
    id: uuid::Uuid,
) -> Result<Option<serde_json::Value>, Error> {
    let dir = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-operations");
    let path = dir.join(format!("{id}.retirement-work.json"));
    if !path.try_exists()? {
        return Ok(None);
    }
    let r: Recovery = serde_json::from_slice(
        &installation_security::read_protected(&path).map_err(|_| Error::Identity)?,
    )
    .map_err(|_| Error::Storage)?;
    validate(&r, id)?;
    let removed = matches!(
        r.phase,
        Phase::PayloadRemoved | Phase::CredentialsRemoved | Phase::StateRemoved | Phase::Finished
    );
    let gone = r.carrier_released
        || (r.phase == Phase::Finished && platform::carrier_gone(&r.deployment, id)?);
    let state = observed_state(r.phase, gone, r.credentials_complete, r.package_managed);
    Ok(Some(
        serde_json::json!({"operation":id,"state":state,"phase":r.phase,"payloadRemoved":removed,"credentialsComplete":r.credentials_complete,"carrierReleased":gone,"acknowledged":r.acknowledged}),
    ))
}

pub(super) fn run(task: uuid::Uuid, binary: &Path) -> Result<(), Error> {
    if !path(task)?.exists() {
        let d = Deployment::metadata(&Deployment::default_path()?)?;
        if d.service.path != binary {
            return Err(Error::Identity);
        }
        let permit = if d.modules.mdm.enabled() {
            RemovalPermit::Mdm(Box::new(agent_client::inspect_unmanage(
                &d.state_root.join("communication"),
                &d.network()?,
                d.installation_id,
                task,
            )?))
        } else {
            #[cfg(target_os = "macos")]
            {
                RemovalPermit::Da(released_da(&d, task)?)
            }
            #[cfg(not(target_os = "macos"))]
            {
                return Err(Error::Unsupported);
            }
        };
        prepare(&d, permit, false)?;
    }
    let recovery = load(task)?;
    if recovery.deployment.service.path != binary {
        return Err(Error::Identity);
    }
    if std::env::current_exe()? == recovery.deployment.service.path {
        // Authorized product uninstall hands off to the same independent carrier;
        // it never attempts to delete its own active payload inline.
        if !recovery.carrier_released {
            platform::launch_unmanage(&recovery.deployment, task)?;
        }
        return Ok(());
    }
    let result = execute(task, false);
    if let Err(error) = result {
        outcome::failed(error);
    } else if load(task)?.outcome == Some(UnmanageOutcome::Completed) {
        outcome::succeeded();
    } else {
        outcome::unknown();
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
    #[cfg(target_os = "macos")]
    #[test]
    fn partial_da_journal_removal_replays_the_credential_checkpoint() {
        use rss_mdm_agent_wire::da::RevocationReceipt;
        let temp = crate::protocol_test_support::Root::new();
        let mut deployment = crate::deployment::tests::deployment();
        deployment.state_root = temp.path.join("state");
        let backend = agent_client::da::Backend {
            origin: "https://fixture.example".into(),
            browser: None,
            ca_pem: String::new(),
            instance: uuid::Uuid::new_v4(),
            tenant: uuid::Uuid::new_v4(),
        };
        let receipt = RevocationReceipt {
            service_id: backend.instance,
            tenant: backend.tenant,
            device: rss_mdm_agent_wire::DeviceId::new_v4(),
            credential: uuid::Uuid::new_v4(),
            operation_id: uuid::Uuid::new_v4(),
        };
        deployment.modules.mdm = Module::Disabled;
        deployment.modules.da = Module::Enabled { backend };
        let root = deployment.da_root().unwrap();
        platform_private_storage::directory(root.parent().unwrap()).unwrap();
        platform_private_storage::directory(&root).unwrap();
        std::fs::write(root.join("journal-fragment"), b"partial").unwrap();
        assert!(verify_da_state(&deployment, Phase::PayloadRemoved, &receipt).is_err());
        verify_da_state(&deployment, Phase::CredentialsRemoved, &receipt).unwrap();
        remove_tree(&root).unwrap();
        verify_da_state(&deployment, Phase::CredentialsRemoved, &receipt).unwrap();
        assert_eq!(
            observed_state(Phase::Prepared, false, None, false),
            "pending"
        );
        assert_eq!(
            observed_state(Phase::Finished, false, Some(true), false),
            "pending"
        );
        assert_eq!(
            observed_state(Phase::Finished, true, Some(false), false),
            "unknown"
        );
        assert_eq!(
            observed_state(Phase::Finished, true, Some(true), false),
            "completed"
        );
    }
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
