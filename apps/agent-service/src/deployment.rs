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
    pub ipc_version: u8,
    pub origin: String,
    pub tenant: uuid::Uuid,
    pub signing_keys: BTreeMap<String, String>,
    pub ca_file: Option<PathBuf>,
    pub enrollment: uuid::Uuid,
    pub registration_operation: uuid::Uuid,
    pub state_root: PathBuf,
    pub service: installation_security::Artifact,
    pub clients: PeerPolicy,
    pub execution: ExecutionConfig,
    pub helper_work_roots: BTreeMap<String, PathBuf>,
}
impl Deployment {
    /// Assemble the one authenticated listener without creating device identity on startup.
    pub fn assemble(self) -> Result<Box<dyn execution_runner::host::Handler>, Error> {
        use execution_runner::host::Readiness;
        self.require_service()?;
        let never_initialized = match never_initialized(&self.state_root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("agent_service storage not ready: {error}");
                return Ok(Box::new(Diagnostic {
                    policy: self.clients,
                    readiness: Readiness::NotReady,
                }));
            }
        };
        let readiness = if never_initialized {
            Readiness::RegistrationRequired
        } else {
            match self.open() {
                Ok(service) => {
                    let helper_policy = if self.helper_work_roots.is_empty() {
                        None
                    } else {
                        Some(PeerPolicy {
                            images: vec![self.service.clone()],
                            subjects: self.helper_work_roots.keys().cloned().collect(),
                            interactive: true,
                        })
                    };
                    return Ok(Box::new(service.spawn(self.clients, helper_policy)?));
                }
                Err(error) => {
                    eprintln!("agent_service startup not ready: {error}");
                    Readiness::NotReady
                }
            }
        };
        Ok(Box::new(Diagnostic {
            policy: self.clients,
            readiness,
        }))
    }
    pub fn default_path() -> Result<PathBuf, Error> {
        installation_security::deployment_path().map_err(|_| Error::Configuration)
    }
    pub fn load(path: &Path) -> Result<Self, Error> {
        let bytes =
            installation_security::read_protected(path).map_err(|_| Error::Configuration)?;
        let value: Self = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        if value.version != execution_runner::host::DEPLOYMENT_VERSION
            || value.ipc_version != execution_runner::host::IPC_VERSION
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
            execution_context: wire::SoftwareExecutionContext {
                revision: 1,
                os_version: native_process::os_version::current()?,
                system_broker: true,
                interactive_user: None,
                source_credentials: vec![],
                msix_sideload: false,
                msix_unsigned: false,
            },
            origin: url::Url::parse(&self.origin).map_err(|_| Error::Configuration)?,
            tenant: self.tenant,
            platform,
            architecture,
            keys,
            transport: Transport::Https,
            ca_pem: self
                .ca_file
                .as_ref()
                .map(|p| installation_security::read_protected(p).map_err(|_| Error::Configuration))
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
        installation_security::protected(self.state_root.parent().ok_or(Error::Configuration)?)
            .map_err(|_| Error::Storage)?;
        native_process::private_storage::directory(&self.state_root)?;
        installation_security::protected(&self.state_root).map_err(|_| Error::Storage)?;
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
                    self.capabilities(),
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
        installation_security::protected(&self.state_root).map_err(|_| Error::Storage)?;
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
    fn capabilities(&self) -> Vec<wire::Capability> {
        use crate::SoftwareManagerKind as M;
        use wire::Capability as C;
        let mut values = vec![C::InventoryCollectionV5, C::TaskExecuteV5];
        let has = |profile| {
            self.execution
                .interpreters
                .iter()
                .any(|i| i.profile == profile)
        };
        for manager in &self.execution.managers {
            match manager.executor {
                M::Msi if cfg!(windows) && has(wire::ExecutorProfile::PowerShell7) => {
                    values.extend([C::SoftwareMsiSystemV5, C::SoftwareMsiUserV5])
                }
                M::PackageInstaller
                    if cfg!(target_os = "macos") && has(wire::ExecutorProfile::PosixSh) =>
                {
                    values.push(C::SoftwarePkgSystemV5)
                }

                _ => (),
            }
        }
        let profile = if cfg!(windows) {
            wire::ExecutorProfile::PowerShell7
        } else {
            wire::ExecutorProfile::PosixSh
        };
        if self
            .execution
            .interpreters
            .iter()
            .any(|i| i.profile == profile)
        {
            values.extend(if cfg!(windows) {
                [
                    C::SoftwareBundleWindowsSystemV5,
                    C::SoftwareBundleWindowsUserV5,
                ]
            } else {
                [C::SoftwareBundleMacosSystemV5, C::SoftwareBundleMacosUserV5]
            });
        }
        values.sort();
        values.dedup();
        values
    }
    fn helpers(&self) -> crate::UserResources {
        crate::UserResources {
            image: self.service.clone(),
            work_roots: self.helper_work_roots.clone(),
        }
    }
}

