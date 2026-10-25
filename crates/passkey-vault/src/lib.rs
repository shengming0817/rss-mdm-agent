//! Personal, OS-user-owned passkeys. No enterprise identity or recovery authority.
pub mod ceremony;
mod crypto;
mod model;
pub mod platform;
mod store;
mod vault;

pub use model::{Entry, Status, UnlockMethod};
use schemars::JsonSchema;
use serde::Serialize;
pub use vault::{Interaction, SystemProtection, Vault};

/// Closed errors deliberately carry no path, credential, OS message or secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    #[error("vault is locked")]
    Locked,
    #[error("operation cancelled or stale")]
    Cancelled,
    #[error("vault is busy")]
    Busy,
    #[error("invalid input")]
    Input,
    #[error("credential not found")]
    NotFound,
    #[error("credential excluded")]
    Excluded,
    #[error("authentication failed")]
    Authentication,
    #[error("system protection unavailable")]
    SystemUnavailable,
    #[error("vault data unavailable or corrupt")]
    Corrupt,
    #[error("unsupported vault format")]
    Unsupported,
    #[error("OS owner mismatch")]
    Owner,
    #[error("storage operation failed")]
    Storage,
    #[error("vault capacity exceeded")]
    Capacity,
    #[error("vault changed; retry the operation")]
    Conflict,
    #[error("protocol request rejected")]
    Protocol,
}
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;

/// A derived identity projection; it contains no private key material.
pub struct IndexSnapshot {
    pub vault_id: String,
    pub revision: u64,
    pub entries: Vec<Entry>,
}
