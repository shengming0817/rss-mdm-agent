//! Fixed software adapters sharing NativeRunner's process owner and execution journal.
mod bundle;
mod commands;
mod tree;
use execution_app::Error;
use execution_contract::*;
use sha2::{Digest as _, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek},
    path::PathBuf,
    sync::Arc,
};

/// Host-owned, independently refreshed software facts, not values decoded from an IPC request.
/// Production composition and source trust are supplied by the consuming product.
pub trait SoftwareProbe: Send + Sync {
    /// Establish current incoming dependency use for the exact package under the manager lock.
    fn dependency_use(&self, plan: &FrozenPlan) -> Result<DependencyUse, Error>;
    /// Compare the exact installed/desired version pair using this package ecosystem.
    fn comparison(
        &self,
        plan: &FrozenPlan,
        installed: &PackageValue,
    ) -> Result<Option<VersionComparison>, Error>;
    /// Independently reconcile installer/delegated work for this attempt. A lock, PID absence,
    /// receipt or installed target alone is insufficient. None means no quiescence proof.
    /// Recovery may return proof without reconstructing a historical process exit or output count.
    fn quiescence(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
    ) -> Result<Option<execution_contract::EvidenceRef>, Error>;
}
/// Fixed protected materialization paths. None of these paths confer execution permission.
pub struct SoftwareArtifacts {
    /// Protected MSI/PKG/ZIP/formula/manifest file; exact bytes are bound by the plan.
    pub payload: PathBuf,
    /// Protected exact package manager binary, never selected from PATH.
    pub manager: PathBuf,
    /// Shared OS lock root, identical across system/user helpers that affect the same resources.
    pub lock_root: PathBuf,
    /// Independent local ecosystem observations, called under both locks.
    pub probe: Arc<dyn SoftwareProbe>,
    #[cfg(test)]
    pub(crate) fixture_owned: bool,
}
pub(crate) struct PreparationControl {
    pub deadline: std::time::Instant,
    pub cancelled: Arc<std::sync::atomic::AtomicBool>,
}
impl PreparationControl {
    pub(crate) fn check(&self) -> Result<(), Error> {
        if self.cancelled.load(std::sync::atomic::Ordering::Acquire)
            || std::time::Instant::now() >= self.deadline
        {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self {
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}
pub(crate) struct Lease {
    _locks: Vec<File>,
    _files: Vec<File>,
    _paths: Vec<crate::platform::PathLease>,
    expanded: Option<bundle::Expanded>,
    pub args: Vec<String>,
    pub before: SoftwareState,
}
fn hash_file(
    file: &mut File,
    max: u64,
    control: Option<&PreparationControl>,
) -> Result<String, Error> {
    if max == 0 || file.metadata().map_err(|_| Error::Unavailable)?.len() > max {
        return Err(Error::Capacity);
    }
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        if let Some(control) = control {
            control.check()?;
        }
        let n = file.read(&mut buffer).map_err(|_| Error::Unavailable)?;
        if n == 0 {
            break;
        }
        bytes = bytes.checked_add(n as u64).ok_or(Error::Capacity)?;
        if bytes > max {
            return Err(Error::Capacity);
        }
        hash.update(&buffer[..n]);
    }
    file.rewind().map_err(|_| Error::Unavailable)?;
    Ok(format!("{:x}", hash.finalize()))
}
/// Independently inspect installed bytes. Permission errors, links and unknown bytes remain unknown.
pub(crate) fn detect(spec: &SoftwareSpec) -> SoftwareState {
    let path = std::path::Path::new(&spec.detection.path);
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Absence only under an existing, protected parent; no missing/redirected ancestor inference.
            if path
                .parent()
                .is_some_and(|p| crate::platform::protected_path(p, true).is_ok())
            {
                SoftwareState::Absent {}
            } else {
                SoftwareState::Unknown {}
            }
        }
        Ok(meta) if meta.is_file() => {
            let result = crate::platform::open_file(path)
                .and_then(|mut f| hash_file(&mut f, spec.detection.max_bytes, None));
            match result.ok().and_then(|h| {
                spec.detection
                    .versions
                    .iter()
                    .find(|v| v.sha256.as_str() == h)
            }) {
                Some(v) => SoftwareState::Present {
                    version: v.version.clone(),
                },
                None => SoftwareState::Unknown {},
            }
        }
        _ => SoftwareState::Unknown {},
    }
}
impl SoftwareArtifacts {
    pub(crate) fn prepare(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        provenance: SoftwareProvenance,
        work_root: &std::path::Path,
        control: &PreparationControl,
    ) -> Result<Lease, Error> {
        control.check()?;
        let p = plan.spec();
        let s = p.execution.software().ok_or(Error::InvalidInput)?;
        crate::platform::identity(&p.run_as, &p.session_requirement)?;
        #[cfg(test)]
        let immutable = !self.fixture_owned;
        #[cfg(not(test))]
        let immutable = true;
        let mut paths = vec![
            crate::platform::PathLease::source(&self.lock_root, false)?,
            crate::platform::PathLease::source(&self.payload, immutable)?,
            crate::platform::PathLease::source(&self.manager, immutable)?,
        ];
        let mut locks = Vec::new();
        for key in s.lock_keys() {
            let path = self.lock_root.join(key);
            let mut options = OpenOptions::new();
            options.read(true).write(true).create(true).truncate(false);
            #[cfg(target_os = "macos")]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
            }
            let file = options.open(&path).map_err(|_| Error::Denied)?;
            paths.push(crate::platform::PathLease::source(&path, false)?);
            file.try_lock().map_err(|_| Error::Conflict)?;
            locks.push(file);
        }
        let before = detect(s);
        let owner = if provenance.ownership == Ownership::OrganizationManaged
            && provenance.state.as_ref() == Some(&before)
        {
            Ownership::OrganizationManaged
        } else if provenance.ownership == Ownership::UserExisting {
            Ownership::UserExisting
        } else {
            Ownership::Unknown
        };
        let evidence = EvidenceRef {
            reference: s.snapshot.clone(),
            kind: if matches!(p.request.authority, Authority::Test { .. }) {
                EvidenceKind::TestResult
            } else {
                EvidenceKind::StateObserved
            },
            runner: Id::new("software-detector").expect("constant"),
        };
        let detected = match &before {
            SoftwareState::Absent {} => software_plan::Detection::Absent { evidence },
            SoftwareState::Present { version } => software_plan::Detection::Present {
                version: version.clone(),
                ownership: owner,
                dependencies: self.probe.dependency_use(plan)?,
                evidence,
            },
            SoftwareState::Unknown {} => return Err(Error::Unavailable),
        };
        let comparison = match &before {
            SoftwareState::Present { version } => self.probe.comparison(plan, version)?,
            _ => None,
        };
        if comparison != s.comparison {
            return Err(Error::Conflict);
        }
        let intent = software_plan::SoftwareIntent {
            authority: p.request.authority.clone(),
            target: p.request.target.clone(),
            policy: p.policy.clone(),
            package: s.package.clone(),
            desired: s.desired.clone(),
        };
        let snapshot = software_plan::PlanningSnapshot {
            revision: s.snapshot.clone(),
            authority: intent.authority.clone(),
            target: intent.target.clone(),
            policy: intent.policy.clone(),
            package: s.package.clone(),
            detection: detected,
            comparison,
            installer: InstallerCapabilities {
                artifact: s.manager.clone(),
                manager: s.package.manager.clone(),
                can_detect: true,
                operations: if s.uninstall.is_some() {
                    vec![
                        MutationKind::Install,
                        MutationKind::Upgrade,
                        MutationKind::Downgrade,
                        MutationKind::Uninstall,
                    ]
                } else {
                    vec![MutationKind::Install, MutationKind::Upgrade]
                },
                upgrade_strategy: UpgradeStrategy::InPlace,
                restart: RestartBehavior::MayRequire,
                dependency_impact: s.dependencies,
            },
            management: s.management,
            readiness: software_plan::Readiness::Ready,
        };
        let decision = software_plan::decide(
            &intent,
            &snapshot,
            software_plan::PlanningLimits {
                max_text_bytes: 1024,
                max_capabilities: 4,
            },
        )
        .map_err(|_| Error::Denied)?;
        if !matches!(decision.outcome(),software_plan::DecisionOutcome::Mutate(m) if m.kind()==s.mutation)
        {
            return Err(Error::Conflict);
        }
        let mut payload = crate::platform::open_file(&self.payload)?;
        let limit = s.bundle.map_or(4 * 1024 * 1024 * 1024, |b| b.archive_bytes);
        if hash_file(&mut payload, limit, Some(control))? != s.payload.sha256.as_str() {
            return Err(Error::Denied);
        }
        if s.adapter == SoftwareKind::Winget {
            let mut bytes = Vec::new();
            std::io::Read::by_ref(&mut payload)
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::Unavailable)?;
            if bytes.len() > 1024 * 1024 {
                return Err(Error::Capacity);
            }
            let manifest: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| Error::InvalidInput)?;
            let expected_version = match &s.desired {
                DesiredState::Present { version, .. } => Some(version.as_str()),
                DesiredState::Absent => None,
            };
            if manifest.get("PackageIdentifier").and_then(|v| v.as_str())
                != Some(s.resource.as_str())
                || expected_version.is_some_and(|v| {
                    manifest.get("PackageVersion").and_then(|v| v.as_str()) != Some(v)
                })
                || !manifest
                    .get("Installers")
                    .and_then(|v| v.as_array())
                    .is_some_and(|rows| {
                        rows.iter().any(|r| {
                            r.get("Architecture").and_then(|v| v.as_str())
                                == Some(s.package.architecture.as_str())
                        })
                    })
            {
                return Err(Error::Denied);
            }
            payload.rewind().map_err(|_| Error::Unavailable)?;
        }
        let mut manager = crate::platform::open_file(&self.manager)?;
        if hash_file(&mut manager, 256 * 1024 * 1024, Some(control))? != s.manager.sha256.as_str() {
            return Err(Error::Denied);
        }
        let expanded = if s.adapter.is_bundle() {
            Some(bundle::extract(
                payload.try_clone().map_err(|_| Error::Unavailable)?,
                s,
                work_root.join(format!("software-{}", attempt.as_str())),
                control,
            )?)
        } else {
            None
        };
        let args = commands::arguments(s, &self.manager, &self.payload)?;
        Ok(Lease {
            _locks: locks,
            _files: vec![payload, manager],
            _paths: paths,
            expanded,
            args,
            before,
        })
    }
}
impl Lease {
    pub(crate) fn entry(&self, spec: &SoftwareSpec) -> Result<Option<PathBuf>, Error> {
        self.expanded
            .as_ref()
            .map(|e| {
                let entry = if spec.mutation == MutationKind::Uninstall {
                    e.manifest.uninstall.as_ref().ok_or(Error::Unsupported)?
                } else {
                    &e.manifest.install
                };
                let expected = if spec.mutation == MutationKind::Uninstall {
                    spec.uninstall.as_ref().ok_or(Error::Unsupported)?
                } else {
                    return Ok(e.root.join(entry));
                };
                if !e
                    .manifest
                    .files
                    .iter()
                    .any(|f| &f.path == entry && f.sha256 == expected.sha256)
                {
                    return Err(Error::Denied);
                }
                Ok(e.root.join(entry))
            })
            .transpose()
    }
}

/// Canonical install entry bytes for native managers. Stage these in protected storage and bind their digest.
pub fn install_entry(adapter: SoftwareKind) -> Option<&'static [u8]> {
    commands::wrapper(adapter)
}
