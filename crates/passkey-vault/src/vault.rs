use crate::{
    crypto::{self, Key, PasswordSlot},
    model::{self, Content, Envelope, LocalProtection},
    store::Store,
    Entry, Error, Result, Status, UnlockMethod,
};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use zeroize::Zeroizing;

/// Trusted native adapter. Authentication must be interactive and local to this OS user.
/// Implementations must not forward keys/passwords to enterprise services or ordinary IPC.
pub trait SystemProtection: Send + Sync {
    fn verify(&self) -> Result<()>;
    fn protect(&self, reference: &str, key: &[u8]) -> Result<Vec<u8>>;
    fn unprotect(&self, reference: &str, protected: &[u8]) -> Result<Zeroizing<Vec<u8>>>;
}
pub(crate) struct Session {
    pub id: String,
    pub key: Key,
}
/// Opaque cancellation snapshot captured before native input or worker dispatch.
pub struct Interaction {
    instance: uuid::Uuid,
    epoch: u64,
}
pub struct Vault {
    instance: uuid::Uuid,
    pub(crate) store: Store,
    system: Arc<dyn SystemProtection>,
    session: Mutex<Option<Session>>,
    pub(crate) epoch: AtomicU64,
    operation: Mutex<()>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    owner: String,
    vault_id: String,
    key_id: String,
    published: bool,
}

