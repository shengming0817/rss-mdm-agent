use crate::{DeviceSecrets, DeviceService, ExecutionConfig, SystemClock};
use agent_client::{wire, Client, Config, Error, Limits, OpenMode, Transport};
use base64::Engine;
use execution_app::ProductionStartup;
use execution_runner::host::{current_subject, PeerPolicy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

/// Administrator-owned deployment data. It contains references and public trust pins, never secrets.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    pub version: u32,
    pub origin: String,
    pub tenant: uuid::Uuid,
    pub signing_keys: BTreeMap<String, String>,
    pub ca_file: Option<PathBuf>,
    pub enrollment: uuid::Uuid,
    pub registration_operation: uuid::Uuid,
    pub state_root: PathBuf,
    pub service: local_service::Artifact,
    pub clients: PeerPolicy,
    pub execution: ExecutionConfig,
    pub helper_work_roots: BTreeMap<String, PathBuf>,
}
impl Deployment {
    pub fn default_path() -> Result<PathBuf, Error> {
        let path = local_service::policy_path().map_err(|_| Error::Configuration)?;
        Ok(path
            .parent()
            .and_then(Path::parent)
            .ok_or(Error::Configuration)?
            .join("execution.json"))
    }
    pub fn load(path: &Path) -> Result<Self, Error> {
        let bytes = local_service::read_protected(path).map_err(|_| Error::Configuration)?;
        let value: Self = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        if value.version != 1
            || value.enrollment.is_nil()
            || value.registration_operation.is_nil()
            || !value.state_root.is_absolute()
            || !value.execution.material_root.is_absolute()
            || value.execution.material_root.starts_with(&value.state_root)
            || value.state_root.starts_with(&value.execution.material_root)
            || value.helper_work_roots.len() > 64
            || value.helper_work_roots.iter().any(|(subject, path)| {
                subject.is_empty() || subject == "0" || subject == "S-1-5-18" || !path.is_absolute()
            })
        {
            return Err(Error::Configuration);
        }
        value.clients.validate()?;
        value
            .service
            .verify(&value.service.path)
            .map_err(|_| Error::Configuration)?;
        value.network()?.validate()?;
        Ok(value)
    }
    pub fn server_policy(&self) -> PeerPolicy {
        PeerPolicy {
            images: vec![self.service.clone()],
            subjects: vec![if cfg!(windows) {
                "S-1-5-18".into()
            } else {
                "0".into()
            }],
            interactive: false,
        }
    }
    /// Construct only the current configured login's mechanism; this never opens device storage.
    pub fn user_helper(&self) -> Result<execution_runner::helper::Helper, Error> {
        self.service
            .verify(&std::env::current_exe()?)
            .map_err(|_| Error::Identity)?;
        let subject = current_subject()?;
        let root = self
            .helper_work_roots
            .get(&subject)
            .ok_or(Error::Identity)?
            .clone();
        Ok(execution_runner::helper::Helper::new(
            self.server_policy(),
            root,
            self.execution.processes,
            crate::plan::storage_limits().input,
        )?)
    }
    fn require_service(&self) -> Result<(), Error> {
        if !self.server_policy().subjects.contains(&current_subject()?) {
            return Err(Error::Identity);
        }
        self.service
            .verify(&std::env::current_exe()?)
            .map_err(|_| Error::Identity)
    }
    pub fn network(&self) -> Result<Config, Error> {
        let platform = if cfg!(target_os = "macos") {
            wire::TaskPlatform::Macos
        } else if cfg!(windows) {
            wire::TaskPlatform::Windows
        } else {
            return Err(Error::Unsupported);
        };
        let architecture = if cfg!(target_arch = "aarch64") {
            wire::TaskArchitecture::Aarch64
        } else if cfg!(target_arch = "x86_64") {
            wire::TaskArchitecture::X86_64
        } else {
            return Err(Error::Unsupported);
        };
        let keys = self
            .signing_keys
            .iter()
            .map(|(k, v)| {
                Ok((
                    k.clone(),
                    base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .decode(v)
                        .map_err(|_| Error::Configuration)?,
                ))
            })
            .collect::<Result<_, Error>>()?;
        Ok(Config {
            origin: url::Url::parse(&self.origin).map_err(|_| Error::Configuration)?,
            tenant: self.tenant,
            platform,
            architecture,
            keys,
            transport: Transport::Https,
            ca_pem: self
                .ca_file
                .as_ref()
                .map(|p| local_service::read_protected(p).map_err(|_| Error::Configuration))
                .transpose()?,
            limits: Limits {
                pending_reports: 128,
                pending_tasks: 128,
                response_bytes: 8 * 1024 * 1024,
                artifact_bytes: 8 * 1024 * 1024 * 1024,
                cache_bytes: 32 * 1024 * 1024 * 1024,
                database_pages: 262144,
                connect_timeout: Duration::from_secs(1),
                read_timeout: Duration::from_secs(5),
                request_timeout: Duration::from_secs(2),
                transfer_timeout: Duration::from_secs(1800),
            },
        })
    }
    fn namespace(&self) -> Result<String, Error> {
        Ok(format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    &self.origin,
                    self.tenant,
                    self.enrollment,
                    self.registration_operation
                ))
                .map_err(|_| Error::Configuration)?
            )
        ))
    }
    /// Explicit, idempotent registration initialization. Existing databases are only opened, never reset.
    pub async fn initialize(&self, password: &wire::Secret) -> Result<(), Error> {
        self.require_service()?;
        let namespace = self.namespace()?;
        let marker = self.state_root.join("identity-binding");
        if self.state_root.exists()
            && !marker.exists()
            && std::fs::read_dir(&self.state_root)?.next().is_some()
        {
            return Err(Error::Storage);
        }
        local_service::protected(self.state_root.parent().ok_or(Error::Configuration)?)
            .map_err(|_| Error::Storage)?;
        native_process::private_storage::directory(&self.state_root)?;
        local_service::protected(&self.state_root).map_err(|_| Error::Storage)?;
        if marker.exists() {
            if native_process::private_storage::read(&marker, 128)? != namespace.as_bytes() {
                return Err(Error::Identity);
            }
        } else {
            native_process::private_storage::write_new(&marker, namespace.as_bytes())?;
        }
        let communication = self.state_root.join("communication");
        let journal = self.state_root.join("execution.sqlite");
        let initialized = self.state_root.join("execution-initialized");
        if (journal.exists() && !communication.join("communication.sqlite").exists())
            || (initialized.exists() && !journal.exists())
        {
            return Err(Error::Storage);
        }
        let secrets = self.state_root.join("secrets");
        for path in [&communication, &secrets, &self.execution.work_root] {
            native_process::private_storage::directory(path)?;
        }
        execution_runner::staging::initialize(&self.execution.material_root)?;
        let secrets = DeviceSecrets::open(&secrets, &namespace)?;
        secrets.bind_storage(&self.state_root)?;
        secrets.import_enrollment("enrollment", password)?;
        let mode = if communication.join("communication.sqlite").exists() {
            OpenMode::Existing
        } else {
            OpenMode::Create
        };
        let clock = SystemClock::new()?;
        let mut client = Client::open(
            &communication,
            self.network()?,
            mode,
            secrets,
            clock.clone(),
        )?;
        if client.registration().is_err() {
            if journal.exists() || initialized.exists() {
                return Err(Error::Identity);
            }
            client
                .register(
                    self.registration_operation,
                    self.enrollment,
                    "enrollment",
                    "device",
                    vec![
                        wire::Capability::InventoryCollectionV5,
                        wire::Capability::TaskExecuteV5,
                        wire::Capability::SoftwareExecuteV5,
                    ],
                )
                .await?;
        }
        let mode = if journal.exists() {
            ProductionStartup::Open
        } else {
            ProductionStartup::Create
        };
        let service = DeviceService::open(
            client,
            &journal,
            mode,
            self.execution.clone(),
            clock,
            self.helpers(),
        )?;
        drop(service);
        if !initialized.exists() {
            native_process::private_storage::write_new(&initialized, namespace.as_bytes())?;
        } else if native_process::private_storage::read(&initialized, 128)? != namespace.as_bytes()
        {
            return Err(Error::Identity);
        }
        Ok(())
    }
    pub fn open(&self) -> Result<DeviceService, Error> {
        self.require_service()?;
        let namespace = self.namespace()?;
        local_service::protected(&self.state_root).map_err(|_| Error::Storage)?;
        if native_process::private_storage::read(&self.state_root.join("identity-binding"), 128)?
            != namespace.as_bytes()
        {
            return Err(Error::Identity);
        }
        let secrets = DeviceSecrets::open(&self.state_root.join("secrets"), &namespace)?;
        secrets.verify_storage(&self.state_root)?;
        let clock = SystemClock::new()?;
        let client = Client::open(
            &self.state_root.join("communication"),
            self.network()?,
            OpenMode::Existing,
            secrets,
            clock.clone(),
        )?;
        DeviceService::open(
            client,
            &self.state_root.join("execution.sqlite"),
            ProductionStartup::Open,
            self.execution.clone(),
            clock,
            self.helpers(),
        )
    }
    fn helpers(&self) -> crate::UserResources {
        crate::UserResources {
            image: self.service.clone(),
            work_roots: self.helper_work_roots.clone(),
        }
    }
}
