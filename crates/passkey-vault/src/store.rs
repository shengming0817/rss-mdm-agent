use crate::{
    model::{Envelope, MAX_FILE},
    Error, Result,
};
use platform_private_storage::PrivateDirectory;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) struct Store {
    pub root: PathBuf,
    pub owner: String,
}
pub(crate) struct Transaction {
    directory: PrivateDirectory,
    _lock: File,
    root: PathBuf,
}
impl Store {
    pub fn new(root: &Path, owner: String) -> Result<Self> {
        PrivateDirectory::create(root).map_err(|_| Error::Storage)?;
        Ok(Self {
            root: root.into(),
            owner,
        })
    }
    pub fn transaction(&self) -> Result<Transaction> {
        let directory = PrivateDirectory::open(&self.root).map_err(|_| Error::Owner)?;
        let lock = match directory.create_file(Path::new("vault.lock")) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => directory
                .open_file(Path::new("vault.lock"))
                .map_err(|_| Error::Storage)?,
            Err(_) => return Err(Error::Storage),
        };
        platform_private_storage::validate_single_link(&self.root.join("vault.lock"))
            .map_err(|_| Error::Owner)?;
        lock.try_lock().map_err(|_| Error::Busy)?;
        Ok(Transaction {
            directory,
            _lock: lock,
            root: self.root.clone(),
        })
    }
}
impl Transaction {
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        match self.directory.read(Path::new(name), MAX_FILE) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(Error::Storage),
        }
    }
    pub fn load(&self, owner: &str) -> Result<Option<Envelope>> {
        let Some(bytes) = self.read("vault.json")? else {
            return Ok(None);
        };
        let value: Envelope = serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt)?;
        value.validate(owner)?;
        Ok(Some(value))
    }
    pub fn write(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_FILE {
            return Err(Error::Capacity);
        }
        // One staging pathname under the exclusive stable lock. Reopening only discards
        // uncommitted ciphertext, never the last published snapshot.
        let stage = Path::new("vault.stage");
        if self.directory.open_file(stage).is_ok() {
            std::fs::remove_file(self.root.join(stage)).map_err(|_| Error::Storage)?;
        }
        let mut file = self
            .directory
            .create_file(stage)
            .map_err(|_| Error::Storage)?;
        file.write_all(bytes).map_err(|_| Error::Storage)?;
        file.sync_all().map_err(|_| Error::Storage)?;
        drop(file);
        #[cfg(test)]
        crash_at(name, "staged");
        self.directory
            .replace(stage, Path::new(name))
            .map_err(|_| Error::Storage)?;
        #[cfg(test)]
        crash_at(name, "published");
        Ok(())
    }
    pub fn remove_index_notice(&self) -> Result<()> {
        match std::fs::remove_file(self.root.join("provider-index-dirty")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(Error::Storage),
        }
    }
    pub fn save(&self, value: &Envelope) -> Result<()> {
        // A crash may leave a conservative dirty index; never an authoritative stale index.
        self.write("provider-index-dirty", b"dirty")?;
        self.write(
            "vault.json",
            &serde_json::to_vec(value).map_err(|_| Error::Corrupt)?,
        )
    }
}

#[cfg(test)]
pub(crate) fn crash_at(name: &str, phase: &str) {
    // Only a child test process sets this variable. No failpoint exists in production.
    let point = std::env::var("RSS_VAULT_TEST_CRASH").unwrap_or_default();
    if (name == "vault.json" && point == phase) || point == format!("{name}:{phase}") {
        std::process::exit(86);
    }
}
