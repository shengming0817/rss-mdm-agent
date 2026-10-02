// ref: microsoft/windows-rs 0.61.3 Windows/Management/Deployment/mod.rs;
// Rust std os/windows/fs.rs (share_mode); tempfile 3.27.0 src/dir/mod.rs (explicit close).
use super::*;
use ::windows::{
    core::HSTRING,
    ApplicationModel::Package,
    Foundation::Uri,
    Management::Deployment::{
        AddPackageOptions, DeploymentResult, PackageManager, StagePackageOptions,
    },
    System::ProcessorArchitecture,
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
};
use std::{fs, io::Read, path::Path};
use wire::{
    SoftwareTaskMsix as Msix, SoftwareTaskMsixDeployment as Deployment,
    SoftwareTaskMsixIdentity as Identity,
};

pub(super) fn execute(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    match &request.action.behavior {
        wire::SoftwareTaskBehavior::Exe(exe) => executable(request, exe, before),
        wire::SoftwareTaskBehavior::Msix(msix) => deployment(request, msix, before),
        _ => Err(Error::Unsupported),
    }
}
fn authenticode(request: &WorkerRequest, key: &str) -> Result<(), Error> {
    for signature in request
        .action
        .signatures
        .iter()
        .filter(|s| s.artifact == key)
    {
        if !matches!(
            signature.mechanism,
            wire::SoftwareTaskSignatureMechanism::Authenticode
                | wire::SoftwareTaskSignatureMechanism::Msix
        ) {
            return Err(Error::Untrusted);
        }
        let args = [
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-File".into(),
            request.tool("authenticode")?.to_string_lossy().into_owned(),
            "-Path".into(),
            request.material(key)?.to_string_lossy().into_owned(),
            "-Publisher".into(),
            signature.publisher.clone(),
        ];
        if run_tool(request, &request.tool("pwsh")?, &args)?.0 != 0 {
            return Err(Error::Untrusted);
        }
    }
    Ok(())
}
fn executable(
    request: &WorkerRequest,
    exe: &wire::SoftwareTaskExe,
    before: Option<&SoftwareState>,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    if request.operation == WorkerOperation::Detect {
        return Ok(result(
            0,
            Some(detect(request, &exe.detect)?),
            String::new(),
        ));
    }
    if !matches!(exe.detect, wire::SoftwareTaskDetection::Script { .. })
        && before != Some(&detect(request, &exe.detect)?)
    {
        return Err(Error::Conflict);
    }
    let (key, invocation) = match request.operation {
        WorkerOperation::Install => (&exe.installer, &exe.install),
        WorkerOperation::Upgrade => (&exe.installer, &exe.upgrade_invocation),
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious => {
            let removal = exe.uninstall.as_ref().ok_or(Error::Unsupported)?;
            (&removal.installer, &removal.invocation)
        }
        _ => return Err(Error::Unsupported),
    };
    if !request.action.signatures.iter().any(|signature| {
        signature.artifact == *key
            && signature.mechanism == wire::SoftwareTaskSignatureMechanism::Authenticode
    }) {
        return Err(Error::Unsupported);
    }
    for artifact in request
        .action
        .signatures
        .iter()
        .map(|signature| &signature.artifact)
        .collect::<std::collections::BTreeSet<_>>()
    {
        authenticode(request, artifact)?;
    }
    // Every sidecar is copied from leased exact content into one private offline layout.
    // No command-line switches, registry removal strings, URLs or bootstrap downloads are added.
    fs::create_dir(&request.resource_root)?;
    let layout = request.resource_root.join("layout");
    fs::create_dir(&layout)?;
    let mut executable = None;
    let mut leases = Vec::new();
    let mut total = 0u64;
    for (relative, artifact_key) in &exe.layout {
        if !portable_path(relative) {
            return Err(Error::Untrusted);
        }
        let source = request.material(artifact_key)?;
        total = total
            .checked_add(fs::metadata(&source)?.len())
            .ok_or(Error::Capacity)?;
        if total > 8 * 1024 * 1024 * 1024 {
            return Err(Error::Capacity);
        }
        let target = layout.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_new(&source, &target)?;
        leases.push(execution_runner::staging::verify_staged(
            &target,
            &request
                .materials
                .get(artifact_key)
                .ok_or(Error::Untrusted)?
                .artifact
                .sha256,
            &request.resource_root,
            &request.run_as,
            &request.session,
        )?);
        if artifact_key == key {
            if executable.replace(target).is_some() {
                return Err(Error::Untrusted);
            }
        }
    }
    let separate_removal = executable.is_none()
        && matches!(
            request.operation,
            WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
        );
    if separate_removal {
        let path = layout.join("rss-removal.exe");
        copy_new(&request.material(key)?, &path)?;
        leases.push(execution_runner::staging::verify_staged(
            &path,
            &request
                .materials
                .get(key)
                .ok_or(Error::Untrusted)?
                .artifact
                .sha256,
            &request.resource_root,
            &request.run_as,
            &request.session,
        )?);
        executable = Some(path);
    }
    let image = executable.ok_or(Error::Untrusted)?;
    if !image
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
    {
        return Err(Error::Untrusted);
    }
    let (code, stdout, stderr) = run_tool(request, &image, &invocation.arguments)?;
    // Root/Job completion cannot bind work delegated through services or WMI to this attempt.
    // Retain the exact layout for that activity; never infer closure from desired-state detection.
    let mut completion = result(
        code,
        None,
        format!(
            "{}{}\nexternal installer completion is unproven; retained layout and software claim",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr),
        ),
    );
    completion.1.closed = false;
    Ok(completion)
}

