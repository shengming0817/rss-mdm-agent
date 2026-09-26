use super::{hash_file, PreparationControl};
use execution_app::Error;
use execution_contract::*;
use std::{fs::File, path::Path};

/// Held filesystem observations with protected parent traversal and a pre-launch identity recheck.
/// The target handle identifies the observed object; native managers retain responsibility for
/// their own mutation transaction against non-cooperating external installers.
pub(super) struct TargetGuard {
    pub binding: SoftwareResource,
    _parent: File,
    target: Option<File>,
    _paths: crate::platform::PathLease,
}
impl TargetGuard {
    pub(super) fn open(path: &Path) -> Result<Self, Error> {
        let parent = path.parent().ok_or(Error::Denied)?;
        let paths = crate::platform::PathLease::source(parent, false)?;
        let directory = crate::platform::open_directory(parent)?;
        let target = match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() => Some(crate::platform::open_observed_file(path)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            _ => return Err(Error::Denied),
        };
        let binding = SoftwareResource {
            parent: crate::platform::file_identity(&directory)?,
            object: target
                .as_ref()
                .map(crate::platform::file_identity)
                .transpose()?,
        };
        Ok(Self {
            binding,
            _parent: directory,
            target,
            _paths: paths,
        })
    }
    pub(super) fn observe(
        &mut self,
        spec: &SoftwareSpec,
        control: &PreparationControl,
    ) -> super::DetectionResult {
        use SoftwareDetectionFailure as F;
        let unknown = |reason| super::DetectionResult {
            state: SoftwareState::Unknown { reason },
            object: None,
        };
        if control.check().is_err() {
            return unknown(F::BudgetExceeded);
        }
        if self.binding.parent != spec.resource_binding.parent {
            return unknown(F::Unavailable);
        }
        let Some(file) = self.target.as_mut() else {
            return super::DetectionResult {
                state: SoftwareState::Absent {},
                object: None,
            };
        };
        match hash_file(file, spec.detection.max_bytes, control) {
            Ok(hash) => match spec
                .detection
                .versions
                .iter()
                .find(|v| v.sha256.as_str() == hash)
            {
                Some(v) => super::DetectionResult {
                    state: SoftwareState::Present {
                        version: v.version.clone(),
                    },
                    object: self.binding.object.clone(),
                },
                None => unknown(F::UnrecognizedVersion),
            },
            Err(Error::Capacity) => unknown(F::BudgetExceeded),
            Err(_) if control.check().is_err() => unknown(F::BudgetExceeded),
            Err(_) => unknown(F::Unavailable),
        }
    }
    pub(super) fn recheck(
        &self,
        spec: &SoftwareSpec,
        before: &SoftwareState,
        control: &PreparationControl,
    ) -> Result<(), Error> {
        let mut current = Self::open(Path::new(&spec.detection.path))?;
        if current.binding != self.binding || current.observe(spec, control).state != *before {
            return Err(Error::Conflict);
        }
        Ok(())
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn replacement_and_hardlink_aliases_use_physical_identity() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".cache")
            .join(format!("resource-guard-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("Installed");
        std::fs::write(&target, "same").unwrap();
        let guard = TargetGuard::open(&target).unwrap();
        let alias = root.join("alias");
        std::fs::hard_link(&target, &alias).unwrap();
        assert_eq!(guard.binding, TargetGuard::open(&alias).unwrap().binding);
        let case_alias = root.join("installed");
        if case_alias.exists() {
            assert_eq!(
                guard.binding,
                TargetGuard::open(&case_alias).unwrap().binding
            );
        }
        std::fs::rename(&target, root.join("old")).unwrap();
        std::fs::write(&target, "same").unwrap();
        assert_ne!(guard.binding, TargetGuard::open(&target).unwrap().binding);
        std::fs::remove_dir_all(root).unwrap();
    }
}
