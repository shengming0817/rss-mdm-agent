//! Unsupported platforms fail before spawning or materializing any work.
use execution_app::Error;
use execution_contract::*;
use std::{fs::File, path::Path};
pub(crate) fn protected_path(_: &Path, _: bool) -> Result<(), Error> {
    Err(Error::Unsupported)
}
pub(crate) fn open_file(_: &Path) -> Result<File, Error> {
    Err(Error::Unsupported)
}
pub(crate) fn identity(_: &RunAs, _: &SessionRequirement) -> Result<(), Error> {
    Err(Error::Unsupported)
}
pub(crate) fn profile(_: &VersionedRef) -> Result<(), Error> {
    Err(Error::Unsupported)
}
pub(crate) fn arguments(_: &VersionedRef, _: &[String], _: &Path) -> Result<(), Error> {
    Err(Error::Unsupported)
}
pub(crate) struct Owner;
impl Owner {
    pub(crate) fn prepare(_: &mut tokio::process::Command) -> Result<Self, Error> {
        Err(Error::Unsupported)
    }
    pub(crate) fn scope(&self) -> ProcessScope {
        unreachable!("unsupported owner cannot be constructed")
    }
    pub(crate) fn stop(&self) {}
    pub(crate) fn terminate(&mut self) {}
    pub(crate) fn quiescent(&self) -> bool {
        false
    }
}
pub(crate) fn encoding(_: ArtifactEncoding) -> Result<(), Error> {
    Err(Error::Unsupported)
}
pub(crate) fn payload(
    _: File,
    _: &[u8],
    _: &Path,
    _: &AttemptId,
    _: &VersionedRef,
) -> Result<crate::materialize::Payload, Error> {
    Err(Error::Unsupported)
}
pub(crate) struct WorkingDirectory;
impl WorkingDirectory {
    pub(crate) fn open(_: &Path) -> Result<Self, Error> {
        Err(Error::Unsupported)
    }
    pub(crate) fn configure(&self, _: &mut std::process::Command, _: &File) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

pub(crate) async fn spawn(
    _: &mut tokio::process::Command,
    _: &mut Owner,
    _: &std::sync::atomic::AtomicBool,
    _: std::time::Instant,
) -> std::io::Result<tokio::process::Child> {
    Err(std::io::Error::other("unsupported platform"))
}

pub(crate) struct PathLease;
impl PathLease {
    pub(crate) fn source(_: &Path, _: bool) -> Result<Self, Error> {
        Err(Error::Unsupported)
    }
}
