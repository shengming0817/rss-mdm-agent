use super::*;
/// Resolved before platform dispatch or installation state is touched.
enum Action {
    Install,
    Remove,
    Uninstall,
    Status,
    Refresh,
    Apply,
    Recover,
    Import,
}
impl Action {
    fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "install" => Ok(Self::Install),
            "remove" => Ok(Self::Remove),
            "uninstall" => Ok(Self::Uninstall),
            "status" => Ok(Self::Status),
            "refresh" => Ok(Self::Refresh),
            "apply" => Ok(Self::Apply),
            "recover" => Ok(Self::Recover),
            "import" => Ok(Self::Import),
            _ => Err(Error::Configuration),
        }
    }
}
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
                "--configuration",
                "--package-path",
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
    let action = Action::parse(&action)?;
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
    match action {
        Action::Install => {
            let binary = opts.path("--binary")?;
            let config = opts.config()?;
            let deployment = Deployment::load(&config)?;
            if deployment.service.path != binary {
                return Err(Error::Identity);
            }
            platform::register(opts.system()?, &binary, &config)?;
        }
        Action::Remove => {
            let binary = opts.path("--binary")?;
            let config = opts.config()?;
            if opts.system()? {
                let deployment = Deployment::load(&config)?;
                platform::quiescent(&deployment, &config)?;
            }
            platform::remove(opts.system()?, &binary, &config)?;
        }
        Action::Uninstall => {
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
        Action::Status => {
            platform::status(opts.system()?)?;
        }
        Action::Refresh => {
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
        Action::Apply => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            apply(
                &opts.path("--candidate")?,
                opts.values.get("--profile"),
                &opts.config()?,
            )?;
            if let Some(package) = opts.values.get("--package-path") {
                // Installation succeeds independently of registration. Invalid material is reported,
                // never trusted or silently replaced with another identity.
                let result = discover_configuration(package, &opts.config()?);
                match result {
                    Ok(()) => (),
                    Err(ImportFailure::Configuration(error)) => {
                        eprintln!("agent_configuration_import: {error}")
                    }
                    Err(ImportFailure::Lifecycle(error)) => return Err(error),
                }
            }
        }
        Action::Recover => {
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
        Action::Import => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            import_configuration(&opts.path("--configuration")?, &opts.config()?)
                .map_err(ImportFailure::error)?;
        }
    }
    println!(
        "installation operation completed; authenticated readiness must be checked separately"
    );
    Ok(())
}
pub(super) fn preserved(old: &Deployment, next: &Deployment) -> Result<(), Error> {
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

#[derive(Debug, PartialEq, Eq)]
enum ImportFailure {
    Configuration(Error),
    Lifecycle(Error),
}
impl ImportFailure {
    fn error(self) -> Error {
        match self {
            Self::Configuration(e) | Self::Lifecycle(e) => e,
        }
    }
}
impl From<Error> for ImportFailure {
    fn from(e: Error) -> Self {
        Self::Configuration(e)
    }
}
impl From<std::io::Error> for ImportFailure {
    fn from(_: std::io::Error) -> Self {
        Self::Configuration(Error::Storage)
    }
}
fn discover_configuration(package: &Path, config: &Path) -> Result<(), ImportFailure> {
    if !package.is_absolute()
        || package.components().any(|v| {
            matches!(
                v,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
        || package.extension().and_then(|v| v.to_str())
            != Some(if cfg!(windows) { "exe" } else { "pkg" })
    {
        return Err(Error::Configuration.into());
    }
    let path = package
        .parent()
        .ok_or(Error::Configuration)?
        .join(agent_client::CONFIGURATION_FILENAME);
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(_) => import_configuration(&path, config),
    }
}
fn restore_registration(
    stop: impl FnOnce() -> Result<(), Error>,
    import: impl FnOnce() -> Result<(), Error>,
    restart: impl FnOnce() -> Result<(), Error>,
) -> Result<(), ImportFailure> {
    let stopped = stop();
    let imported = match stopped {
        Ok(()) => import(),
        Err(error) => Err(error),
    };
    if let Err(error) = imported {
        eprintln!("agent_configuration_prepare: {error}");
    }
    // Even a failed stop may have already deregistered the job. Restore its same owner;
    // the communication lease still excludes any overlapping execution owner.
    let restarted = restart();
    if let Err(error) = restarted {
        return Err(ImportFailure::Lifecycle(error));
    }
    stopped.map_err(ImportFailure::Lifecycle)?;
    imported.map_err(ImportFailure::Configuration)
}
fn import_configuration(path: &Path, config: &Path) -> Result<(), ImportFailure> {
    let deployment = Deployment::load(config).map_err(ImportFailure::Lifecycle)?;
    let bytes = zeroize::Zeroizing::new(installation_security::read_external(
        path,
        agent_client::CONFIGURATION_BYTES,
    )?);
    let file: rss_mdm_agent_wire::enroll::configuration::ConfigurationFile =
        serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    agent_client::verify_configuration(&file, &deployment.network()?, SystemClock::new()?.now()?)?;
    let candidate_sha256 = publication_digest(&deployment).map_err(ImportFailure::Lifecycle)?;
    restore_registration(
        || platform::stop(true, &deployment.service.path, config),
        || {
            let mut client = deployment.communication(true)?;
            if client.enrollment_status()?.registered {
                return Ok(());
            }
            client.prepare_configuration(file, candidate_sha256, deployment.capabilities())
        },
        || platform::register(true, &deployment.service.path, config),
    )
}
#[cfg(test)]
mod configuration_restart_tests {
    use super::*;
    #[test]
    fn stop_and_restart_failures_never_become_configuration_only_failures() {
        use std::cell::Cell;
        let imported = Cell::new(false);
        let restarted = Cell::new(false);
        let result = restore_registration(
            || Err(Error::Unavailable),
            || {
                imported.set(true);
                Ok(())
            },
            || {
                restarted.set(true);
                Ok(())
            },
        );
        assert_eq!(result, Err(ImportFailure::Lifecycle(Error::Unavailable)));
        assert!(!imported.get());
        assert!(restarted.get());
        assert_eq!(
            restore_registration(|| Ok(()), || Err(Error::Denied), || Err(Error::Storage)),
            Err(ImportFailure::Lifecycle(Error::Storage))
        );
        assert_eq!(
            restore_registration(|| Ok(()), || Err(Error::Denied), || Ok(())),
            Err(ImportFailure::Configuration(Error::Denied))
        );
    }
}

pub(super) fn initial_profile() -> Result<Profile, Error> {
    if execution_ipc::host::active_user_session().is_err() {
        // No interactive subject gains IPC access merely because a machine is being enrolled.
        return Ok(Profile {
            subjects: vec![],
            helper_work_roots: BTreeMap::new(),
        });
    }
    platform::interactive_profile()
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
            None => initial_profile()?,
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

pub(super) fn installation_lock(config: &Path, system: bool) -> Result<std::fs::File, Error> {
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
    fn unknown_commands_are_rejected_before_platform_dispatch_or_installation_state() {
        for command in ["upgrade-device-pki", "unknown-command"] {
            for explicit_config in [false, true] {
                std::thread::spawn(move || {
                    let root = crate::protocol_test_support::Root::new();
                    let absent = root.path.join("absent");
                    let mut args = vec![OsString::from(command), "--scope".into(), "system".into()];
                    if explicit_config {
                        args.extend([
                            "--config".into(),
                            absent.join("execution.json").into(),
                            "--candidate".into(),
                            absent.join("candidate.json").into(),
                        ]);
                    }
                    assert_eq!(crate::installation::run(&args), Err(Error::Configuration));
                    assert!(!absent.exists());
                    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
                    let report = outcome::snapshot();
                    assert_eq!(report.phase, "preflight");
                    assert_eq!(report.failure_phase.as_deref(), Some("preflight"));
                })
                .join()
                .unwrap();
            }
        }
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
