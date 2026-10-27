//! Package Manager owns the service and immutable payload; the installation owner retains device state.
//! ref: Microsoft desktop6:Service; windows-rs ApplicationModel::Package::Current.
use super::*;
pub(super) fn root() -> Result<PathBuf, Error> {
    let package = ::windows::ApplicationModel::Package::Current().map_err(|_| Error::Identity)?;
    if package
        .Id()
        .and_then(|id| id.Name())
        .map_err(|_| Error::Identity)?
        .to_string_lossy()
        != "RSS.MDM.Agent"
    {
        return Err(Error::Identity);
    }
    let path = package
        .InstalledLocation()
        .and_then(|location| location.Path())
        .map_err(|_| Error::Identity)?;
    let root = PathBuf::from(path.to_string_lossy());
    installation_security::protected(&root).map_err(|_| Error::Identity)?;
    Ok(root)
}
/// Initialize/refresh the shared machine configuration from this OS-authenticated installed package.
/// It never creates, removes or starts the package-managed SCM service.
pub fn initialize() -> Result<Deployment, Error> {
    if execution_ipc::host::current_subject().map_err(crate::error::app_error)? != "S-1-5-18" {
        return Err(Error::Identity);
    }
    let root = root()?;
    if std::env::current_exe()?.canonicalize()?.parent() != Some(root.canonicalize()?.as_path()) {
        return Err(Error::Identity);
    }
    let bytes = installation_security::read_protected(&root.join("candidate.json"))
        .map_err(|_| Error::Identity)?;
    let mut candidate: Candidate =
        serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
    let original = candidate.root.clone();
    let rebase = |path: &Path| -> Result<PathBuf, Error> {
        let relative = path.strip_prefix(&original).map_err(|_| Error::Identity)?;
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(Error::Identity);
        }
        Ok(root.join(relative))
    };
    candidate.service.path = rebase(&candidate.service.path)?;
    candidate.desktop.path = rebase(&candidate.desktop.path)?;
    candidate.cli.path = rebase(&candidate.cli.path)?;
    candidate.runtime_root = rebase(&candidate.runtime_root)?;
    candidate.root = root;
    if candidate.format()? != "windows_msix" {
        return Err(Error::Identity);
    }
    candidate.validate()?;
    let config = Deployment::default_path()?;
    let _lock = super::lifecycle::installation_lock(&config, true)?;
    let old = match std::fs::symlink_metadata(&config) {
        Ok(_) => Some(Deployment::metadata(&config)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let profile = match &old {
        Some(old) => Profile {
            subjects: old.clients.subjects.clone(),
            helper_work_roots: old.helper_work_roots.clone(),
        },
        None => super::lifecycle::initial_profile()?,
    };
    let bytes = json(&candidate)?;
    let digest = hash(&bytes);
    let manifests = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-manifests");
    platform::public_directory(&manifests)?;
    retain_public(&manifests.join(format!("{digest}.json")), &bytes)?;
    let mut next = candidate.deployment(digest, profile, &config)?;
    if let Some(old) = old {
        super::lifecycle::inherit(&old, &mut next)?;
        // The OS stopped the previous package service. Retained identity/journal are reopened below.
        if let Some(ca) = &old.ca_file {
            if installation_security::read_protected(ca).map_err(|_| Error::Identity)?
                != candidate.ca_pem.as_ref().ok_or(Error::Identity)?.as_bytes()
            {
                return Err(Error::Identity);
            }
        }
    }
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
    crate::local_permissions::initialize(&next.execution.local_permissions)?;
    publish(&config, &json(&next)?)?;
    Deployment::load(&config)
}
