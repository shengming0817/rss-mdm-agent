use crate::ProductionStartup;
use crate::{DeviceSecrets, DeviceService, ExecutionConfig, SystemClock};
use agent_client::{Client, Config, Error, Limits, OpenMode, Transport};
use base64::Engine;
use execution_ipc::host::{current_subject, PeerPolicy};
use rss_mdm_agent_wire::runtime as wire;
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
    pub installation_id: uuid::Uuid,
    pub device_ca_file: PathBuf,
    pub registration_clients: PeerPolicy,
    pub state_root: PathBuf,
    pub service: installation_security::Artifact,
    pub clients: PeerPolicy,
    pub execution: ExecutionConfig,
    pub helper_work_roots: BTreeMap<String, PathBuf>,
}
impl Deployment {
    /// Assemble the machine owner. Registration is driven only by explicit IPC or retained operations.
    pub fn assemble(self) -> Result<Box<dyn execution_ipc::host::Handler>, Error> {
        self.require_service()?;
        match self.preflight() {
            Ok(()) => Ok(Box::new(crate::registration::RegistrationOwner::new(self)?)),
            Err(error) => {
                execution_ipc::record_startup_failure(startup_failure(error));
                Ok(Box::new(Diagnostic {
                    policy: self.clients,
                    readiness: execution_ipc::host::Readiness::NotReady,
                }))
            }
        }
    }
    pub fn default_path() -> Result<PathBuf, Error> {
        installation_security::deployment_path().map_err(|_| Error::Configuration)
    }
    pub fn load(path: &Path) -> Result<Self, Error> {
        let bytes =
            installation_security::read_protected(path).map_err(|_| Error::Configuration)?;
        let value: Self = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        if value.version != execution_ipc::host::DEPLOYMENT_VERSION
            || value.ipc_version != execution_ipc::host::IPC_VERSION
            || value.installation_id.is_nil()
            || !value.device_ca_file.is_absolute()
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
        value
            .registration_clients
            .validate()
            .map_err(crate::error::app_error)?;
        if value.registration_clients.images.len() != 1 || !value.registration_clients.interactive {
            return Err(Error::Configuration);
        }
        value.clients.validate().map_err(crate::error::app_error)?;
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
        let subject = current_subject().map_err(crate::error::app_error)?;
        let root = self
            .helper_work_roots
            .get(&subject)
            .ok_or(Error::Identity)?
            .clone();
        execution_runner::helper::Helper::new(
            self.server_policy(),
            root,
            self.execution.processes,
            crate::backend::plan::storage_limits().input,
        )
        .map_err(crate::error::app_error)
    }
    fn require_service(&self) -> Result<(), Error> {
        if !self
            .server_policy()
            .subjects
            .contains(&current_subject().map_err(crate::error::app_error)?)
        {
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
                os_version: crate::os_version::current()?,
                system_broker: true,
                interactive_user: None,
                source_credentials: vec![],
                msix_sideload: crate::software_worker::sideload_allowed()?,
                msix_unsigned: cfg!(windows) && crate::os_version::current()? >= [10, 0, 19041, 0],
            },
            origin: url::Url::parse(&self.origin).map_err(|_| Error::Configuration)?,
            tenant: self.tenant,
            platform,
            architecture,
            keys,
            transport: Transport::Https,
            device_ca_pem: installation_security::read_protected(&self.device_ca_file)
                .map_err(|_| Error::Configuration)?,
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
                serde_json::to_vec(&(&self.origin, self.tenant, self.installation_id))
                    .map_err(|_| Error::Configuration)?
            )
        ))
    }
    /// Check every existing database before any secret, directory, lock or material initialization.
    pub(crate) fn preflight(&self) -> Result<(), Error> {
        let network = self.network()?;
        let root = &self.state_root;
        if never_initialized(root)? {
            return Ok(());
        }
        if platform_private_storage::read(&root.join("identity-binding"), 128)?
            != self.namespace()?.as_bytes()
        {
            return Err(Error::Identity);
        }
        agent_client::validate_existing(&root.join("communication"), &network)?;
        if root.join("execution-initialized").exists() && !root.join("execution.sqlite").exists() {
            return Err(Error::Storage);
        }
        if root.join("execution.sqlite").exists() {
            let binding = agent_client::inspect_binding(&root.join("communication"), &network)?;
            let (context, _) =
                crate::backend::plan::context(network.origin.as_str(), network.tenant, &binding)?;
            execution_sqlite::Store::validate_existing(
                &root.join("execution.sqlite"),
                &context.authority,
                crate::backend::plan::storage_limits(),
            )
            .map_err(|_| Error::Schema)?;
        }
        Ok(())
    }
    pub(crate) fn communication(
        &self,
        create: bool,
    ) -> Result<Client<DeviceSecrets, SystemClock>, Error> {
        self.require_service()?;
        self.preflight()?;
        let namespace = self.namespace()?;
        let root = &self.state_root;
        let absent = never_initialized(root)?;
        if absent && !create {
            return Err(Error::Identity);
        }
        if absent {
            platform_private_storage::directory(root)?;
            installation_security::protected(root).map_err(|_| Error::Storage)?;
            platform_private_storage::write_new(
                &root.join("identity-binding"),
                namespace.as_bytes(),
            )?;
            for path in [root.join("communication"), root.join("secrets")] {
                platform_private_storage::directory(&path)?;
            }
        }
        let secrets = DeviceSecrets::open(&root.join("secrets"), &namespace)?;
        if absent {
            secrets.bind_storage(root)?;
        } else {
            secrets.verify_storage(root)?;
        }
        Client::open(
            &root.join("communication"),
            self.network()?,
            if absent {
                OpenMode::Create
            } else {
                OpenMode::Existing
            },
            secrets,
            SystemClock::new()?,
        )
    }
    /// Import protected grant metadata and its secret, then drive the original certificate lifecycle.
    pub async fn initialize(
        &self,
        grant: agent_client::GrantImport,
        secret: &wire::Secret,
    ) -> Result<(), Error> {
        let mut client = self.communication(true)?;
        let reference = format!("bootstrap-{}", grant.operation_id);
        client.import_bootstrap_secret(&reference, secret)?;
        client.import_grant(grant, &reference, self.capabilities())?;
        client.drive_enrollment().await?;
        self.device_service(client).map(drop)
    }
    /// Manual same-registration renewal; the exclusive owner lease requires the daemon to be stopped.
    pub async fn renew(&self) -> Result<(), Error> {
        let mut client = self.communication(false)?;
        client.renew_certificate().await?;
        Ok(())
    }
    pub(crate) fn activate(
        &self,
        client: Client<DeviceSecrets, SystemClock>,
    ) -> Result<crate::service::ServiceHandle, Error> {
        self.device_service(client)?
            .spawn(self.clients.clone(), self.helper_policy())
    }
    fn device_service(
        &self,
        client: Client<DeviceSecrets, SystemClock>,
    ) -> Result<DeviceService, Error> {
        platform_private_storage::directory(&self.execution.work_root)?;
        execution_runner::staging::initialize(&self.execution.material_root)
            .map_err(crate::error::app_error)?;
        let journal = self.state_root.join("execution.sqlite");
        let service = DeviceService::open(
            client,
            &journal,
            if journal.exists() {
                ProductionStartup::Open
            } else {
                ProductionStartup::Create
            },
            self.execution.clone(),
            SystemClock::new()?,
            self.helpers(),
        )?;
        let marker = self.state_root.join("execution-initialized");
        if !marker.exists() {
            platform_private_storage::write_new(&marker, self.namespace()?.as_bytes())?;
        }
        Ok(service)
    }
    pub(crate) fn helper_policy(&self) -> Option<PeerPolicy> {
        if self.helper_work_roots.is_empty() {
            None
        } else {
            Some(PeerPolicy {
                images: vec![self.service.clone()],
                subjects: self.helper_work_roots.keys().cloned().collect(),
                interactive: true,
            })
        }
    }
    /// Read-only refresh preflight while the current service remains running.
    /// No identity initialization, journal recovery or secret import is allowed here.
    pub fn validate_persistent(&self) -> Result<(), Error> {
        self.require_service()?;
        self.preflight()?;
        let namespace = self.namespace()?;
        installation_security::protected(&self.state_root).map_err(|_| Error::Storage)?;
        validate_markers(&self.state_root, &namespace)?;
        let secrets = DeviceSecrets::open(&self.state_root.join("secrets"), &namespace)?;
        secrets.verify_storage(&self.state_root)?;
        let network = self.network()?;
        let registration = agent_client::validate_identity(
            &self.state_root.join("communication"),
            &network,
            &secrets,
            &SystemClock::new()?,
        )?;
        let (binding, _) =
            crate::backend::plan::context(network.origin.as_str(), network.tenant, &registration)?;
        execution_sqlite::Store::validate_existing(
            &self.state_root.join("execution.sqlite"),
            &binding.authority,
            crate::backend::plan::storage_limits(),
        )
        .map_err(|_| Error::Storage)?;
        platform_private_storage::validate(&self.execution.work_root)?;
        Ok(())
    }
    pub fn open(&self) -> Result<DeviceService, Error> {
        self.require_service()?;
        self.preflight()?;
        let namespace = self.namespace()?;
        installation_security::protected(&self.state_root).map_err(|_| Error::Storage)?;
        if platform_private_storage::read(&self.state_root.join("identity-binding"), 128)?
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
    pub(crate) fn capabilities(&self) -> Vec<wire::Capability> {
        use crate::SoftwareManagerKind as M;
        use wire::Capability as C;
        let mut values = vec![C::InventoryCollectionV1, C::TaskExecuteV1];
        let has = |profile| {
            self.execution
                .interpreters
                .iter()
                .any(|i| i.profile == profile)
        };
        for manager in &self.execution.managers {
            match manager.executor {
                M::Msi if cfg!(windows) && has(wire::ExecutorProfile::PowerShell7) => {
                    values.extend([C::SoftwareMsiSystemV1, C::SoftwareMsiUserV1])
                }
                M::PackageInstaller
                    if cfg!(target_os = "macos") && has(wire::ExecutorProfile::PosixSh) =>
                {
                    values.push(C::SoftwarePkgSystemV1)
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
                    C::SoftwareBundleWindowsSystemV1,
                    C::SoftwareBundleWindowsUserV1,
                ]
            } else {
                [C::SoftwareBundleMacosSystemV1, C::SoftwareBundleMacosUserV1]
            });
        }
        if cfg!(windows) && has(wire::ExecutorProfile::PowerShell7) {
            values.extend([
                C::SoftwareExeSystemV1,
                C::SoftwareExeUserV1,
                C::SoftwareMsixRegistrationUserV1,
                C::SoftwareMsixProvisioningSystemV1,
            ]);
        }
        if cfg!(target_os = "macos")
            && has(wire::ExecutorProfile::PosixSh)
            && [
                "/usr/bin/hdiutil",
                "/usr/sbin/diskutil",
                "/usr/bin/codesign",
                "/usr/sbin/spctl",
                "/usr/bin/lipo",
            ]
            .iter()
            .all(|p| installation_security::protected(std::path::Path::new(p)).is_ok())
        {
            values.extend([C::SoftwareDmgAppSystemV1, C::SoftwareDmgAppUserV1]);
            if self
                .execution
                .managers
                .iter()
                .any(|m| m.executor == M::PackageInstaller)
            {
                values.push(C::SoftwareDmgPkgSystemV1);
            }
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

pub(crate) fn startup_failure(error: Error) -> execution_contract::ProcessFailureKind {
    use execution_contract::ProcessFailureKind as K;
    match error {
        Error::Identity
        | Error::CertificateExpired
        | Error::CredentialRevoked
        | Error::CredentialRetired => K::Unbound,
        Error::Denied | Error::Untrusted => K::Denied,
        Error::Protocol | Error::Configuration => K::InvalidInput,
        Error::Schema | Error::Unsupported => K::Unsupported,
        Error::Capacity => K::Capacity,
        Error::Conflict => K::Conflict,
        Error::Storage
        | Error::Unavailable
        | Error::Clock
        | Error::Expired
        | Error::IssuanceUnknown
        | Error::ActivationUnknown => K::Unavailable,
    }
}

fn validate_markers(root: &Path, namespace: &str) -> Result<(), Error> {
    for name in ["identity-binding", "execution-initialized"] {
        if platform_private_storage::read(&root.join(name), 128)? != namespace.as_bytes() {
            return Err(Error::Identity);
        }
    }
    Ok(())
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
    readiness: execution_ipc::host::Readiness,
}
impl execution_ipc::host::Handler for Diagnostic {
    fn peer_policy(&self) -> Option<PeerPolicy> {
        Some(self.policy.clone())
    }
    fn handle(
        &mut self,
        peer: &execution_ipc::host::Peer,
        request: execution_ipc::host::Request,
    ) -> execution_ipc::host::Reply {
        use execution_ipc::host::{Reply, Request, ServiceStatus};
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
    fn startup_failure_mapping_is_closed_and_retains_identity_schema_storage_distinctions() {
        use execution_contract::ProcessFailureKind as K;
        assert_eq!(startup_failure(Error::Identity), K::Unbound);
        assert_eq!(startup_failure(Error::Schema), K::Unsupported);
        assert_eq!(startup_failure(Error::Storage), K::Unavailable);
        assert_eq!(startup_failure(Error::Configuration), K::InvalidInput);
        assert_eq!(startup_failure(Error::Untrusted), K::Denied);
    }
    #[test]
    fn refresh_requires_both_original_markers_without_initializing_missing_state() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("refresh-markers-{}", uuid::Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        assert!(validate_markers(&root, "original").is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        platform_private_storage::write_new(&root.join("identity-binding"), b"original").unwrap();
        assert!(validate_markers(&root, "original").is_err());
        platform_private_storage::write_new(&root.join("execution-initialized"), b"original")
            .unwrap();
        validate_markers(&root, "original").unwrap();
        assert!(matches!(
            validate_markers(&root, "other"),
            Err(Error::Identity)
        ));
        assert_eq!(
            platform_private_storage::read(&root.join("identity-binding"), 128).unwrap(),
            b"original"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn old_communication_format_blocks_deployment_preflight_before_local_preparation() {
        use base64::Engine;
        let root = crate::protocol_test_support::Root::new();
        let mut deployment: Deployment = serde_json::from_value(serde_json::json!({
            "version": execution_ipc::host::DEPLOYMENT_VERSION,
            "ipc_version": execution_ipc::host::IPC_VERSION,
            "origin":"https://localhost:443/", "tenant": uuid::Uuid::new_v4(),
            "installation_id":uuid::Uuid::new_v4(),
            "signing_keys":{"test":base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([1u8;32])},
            // Bounded root-protected public bytes. Schema rejection precedes CA parsing;
            // this preflight test does not assert PKI acceptance or create native keys.
            "device_ca_file":"/private/etc/hosts", "ca_file":null,
            "state_root":root.path,
            "service":{"path":"/unused","sha256":"a".repeat(64),"cdhash":null},
            "clients":{"images":[],"subjects":[],"interactive":false},
            "registration_clients":{"images":[],"subjects":[],"interactive":true},
            "helper_work_roots":{},
            "execution":{"work_root":root.path.join("work"), "material_root":root.path.join("materials"),
                "interpreters":[],"managers":[],"processes":1}
        })).unwrap();
        // Preflight precedes protected credential storage for both initialization and startup.
        platform_private_storage::write_new(
            &root.path.join("identity-binding"),
            deployment.namespace().unwrap().as_bytes(),
        )
        .unwrap();
        let communication = root.path.join("communication");
        platform_private_storage::directory(&communication).unwrap();
        let db = communication.join("communication.sqlite");
        drop(platform_private_storage::create_new(&db).unwrap());
        let connection = rusqlite::Connection::open(&db).unwrap();
        connection
            .execute_batch("PRAGMA application_id=1380008771; PRAGMA user_version=4;")
            .unwrap();
        drop(connection);
        let before = std::fs::read(&db).unwrap();
        assert_eq!(deployment.preflight(), Err(Error::Schema));
        assert_eq!(std::fs::read(&db).unwrap(), before);
        assert!(!communication.join("client.lock").exists());
        assert!(!root.path.join("secrets").exists());
        assert!(!deployment.execution.work_root.exists());
        assert!(!deployment.execution.material_root.exists());
        // A different installation is rejected before it can interpret this old store.
        deployment.installation_id = uuid::Uuid::new_v4();
        assert_eq!(deployment.preflight(), Err(Error::Identity));
        assert_eq!(std::fs::read(&db).unwrap(), before);
    }
    #[test]
    fn review_regression_native_capability_requires_its_interpreter() {
        let image = installation_security::Artifact {
            path: "/unused".into(),
            sha256: "a".repeat(64),
            cdhash: None,
        };
        let mut deployment = Deployment {
            version: execution_ipc::host::DEPLOYMENT_VERSION,
            ipc_version: execution_ipc::host::IPC_VERSION,
            origin: String::new(),
            tenant: uuid::Uuid::new_v4(),
            signing_keys: Default::default(),
            ca_file: None,
            installation_id: uuid::Uuid::new_v4(),
            device_ca_file: PathBuf::new(),
            registration_clients: PeerPolicy {
                images: vec![image.clone()],
                subjects: vec![],
                interactive: true,
            },
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
            wire::Capability::SoftwareMsiSystemV1
        } else {
            wire::Capability::SoftwarePkgSystemV1
        }));
    }
}
