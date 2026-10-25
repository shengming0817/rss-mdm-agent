//! Embeddable current-user Windows authenticator. Hosts own composition, not UV.
//! ref: microsoft/webauthn@ef82c157125a0490e05f6ea82a7adb1b8e1bad08
//! pluginauthenticator.idl and webauthnplugin.h (MIT).
#[cfg(any(windows, test))]
mod cache;
#[cfg(windows)]
mod native;
mod operation;
#[cfg(any(windows, test))]
mod protocol;
#[cfg(any(windows, test))]
mod signature;
pub use operation::{Operation, OperationKind, Operations};
pub use passkey_vault::{Error, Result};

/// Stable component identity shared by all packaging hosts. Never select from IPC.
pub const COMPONENT: &str = "RSS.PersonalPasskeys";
pub const CLSID: &str = "f1fd3a7c-27c1-4f88-bf89-6ac6525c2929";
pub const MINIMUM_BUILD: u32 = 26100;
pub const MINIMUM_REVISION: u32 = 7171;
pub const SDK: &str = "10.0.26100.7175";

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub registered: bool,
    pub enabled: bool,
    pub locked: bool,
}
/// Display-only host hook. It cannot supply UV or request authenticity claims.
pub trait HostUi: Send + Sync {
    fn choose(
        &self,
        request: &passkey_vault::ceremony::Context<'_>,
        operation: &Operation,
    ) -> Result<Option<Vec<u8>>>;
    fn open_management(&self) -> Result<()>;
}
#[cfg(windows)]
pub use native::{registration, NativeUi, ProviderRuntime};

#[cfg(test)]
mod tests;
