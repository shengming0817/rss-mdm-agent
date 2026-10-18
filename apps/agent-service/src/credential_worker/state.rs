use super::*;
use fs2::FileExt;
use serde::de::DeserializeOwned;
#[cfg(any(windows, test))]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Prepared,
    Issued,
    Finished,
}
#[cfg(any(windows, test))]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub task: w::CredentialTaskSpec,
    pub phase: Phase,
    pub protected: Vec<u8>,
    pub subject: Option<w::CredentialSubject>,
}
pub(super) fn name(task: &w::CredentialTaskSpec, suffix: &str) -> String {
    format!(
        "{}-{}-{}-{}.{}",
        task.registration_id, task.generation, task.task_id, task.attempt_id, suffix
    )
}
pub(super) fn lock(root: &Path) -> Result<std::fs::File, Error> {
    let dir = platform_private_storage::PrivateDirectory::open(root)?;
    match dir.create_file(Path::new("owner.lock")) {
        Ok(file) => {
            file.sync_all()?;
            drop(file);
            dir.sync()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.into()),
    }
    let file = dir.open_file(Path::new("owner.lock"))?;
    file.try_lock_exclusive().map_err(|_| Error::Unavailable)?;
    Ok(file)
}
pub(super) fn read_optional<T: DeserializeOwned>(
    root: &Path,
    name: &str,
) -> Result<Option<T>, Error> {
    let dir = platform_private_storage::PrivateDirectory::open(root)?;
    match dir.read(Path::new(name), 4 * 1024 * 1024) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| Error::Storage),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn write(root: &Path, name: &str, value: &impl Serialize) -> Result<(), Error> {
    let dir = platform_private_storage::PrivateDirectory::open(root)?;
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Storage)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(Error::Capacity);
    }
    let stage = format!("{}.tmp", uuid::Uuid::new_v4());
    let mut file = dir.create_file(Path::new(&stage))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    dir.replace(Path::new(&stage), Path::new(name))?;
    dir.sync()?;
    Ok(())
}
#[cfg(any(windows, test))]
pub(super) fn secret(native: &impl Native, record: &Record) -> Result<Zeroizing<String>, Error> {
    let bytes = native.unprotect(&record.protected)?;
    // Protect the binding with the material, not merely the filesystem filename.
    let (binding, secret): (w::CredentialTaskSpec, String) =
        serde_json::from_slice(&bytes).map_err(|_| Error::Storage)?;
    let secret = Zeroizing::new(secret);
    if binding != record.task {
        return Err(Error::Untrusted);
    }
    Ok(secret)
}
#[cfg(any(windows, test))]
pub(super) fn seal(
    native: &impl Native,
    task: &w::CredentialTaskSpec,
    secret: &str,
) -> Result<Vec<u8>, Error> {
    let bytes = Zeroizing::new(serde_json::to_vec(&(task, secret)).map_err(|_| Error::Protocol)?);
    native.protect(&bytes)
}

pub(super) fn remove(root: &Path, name: &str) -> Result<(), Error> {
    let dir = platform_private_storage::PrivateDirectory::open(root)?;
    match dir.open_file(Path::new(name)) {
        Ok(file) => {
            drop(file);
            std::fs::remove_file(root.join(name))?;
            dir.sync()?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
