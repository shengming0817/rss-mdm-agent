use crate::{
    crypto::{PasswordSlot, Sealed},
    Error, Result,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub(crate) const MAX_FILE: usize = 32 * 1024 * 1024;
pub(crate) const MAX_ENTRIES: usize = 10_000;

#[derive(Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnlockMethod {
    System,
    Password,
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(rename = "PersonalVaultEntry")]
pub struct Entry {
    pub credential_id: Vec<u8>,
    pub rp_id: String,
    pub user_handle: Vec<u8>,
    pub username: String,
    pub display_name: String,
    pub backup_eligible: bool,
    pub backed_up: bool,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "PersonalVaultStatus")]
pub struct Status {
    pub initialized: bool,
    pub unlocked: bool,
    pub password_enabled: bool,
    pub revision: u64,
    pub entries: Vec<Entry>,
}
#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    #[zeroize(skip)]
    pub entry: Entry,
    pub cose: Vec<u8>,
}
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct Content {
    pub records: Vec<Record>,
}
impl Content {
    pub fn validate(&self) -> Result<()> {
        if self.records.len() > MAX_ENTRIES {
            return Err(Error::Capacity);
        }
        let mut ids = std::collections::HashSet::new();
        for r in &self.records {
            let e = &r.entry;
            if e.credential_id.is_empty()
                || e.credential_id.len() > 1023
                || e.user_handle.is_empty()
                || e.user_handle.len() > 64
                || e.rp_id.len() > 253
                || e.username.len() > 1024
                || e.display_name.len() > 1024
                || !e.backup_eligible
                || e.backed_up
                || r.cose.len() > 1024
                || !ids.insert(e.credential_id.as_slice())
            {
                return Err(Error::Corrupt);
            }
        }
        Ok(())
    }
}

/// Only this section is device-bound. Never send it to a synchronization service.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalProtection {
    pub owner: String,
    pub reference: String,
    pub protected_root: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Envelope {
    pub format: u32,
    pub vault_id: String,
    pub key_id: String,
    pub revision: u64,
    pub local: LocalProtection,
    pub password: Option<PasswordSlot>,
    pub content: Sealed,
}
impl Envelope {
    pub fn aad(&self, purpose: &str) -> Vec<u8> {
        aad(&self.vault_id, &self.key_id, purpose)
    }
    pub fn validate(&self, owner: &str) -> Result<()> {
        if self.format != 1 {
            return Err(Error::Unsupported);
        }
        if self.local.owner != owner {
            return Err(Error::Owner);
        }
        if uuid::Uuid::parse_str(&self.vault_id).is_err()
            || uuid::Uuid::parse_str(&self.key_id).is_err()
            || uuid::Uuid::parse_str(&self.local.reference).is_err()
            || self.revision == 0
            || self.local.protected_root.len() > 65536
        {
            return Err(Error::Corrupt);
        }
        Ok(())
    }
}
pub(crate) fn aad(vault: &str, key: &str, purpose: &str) -> Vec<u8> {
    format!("rss-personal-passkey-v1:{vault}:{key}:{purpose}").into_bytes()
}
