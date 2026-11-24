use super::*;
/// Resolved before platform dispatch or installation state is touched.
enum Action {
    Install,
    Remove,
    RemoveLabRegistration,
    Uninstall,
    Status,
    Refresh,
    Apply,
    Recover,
    Import,
    ImportConnections,
    DaPrepare,
    DaImport,
    DaRenew,
    #[cfg(windows)]
    MsiApply,
    #[cfg(windows)]
    MsiRemove,
    #[cfg(windows)]
    MsiSave,
    #[cfg(windows)]
    MsiRollback,
}
impl Action {
    fn parse(value: &str) -> Result<Self, Error> {
        match value {
            #[cfg(windows)]
            "msi-remove" => Ok(Self::MsiRemove),
            #[cfg(windows)]
            "msi-apply" => Ok(Self::MsiApply),
            #[cfg(windows)]
            "msi-save" => Ok(Self::MsiSave),
            #[cfg(windows)]
            "msi-rollback" => Ok(Self::MsiRollback),
            "install" => Ok(Self::Install),
            "remove" => Ok(Self::Remove),
            "remove-lab-registration" => Ok(Self::RemoveLabRegistration),
            "uninstall" => Ok(Self::Uninstall),
            "status" => Ok(Self::Status),
            "refresh" => Ok(Self::Refresh),
            "apply" => Ok(Self::Apply),
            "recover" => Ok(Self::Recover),
            "import" => Ok(Self::Import),
            "da-prepare" => Ok(Self::DaPrepare),
            "da-import" => Ok(Self::DaImport),
            "da-renew" => Ok(Self::DaRenew),
            "import-connections" => Ok(Self::ImportConnections),
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
                "--connections",
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
        let id = uuid::Uuid::parse_str(id).map_err(|_| Error::Configuration)?;
        #[cfg(windows)]
        if platform::worker_operation(id)? {
            return platform::operation_status(id);
        }
        if !opts.system()? {
            return Err(Error::Denied);
        }
        if platform::dispatch(args)? {
            return Ok(());
        }
        if apply::status(&opts.config()?, id)? {
            return Ok(());
        }
        return platform::operation_status(id);
    }
    if action == "validate-trust" {
        let bytes = installation_security::read_external(
            &opts.path("--candidate")?,
            rss_mdm_agent_wire::connections::MAX_FILE_BYTES,
        )
        .map_err(|_| Error::Configuration)?;
        let candidate: Candidate =
            serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        return candidate.validate_trust();
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
    #[cfg(windows)]
    if opts.system()?
        && !matches!(
            action,
            Action::Apply
                | Action::Status
                | Action::MsiSave
                | Action::MsiApply
                | Action::MsiRemove
                | Action::MsiRollback
        )
    {
        allow_configuration_write(&opts.config()?)?;
    }
    let can_resume = matches!(action, Action::Apply | Action::Status);
    if opts.system()? && !can_resume {
        apply::ensure_idle(&opts.config()?)?;
    }
    match action {
        #[cfg(windows)]
        Action::MsiApply | Action::MsiSave | Action::MsiRollback | Action::MsiRemove => {
            let config = opts.config()?;
            if !opts.system()? || config != Deployment::default_path()? {
                return Err(Error::Denied);
            }
            if !matches!(action, Action::MsiSave) {
                require_msi_sequence(&config)?;
            }
            if matches!(action, Action::MsiRemove) {
                return remove_package(&opts);
            }
            if matches!(action, Action::MsiApply) {
                return apply_msi_package(&opts.path("--candidate")?, &config);
            }
            return msi_transaction(action, &config);
        }
        Action::Install => {
            let binary = opts.path("--binary")?;
            let config = opts.config()?;
            let deployment = Deployment::load(&config)?;
            if deployment.service.path != binary {
                return Err(Error::Identity);
            }
            platform::register(opts.system()?, &binary, &config)?;
        }
        Action::RemoveLabRegistration => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            let config = opts.config()?;
            let binary = opts.path("--binary")?;
            let deployment = Deployment::metadata(&config)?;
            if config == Deployment::default_path()?
                || config.canonicalize()? == Deployment::default_path()?
                || deployment.service.path != binary
            {
                return Err(Error::Identity);
            }
            // This only removes a custom lab registration whose native arguments
            // bind the exact config and image. The default production registration
            // cannot match; credentials, payload and journal are retained.
            platform::quiescent(&deployment, &config)?;
            platform::remove(true, &binary, &config)?;
        }
        Action::Remove => {
            if opts.system()? {
                remove_package(&opts)?;
            } else {
                platform::remove(false, &opts.path("--binary")?, &opts.config()?)?;
            }
        }
        Action::Uninstall => {
            if !opts.system()? || opts.config()? != Deployment::default_path()? {
                return Err(Error::Identity);
            }
            let task = opts
                .values
                .get("--operation")
                .and_then(|v| v.to_str())
                .ok_or(Error::Denied)?;
            unmanage::run(
                uuid::Uuid::parse_str(task).map_err(|_| Error::Denied)?,
                &opts.path("--binary")?,
            )?;
        }
        Action::Status => {
            platform::status(opts.system()?)?;
            if opts.system()? {
                apply::module_status(&opts.config()?)?;
            }
        }
        Action::Refresh => {
            if !opts.system()? {
                return Err(Error::Configuration);
            }
            let old = Deployment::metadata(&opts.config()?)?;
            let next = Deployment::metadata(&opts.path("--candidate-config")?)?;
            if old.modules != next.modules {
                return Err(Error::Conflict);
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
            let candidate = opts
                .values
                .get("--candidate")
                .map(|_| opts.path("--candidate"))
                .transpose()?;
            let profile = opts
                .values
                .get("--profile")
                .map(|_| {
                    installation_security::read_protected(&opts.path("--profile")?)
                        .map_err(|_| Error::Identity)
                })
                .transpose()?;
            let operation = opts
                .values
                .get("--operation")
                .map(|p| {
                    p.to_str()
                        .and_then(|s| uuid::Uuid::parse_str(s).ok())
                        .filter(|id| !id.is_nil())
                        .ok_or(Error::Configuration)
                })
                .transpose()?;
            if candidate.is_none() && profile.is_none() && operation.is_none() {
                return Err(Error::Configuration);
            }
            let source = if candidate.is_some() {
                apply::Source::Package
            } else {
                apply::Source::Administrator
            };
            apply::run(
                &opts.config()?,
                candidate.as_deref(),
                profile.as_deref(),
                operation,
                source,
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
        Action::DaPrepare | Action::DaImport | Action::DaRenew => {
            if !opts.system()? {
                return Err(Error::Denied);
            }
            let d = Deployment::load(&opts.config()?)?;
            match action {
                Action::DaPrepare => {
                    let operation = opts
                        .values
                        .get("--operation")
                        .and_then(|p| p.to_str())
                        .and_then(|v| uuid::Uuid::parse_str(v).ok())
                        .ok_or(Error::Configuration)?;
                    crate::da_service::prepare(&d, operation, &opts.path("--grant")?)
                }
                Action::DaImport => crate::da_service::import(&d, &opts.path("--grant")?),
                _ => crate::da_service::renew(&d),
            }?;
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
        Action::ImportConnections => {
            if !opts.system()? {
                return Err(Error::Identity);
            }
            let config = opts.config()?;
            let deployment = Deployment::load(&config)?;
            let bytes = installation_security::read_protected(&opts.path("--connections")?)
                .map_err(|_| Error::Configuration)?;
            if bytes.len() > rss_mdm_agent_wire::connections::MAX_FILE_BYTES {
                return Err(Error::Capacity);
            }
            let document = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
            let network = deployment.network()?;
            agent_client::verify_connections(
                &document,
                network.service_id,
                network.tenant,
                &network.registration_keys,
            )?;
            restore_registration(
                || platform::stop(true, &deployment.service.path, &config),
                || deployment.communication(false)?.import_connections(&bytes),
                || platform::register(true, &deployment.service.path, &config),
            )
            .map_err(ImportFailure::error)?;
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
fn remove_package(opts: &Options) -> Result<(), Error> {
    let task = opts
        .values
        .get("--operation")
        .and_then(|v| v.to_str())
        .ok_or(Error::Denied)?;
    unmanage::package_remove(
        &opts.config()?,
        &opts.path("--binary")?,
        uuid::Uuid::parse_str(task).map_err(|_| Error::Denied)?,
    )
}

// Fixed packages own image/trust metadata; protected administrator deployment
// owns execution configuration. Reinstalling must not reset pinned interpreters,
// package managers, local permissions or budgets to the installer's defaults.
pub(super) fn inherit(old: &Deployment, next: &mut Deployment) -> Result<(), Error> {
    next.modules = old.modules.clone();
    next.installation_id = old.installation_id;
    next.software_read_grants = old.software_read_grants.clone();
    if let (Some(previous), Some(next_cli)) = (&old.cli, &mut next.cli) {
        next_cli.machine_actions = previous.machine_actions.clone();
    }
    preserved(old, next)?;
    next.execution = old.execution.clone();
    Ok(())
}

pub(super) fn preserved(old: &Deployment, next: &Deployment) -> Result<(), Error> {
    if let (Ok(a), Ok(b)) = (old.mdm(), next.mdm()) {
        if (a.instance, a.tenant, &a.ca_pem, &a.signing_keys)
            != (b.instance, b.tenant, &b.ca_pem, &b.signing_keys)
        {
            return Err(Error::Identity);
        }
    }
    if old.software_read_grants != next.software_read_grants
        || old.installation_id != next.installation_id
        || old.state_root != next.state_root
        || old.helper_work_roots != next.helper_work_roots
        || old.execution.work_root != next.execution.work_root
        || old.execution.material_root != next.execution.material_root
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
pub(super) fn preflight_refresh(
    old: &Deployment,
    config: &Path,
    next: &Deployment,
    next_config: &Path,
) -> Result<(), Error> {
    Deployment::load(next_config)?;
    preserved(old, next)?;
    #[cfg(target_os = "macos")]
    if old.modules.mdm.enabled() || old.platform_sso.is_some() {
        if old.modules.mdm.enabled()
            && next.modules.mdm.enabled()
            && old.platform_sso.is_some()
            && next.platform_sso.is_none()
        {
            return Err(Error::Configuration);
        }
        crate::service::platform_sso::upgrade_preflight(old, next.platform_sso.is_some())?;
    }
    if old.service.path.exists() {
        Deployment::load_persistent(config)?;
    }
    preflight_candidate_state(next, next_config)
}
pub(super) fn preflight_candidate_state(
    next: &Deployment,
    next_config: &Path,
) -> Result<(), Error> {
    persistent(&next.service.path, next_config)?;
    #[cfg(windows)]
    if next.modules.mdm.enabled() || next.modules.da.enabled() {
        crate::access_component::preflight(next)?;
    }
    Ok(())
}
// Package callbacks may exit between issuing STOP and observing process exit.
pub(super) fn stop_for_maintenance(deployment: &Deployment, config: &Path) -> Result<(), Error> {
    let evidence = config.with_extension("service-stop");
    platform::stop_for_apply(deployment, config, &evidence)?;
    std::fs::remove_file(evidence)?;
    Ok(())
}
fn refresh(
    current: &Path,
    config: &Path,
    candidate: &Path,
    next_config: &Path,
) -> Result<(), Error> {
    let old = Deployment::metadata(config)?;
    outcome::phase("refresh-preflight")?;
    let next = Deployment::metadata(next_config)?;
    if old.service.path != current || next.service.path != candidate {
        return Err(Error::Identity);
    }
    preflight_refresh(&old, config, &next, next_config)?;
    outcome::phase("refresh-quiescence")?;
    transition(|action| {
        outcome::phase(action.phase())?;
        match action {
            Step::StopCurrent => stop_for_maintenance(&old, config),
            Step::RemoveCurrent => platform::remove(true, current, config),
            Step::PublishCandidate => {
                crate::local_permissions::initialize(&next.execution.local_permissions)?;
                publish(config, &json(&next)?)
            }
            Step::RegisterCandidate => platform::register(true, candidate, config),
            Step::StopCandidate => stop_for_maintenance(&next, config),
            Step::RemoveCandidate => platform::remove(true, candidate, config),
            Step::RestoreCurrent => publish(config, &json(&old)?),
            Step::RegisterCurrent => platform::restore_registration(current, config),
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
        outcome::failed(error);
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
    let installer_format = installer_format(&deployment).map_err(ImportFailure::Lifecycle)?;
    restore_registration(
        || {
            outcome::phase("configuration-stop")?;
            platform::stop(true, &deployment.service.path, config)
        },
        || {
            outcome::phase("configuration-prepare")?;
            let mut client = deployment.communication(true)?;
            if client.enrollment_status()?.registered {
                return Ok(());
            }
            client.prepare_configuration(file, installer_format, deployment.capabilities())
        },
        || {
            outcome::phase("configuration-restart")?;
            platform::register(true, &deployment.service.path, config)
        },
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

pub(super) fn initial_profile(mdm: bool) -> Result<LocalAccess, Error> {
    let subject = execution_ipc::host::active_user_subject().ok();
    if mdm && subject.is_some() {
        return platform::interactive_profile();
    }
    Ok(LocalAccess {
        subjects: subject.into_iter().collect(),
        helper_work_roots: BTreeMap::new(),
    })
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

#[cfg(windows)]
fn apply_msi_package(candidate_path: &Path, config: &Path) -> Result<(), Error> {
    outcome::phase("candidate-preflight")?;
    let candidate_bytes =
        installation_security::read_protected(candidate_path).map_err(|_| Error::Identity)?;
    let candidate: Candidate =
        serde_json::from_slice(&candidate_bytes).map_err(|_| Error::Configuration)?;
    candidate.validate()?;
    let artifact = hash(&candidate_bytes);
    let active = config.try_exists()?;
    let inactive = apply::inactive_deployment(config)?;
    let old = if active {
        Some(Deployment::metadata(config)?)
    } else if inactive.try_exists()? {
        Some(Deployment::metadata(&inactive)?)
    } else {
        None
    };
    if candidate.format()? != "windows_msi" {
        return Err(Error::Unsupported);
    }
    let profile = match &old {
        Some(d) => LocalAccess::from_deployment(d),
        None => initial_profile(candidate.defaults.mdm.enabled())?,
    };
    platform::public_directory(config.parent().ok_or(Error::Configuration)?)?;
    let manifests = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-manifests");
    platform::public_directory(&manifests)?;
    retain_public(
        &manifests.join(format!("{artifact}.json")),
        &candidate_bytes,
    )?;
    let mut next = candidate.deployment(artifact, profile, config, old.as_ref())?;
    if let Some(old) = &old {
        next.modules = old.modules.clone();
        next.modules.validate_for(candidate.assembly)?;
        inherit(old, &mut next)?;
    }
    let staged = config.with_extension(format!("candidate-{}", uuid::Uuid::new_v4()));
    write_new(&staged, &json(&next)?)?;
    let result = (|| {
        if let Some(old) = old.as_ref().filter(|_| active) {
            return refresh(&old.service.path, config, &next.service.path, &staged);
        }
        if old.is_none() && next.state_root.try_exists()? {
            return Err(Error::Identity);
        }
        // Existing trust files are never overwritten, including failed or retired installations.
        outcome::phase("publish-trust")?;
        Deployment::load(&staged)?;
        if old.is_some() {
            preflight_candidate_state(&next, &staged)?;
        }
        outcome::phase("publish-configuration")?;
        crate::local_permissions::initialize(&next.execution.local_permissions)?;
        publish_new(config, &json(&next)?)?;
        outcome::phase("register-candidate")?;
        platform::register(true, &next.service.path, config)
    })();
    let _ = std::fs::remove_file(staged);
    result?;
    if inactive.try_exists()? {
        std::fs::remove_file(inactive)?;
    }
    Ok(())
}

// Save and Rollback run in the native execute script under separate invocations. Only
// exclude overlapping writes while Installer is executing; its rollback bytes belong
// to Installer and are never read or discarded by profile maintenance.
#[cfg(windows)]
pub(super) fn allow_configuration_write(config: &Path) -> Result<(), Error> {
    if config == Deployment::default_path()?
        && config.with_extension("msi-rollback").try_exists()?
        && platform::msi_executing()?
    {
        return Err(Error::Conflict);
    }
    Ok(())
}
#[cfg(windows)]
fn require_msi_sequence(config: &Path) -> Result<(), Error> {
    if config != Deployment::default_path()?
        || !config.with_extension("msi-rollback").try_exists()?
        || !platform::msi_executing()?
    {
        return Err(Error::Conflict);
    }
    Ok(())
}
// MSI owns file rollback; the installation owner owns service/configuration compensation.
#[cfg(windows)]
fn msi_transaction(action: Action, config: &Path) -> Result<(), Error> {
    let snapshot = config.with_extension("msi-rollback");
    match action {
        Action::MsiSave => {
            if !platform::msi_executing()? {
                return Err(Error::Conflict);
            }
            let old = match std::fs::symlink_metadata(config) {
                Ok(_) => Some(Deployment::metadata(config)?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            publish(&snapshot, &json(&old)?)?;
        }
        Action::MsiRollback => {
            let bytes = match installation_security::read_protected(&snapshot) {
                Ok(bytes) => bytes,
                Err(_) if !snapshot.exists() => return Ok(()),
                Err(_) => return Err(Error::Identity),
            };
            let old: Option<Deployment> =
                serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
            let current = match std::fs::symlink_metadata(config) {
                Ok(_) => Some(Deployment::metadata(config)?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            if let Some(old) = &old {
                old.service
                    .verify(&old.service.path)
                    .map_err(|_| Error::Identity)?;
                validate_native_artifact(&old.service)?;
                if let Some(current) = &current {
                    preserved(old, current)?;
                }
            }
            if let Some(d) = current.as_ref().or(old.as_ref()) {
                stop_for_maintenance(d, config)?;
            }
            if let Some(current) = &current {
                platform::remove(true, &current.service.path, config)?;
            }
            if let Some(old) = old {
                publish(config, &json(&old)?)?;
                platform::restore_registration(&old.service.path, config)?;
            } else if current.is_some() {
                let inactive = apply::inactive_deployment(config)?;
                let profiles = inactive.parent().ok_or(Error::Configuration)?;
                platform::public_directory(profiles.parent().ok_or(Error::Configuration)?)?;
                platform_private_storage::directory(profiles)?;
                platform::replace(config, &inactive)?;
            }
            std::fs::remove_file(snapshot)?;
        }
        _ => return Err(Error::Configuration),
    }
    Ok(())
}