fn never_initialized(root: &Path) -> Result<bool, Error> {
    match std::fs::symlink_metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            installation_security::protected(root.parent().ok_or(Error::Configuration)?)
                .map_err(|_| Error::Storage)?;
            Ok(true)
        }
        Err(_) => Err(Error::Storage),
        Ok(_) => {
            installation_security::protected(root).map_err(|_| Error::Storage)?;
            Ok(std::fs::read_dir(root)?.next().is_none())
        }
    }
}

struct Diagnostic {
    policy: PeerPolicy,
    readiness: execution_runner::host::Readiness,
}
impl execution_runner::host::Handler for Diagnostic {
    fn peer_policy(&self) -> Option<PeerPolicy> {
        Some(self.policy.clone())
    }
    fn handle(
        &mut self,
        peer: &execution_runner::host::Peer,
        request: execution_runner::host::Request,
    ) -> execution_runner::host::Reply {
        use execution_runner::host::{Reply, Request, ServiceStatus};
        if peer.authenticate(&self.policy).is_err() {
            return Reply::Rejected;
        }
        match request {
            Request::ServiceStatus {} => Reply::ServiceStatus {
                value: ServiceStatus::new(self.readiness.clone()),
            },
            _ => Reply::Rejected,
        }
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        Ok(())
    }
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_regression_native_capability_requires_its_interpreter() {
        let image = installation_security::Artifact {
            path: "/unused".into(),
            sha256: "a".repeat(64),
            cdhash: None,
        };
        let mut deployment = Deployment {
            version: execution_runner::host::DEPLOYMENT_VERSION,
            ipc_version: execution_runner::host::IPC_VERSION,
            origin: String::new(),
            tenant: uuid::Uuid::new_v4(),
            signing_keys: Default::default(),
            ca_file: None,
            enrollment: uuid::Uuid::new_v4(),
            registration_operation: uuid::Uuid::new_v4(),
            state_root: PathBuf::new(),
            service: image.clone(),
            clients: PeerPolicy {
                images: vec![],
                subjects: vec![],
                interactive: false,
            },
            helper_work_roots: Default::default(),
            execution: ExecutionConfig {
                work_root: PathBuf::new(),
                material_root: PathBuf::new(),
                processes: 1,
                interpreters: vec![crate::Interpreter {
                    profile: wire::ExecutorProfile::Osquery,
                    image: image.clone(),
                }],
                managers: vec![crate::SoftwareManager {
                    executor: if cfg!(windows) {
                        crate::SoftwareManagerKind::Msi
                    } else {
                        crate::SoftwareManagerKind::PackageInstaller
                    },
                    image: image.clone(),
                }],
            },
        };
        assert!(!deployment.capabilities().iter().any(|v| v.is_software()));
        deployment.execution.interpreters.push(crate::Interpreter {
            profile: if cfg!(windows) {
                wire::ExecutorProfile::PowerShell7
            } else {
                wire::ExecutorProfile::PosixSh
            },
            image,
        });
        assert!(deployment.capabilities().contains(&if cfg!(windows) {
            wire::Capability::SoftwareMsiSystemV5
        } else {
            wire::Capability::SoftwarePkgSystemV5
        }));
    }
}
