use super::*;
#[derive(Default)]
struct Options {
    values: BTreeMap<String, PathBuf>,
}
impl Options {
    fn parse(args: &[OsString]) -> Result<(String, Self), Error> {
        let action = args
            .first()
            .and_then(|s| s.to_str())
            .ok_or(Error::Configuration)?
            .to_owned();
        let mut values = BTreeMap::new();
        let tail = &args[1..];
        if !tail.len().is_multiple_of(2) {
            return Err(Error::Configuration);
        }
        for pair in tail.chunks_exact(2) {
            let key = pair[0].to_str().ok_or(Error::Configuration)?;
            if ![
                "--scope",
                "--binary",
                "--config",
                "--candidate-binary",
                "--candidate-config",
                "--candidate",
                "--profile",
                "--envelope",
                "--grant",
                "--secret-file",
                "--operation",
            ]
            .contains(&key)
                || values.insert(key.into(), PathBuf::from(&pair[1])).is_some()
            {
                return Err(Error::Configuration);
            }
        }
        Ok((action, Self { values }))
    }
    fn path(&self, key: &str) -> Result<PathBuf, Error> {
        let path = self.values.get(key).ok_or(Error::Configuration)?;
        if !path.is_absolute()
            || path.components().any(|p| {
                matches!(
                    p,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(Error::Configuration);
        }
        Ok(path.clone())
    }
    fn config(&self) -> Result<PathBuf, Error> {
        if self.values.contains_key("--config") {
            self.path("--config")
        } else {
            Deployment::default_path()
        }
    }
    fn system(&self) -> Result<bool, Error> {
        match self
            .values
            .get("--scope")
            .and_then(|p| p.to_str())
            .unwrap_or("system")
        {
            "system" => Ok(true),
            "user" => Ok(false),
            _ => Err(Error::Configuration),
        }
    }
}
pub(super) fn run(args: &[OsString]) -> Result<(), Error> {
    let (action, opts) = Options::parse(args)?;
    if action == "target" {
        println!(
            "{}",
            serde_json::to_string(&Target::current().map_err(|_| Error::Unsupported)?)
                .map_err(|_| Error::Configuration)?
        );
        return Ok(());
    }
    if action == "schema" {
        println!(
            "{}",
            serde_json::to_string_pretty(&bootstrap_schema()?).map_err(|_| Error::Configuration)?
        );
        return Ok(());
    }
    if action == "operation-status" {
        let id = opts
            .values
            .get("--operation")
            .and_then(|p| p.to_str())
            .ok_or(Error::Configuration)?;
        return platform::operation_status(
            uuid::Uuid::parse_str(id).map_err(|_| Error::Configuration)?,
        );
    }
    if opts.system()? {
        if platform::dispatch(args)? {
            return Ok(());
        }
    } else if execution_ipc::host::current_subject().map_err(crate::error::app_error)?
        == if cfg!(windows) { "S-1-5-18" } else { "0" }
    {
        return Err(Error::Identity);
    }
    let _lock = installation_lock(&opts.config()?, opts.system()?)?;
    outcome::phase("locked-preflight")?;
    match action.as_str() {
        "install" => {
            let binary = opts.path("--binary")?;
            let config = opts.config()?;
            let deployment = Deployment::load(&config)?;
            if deployment.service.path != binary {
                return Err(Error::Identity);
            }
            platform::register(opts.system()?, &binary, &config)?;
        }
        "remove" => {
            let binary = opts.path("--binary")?;
            let config = opts.config()?;
            if opts.system()? {
                let deployment = Deployment::load(&config)?;
                platform::quiescent(&deployment, &config)?;
            }
            platform::remove(opts.system()?, &binary, &config)?;
        }
        "uninstall" => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            let config = opts.config()?;
            if config != Deployment::default_path()? {
                return Err(Error::Configuration);
            }
            let deployment = Deployment::metadata(&config)?;
            let root = deployment
                .service
                .path
                .parent()
                .ok_or(Error::Configuration)?;
            if opts.path("--binary")? != deployment.service.path
                || root.parent() != Some(platform::payload_parent()?.as_path())
                || [
                    &deployment.state_root,
                    &deployment.execution.material_root,
                    &deployment.execution.work_root,
                ]
                .iter()
                .any(|p| p.starts_with(root))
            {
                return Err(Error::Identity);
            }
            let candidate = Candidate::retained(&config, &deployment)?;
            if candidate.root != root {
                return Err(Error::Identity);
            }
            outcome::phase("uninstall-quiescence")?;
            platform::quiescent(&deployment, &config)?;
            outcome::phase("uninstall-registration")?;
            platform::remove(true, &deployment.service.path, &config)?;
            outcome::phase("uninstall-payload")?;
            platform::uninstall_payload(&candidate)?;
        }
        "status" => {
            platform::status(opts.system()?)?;
        }
        "refresh" => {
            if !opts.system()? {
                return Err(Error::Configuration);
            }
            refresh(
                &opts.path("--binary")?,
                &opts.config()?,
                &opts.path("--candidate-binary")?,
                &opts.path("--candidate-config")?,
            )?;
        }
        "upgrade-device-pki" => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            upgrade_device_pki(&opts.config()?, &opts.path("--candidate")?)?;
        }
        "apply" => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            apply(
                &opts.path("--candidate")?,
                opts.values.get("--profile"),
                &opts.config()?,
            )?;
        }
        "recover" => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            let config = opts.config()?;
            let deployment = Deployment::load(&config)?;
            let grant: GrantImport = serde_json::from_slice(
                &installation_security::read_protected(&opts.path("--grant")?)
                    .map_err(|_| Error::Configuration)?,
            )
            .map_err(|_| Error::Configuration)?;
            grant.validate(deployment.network()?.platform, SystemClock::new()?.now()?)?;
            let bytes =
                zeroize::Zeroizing::new(platform::read_secret(&opts.path("--secret-file")?)?);
            let secret = rss_mdm_agent_wire::runtime::Secret::parse(
                std::str::from_utf8(&bytes)
                    .map_err(|_| Error::Identity)?
                    .trim(),
            )?;
            platform::stop(true, &deployment.service.path, &config)?;
            let result =
                crate::service::owner_runtime()?.block_on(deployment.recover(grant, &secret));
            let restart = platform::register(true, &deployment.service.path, &config);
            result?;
            restart?;
        }
        "import" => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            let config = opts.config()?;
            let deployment = Deployment::load(&config)?;
            let envelope = opts.path("--envelope")?;
            let metadata: Bootstrap = serde_json::from_slice(
                &installation_security::read_protected(&envelope)
                    .map_err(|_| Error::Configuration)?,
            )
            .map_err(|_| Error::Configuration)?;
            metadata.validate(&deployment, SystemClock::new()?.now()?)?;
            let bytes =
                zeroize::Zeroizing::new(platform::read_secret(&opts.path("--secret-file")?)?);
            let secret = rss_mdm_agent_wire::runtime::Secret::parse(
                std::str::from_utf8(&bytes)
                    .map_err(|_| Error::Identity)?
                    .trim(),
            )
            .map_err(|_| Error::Identity)?;
            platform::stop(true, &deployment.service.path, &config)?;
            let result = initialize(&deployment, &envelope, &secret);
            let restart = platform::register(true, &deployment.service.path, &config);
            result?;
            restart?;
        }
        _ => return Err(Error::Configuration),
    }
    println!(
        "installation operation completed; authenticated readiness must be checked separately"
    );
    Ok(())
}
fn preserved(old: &Deployment, next: &Deployment) -> Result<(), Error> {
    if old.origin != next.origin
        || old.tenant != next.tenant
        || old.installation_id != next.installation_id
        || old.state_root != next.state_root
        || old.helper_work_roots != next.helper_work_roots
        || old.execution.work_root != next.execution.work_root
        || old.execution.material_root != next.execution.material_root
        || old.ca_file != next.ca_file
    {
        return Err(Error::Identity);
    }
    Ok(())
}
fn persistent(binary: &Path, config: &Path) -> Result<(), Error> {
    command(
        binary,
        &[
            "--config".into(),
            config.into(),
            "--validate-persistent".into(),
        ],
    )
    .map(|_| ())
}
fn refresh(
    current: &Path,
    config: &Path,
    candidate: &Path,
    next_config: &Path,
) -> Result<(), Error> {
    let old = Deployment::metadata(config)?;
    outcome::phase("refresh-preflight")?;
    let next = Deployment::load(next_config)?;
    if old.service.path != current || next.service.path != candidate {
        return Err(Error::Identity);
    }
    preserved(&old, &next)?;
    if current.exists() {
        Deployment::load_persistent(config)?;
        persistent(current, config)?;
    }
    persistent(candidate, next_config)?;
    outcome::phase("refresh-quiescence")?;
    platform::quiescent(&old, config)?;
    transition(|action| {
        outcome::phase(action.phase())?;
        match action {
            Step::StopCurrent => platform::stop(true, current, config),
            Step::RemoveCurrent => platform::remove(true, current, config),
            Step::PublishCandidate => publish(config, &json(&next)?),
            Step::RegisterCandidate => platform::register(true, candidate, config),
            Step::StopCandidate => platform::stop(true, candidate, config),
            Step::RemoveCandidate => platform::remove(true, candidate, config),
            Step::RestoreCurrent => publish(config, &json(&old)?),
            Step::RegisterCurrent => platform::register(true, current, config),
        }
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    StopCurrent,
    RemoveCurrent,
    PublishCandidate,
    RegisterCandidate,
    StopCandidate,
    RemoveCandidate,
    RestoreCurrent,
    RegisterCurrent,
}
impl Step {
    fn phase(self) -> &'static str {
        match self {
            Self::StopCurrent => "stop-current",
            Self::RemoveCurrent => "remove-current",
            Self::PublishCandidate => "publish-candidate",
            Self::RegisterCandidate => "register-candidate",
            Self::StopCandidate => "stop-candidate",
            Self::RemoveCandidate => "remove-candidate",
            Self::RestoreCurrent => "restore-current",
            Self::RegisterCurrent => "register-current",
        }
    }
}
fn transition(mut perform: impl FnMut(Step) -> Result<(), Error>) -> Result<(), Error> {
    perform(Step::StopCurrent)?;
    if let Err(error) = perform(Step::RemoveCurrent) {
        outcome::failed(error);
        outcome::compensation("running");
        perform(Step::RegisterCurrent)?;
        outcome::compensation("succeeded");
        return Err(error);
    }
    if let Err(error) = perform(Step::PublishCandidate) {
        outcome::failed(error);
        outcome::compensation("running");
        perform(Step::RestoreCurrent)?;
        perform(Step::RegisterCurrent)?;
        outcome::compensation("succeeded");
        return Err(error);
    }
    if let Err(error) = perform(Step::RegisterCandidate) {
        outcome::failed(error);
        outcome::compensation("running");
        // Do not restore the old pointer until the candidate's activity is proven stopped.
        perform(Step::StopCandidate)?;
        perform(Step::RemoveCandidate)?;
        perform(Step::RestoreCurrent)?;
        perform(Step::RegisterCurrent)?;
        outcome::compensation("succeeded");
        return Err(error);
    }
    Ok(())
}

fn apply(candidate_path: &Path, profile: Option<&PathBuf>, config: &Path) -> Result<(), Error> {
    outcome::phase("candidate-preflight")?;
    let (candidate, artifact) = Candidate::load(candidate_path)?;
    let old = match std::fs::symlink_metadata(config) {
        Ok(_) => Some(Deployment::metadata(config)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let profile = match profile {
        Some(path) => serde_json::from_slice::<Profile>(
            &installation_security::read_protected(path).map_err(|_| Error::Configuration)?,
        )
        .map_err(|_| Error::Configuration)?,
        None => match &old {
            Some(d) => Profile {
                subjects: d.clients.subjects.clone(),
                helper_work_roots: d.helper_work_roots.clone(),
            },
            None => platform::interactive_profile()?,
        },
    };
    platform::preflight_desktop_entry(&candidate)?;
    platform::public_directory(config.parent().ok_or(Error::Configuration)?)?;
    let manifests = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-manifests");
    platform::public_directory(&manifests)?;
    let candidate_bytes =
        installation_security::read_protected(candidate_path).map_err(|_| Error::Identity)?;
    if hash(&candidate_bytes) != artifact {
        return Err(Error::Identity);
    }
    retain_public(
        &manifests.join(format!("{artifact}.json")),
        &candidate_bytes,
    )?;
    let mut next = candidate.deployment(artifact, profile, config)?;
    if let Some(old) = old {
        next.installation_id = old.installation_id;
        preserved(&old, &next)?;
        if let Some(ca) = &old.ca_file {
            if installation_security::read_protected(ca).map_err(|_| Error::Identity)?
                != candidate.ca_pem.as_ref().ok_or(Error::Identity)?.as_bytes()
            {
                return Err(Error::Identity);
            }
        }
        let staged = config.with_extension(format!("candidate-{}", uuid::Uuid::new_v4()));
        write_new(&staged, &json(&next)?)?;
        let result = refresh(&old.service.path, config, &next.service.path, &staged);
        let _ = std::fs::remove_file(staged);
        result?;
    } else {
        if std::fs::symlink_metadata(&next.state_root).is_ok() {
            return Err(Error::Identity);
        }
        // Existing trust files are never overwritten, including failed or retired installations.
        outcome::phase("publish-trust")?;
        if let Some(ca) = &next.ca_file {
            retain_public(
                ca,
                candidate
                    .ca_pem
                    .as_ref()
                    .ok_or(Error::Configuration)?
                    .as_bytes(),
            )?;
        }
        outcome::phase("publish-configuration")?;
        publish_new(config, &json(&next)?)?;
        Deployment::load(config)?;
        outcome::phase("register-candidate")?;
        platform::register(true, &next.service.path, config)?;
    }
    outcome::phase("desktop-entry")?;
    platform::desktop_entry(&candidate)?;
    Ok(())
}

// Only this explicit owner operation reads the immediately preceding formats.
fn prior_format(
    mut value: serde_json::Value,
    version: u64,
    field: &str,
    next: u64,
) -> Result<serde_json::Value, Error> {
    let object = value.as_object_mut().ok_or(Error::Configuration)?;
    if object.get("version").and_then(|v| v.as_u64()) != Some(version)
        || object
            .remove(field)
            .is_none_or(|v| v.as_str().is_none_or(str::is_empty))
    {
        return Err(Error::Configuration);
    }
    object.insert("version".into(), next.into());
    Ok(value)
}
fn upgrade_device_pki(config: &Path, candidate_path: &Path) -> Result<(), Error> {
    let (candidate, artifact) = Candidate::load(candidate_path)?;
    candidate
        .service
        .verify(&std::env::current_exe()?)
        .map_err(|_| Error::Identity)?;
    let raw = installation_security::read_protected(config).map_err(|_| Error::Identity)?;
    let value: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|_| Error::Configuration)?;
    if value.get("version").and_then(|v| v.as_u64()) == Some(5) {
        // Publication is atomic. A restart can only finish this same already-published candidate.
        let current = Deployment::load(config)?;
        resume_matches(
            &current.installation_artifact,
            &current.service.path,
            &artifact,
            &candidate.service.path,
        )?;
        run_upgrade(true, |step| match step {
            UpgradeStep::Register => platform::register(true, &current.service.path, config),
            UpgradeStep::Desktop => platform::desktop_entry(&candidate),
            _ => Err(Error::Configuration),
        })?;
        return Ok(());
    }
    let old: Deployment = serde_json::from_value(prior_format(value, 4, "device_ca_file", 5)?)
        .map_err(|_| Error::Configuration)?;
    let manifests = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-manifests");
    let legacy_bytes = installation_security::read_protected(
        &manifests.join(format!("{}.json", old.installation_artifact)),
    )
    .map_err(|_| Error::Identity)?;
    if hash(&legacy_bytes) != old.installation_artifact {
        return Err(Error::Identity);
    }
    let legacy: Candidate = serde_json::from_value(prior_format(
        serde_json::from_slice(&legacy_bytes).map_err(|_| Error::Configuration)?,
        1,
        "device_ca_pem",
        2,
    )?)
    .map_err(|_| Error::Configuration)?;
    legacy.validate()?;
    if legacy.service.path != old.service.path
        || legacy.origin != old.origin
        || legacy.tenant != old.tenant
    {
        return Err(Error::Identity);
    }
    old.service
        .verify(&old.service.path)
        .map_err(|_| Error::Identity)?;
    let profile = Profile {
        subjects: old.clients.subjects.clone(),
        helper_work_roots: old.helper_work_roots.clone(),
    };
    let mut next = candidate.deployment(artifact.clone(), profile, config)?;
    next.installation_id = old.installation_id;
    preserved(&old, &next)?;
    let ca = old.ca_file.as_ref().ok_or(Error::Configuration)?;
    if installation_security::read_protected(ca).map_err(|_| Error::Identity)?
        != candidate
            .ca_pem
            .as_ref()
            .ok_or(Error::Configuration)?
            .as_bytes()
    {
        return Err(Error::Identity);
    }
    old.preflight()?; // Same communication schema and execution authority, without requiring the retired key.
    let converted = json(&legacy)?;
    let converted_digest = hash(&converted);
    let bytes =
        installation_security::read_protected(candidate_path).map_err(|_| Error::Identity)?;
    if hash(&bytes) != artifact {
        return Err(Error::Identity);
    }
    let mut lease = None;
    run_upgrade(false, |step| match step {
        UpgradeStep::Quiescent => platform::quiescent(&old, config),
        UpgradeStep::Stop => platform::stop(true, &old.service.path, config),
        UpgradeStep::Lease => {
            lease = take_communication_lease(
                &old.state_root.join("communication"),
                old.initialized()?,
            )?;
            old.preflight()
        }
        UpgradeStep::Retain => {
            retain_public(
                &manifests.join(format!("{converted_digest}.json")),
                &converted,
            )?;
            retain_public(&manifests.join(format!("{artifact}.json")), &bytes)
        }
        UpgradeStep::Remove => platform::remove(true, &old.service.path, config),
        UpgradeStep::Publish => publish(config, &json(&next)?),
        UpgradeStep::Release => {
            lease.take();
            Ok(())
        }
        UpgradeStep::Register => platform::register(true, &next.service.path, config),
        UpgradeStep::Desktop => platform::desktop_entry(&candidate),
    })
}
fn take_communication_lease(
    root: &Path,
    initialized: bool,
) -> Result<Option<std::fs::File>, Error> {
    if !initialized {
        return Ok(None);
    }
    let file = platform_private_storage::open_existing(&root.join("client.lock"))?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|_| Error::Conflict)?;
    Ok(Some(file))
}
pub(crate) fn resume_matches(
    current: &str,
    service: &Path,
    candidate: &str,
    next: &Path,
) -> Result<(), Error> {
    if current == candidate && service == next {
        Ok(())
    } else {
        Err(Error::Conflict)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum UpgradeStep {
    Quiescent,
    Stop,
    Lease,
    Retain,
    Remove,
    Publish,
    Release,
    Register,
    Desktop,
}
impl UpgradeStep {
    fn phase(self) -> &'static str {
        match self {
            Self::Quiescent => "upgrade-quiescence",
            Self::Stop => "upgrade-stop-current",
            Self::Lease => "upgrade-communication-lease",
            Self::Retain => "upgrade-retain-candidate",
            Self::Remove => "upgrade-remove-current",
            Self::Publish => "upgrade-publish-configuration",
            Self::Release => "upgrade-release-lease",
            Self::Register => "upgrade-register-candidate",
            Self::Desktop => "upgrade-desktop-entry",
        }
    }
}
// A failed upgrade never restarts an old owner against a new format. Exact retries finish publication.
pub(crate) fn run_upgrade(
    published: bool,
    mut perform: impl FnMut(UpgradeStep) -> Result<(), Error>,
) -> Result<(), Error> {
    if !published {
        for step in [
            UpgradeStep::Quiescent,
            UpgradeStep::Stop,
            UpgradeStep::Lease,
            UpgradeStep::Retain,
            UpgradeStep::Remove,
            UpgradeStep::Publish,
            UpgradeStep::Release,
        ] {
            outcome::phase(step.phase())?;
            perform(step)?;
        }
    }
    outcome::phase(UpgradeStep::Register.phase())?;
    perform(UpgradeStep::Register)?;
    outcome::phase(UpgradeStep::Desktop.phase())?;
    perform(UpgradeStep::Desktop)
}

fn installation_lock(config: &Path, system: bool) -> Result<std::fs::File, Error> {
    let root = config.parent().ok_or(Error::Configuration)?;
    if system {
        platform::public_directory(root)?;
    }
    let path = root.join("installation.lock");
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(system).create(system).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o644).custom_flags(libc::O_NOFOLLOW);
    }
    let file = opts.open(&path)?;
    installation_security::protected(&path).map_err(|_| Error::Identity)?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|_| Error::Conflict)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrade_receipt_records_each_failed_step_and_published_retry() {
        // Each installer operation owns one thread-local receipt, as the real worker does.
        for published in [false, true] {
            for (failed, expected) in [
                (UpgradeStep::Register, "upgrade-register-candidate"),
                (UpgradeStep::Desktop, "upgrade-desktop-entry"),
            ] {
                std::thread::spawn(move || {
                    outcome::phase("locked-preflight").unwrap();
                    let error = run_upgrade(published, |step| {
                        assert_eq!(outcome::snapshot().phase, step.phase());
                        if step == failed {
                            Err(Error::Unavailable)
                        } else {
                            Ok(())
                        }
                    })
                    .unwrap_err();
                    outcome::failed(error);
                    let report = outcome::snapshot();
                    assert_eq!(report.phase, expected);
                    assert_eq!(report.failure_phase.as_deref(), Some(expected));
                })
                .join()
                .unwrap();
            }
        }
    }
    #[test]
    fn owner_upgrade_accepts_only_immediate_previous_shape_and_preserves_identity_fields() {
        let original = serde_json::json!({"version":4,"device_ca_file":"/protected/old-ca.pem","installation_id":"retained","state_root":"/retained/state","tenant":"retained-tenant"});
        let converted = prior_format(original.clone(), 4, "device_ca_file", 5).unwrap();
        assert_eq!(converted["version"], 5);
        assert_eq!(converted["installation_id"], original["installation_id"]);
        assert_eq!(converted["state_root"], original["state_root"]);
        assert!(prior_format(converted, 4, "device_ca_file", 5).is_err());
        let mut older = original.clone();
        older["version"] = 3.into();
        assert!(prior_format(older, 4, "device_ca_file", 5).is_err());
        let mut incomplete = original;
        incomplete.as_object_mut().unwrap().remove("device_ca_file");
        assert!(prior_format(incomplete, 4, "device_ca_file", 5).is_err());
    }
    #[test]
    fn upgrade_uninitialized_lease_does_not_create_state_but_partial_state_is_rejected() {
        let root = crate::protocol_test_support::Root::new();
        let absent = root.path.join("absent");
        assert!(take_communication_lease(&absent, false).unwrap().is_none());
        assert!(!absent.exists());
        assert!(take_communication_lease(&root.path, true).is_err());
        assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
        let file = platform_private_storage::create_new(&root.path.join("client.lock")).unwrap();
        fs2::FileExt::try_lock_exclusive(&file).unwrap();
        assert!(matches!(
            take_communication_lease(&root.path, true),
            Err(Error::Conflict)
        ));
        drop(file);
        assert!(take_communication_lease(&root.path, true)
            .unwrap()
            .is_some());
    }
    #[test]
    fn uncertain_native_activity_blocks_publication_and_rollback() {
        let mut events = vec![];
        assert!(transition(|step| {
            events.push(step);
            if step == Step::StopCurrent {
                Err(Error::Unavailable)
            } else {
                Ok(())
            }
        })
        .is_err());
        assert_eq!(events, vec![Step::StopCurrent]);
        events.clear();
        assert!(transition(|step| {
            events.push(step);
            if matches!(step, Step::RegisterCandidate | Step::StopCandidate) {
                Err(Error::Unavailable)
            } else {
                Ok(())
            }
        })
        .is_err());
        assert!(!events.contains(&Step::RestoreCurrent));
        assert!(!events.contains(&Step::RegisterCurrent));
        events.clear();
        assert!(transition(|step| {
            events.push(step);
            if step == Step::RegisterCandidate {
                Err(Error::Unavailable)
            } else {
                Ok(())
            }
        })
        .is_err());
        assert_eq!(
            &events[events.len() - 2..],
            &[Step::RestoreCurrent, Step::RegisterCurrent]
        );
        assert!(
            events
                .iter()
                .position(|s| *s == Step::RemoveCandidate)
                .unwrap()
                < events
                    .iter()
                    .position(|s| *s == Step::RestoreCurrent)
                    .unwrap()
        );
    }
    #[test]
    fn cli_rejects_ambiguous_inputs_before_platform_work() {
        for args in [
            vec!["apply", "--config"],
            vec!["apply", "--config", "/a", "--config", "/b"],
            vec!["apply", "--password", "secret"],
        ] {
            assert!(
                Options::parse(&args.into_iter().map(OsString::from).collect::<Vec<_>>()).is_err()
            );
        }
        let (_, opts) =
            Options::parse(&["apply".into(), "--config".into(), "/a/../b".into()]).unwrap();
        assert!(opts.config().is_err());
    }
}
