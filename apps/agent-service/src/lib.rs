//! Backend-authorized execution through the existing admission and journal owners.
//! No local enterprise approver, source catalog or policy-signing authority is introduced.
mod account_identity;
mod backend;
mod collected_data;
mod collection_environment;
pub mod collection_worker;
pub mod deployment;
mod device_overview;
pub mod enrollment_worker;
mod error;
mod local_execution;
pub mod local_permissions;
pub mod local_settings;
pub mod local_software;
mod material_gc;
mod mdm_state;
pub mod network_check;
mod recovery;
mod registration;
mod service;
pub mod software_worker;
mod telemetry;
use agent_client::{Clock, Error, IdentityProvider};
pub use backend::plan::{Interpreter, SoftwareManager, SoftwareManagerKind};
use base64::Engine;
pub use service::{DeviceService, ExecutionConfig, ProductionStartup, UserResources};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// Device credential provider, isolated from desktop/provider credentials.
pub struct DeviceSecrets(secrets::SecretStore);
impl DeviceSecrets {
    fn storage_binding(root: &Path) -> Result<[u8; 32], Error> {
        use sha2::{Digest as _, Sha256};
        Ok(Sha256::digest(root.canonicalize()?.as_os_str().as_encoded_bytes()).into())
    }
    /// Bind the OS credential namespace to its sole journal location on first initialization.
    /// A new configured directory cannot reuse the same registration to discard old attempts.
    pub(crate) fn bind_storage(&self, root: &Path) -> Result<(), Error> {
        self.0.initialize_catalog()?;
        self.0
            .import("journal-location", &Self::storage_binding(root)?)?;
        Ok(())
    }
    pub(crate) fn verify_storage(&self, root: &Path) -> Result<(), Error> {
        if self.0.read("journal-location")?.as_slice() != Self::storage_binding(root)? {
            return Err(Error::Identity);
        }
        Ok(())
    }
    /// Open the OS-protected namespace selected by the administrator's deployment binding.
    pub fn open(root: &Path, namespace: &str) -> Result<Self, Error> {
        Ok(Self(secrets::SecretStore::open(root, namespace)?))
    }
    /// Import the server-issued enrollment secret once, without overwrite or plaintext persistence.
    pub fn import_bootstrap(
        &self,
        reference: &str,
        value: &rss_mdm_agent_wire::runtime::Secret,
    ) -> Result<(), Error> {
        let bytes = zeroize::Zeroizing::new(
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(value.expose())
                .map_err(|_| Error::Identity)?,
        );
        self.0.import(reference, &bytes)?;
        Ok(())
    }
}
impl IdentityProvider for DeviceSecrets {
    fn software_read_token(
        &self,
        credential: &rss_mdm_agent_wire::runtime::SoftwareReadCredential,
    ) -> Result<zeroize::Zeroizing<String>, Error> {
        let bytes = self
            .0
            .read(&agent_client::native_source::credential_key(credential)?)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| Error::Identity)?;
        agent_client::native_source::validate_read_token(text)?;
        Ok(zeroize::Zeroizing::new(text.into()))
    }
    fn native_identities(&self) -> Result<Vec<agent_client::NativeIdentity>, Error> {
        self.0.native_identities()
    }
    fn import_secret(
        &self,
        reference: &str,
        secret: &rss_mdm_agent_wire::runtime::Secret,
    ) -> Result<(), Error> {
        self.import_bootstrap(reference, secret)
    }
    fn resolve(&self, reference: &str) -> Result<rss_mdm_agent_wire::runtime::Secret, Error> {
        let bytes = self.0.read(reference)?;
        rss_mdm_agent_wire::runtime::Secret::parse(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&*bytes),
        )
        .map_err(|_| Error::Identity)
    }
    fn create_secret(&self, reference: &str) -> Result<rss_mdm_agent_wire::runtime::Secret, Error> {
        let bytes = self.0.credential(reference)?;
        rss_mdm_agent_wire::runtime::Secret::parse(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&*bytes),
        )
        .map_err(|_| Error::Identity)
    }
    fn create_key(&self, reference: &str) -> Result<Arc<dyn rustls::sign::SigningKey>, Error> {
        self.0.signing_key(reference, true)
    }
    fn key(&self, reference: &str) -> Result<Arc<dyn rustls::sign::SigningKey>, Error> {
        self.0.signing_key(reference, false)
    }
}
/// Actual UTC plus an in-process monotonic watermark; durable watermarks remain in existing SQLite.
#[derive(Clone)]
pub struct SystemClock(Arc<Mutex<(u64, Instant)>>);
impl SystemClock {
    /// Capture the current OS clock; invalid time is an error, not a fixture fallback.
    pub fn new() -> Result<Self, Error> {
        Ok(Self(Arc::new(Mutex::new((wall()?, Instant::now())))))
    }
    pub(crate) fn millis(&self) -> Result<u64, Error> {
        let mut previous = self.0.lock().map_err(|_| Error::Clock)?;
        let now = wall()?;
        if now < previous.0
            || now.saturating_add(1000)
                < previous.0.saturating_add(
                    previous.1.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                )
        {
            return Err(Error::Clock);
        }
        *previous = (now, Instant::now());
        Ok(now)
    }
}
fn wall() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Clock)?
        .as_millis()
        .try_into()
        .map_err(|_| Error::Clock)
}
impl Clock for SystemClock {
    fn now(&self) -> Result<i64, Error> {
        (self.millis()? / 1000).try_into().map_err(|_| Error::Clock)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests;

#[cfg(test)]
#[path = "../../../tests/agent-protocol/mod.rs"]
mod protocol_test_support;

#[cfg(test)]
mod scenario_budget;

mod os_version;
mod secrets;

#[cfg(test)]
mod production_storage;

pub mod installation;

#[cfg(windows)]
pub mod access_component;
pub mod access_worker;
/// Fixed recovery credential worker entry.
pub mod credential_worker;

/// Read local performance diagnostics without publishing unconfigured remote telemetry.
pub fn telemetry_probe() -> Result<serde_json::Value, Error> {
    telemetry::performance::probe()
}

/// Root-only bounded Platform SSO lifecycle entry for installation tooling.
pub fn platform_sso_component(
    deployment: &deployment::Deployment,
    args: &[std::ffi::OsString],
) -> Result<(), Error> {
    service::platform_sso::component(deployment, args)
}
