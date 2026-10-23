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
    pub software_read_grants: Vec<agent_client::SoftwareReadGrant>,
    pub version: u32,
    pub ipc_version: u8,
    pub service_id: uuid::Uuid,
    pub connections: rss_mdm_agent_wire::connections::SignedConfiguration,
    pub tenant: uuid::Uuid,
    pub signing_keys: BTreeMap<String, String>,
    pub ca_file: Option<PathBuf>,
    pub installation_id: uuid::Uuid,
    /// SHA256 of the fixed, public installation candidate manifest.
    pub installation_artifact: String,
    pub registration_clients: PeerPolicy,
    pub state_root: PathBuf,
    pub service: installation_security::Artifact,
    pub clients: PeerPolicy,
    pub execution: ExecutionConfig,
    pub helper_work_roots: BTreeMap<String, PathBuf>,
}
impl Deployment {
    /// Provision a source-scoped token into the existing OS namespace, without overwriting it.
    /// The communication lease requires the daemon to be stopped; no registration is created.
    pub fn import_software_credential(&self, target: &str, token: &str) -> Result<(), Error> {
        self.require_service()?;
        agent_client::native_source::validate_read_token(token)?;
        let grant = self
            .software_read_grants
            .iter()
            .find(|g| g.credential.source == target)
            .ok_or(Error::Configuration)?;
        let _communication = self.communication(false)?;
        let store = DeviceSecrets::open(&self.state_root.join("secrets"), &self.namespace()?)?;
        store.verify_storage(&self.state_root)?;
        store.0.import(
            &agent_client::native_source::credential_key(&grant.credential)?,
            token.as_bytes(),
        )?;
        Ok(())
    }
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
    pub(crate) fn metadata(path: &Path) -> Result<Self, Error> {
        let bytes =
            installation_security::read_protected(path).map_err(|_| Error::Configuration)?;
        Self::decode_metadata(&bytes)
    }
    fn decode_metadata(bytes: &[u8]) -> Result<Self, Error> {
        let value: Self = serde_json::from_slice(bytes).map_err(|_| Error::Configuration)?;
        if value.version != execution_ipc::host::DEPLOYMENT_VERSION
            || value.installation_id.is_nil()
            || !crate::installation::digest_valid(&value.installation_artifact)
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
        Ok(value)
    }
    pub fn load(path: &Path) -> Result<Self, Error> {
        let value = Self::metadata(path)?;
        if value.ipc_version != execution_ipc::host::IPC_VERSION {
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
        for image in std::iter::once(&value.service)
            .chain(value.clients.images.iter())
            .chain(value.registration_clients.images.iter())
        {
            crate::installation::validate_native_artifact(image)?;
        }
        value.network()?.validate()?;
        Ok(value)
    }
    /// Retained state validation must survive removal of an old desktop payload.
    pub fn load_persistent(path: &Path) -> Result<Self, Error> {
        let value = Self::metadata(path)?;
        value
            .service
            .verify(&value.service.path)
            .map_err(|_| Error::Identity)?;
        crate::installation::validate_native_artifact(&value.service)?;
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
        let architecture =
            match installation_security::Target::current().map_err(|_| Error::Unsupported)? {
                installation_security::Target::WindowsAmd64 => wire::TaskArchitecture::X86_64,
                installation_security::Target::WindowsArm64
                | installation_security::Target::MacosArm64 => wire::TaskArchitecture::Aarch64,
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
            service_id: self.service_id,
            connections: self.connections.clone(),
            software_read_grants: self.software_read_grants.clone(),
            execution_context: wire::SoftwareExecutionContext {
                revision: 1,
                os_version: crate::os_version::current()?,
                system_broker: true,
                user_helper: true,
                interactive_user: None,
                source_credentials: self
                    .software_read_grants
                    .iter()
                    .map(|g| g.credential.clone())
                    .collect(),
                msix_sideload: crate::software_worker::sideload_allowed()?,
                msix_unsigned: cfg!(windows) && crate::os_version::current()? >= [10, 0, 19041, 0],
            },
            origin: url::Url::parse(&self.connections.configuration.preferred_origin)
                .map_err(|_| Error::Configuration)?,
            tenant: self.tenant,
            platform,
            architecture,
            registration_keys: keys,
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
    pub(crate) fn namespace(&self) -> Result<String, Error> {
        Ok(format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(self.service_id, self.tenant, self.installation_id))
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
                crate::backend::plan::context(network.service_id, network.tenant, &binding)?;
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
    /// Explicit offline administrator recovery; the communication lease excludes a running daemon.
    pub async fn recover(
        &self,
        grant: agent_client::GrantImport,
        secret: &rss_mdm_agent_wire::runtime::Secret,
    ) -> Result<(), Error> {
        let mut client = self.communication(false)?;
        let reference = format!("bootstrap-{}", grant.operation_id);
        client.validate_recovery_import(&grant, &reference)?;
        client.import_bootstrap_secret(&reference, secret)?;
        client.import_recovery_grant(grant, &reference)?;
        client.drive_pki().await?;
        client.drive_task_trust().await?;
        client.runtime_readiness()
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
        let mut service = DeviceService::open(
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
        service.installation = Some(self.clone());
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
        if never_initialized(&self.state_root)? {
            return Ok(());
        }
        let namespace = self.namespace()?;
        let secrets = DeviceSecrets::open(&self.state_root.join("secrets"), &namespace)?;
        secrets.verify_storage(&self.state_root)?;
        agent_client::validate_stored_identity(
            &self.state_root.join("communication"),
            &self.network()?,
            &secrets,
        )?;
        if self.state_root.join("execution-initialized").exists() {
            validate_markers(&self.state_root, &namespace)?;
            // preflight already checked the retained journal against the canonical network
            // origin. Recomputing its authority from the raw deployment URL changes its
            // identity when the otherwise equivalent origin omits the trailing slash.
            platform_private_storage::validate(&self.execution.work_root)?;
            #[cfg(windows)]
            validate_access_storage(&self.execution, self.service_id)?;
        }
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
        let mut values = vec![
            C::InventoryCollectionV1,
            C::TelemetryCollectV1,
            C::TelemetryCounterLocalV1,
            C::TaskExecuteV1,
            C::MdmEnrollmentV1,
        ];
        if !crate::collection_worker::engines().is_empty() {
            values.push(C::InventoryNativeV1);
        }
        if crate::installation::unmanage_supported(self) {
            values.push(C::DeviceUnmanageV1);
        }
        if cfg!(windows) {
            values.push(C::CredentialsWindowsV1);
            if cfg!(target_arch = "x86_64") {
                values.push(C::DeviceAccessV1);
            }
        }
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
        Error::Denied | Error::Untrusted | Error::NotFound => K::Denied,
        Error::Protocol | Error::Configuration => K::InvalidInput,
        Error::Schema | Error::Unsupported => K::Unsupported,
        Error::Capacity => K::Capacity,
        Error::Conflict | Error::EnrollmentPaused => K::Conflict,
        Error::Storage
        | Error::Network
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
        let Ok(subject) = peer.authenticate(&self.policy) else {
            return Reply::Rejected;
        };
        match request {
            Request::DeviceOverview {} if peer.session_binding().is_ok() => {
                crate::device_overview::read(&subject, peer.session())
            }
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

#[cfg(any(windows, test))]
fn validate_access_storage(
    config: &ExecutionConfig,
    installation: uuid::Uuid,
) -> Result<(), Error> {
    // Candidate refresh checks existing state before stopping the current service.
    // Never repair or initialize an old deployment as part of admission.
    device_access::Store::open(&crate::access_worker::root(config), installation)
        .map(|_| ())
        .map_err(|_| Error::Storage)
}

#[cfg(test)]
pub(crate) mod tests {
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
    fn metadata_rejects_previous_version_and_retired_fields_without_conversion() {
        let root = crate::protocol_test_support::Root::new();
        let image = serde_json::json!({"path":"/unused", "sha256":"a".repeat(64)});
        let peer =
            serde_json::json!({"images":[image.clone()], "subjects":["501"], "interactive":true});
        let current = serde_json::json!({
            "version": execution_ipc::host::DEPLOYMENT_VERSION,
            "ipc_version": execution_ipc::host::IPC_VERSION,
            "service_id":crate::protocol_test_support::SERVICE_ID, "connections":crate::protocol_test_support::signed_connections("https://agent.example.org".into(), uuid::Uuid::from_u128(2), 1), "tenant":uuid::Uuid::from_u128(2),
            "software_read_grants":[], "signing_keys":{}, "ca_file":root.path.join("https-ca.pem"),
            "installation_id":uuid::Uuid::new_v4(), "installation_artifact":"a".repeat(64),
            "registration_clients":peer.clone(), "clients":peer, "service":image,
            "state_root":root.path.join("state"), "helper_work_roots":{},
            "execution":{"work_root":root.path.join("work"), "material_root":root.path.join("materials"),
                "processes":1, "interpreters":[], "managers":[]}
        });
        let bytes = serde_json::to_vec(&current).unwrap();
        let accepted = Deployment::decode_metadata(&bytes).unwrap();
        assert_eq!(accepted.installation_artifact, "a".repeat(64));
        for old in [
            {
                let mut v = current.clone();
                v["version"] = 2.into();
                v
            },
            {
                let mut v = current;
                v["device_ca_file"] = "/protected/device-ca.pem".into();
                v
            },
        ] {
            let original = serde_json::to_vec(&old).unwrap();
            assert!(matches!(
                Deployment::decode_metadata(&original),
                Err(Error::Configuration)
            ));
        }
    }
    #[test]
    fn refresh_requires_existing_installation_bound_access_storage() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("refresh-access-{}", uuid::Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let mut deployment = deployment();
        deployment.execution.material_root = root.clone();
        let offline = crate::access_worker::root(&deployment.execution);
        assert!(validate_access_storage(&deployment.execution, deployment.service_id).is_err());
        assert!(
            !offline.exists(),
            "preflight must not initialize missing state"
        );
        platform_private_storage::directory(&offline).unwrap();
        device_access::Store::create(&offline, deployment.service_id).unwrap();
        validate_access_storage(&deployment.execution, deployment.service_id).unwrap();
        assert!(validate_access_storage(&deployment.execution, uuid::Uuid::new_v4()).is_err());
        std::fs::remove_dir_all(root).unwrap();
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
    pub(crate) fn deployment() -> Deployment {
        let image = installation_security::Artifact {
            path: "/unused".into(),
            sha256: "a".repeat(64),
            cdhash: None,
        };
        Deployment {
            service_id: crate::protocol_test_support::SERVICE_ID,
            connections: crate::protocol_test_support::signed_connections(
                "https://agent.example.org".into(),
                uuid::Uuid::from_u128(2),
                1,
            ),
            software_read_grants: vec![],
            version: execution_ipc::host::DEPLOYMENT_VERSION,
            ipc_version: execution_ipc::host::IPC_VERSION,
            tenant: uuid::Uuid::new_v4(),
            signing_keys: Default::default(),
            ca_file: None,
            installation_artifact: "a".repeat(64),
            installation_id: uuid::Uuid::new_v4(),
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
        }
    }
    #[tokio::test]
    async fn native_capability_matches_production_engines_without_interpreters_or_managers() {
        let mut deployment = deployment();
        deployment.execution.interpreters.clear();
        deployment.execution.managers.clear();
        let capabilities = deployment.capabilities();
        let environment = crate::collection_environment::observe(&[]).await.unwrap();
        assert_eq!(
            environment.native_engines,
            crate::collection_worker::engines()
        );
        let native = capabilities.contains(&wire::Capability::InventoryNativeV1);
        assert_eq!(native, cfg!(any(windows, target_os = "macos")));
        assert_eq!(native, !environment.native_engines.is_empty());
        assert!(capabilities.windows(2).all(|pair| pair[0] < pair[1]));
    }
    #[test]
    fn software_capability_requires_its_interpreter() {
        let mut deployment = deployment();
        let image = deployment.service.clone();
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