impl Vault {
    pub fn open(root: &Path, system: Arc<dyn SystemProtection>) -> Result<Self> {
        let owner = crate::platform::owner()?;
        let vault = Self {
            instance: uuid::Uuid::new_v4(),
            store: Store::new(root, owner)?,
            system,
            session: Mutex::new(None),
            epoch: AtomicU64::new(0),
            operation: Mutex::new(()),
        };
        // Repair only a publication receipt, never missing/corrupt credential data.
        let tx = vault.store.transaction()?;
        if let Ok(Some(value)) = tx.load(&vault.store.owner) {
            Self::published(&tx, &value)?;
        }
        drop(tx);
        Ok(vault)
    }
    fn published(tx: &crate::store::Transaction, value: &Envelope) -> Result<()> {
        let record = Pending {
            owner: value.local.owner.clone(),
            vault_id: value.vault_id.clone(),
            key_id: value.key_id.clone(),
            published: true,
        };
        let bytes = serde_json::to_vec(&record).map_err(|_| Error::Corrupt)?;
        if tx.read("initializing.json")?.as_deref() != Some(bytes.as_slice()) {
            tx.write("initializing.json", &bytes)?;
        }
        Ok(())
    }
    pub fn begin_interaction(&self) -> Interaction {
        Interaction {
            instance: self.instance,
            epoch: self.generation(),
        }
    }
    fn interaction(&self, token: &Interaction) -> Result<u64> {
        if token.instance != self.instance {
            return Err(Error::Cancelled);
        }
        self.current(token.epoch)?;
        Ok(token.epoch)
    }
    pub(crate) fn generation(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }
    pub(crate) fn current(&self, epoch: u64) -> Result<()> {
        if self.generation() != epoch {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
    /// Lock is never queued behind a password KDF or native dialog.
    pub fn lock(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut session) = self.session.lock() {
            *session = None;
        }
    }
    pub(crate) fn session_key(&self, value: &Envelope) -> Result<Key> {
        let guard = self.session.lock().map_err(|_| Error::Locked)?;
        let session = guard.as_ref().ok_or(Error::Locked)?;
        if session.id != value.vault_id {
            return Err(Error::Conflict);
        }
        Ok(Zeroizing::new(*session.key))
    }
    fn install(&self, value: &Envelope, key: Key, epoch: u64) -> Result<()> {
        let mut guard = self.session.lock().map_err(|_| Error::Locked)?;
        self.current(epoch)?;
        *guard = Some(Session {
            id: value.vault_id.clone(),
            key,
        });
        Ok(())
    }
    pub(crate) fn content(value: &Envelope, key: &Key) -> Result<Content> {
        let bytes = crypto::open(key, &value.aad("content"), &value.content)?;
        let content: Content = serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt)?;
        content.validate()?;
        Ok(content)
    }
    pub(crate) fn encrypt_content(
        value: &mut Envelope,
        key: &Key,
        content: &Content,
    ) -> Result<()> {
        content.validate()?;
        let bytes = Zeroizing::new(serde_json::to_vec(content).map_err(|_| Error::Corrupt)?);
        value.content = crypto::seal(key, &value.aad("content"), &bytes)?;
        value.revision = value.revision.checked_add(1).ok_or(Error::Capacity)?;
        Ok(())
    }
    fn read(&self) -> Result<Envelope> {
        self.store
            .transaction()?
            .load(&self.store.owner)?
            .ok_or(Error::NotFound)
    }
    fn authenticated_key(
        &self,
        value: &Envelope,
        method: UnlockMethod,
        password: Option<&str>,
    ) -> Result<Key> {
        match method {
            UnlockMethod::Password => value
                .password
                .as_ref()
                .ok_or(Error::Authentication)?
                .unwrap(&value.aad("password-root"), password.ok_or(Error::Input)?),
            UnlockMethod::System => {
                self.system.verify()?;
                let bytes = self
                    .system
                    .unprotect(&value.local.reference, &value.local.protected_root)?;
                if bytes.len() != 32 {
                    return Err(Error::Corrupt);
                }
                let mut key = Zeroizing::new([0; 32]);
                key.copy_from_slice(&bytes);
                Ok(key)
            }
        }
    }
    pub fn status(&self) -> Result<Status> {
        let value = self.store.transaction()?.load(&self.store.owner)?;
        let Some(value) = value else {
            if let Some(bytes) = self.store.transaction()?.read("initializing.json")? {
                let pending: Pending =
                    serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt)?;
                if pending.published {
                    return Err(Error::Corrupt);
                }
            }
            return Ok(Status {
                initialized: false,
                unlocked: false,
                password_enabled: false,
                revision: 0,
                entries: vec![],
            });
        };
        let key = self.session_key(&value).ok();
        let entries = if let Some(key) = &key {
            Self::content(&value, key)?
                .records
                .into_iter()
                .map(|r| r.entry.clone())
                .collect()
        } else {
            vec![]
        };
        Ok(Status {
            initialized: true,
            unlocked: key.is_some(),
            password_enabled: value.password.is_some(),
            revision: value.revision,
            entries,
        })
    }
    pub fn create(&self, token: &Interaction) -> Result<()> {
        let _operation = self.operation.try_lock().map_err(|_| Error::Busy)?;
        let epoch = self.interaction(token)?;
        self.system.verify()?;
        self.current(epoch)?;
        let tx = self.store.transaction()?;
        if tx.load(&self.store.owner)?.is_some() {
            return Err(Error::Conflict);
        }
        let pending: Pending = match tx.read("initializing.json")? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt)?,
            None => {
                let pending = Pending {
                    owner: self.store.owner.clone(),
                    vault_id: uuid::Uuid::new_v4().to_string(),
                    key_id: uuid::Uuid::new_v4().to_string(),
                    published: false,
                };
                tx.write(
                    "initializing.json",
                    &serde_json::to_vec(&pending).map_err(|_| Error::Corrupt)?,
                )?;
                pending
            }
        };
        if pending.published {
            return Err(Error::Corrupt);
        }
        if pending.owner != self.store.owner
            || uuid::Uuid::parse_str(&pending.vault_id).is_err()
            || uuid::Uuid::parse_str(&pending.key_id).is_err()
        {
            return Err(Error::Owner);
        }
        // A pending initialization contains no credentials. Recover an already-created
        // Keychain item by its exact reference; Windows protection has no side effect.
        let key = match self.system.unprotect(&pending.key_id, &[]) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut key = Zeroizing::new([0; 32]);
                key.copy_from_slice(&bytes);
                key
            }
            Err(Error::NotFound) => Zeroizing::new(crypto::random()?),
            Ok(_) => return Err(Error::Corrupt),
            Err(e) => return Err(e),
        };
        let protected_root = self.system.protect(&pending.key_id, key.as_ref())?;
        let value = Envelope {
            format: 1,
            vault_id: pending.vault_id.clone(),
            key_id: pending.key_id.clone(),
            revision: 1,
            local: LocalProtection {
                owner: self.store.owner.clone(),
                reference: pending.key_id.clone(),
                protected_root,
            },
            password: None,
            content: crypto::seal(
                &key,
                &model::aad(&pending.vault_id, &pending.key_id, "content"),
                b"{\"records\":[]}",
            )?,
        };
        self.current(epoch)?;
        tx.save(&value)?;
        Self::published(&tx, &value)?;
        self.install(&value, key, epoch)
    }
    pub fn unlock(
        &self,
        method: UnlockMethod,
        password: Option<&str>,
        token: &Interaction,
    ) -> Result<()> {
        let _operation = self.operation.try_lock().map_err(|_| Error::Busy)?;
        let epoch = self.interaction(token)?;
        let value = self.read()?;
        let key = self.authenticated_key(&value, method, password)?;
        Self::content(&value, &key)?;
        let latest = self.read()?;
        if latest.vault_id != value.vault_id || latest.revision != value.revision {
            return Err(Error::Conflict);
        }
        self.install(&value, key, epoch)
    }
    /// Fresh authentication, never merely a cached unlocked session, changes unlock methods.
    pub fn set_password(
        &self,
        method: UnlockMethod,
        current: Option<&str>,
        new: &str,
        confirmation: &str,
        token: &Interaction,
    ) -> Result<()> {
        if new != confirmation {
            return Err(Error::Input);
        }
        crypto::validate_password(new)?;
        self.change_unlock(method, current, token, |value, key| {
            value.password = Some(PasswordSlot::new(key, &value.aad("password-root"), new)?);
            Ok(())
        })
    }
    pub fn remove_password(&self, token: &Interaction) -> Result<()> {
        self.change_unlock(UnlockMethod::System, None, token, |value, _| {
            value.password = None;
            Ok(())
        })
    }
    pub fn repair_system(&self, password: &str, token: &Interaction) -> Result<()> {
        self.change_unlock(
            UnlockMethod::Password,
            Some(password),
            token,
            |value, key| {
                self.system.verify()?;
                let reference = uuid::Uuid::new_v4().to_string();
                let protected_root = self.system.protect(&reference, key.as_ref())?;
                value.local = LocalProtection {
                    owner: self.store.owner.clone(),
                    reference,
                    protected_root,
                };
                Ok(())
            },
        )
    }
    fn change_unlock(
        &self,
        method: UnlockMethod,
        password: Option<&str>,
        token: &Interaction,
        change: impl FnOnce(&mut Envelope, &Key) -> Result<()>,
    ) -> Result<()> {
        let _operation = self.operation.try_lock().map_err(|_| Error::Busy)?;
        let epoch = self.interaction(token)?;
        let mut value = self.read()?;
        let key = self.authenticated_key(&value, method, password)?;
        Self::content(&value, &key)?;
        change(&mut value, &key)?;
        let tx = self.store.transaction()?;
        let latest = tx.load(&self.store.owner)?.ok_or(Error::Conflict)?;
        if latest.vault_id != value.vault_id || latest.revision != value.revision {
            return Err(Error::Conflict);
        }
        value.revision = value.revision.checked_add(1).ok_or(Error::Capacity)?;
        self.current(epoch)?;
        tx.save(&value)?;
        self.install(&value, key, epoch)
    }
    pub fn delete(&self, credential: &[u8], revision: u64, token: &Interaction) -> Result<()> {
        let epoch = self.interaction(token)?;
        let tx = self.store.transaction()?;
        let mut value = tx.load(&self.store.owner)?.ok_or(Error::NotFound)?;
        if value.revision != revision {
            return Err(Error::Conflict);
        }
        let key = self.session_key(&value)?;
        let mut content = Self::content(&value, &key)?;
        let count = content.records.len();
        content
            .records
            .retain(|r| r.entry.credential_id != credential);
        if count == content.records.len() {
            return Err(Error::NotFound);
        }
        Self::encrypt_content(&mut value, &key, &content)?;
        self.current(epoch)?;
        tx.save(&value)
    }
    /// Preserve the exact encrypted original before explicitly starting over.
    pub fn reset(&self, confirmation: &str, token: &Interaction) -> Result<()> {
        if confirmation != "DELETE LOCAL VAULT" {
            return Err(Error::Input);
        }
        let _operation = self.operation.try_lock().map_err(|_| Error::Busy)?;
        let epoch = self.interaction(token)?;
        self.system.verify()?;
        self.current(epoch)?;
        let tx = self.store.transaction()?;
        let original = tx.read("vault.json")?.ok_or(Error::NotFound)?;
        let id = uuid::Uuid::new_v4().to_string();
        let key_id = uuid::Uuid::new_v4().to_string();
        let key = Zeroizing::new(crypto::random()?);
        let protected_root = self.system.protect(&key_id, key.as_ref())?;
        let value = Envelope {
            format: 1,
            vault_id: id.clone(),
            key_id: key_id.clone(),
            revision: 1,
            local: LocalProtection {
                owner: self.store.owner.clone(),
                reference: key_id.clone(),
                protected_root,
            },
            password: None,
            content: crypto::seal(
                &key,
                &model::aad(&id, &key_id, "content"),
                b"{\"records\":[]}",
            )?,
        };
        tx.write(&format!("retired-{id}.json"), &original)?;
        self.current(epoch)?;
        tx.save(&value)?;
        Self::published(&tx, &value)?;
        self.install(&value, key, epoch)
    }
    pub fn entries(&self, rp_id: &str) -> Result<Vec<Entry>> {
        Ok(self
            .status()?
            .entries
            .into_iter()
            .filter(|e| e.rp_id == rp_id)
            .collect())
    }
}
