use crate::{wire, Error};
use std::{collections::BTreeMap, time::Duration};
use url::Url;
use uuid::Uuid;
/// Explicit network choice, independent of execution authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// Certificate and hostname verified HTTPS.
    Https,
    /// Explicit loopback fixture only.
    TestLoopback,
}
/// Finite product-selected bounds; no production defaults.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Unconfirmed reports.
    pub pending_reports: usize,
    /// Task associations and pending events.
    pub pending_tasks: usize,
    /// Complete JSON response envelope.
    pub response_bytes: usize,
    /// Individual artifact bytes.
    pub artifact_bytes: u64,
    /// All partial and complete cache bytes.
    pub cache_bytes: u64,
    /// SQLite page ceiling.
    pub database_pages: u32,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Body idle timeout.
    pub read_timeout: Duration,
    /// Metadata request timeout.
    pub request_timeout: Duration,
    /// Streaming transfer timeout.
    pub transfer_timeout: Duration,
}
impl Limits {
    /// Small fixture bounds, not a capacity promise.
    pub fn test_defaults() -> Self {
        Self {
            pending_reports: 32,
            pending_tasks: 32,
            response_bytes: 8 * 1024 * 1024,
            artifact_bytes: 8 * 1024 * 1024,
            cache_bytes: 32 * 1024 * 1024,
            database_pages: 8192,
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(10),
            transfer_timeout: Duration::from_secs(30),
        }
    }
}
/// Independently trusted deployment context; network payloads cannot change it.
#[derive(Clone)]
pub struct Config {
    /// Fixed origin with no credential/query/fragment/path prefix.
    pub origin: Url,
    /// Trusted tenant, absent from BindingProfile.
    pub tenant: Uuid,
    /// Actual OS.
    pub platform: wire::TaskPlatform,
    /// Current locally observed execution context; never supplied by the remote server.
    pub execution_context: wire::SoftwareExecutionContext,
    /// Actual architecture.
    pub architecture: wire::TaskArchitecture,
    /// Trusted Ed25519 public keys.
    pub keys: BTreeMap<String, Vec<u8>>,
    /// Explicit budgets.
    pub limits: Limits,
    /// Network mode.
    pub transport: Transport,
    /// Protected HTTPS bootstrap anchor; required for HTTPS and independent of device trust.
    pub ca_pem: Option<Vec<u8>>,
}
impl Config {
    /// Validate before storage or I/O.
    pub fn validate(&self) -> Result<(), Error> {
        self.execution_context.validate_for(self.platform)?;
        let o = &self.origin;
        let l = self.limits;
        let network = match self.transport {
            Transport::Https => o.scheme() == "https",
            Transport::TestLoopback => {
                o.scheme() == "http"
                    && o.host_str()
                        .is_some_and(|h| matches!(h, "127.0.0.1" | "[::1]" | "::1"))
            }
        };
        if o.as_str().len() > 2048
            || !network
            || (self.transport == Transport::Https && self.ca_pem.is_none())
            || self.tenant.is_nil()
            || o.path() != "/"
            || !o.username().is_empty()
            || o.password().is_some()
            || o.query().is_some()
            || o.fragment().is_some()
            || self.keys.is_empty()
            || self.keys.len() > 32
            || self.keys.iter().any(|(id, k)| {
                id.is_empty()
                    || id.len() > 128
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    || k.len() != 32
            })
            || l.pending_reports == 0
            || l.pending_reports > 4096
            || l.pending_tasks == 0
            || l.pending_tasks > 4096
            || l.response_bytes == 0
            || l.response_bytes > 16 * 1024 * 1024
            || l.artifact_bytes == 0
            || l.artifact_bytes > 1_099_511_627_776
            || l.cache_bytes < l.artifact_bytes
            || l.cache_bytes > i64::MAX as u64
            || l.database_pages == 0
            || [
                l.connect_timeout,
                l.read_timeout,
                l.request_timeout,
                l.transfer_timeout,
            ]
            .iter()
            .any(|v| v.is_zero() || *v > Duration::from_secs(86400))
        {
            return Err(Error::Configuration);
        }
        if let Some(pem) = &self.ca_pem {
            crate::validate_public_ca(pem)?;
        }
        Ok(())
    }
}
/// Platform-owned bootstrap secrets and non-exportable device signing keys.
/// References are frozen by the communication owner before network operations.
pub trait IdentityProvider: MachineTrust {
    /// Resolve an existing protected bootstrap secret.
    fn resolve(&self, reference: &str) -> Result<crate::Secret, Error>;
    /// Import an administrator-provided secret once, without overwriting an existing value.
    fn import_secret(&self, reference: &str, secret: &crate::Secret) -> Result<(), Error>;
    /// Create once; exact retries must return the original protected secret.
    fn create_secret(&self, reference: &str) -> Result<crate::Secret, Error>;
    /// Create once during explicit enrollment preparation; never overwrite a key.
    fn create_key(
        &self,
        reference: &str,
    ) -> Result<std::sync::Arc<dyn rustls::sign::SigningKey>, Error>;
    /// Open the exact existing key. Missing/denied must not create a replacement.
    fn key(&self, reference: &str) -> Result<std::sync::Arc<dyn rustls::sign::SigningKey>, Error>;
}
/// Precise machine certificate location; intermediates must never become standalone anchors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MachineStore {
    /// Windows LocalMachine Root or macOS System Keychain with Admin root trust.
    Root,
    /// Windows LocalMachine CA or macOS System Keychain without standalone trust.
    Intermediate,
}
/// Outcome of an add-only native operation; an existing certificate remains externally owned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateAdd {
    /// This completed operation added the exact certificate.
    Created,
    /// Exact material was already present; ownership cannot be inferred.
    Present,
}
/// Machine trust adapter. Durable ownership and installation intent belong to the client owner.
pub trait MachineTrust {
    /// Read exact physical material in the selected machine store, independently of trust readiness.
    fn certificate_present(&self, store: MachineStore, der: &[u8]) -> Result<bool, Error>;
    /// Independently read exact DER and the required machine trust settings.
    fn read_certificate(&self, store: MachineStore, der: &[u8]) -> Result<bool, Error>;
    /// Add exact DER without replacing pre-existing material or settings.
    fn add_certificate(&self, store: MachineStore, der: &[u8]) -> Result<CertificateAdd, Error>;
    /// Configure trust only after the caller persists ownership of this exact material.
    fn configure_certificate(&self, store: MachineStore, der: &[u8]) -> Result<(), Error>;
    /// Delete this exact DER; caller must first prove its durable ownership and retirement.
    fn delete_certificate(&self, store: MachineStore, der: &[u8]) -> Result<(), Error>;
}
/// Independently reliable UTC.
pub trait Clock {
    /// Unix seconds; uncertainty must fail.
    fn now(&self) -> Result<i64, Error>;
}
/// Explicit initialization or recovery; no implicit replacement.
#[derive(Clone, Copy)]
pub enum OpenMode {
    /// New communication database in an existing private root.
    Create,
    /// Supported existing database.
    Existing,
}
