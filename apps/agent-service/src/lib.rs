//! Backend-authorized execution through the existing admission and journal owners.
//! No local enterprise approver, source catalog or policy-signing authority is introduced.
mod backend;
pub mod deployment;
mod error;
mod material_gc;
mod recovery;
mod registration;
mod service;
pub mod software_worker;
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
        value: &agent_client::Secret,
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
    fn import_secret(&self, reference: &str, secret: &agent_client::Secret) -> Result<(), Error> {
        self.import_bootstrap(reference, secret)
    }
    fn resolve(&self, reference: &str) -> Result<agent_client::Secret, Error> {
        let bytes = self.0.read(reference)?;
        agent_client::Secret::parse(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&*bytes),
        )
        .map_err(|_| Error::Identity)
    }
    fn create_secret(&self, reference: &str) -> Result<agent_client::Secret, Error> {
        let bytes = self.0.credential(reference)?;
        agent_client::Secret::parse(
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
fn machine_store(store: agent_client::MachineStore) -> platform_credentials::CertificateStore {
    match store {
        agent_client::MachineStore::Root => platform_credentials::CertificateStore::Root,
        agent_client::MachineStore::Intermediate => {
            platform_credentials::CertificateStore::Intermediate
        }
    }
}
impl agent_client::MachineTrust for DeviceSecrets {
    fn read_certificate(
        &self,
        store: agent_client::MachineStore,
        der: &[u8],
    ) -> Result<bool, Error> {
        platform_credentials::MachineCertificates
            .read(machine_store(store), der)
            .map_err(|_| Error::Storage)
    }
    fn add_certificate(
        &self,
        store: agent_client::MachineStore,
        der: &[u8],
    ) -> Result<agent_client::CertificateAdd, Error> {
        platform_credentials::MachineCertificates
            .add(machine_store(store), der)
            .map(|created| {
                if created {
                    agent_client::CertificateAdd::Created
                } else {
                    agent_client::CertificateAdd::Present
                }
            })
            .map_err(|_| Error::Storage)
    }
    fn configure_certificate(
        &self,
        store: agent_client::MachineStore,
        der: &[u8],
    ) -> Result<(), Error> {
        platform_credentials::MachineCertificates
            .configure(machine_store(store), der)
            .map_err(|_| Error::Storage)
    }
    fn delete_certificate(
        &self,
        store: agent_client::MachineStore,
        der: &[u8],
    ) -> Result<(), Error> {
        platform_credentials::MachineCertificates
            .delete(machine_store(store), der)
            .map_err(|_| Error::Storage)
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

mod os_version;
mod secrets;

#[cfg(test)]
mod production_storage;

pub mod installation;
