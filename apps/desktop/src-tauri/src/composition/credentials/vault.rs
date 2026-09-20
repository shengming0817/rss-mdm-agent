//! Non-secret ownership journal for app-created Keychain entries. External CLI entries are never deleted.
use security_framework::passwords::delete_generic_password;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Closed failure for the private native credential ownership journal.
#[derive(Debug)]
pub struct CredentialError;
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    user: String,
    generation: String,
    active: bool,
}
pub struct Vault {
    path: PathBuf,
    entries: BTreeMap<String, Entry>,
}
impl Vault {
    pub fn open(root: &Path) -> Result<Self, CredentialError> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let path = root.join("credential-refs.json");
        let entries = match fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
        {
            Ok(file) => {
                let stat = file.metadata().map_err(|_| CredentialError)?;
                if !stat.is_file()
                    || stat.mode() & 0o077 != 0
                    || stat.uid() != fs::metadata(root).map_err(|_| CredentialError)?.uid()
                    || stat.len() > 2_097_152
                {
                    return Err(CredentialError);
                }
                let mut bytes = Vec::new();
                file.take(2_097_153)
                    .read_to_end(&mut bytes)
                    .map_err(|_| CredentialError)?;
                if bytes.len() > 2_097_152 {
                    return Err(CredentialError);
                }
                serde_json::from_slice(&bytes).map_err(|_| CredentialError)?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(_) => return Err(CredentialError),
        };
        let mut vault = Self { path, entries };
        // A new desktop process owns no pending credential editor or probe.
        let pending: Vec<_> = vault
            .entries
            .iter()
            .filter(|(_, e)| !e.active)
            .map(|(r, _)| r.clone())
            .collect();
        for reference in pending {
            vault.remove(&reference)?;
        }
        vault.persist()?;
        Ok(vault)
    }
    fn persist(&self) -> Result<(), CredentialError> {
        use std::os::unix::fs::OpenOptionsExt;
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(&serde_json::to_vec(&self.entries)?)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)?;
            fs::File::open(
                self.path
                    .parent()
                    .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?,
            )?
            .sync_all()?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result.map_err(|_| CredentialError)
    }
    pub fn stage(
        &mut self,
        user: &str,
        generation: &str,
        reference: &str,
    ) -> Result<(), CredentialError> {
        if self.entries.len() >= 16384 || self.entries.contains_key(reference) {
            return Err(CredentialError);
        }
        self.entries.insert(
            reference.into(),
            Entry {
                user: user.into(),
                generation: generation.into(),
                active: false,
            },
        );
        if self.persist().is_err() {
            self.entries.remove(reference);
            return Err(CredentialError);
        }
        Ok(())
    }
    pub fn readable(&self, user: &str, generation: &str, reference: &str) -> bool {
        self.entries.get(reference).is_some_and(|entry| {
            entry.user == user && (entry.active || entry.generation == generation)
        })
    }
    pub fn activate(
        &mut self,
        user: &str,
        generation: &str,
        reference: &str,
    ) -> Result<(), CredentialError> {
        if !self.readable(user, generation, reference) {
            return Err(CredentialError);
        }
        let previous = self.entries.get(reference).ok_or(CredentialError)?.active;
        self.entries
            .get_mut(reference)
            .ok_or(CredentialError)?
            .active = true;
        if self.persist().is_err() {
            self.entries
                .get_mut(reference)
                .ok_or(CredentialError)?
                .active = previous;
            return Err(CredentialError);
        }
        Ok(())
    }
    fn remove(&mut self, reference: &str) -> Result<(), CredentialError> {
        let Some(entry) = self.entries.get(reference) else {
            return Ok(());
        };
        match delete_generic_password(super::SERVICE, &format!("{}:{reference}", entry.user)) {
            Ok(()) => {}
            Err(e) if e.code() == -25300 => {}
            Err(_) => return Err(CredentialError),
        }
        self.entries.remove(reference);
        self.persist()
    }
    pub fn discard(&mut self, user: &str, reference: &str) -> Result<(), CredentialError> {
        if self
            .entries
            .get(reference)
            .is_some_and(|entry| entry.user == user && !entry.active)
        {
            self.remove(reference)?;
        }
        Ok(())
    }
    pub fn discard_generation(
        &mut self,
        user: &str,
        generation: &str,
    ) -> Result<(), CredentialError> {
        let pending: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| e.user == user && e.generation == generation && !e.active)
            .map(|(r, _)| r.clone())
            .collect();
        for reference in pending {
            self.remove(&reference)?;
        }
        Ok(())
    }
    pub fn collect(&mut self, user: &str, keep: &[String]) -> Result<(), CredentialError> {
        let obsolete: Vec<_> = self
            .entries
            .iter()
            .filter(|(r, e)| e.user == user && e.active && !keep.contains(r))
            .map(|(r, _)| r.clone())
            .collect();
        for reference in obsolete {
            self.remove(&reference)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_is_durable_and_an_editor_cannot_discard_an_active_reference() {
        let root = std::env::temp_dir().join(format!("rss-vault-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let mut vault = Vault::open(&root).unwrap();
        vault.stage("alice", "g1", "reference").unwrap();
        assert!(vault.readable("alice", "g1", "reference"));
        assert!(!vault.readable("alice", "g2", "reference"));
        assert!(!vault.readable("bob", "g1", "reference"));
        vault.activate("alice", "g1", "reference").unwrap();
        vault.discard("alice", "reference").unwrap();
        let restored = Vault::open(&root).unwrap();
        assert!(restored.readable("alice", "g2", "reference"));
        assert!(!restored.readable("bob", "g2", "reference"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "manual macOS Keychain acceptance; creates and removes only a unique synthetic entry"]
    fn native_keychain_reference_lifecycle() {
        use security_framework::passwords::{get_generic_password, set_generic_password};
        let root = std::env::temp_dir().join(format!("rss-vault-native-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let reference = uuid::Uuid::new_v4().to_string();
        let user = uuid::Uuid::new_v4().to_string();
        let account = format!("{user}:{reference}");
        let result = (|| -> Result<(), CredentialError> {
            let mut vault = Vault::open(&root)?;
            vault.stage(&user, "generation", &reference)?;
            set_generic_password(
                super::super::SERVICE,
                &account,
                b"synthetic-acceptance-value",
            )
            .map_err(|_| CredentialError)?;
            if get_generic_password(super::super::SERVICE, &account).map_err(|_| CredentialError)?
                != b"synthetic-acceptance-value"
            {
                return Err(CredentialError);
            }
            vault.activate(&user, "generation", &reference)?;
            vault.discard(&user, &reference)?;
            vault.collect(&user, std::slice::from_ref(&reference))?;
            get_generic_password(super::super::SERVICE, &account).map_err(|_| CredentialError)?;
            vault.collect(&user, &[])?;
            if get_generic_password(super::super::SERVICE, &account)
                .err()
                .map(|e| e.code())
                != Some(-25300)
            {
                return Err(CredentialError);
            }
            if vault.readable(&user, "generation", &reference) {
                return Err(CredentialError);
            }
            Ok(())
        })();
        let _ = delete_generic_password(super::super::SERVICE, &account);
        fs::remove_dir_all(root).unwrap();
        assert!(result.is_ok(), "native Keychain lifecycle did not complete");
    }
}