fn detect(
    request: &WorkerRequest,
    detection: &wire::SoftwareTaskDetection,
) -> Result<SoftwareState, Error> {
    match detection {
        wire::SoftwareTaskDetection::Registry {
            scope, key, value, ..
        } => {
            let value = registry_string(*scope, key, value)?;
            Ok(match value {
                Some(version) => SoftwareState::Present {
                    version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
                },
                None => SoftwareState::Absent {},
            })
        }
        wire::SoftwareTaskDetection::File {
            path,
            version,
            sha256,
            ..
        } => {
            match fs::symlink_metadata(path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(SoftwareState::Absent {})
                }
                Err(e) => return Err(e.into()),
                _ => (),
            }
            let digest = Digest::new(crate::plan::hex(sha256)).map_err(|_| Error::Protocol)?;
            match execution_runner::staging::verify_observed(
                Path::new(path),
                &digest,
                &request.run_as,
                &request.session,
            ) {
                Ok(_) => Ok(SoftwareState::Present {
                    version: PackageValue::new(version).map_err(|_| Error::Protocol)?,
                }),
                Err(_) => Ok(SoftwareState::Unknown {
                    reason: SoftwareDetectionFailure::UnrecognizedVersion,
                }),
            }
        }
        _ => {
            let _ = request;
            Err(Error::Unsupported)
        }
    }
}
fn registry_string(
    scope: wire::SoftwareTaskScope,
    key: &str,
    value: &str,
) -> Result<Option<String>, Error> {
    use ::windows::Win32::System::Registry::*;
    let root = match scope {
        wire::SoftwareTaskScope::System => HKEY_LOCAL_MACHINE,
        wire::SoftwareTaskScope::User => HKEY_CURRENT_USER,
    };
    let mut bytes = [0u16; 1025];
    let mut length = (bytes.len() * 2) as u32;
    let key = ::windows::core::HSTRING::from(key);
    let name = ::windows::core::HSTRING::from(value);
    let status = unsafe {
        RegGetValueW(
            root,
            ::windows::core::PCWSTR(key.as_ptr()),
            ::windows::core::PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            None,
            Some(bytes.as_mut_ptr().cast()),
            Some(&mut length),
        )
    };
    if status.0 == 2 {
        return Ok(None);
    }
    status.ok().map_err(|_| Error::Unavailable)?;
    if length < 2
        || length as usize > bytes.len() * 2
        || length % 2 != 0
        || bytes[length as usize / 2 - 1] != 0
    {
        return Err(Error::Protocol);
    }
    Ok(Some(
        String::from_utf16(&bytes[..length as usize / 2 - 1]).map_err(|_| Error::Protocol)?,
    ))
}
fn matches(package: &Package, identity: &Identity, exact_version: bool) -> Result<bool, Error> {
    let id = package.Id().map_err(|_| Error::Unavailable)?;
    let version = id.Version().map_err(|_| Error::Unavailable)?;
    let architecture = match identity.architecture {
        wire::SoftwareTaskMsixArchitecture::X86_64 => ProcessorArchitecture::X64,
        wire::SoftwareTaskMsixArchitecture::Aarch64 => ProcessorArchitecture::Arm64,
        wire::SoftwareTaskMsixArchitecture::Neutral => ProcessorArchitecture::Neutral,
    };
    Ok(
        id.Name().map_err(|_| Error::Unavailable)?.to_string() == identity.name
            && id.Publisher().map_err(|_| Error::Unavailable)?.to_string() == identity.publisher
            && id.ResourceId().map_err(|_| Error::Unavailable)?.to_string() == identity.resource_id
            && id.Architecture().map_err(|_| Error::Unavailable)? == architecture
            && (!exact_version
                || [
                    version.Major,
                    version.Minor,
                    version.Build,
                    version.Revision,
                ] == identity.version),
    )
}
fn dependency_family(package: &Package, identity: &Identity) -> Result<bool, Error> {
    let id = package.Id().map_err(|_| Error::Unavailable)?;
    Ok(
        id.Name().map_err(|_| Error::Unavailable)?.to_string() == identity.name
            && id.Publisher().map_err(|_| Error::Unavailable)?.to_string() == identity.publisher
            && id.ResourceId().map_err(|_| Error::Unavailable)?.to_string() == identity.resource_id,
    )
}
fn packages(
    manager: &PackageManager,
    msix: &Msix,
    all_staged: bool,
) -> Result<Vec<Package>, Error> {
    if all_staged {
        return Ok(manager
            .FindPackages()
            .map_err(|_| Error::Unavailable)?
            .into_iter()
            .collect());
    }
    match &msix.deployment {
        Deployment::DeviceProvisioning => Ok(manager
            .FindProvisionedPackages()
            .map_err(|_| Error::Unavailable)?
            .into_iter()
            .collect()),
        Deployment::TargetUserRegistration { .. } => Ok(manager
            .FindPackagesByUserSecurityId(&HSTRING::new())
            .map_err(|_| Error::Unavailable)?
            .into_iter()
            .collect()),
    }
}
fn observation(manager: &PackageManager, msix: &Msix) -> Result<SoftwareState, Error> {
    let candidates = packages(manager, msix, false)?
        .into_iter()
        .filter_map(|p| match matches(&p, &msix.identity, false) {
            Ok(true) => Some(Ok(p)),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if candidates.is_empty() {
        return Ok(SoftwareState::Absent {});
    }
    if candidates.len() != 1 {
        return Ok(SoftwareState::Unknown {
            reason: SoftwareDetectionFailure::Unavailable,
        });
    }
    if matches(&candidates[0], &msix.identity, true)? {
        let dependencies = candidates[0]
            .Dependencies()
            .map_err(|_| Error::Unavailable)?;
        if dependencies.Size().map_err(|_| Error::Unavailable)? as usize != msix.dependencies.len()
        {
            return Ok(SoftwareState::Unknown {
                reason: SoftwareDetectionFailure::Unavailable,
            });
        }
        for dependency in dependencies {
            if !msix
                .dependencies
                .iter()
                .any(|approved| matches(&dependency, approved, true).unwrap_or(false))
            {
                return Ok(SoftwareState::Unknown {
                    reason: SoftwareDetectionFailure::Unavailable,
                });
            }
        }

        if let wire::SoftwareTaskMsixContainer::Bundle { members, .. } = &msix.container {
            let material_scope = if msix.deployment == Deployment::DeviceProvisioning {
                packages(manager, msix, true)?
            } else {
                packages(manager, msix, false)?
            };
            for member in members
                .iter()
                .filter(|m| !m.identity.resource_id.is_empty())
            {
                if material_scope
                    .iter()
                    .filter_map(|p| matches(p, &member.identity, true).ok())
                    .filter(|m| *m)
                    .count()
                    != 1
                {
                    return Ok(SoftwareState::Unknown {
                        reason: SoftwareDetectionFailure::Unavailable,
                    });
                }
            }
        }
    }
    let v = candidates[0]
        .Id()
        .and_then(|id| id.Version())
        .map_err(|_| Error::Unavailable)?;
    Ok(SoftwareState::Present {
        version: PackageValue::new(format!(
            "{}.{}.{}.{}",
            v.Major, v.Minor, v.Build, v.Revision
        ))
        .map_err(|_| Error::Protocol)?,
    })
}
fn deployment(
    request: &WorkerRequest,
    msix: &Msix,
    before: Option<&SoftwareState>,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|_| Error::Unavailable)?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    let manager = PackageManager::new().map_err(|_| Error::Unavailable)?;
    if request.operation == WorkerOperation::Detect {
        return Ok(result(0, Some(observation(&manager, msix)?), String::new()));
    }
    if native_process::os_version::current()? < msix.minimum_os
        || (msix.require_sideload && !sideload_allowed()?)
    {
        return Err(Error::Unsupported);
    }
    // Dependencies are exact already-present packages in this same effect scope.
    let existing = packages(&manager, msix, false)?;
    for dependency in &msix.dependencies {
        let family = existing
            .iter()
            .filter_map(|package| match dependency_family(package, dependency) {
                Ok(true) => Some(Ok(package)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if family.len() != 1 || !matches(family[0], dependency, true)? {
            return Err(Error::Untrusted);
        }
    }
    let observed = observation(&manager, msix)?;
    if Some(&observed) != before {
        return Err(Error::Untrusted);
    }
    if matches!(
        request.operation,
        WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
    ) {
        let mut removal_identity = msix.identity.clone();
        if request.operation == WorkerOperation::RemovePrevious {
            let Some(SoftwareState::Present { version }) = before else {
                return Err(Error::Untrusted);
            };
            removal_identity.version = super::material::version(version.as_str())?;
        }
        let matching = existing
            .into_iter()
            .filter_map(|p| match matches(&p, &removal_identity, true) {
                Ok(true) => Some(Ok(p)),
                Ok(false) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if matching.len() != 1 {
            return Err(Error::Untrusted);
        }
        let id = matching[0].Id().map_err(|_| Error::Unavailable)?;
        request
            .native_pending
            .store(true, std::sync::atomic::Ordering::Release);
        let completion = match msix.deployment {
            Deployment::DeviceProvisioning => manager.DeprovisionPackageForAllUsersAsync(
                &id.FamilyName().map_err(|_| Error::Unavailable)?,
            ),
            Deployment::TargetUserRegistration { .. } => {
                manager.RemovePackageAsync(&id.FullName().map_err(|_| Error::Unavailable)?)
            }
        }
        .map_err(|_| Error::Unavailable)?
        .get()
        .map_err(|_| Error::Unavailable)?;
        request
            .native_pending
            .store(false, std::sync::atomic::Ordering::Release);
        return completed(completion);
    }
    for artifact in request
        .action
        .signatures
        .iter()
        .map(|signature| &signature.artifact)
        .collect::<std::collections::BTreeSet<_>>()
    {
        authenticode(request, artifact)?;
    }
    if msix.deployment == Deployment::DeviceProvisioning {
        for candidate in packages(&manager, msix, true)? {
            if matches(&candidate, &msix.identity, false)? {
                let version = candidate
                    .Id()
                    .and_then(|id| id.Version())
                    .map_err(|_| Error::Unavailable)?;
                if [
                    version.Major,
                    version.Minor,
                    version.Build,
                    version.Revision,
                ] > msix.identity.version
                {
                    return Err(Error::Unsupported);
                }
            }
        }
    }
    let selected = match selected_packages(request, msix) {
        Ok(selected) => selected,
        Err(failure) => {
            let mut failed = result(1, None, failure.error.to_string());
            failed.1.closed = failure.closed;
            return Ok(failed);
        }
    };
    let outcome = (|| {
        for (path, _) in &selected.packages {
            let url = url::Url::from_file_path(path).map_err(|_| Error::Protocol)?;
            let uri = Uri::CreateUri(&HSTRING::from(url.as_str())).map_err(|_| Error::Protocol)?;
            let operation = match msix.deployment {
                Deployment::DeviceProvisioning => {
                    let options = StagePackageOptions::new().map_err(|_| Error::Unsupported)?;
                    options
                        .SetAllowUnsigned(false)
                        .map_err(|_| Error::Unsupported)?;
                    request
                        .native_pending
                        .store(true, std::sync::atomic::Ordering::Release);
                    manager.StagePackageByUriAsync(&uri, &options)
                }
                Deployment::TargetUserRegistration { .. } => {
                    let options = AddPackageOptions::new().map_err(|_| Error::Unsupported)?;
                    options
                        .SetAllowUnsigned(msix.allow_unsigned)
                        .map_err(|_| Error::Unsupported)?;
                    options
                        .SetForceUpdateFromAnyVersion(
                            request.action.downgrade == wire::SoftwareTaskDowngrade::Allow,
                        )
                        .map_err(|_| Error::Unsupported)?;
                    request
                        .native_pending
                        .store(true, std::sync::atomic::Ordering::Release);
                    manager.AddPackageByUriAsync(&uri, &options)
                }
            }
            .map_err(|_| Error::Unavailable)?;
            let completion = operation.get().map_err(|_| Error::Unavailable)?;
            request
                .native_pending
                .store(false, std::sync::atomic::Ordering::Release);
            let finished = completed(completion)?;
            if finished.0 != 0 {
                return Ok(finished);
            }
        }
        if msix.deployment == Deployment::DeviceProvisioning {
            let staged = packages(&manager, msix, true)?
                .into_iter()
                .filter_map(|p| match matches(&p, &msix.identity, false) {
                    Ok(true) => Some(Ok(p)),
                    Ok(false) => None,
                    Err(e) => Some(Err(e)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let exact = staged
                .iter()
                .filter_map(|p| {
                    matches(p, &msix.identity, true)
                        .ok()
                        .filter(|m| *m)
                        .map(|_| p)
                })
                .collect::<Vec<_>>();
            if exact.len() != 1 {
                return Err(Error::Untrusted);
            }
            for candidate in &staged {
                let version = candidate
                    .Id()
                    .and_then(|id| id.Version())
                    .map_err(|_| Error::Unavailable)?;
                if [
                    version.Major,
                    version.Minor,
                    version.Build,
                    version.Revision,
                ] > msix.identity.version
                {
                    return Err(Error::Untrusted);
                }
            }
            let family = exact[0]
                .Id()
                .and_then(|id| id.FamilyName())
                .map_err(|_| Error::Unavailable)?;
            request
                .native_pending
                .store(true, std::sync::atomic::Ordering::Release);
            let completion = manager
                .ProvisionPackageForAllUsersAsync(&family)
                .map_err(|_| Error::Unavailable)?
                .get()
                .map_err(|_| Error::Unavailable)?;
            request
                .native_pending
                .store(false, std::sync::atomic::Ordering::Release);
            let finished = completed(completion)?;
            if finished.0 != 0 {
                return Ok(finished);
            }
        }
        Ok(result(0, None, String::new()))
    })();
    // A dispatch/.get failure leaves native activity unproven and its input layout retained.
    // Known completion and errors before dispatch release leases before deleting owned files.
    if !request
        .native_pending
        .load(std::sync::atomic::Ordering::Acquire)
    {
        if let Err(error) = selected.cleanup(request) {
            let mut failed = outcome.unwrap_or_else(|failure| result(1, None, failure.to_string()));
            failed.1.closed = false;
            failed
                .1
                .diagnostics
                .push_str("\nselected package cleanup failed: ");
            failed.1.diagnostics.push_str(&error.to_string());
            return Ok(failed);
        }
    }
    outcome
}

fn completed(completion: DeploymentResult) -> Result<(i32, SoftwareWorkerResult), Error> {
    let code = completion
        .ExtendedErrorCode()
        .map_err(|_| Error::Unavailable)?
        .0;
    let message = completion
        .ErrorText()
        .map_err(|_| Error::Unavailable)?
        .to_string();
    Ok(result(code, None, message))
}
struct SelectedPackages {
    packages: Vec<(PathBuf, Identity)>,
    leases: Vec<execution_runner::staging::RetainedMaterialLease>,
    directory: Option<execution_runner::staging::StagedDirectoryLease>,
    created: Vec<PathBuf>,
    root: Option<PathBuf>,
}
struct SelectionFailure {
    error: Error,
    closed: bool,
}
impl SelectedPackages {
    fn cleanup(mut self, request: &WorkerRequest) -> Result<(), Error> {
        if request
            .native_pending
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(Error::Unavailable);
        }
        // Windows leases deny write/delete sharing. Close them only after native completion,
        // before deleting the exact files created by this extraction, never foreign entries.
        self.leases.clear();
        for path in self.created.into_iter().rev() {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        if let Some(root) = self.root {
            // Keep the directory path stable while removing children, then release its own
            // delete-sharing restriction before the nonrecursive directory removal.
            drop(self.directory.take());
            // Nonrecursive removal fails closed if an unowned entry remains.
            fs::remove_dir(root)?;
        }
        Ok(())
    }
}
fn selected_packages(
    request: &WorkerRequest,
    msix: &Msix,
) -> Result<SelectedPackages, SelectionFailure> {
    let mut selected = SelectedPackages {
        packages: Vec::new(),
        leases: Vec::new(),
        directory: None,
        created: Vec::new(),
        root: None,
    };
    let preparation = (|| -> Result<(), Error> {
        let installer = match &msix.container {
            wire::SoftwareTaskMsixContainer::Package { installer }
            | wire::SoftwareTaskMsixContainer::Bundle { installer, .. } => installer,
        };
        let source = request.material(installer)?;
        match &msix.container {
            wire::SoftwareTaskMsixContainer::Package { .. } => {
                // The original immutable material already has a lease in software_worker::run.
                super::material::manifest(
                    fs::File::open(&source)?,
                    &msix.identity,
                    &msix.dependencies,
                )?;
                selected.packages.push((source, msix.identity.clone()));
            }
            wire::SoftwareTaskMsixContainer::Bundle { members, .. } => {
                fs::create_dir(&request.resource_root)?;
                selected.root = Some(request.resource_root.clone());
                selected.directory = Some(execution_runner::staging::lease_staged_directory(
                    &request.resource_root,
                    &request.run_as,
                    &request.session,
                )?);
                let mut archive =
                    zip::ZipArchive::new(fs::File::open(source)?).map_err(|_| Error::Untrusted)?;
                for (index, member) in members.iter().enumerate() {
                    if !portable_path(&member.path)
                        || archive.file_names().filter(|n| *n == member.path).count() != 1
                    {
                        return Err(Error::Untrusted);
                    }
                    let entry = archive
                        .by_name(&member.path)
                        .map_err(|_| Error::Untrusted)?;
                    if entry.is_symlink()
                        || entry.size() != member.length
                        || member.length > 4 * 1024 * 1024 * 1024
                    {
                        return Err(Error::Untrusted);
                    }
                    let path = request.resource_root.join(format!("member-{index}.msix"));
                    let mut file = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)?;
                    // Record ownership immediately; copy/sync/verification may still fail.
                    selected.created.push(path.clone());
                    let mut bounded = entry.take(member.length + 1);
                    if std::io::copy(&mut bounded, &mut file)? != member.length {
                        return Err(Error::Untrusted);
                    }
                    file.sync_all()?;
                    drop(file);
                    let digest = Digest::new(crate::plan::hex(&member.sha256))
                        .map_err(|_| Error::Protocol)?;
                    selected
                        .leases
                        .push(execution_runner::staging::verify_staged(
                            &path,
                            &digest,
                            &request.resource_root,
                            &request.run_as,
                            &request.session,
                        )?);
                    super::material::manifest(
                        fs::File::open(&path)?,
                        &member.identity,
                        &msix.dependencies,
                    )?;
                    selected.packages.push((path, member.identity.clone()));
                }
                selected
                    .packages
                    .sort_by_key(|(_, identity)| !identity.resource_id.is_empty());
            }
        }
        Ok(())
    })();
    match preparation {
        Ok(()) => Ok(selected),
        Err(error) => match selected.cleanup(request) {
            Ok(()) => Err(SelectionFailure {
                error,
                closed: true,
            }),
            Err(error) => Err(SelectionFailure {
                error,
                closed: false,
            }),
        },
    }
}
fn portable_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != ".." && !p.ends_with(['.', ' ']))
}
fn copy_new(source: &Path, target: &Path) -> Result<(), Error> {
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(target)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(())
}
pub(crate) fn sideload_allowed() -> Result<bool, Error> {
    use ::windows::Win32::System::Registry::*;
    for key in [
        r"SOFTWARE\Policies\Microsoft\Windows\Appx",
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock",
    ] {
        let key = HSTRING::from(key);
        let name = HSTRING::from("AllowAllTrustedApps");
        let mut value = 0u32;
        let mut length = 4u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                ::windows::core::PCWSTR(key.as_ptr()),
                ::windows::core::PCWSTR(name.as_ptr()),
                RRF_RT_REG_DWORD | RRF_SUBKEY_WOW6464KEY,
                None,
                Some((&mut value as *mut u32).cast()),
                Some(&mut length),
            )
        };
        if status.0 == 2 {
            continue;
        }
        status.ok().map_err(|_| Error::Unavailable)?;
        return Ok(length == 4 && value == 1);
    }
    Ok(false)
}
