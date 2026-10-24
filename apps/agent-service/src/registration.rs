//! Registration is an independently authorized interface to the installation execution owner.
use crate::{deployment::Deployment, SystemClock};
#[cfg(test)]
use agent_client::Client;
use agent_client::{EnrollmentPhase, EnrollmentStatus, Error};
#[cfg(test)]
use execution_ipc::host::Readiness;
use execution_ipc::host::{
    CredentialStatus, Handler, Peer, PeerPolicy, RegistrationPhase, RegistrationStatus, Reply,
    Request,
};
#[cfg(test)]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};

pub(crate) fn project(value: EnrollmentStatus) -> RegistrationStatus {
    let phase = match value.phase {
        EnrollmentPhase::Unregistered => RegistrationPhase::Unregistered,
        EnrollmentPhase::Preparing => RegistrationPhase::Preparing,
        EnrollmentPhase::ConfigurationUnknown => RegistrationPhase::ConfigurationUnknown,
        EnrollmentPhase::AwaitingAuthorization => RegistrationPhase::AwaitingAuthorization,
        EnrollmentPhase::Authorized => RegistrationPhase::Authorized,
        EnrollmentPhase::Activating => RegistrationPhase::Activating,
        EnrollmentPhase::Active => RegistrationPhase::Active,
        EnrollmentPhase::IssuanceUnknown => RegistrationPhase::IssuanceUnknown,
        EnrollmentPhase::ActivationUnknown => RegistrationPhase::ActivationUnknown,
        EnrollmentPhase::Rejected => RegistrationPhase::Rejected,
        EnrollmentPhase::Paused => RegistrationPhase::Paused,
    };
    let credential = match value.failure {
        Some(rss_mdm_agent_wire::enroll::ErrorCode::CertificateExpired) => {
            CredentialStatus::Expired
        }
        Some(rss_mdm_agent_wire::enroll::ErrorCode::CredentialRevoked) => CredentialStatus::Revoked,
        Some(rss_mdm_agent_wire::enroll::ErrorCode::CredentialRetired) => CredentialStatus::Retired,
        _ if value.registered => CredentialStatus::Active,
        _ => CredentialStatus::Absent,
    };
    RegistrationStatus {
        task_trust: value
            .task_trust
            .map(|v| execution_ipc::host::TrustInstallationStatus {
                state: match v.state {
                    rss_mdm_agent_wire::task_trust::ObservationState::Received => {
                        execution_ipc::host::TrustInstallationState::Received
                    }
                    rss_mdm_agent_wire::task_trust::ObservationState::Applied => {
                        execution_ipc::host::TrustInstallationState::Applied
                    }
                    rss_mdm_agent_wire::task_trust::ObservationState::Failed => {
                        execution_ipc::host::TrustInstallationState::Failed
                    }
                    rss_mdm_agent_wire::task_trust::ObservationState::Unknown => {
                        execution_ipc::host::TrustInstallationState::Unknown
                    }
                },
                target_revision: v.target_revision,
                actual_revision: v.actual_revision,
                observed_at: v.observed_at,
                acknowledged: value.task_trust_acknowledged,
            }),
        pki_trust: value
            .pki_trust
            .map(|v| execution_ipc::host::TrustInstallationStatus {
                state: match v.state {
                    rss_mdm_agent_wire::pki::ObservationState::Received => {
                        execution_ipc::host::TrustInstallationState::Received
                    }
                    rss_mdm_agent_wire::pki::ObservationState::Applied => {
                        execution_ipc::host::TrustInstallationState::Applied
                    }
                    rss_mdm_agent_wire::pki::ObservationState::Failed => {
                        execution_ipc::host::TrustInstallationState::Failed
                    }
                    rss_mdm_agent_wire::pki::ObservationState::Unknown => {
                        execution_ipc::host::TrustInstallationState::Unknown
                    }
                },
                target_revision: v.target_revision,
                actual_revision: v.actual_revision,
                observed_at: v.observed_at,
                acknowledged: value.pki_trust_acknowledged,
            }),
        failure: value.failure.map(|code| match code {
            rss_mdm_agent_wire::enroll::ErrorCode::GrantExpired => {
                execution_ipc::host::RegistrationFailure::Expired
            }
            rss_mdm_agent_wire::enroll::ErrorCode::RegistrationLimit => {
                execution_ipc::host::RegistrationFailure::Capacity
            }
            rss_mdm_agent_wire::enroll::ErrorCode::ServiceUnavailable => {
                execution_ipc::host::RegistrationFailure::Unavailable
            }
            rss_mdm_agent_wire::enroll::ErrorCode::OperationConflict => {
                execution_ipc::host::RegistrationFailure::Conflict
            }
            _ => execution_ipc::host::RegistrationFailure::Denied,
        }),
        phase,
        grant_id: value.grant_id.map(|id| id.to_string()),
        expires_at: value.expires_at,
        public_key_fingerprint: value.public_key_fingerprint,
        registered: value.registered,
        credential,
    }
}
/// Authentication facade; the one service owner exists before enterprise registration.
pub(crate) struct RegistrationOwner {
    network: agent_client::Config,
    installer_format: Option<rss_mdm_agent_wire::enroll::configuration::InstallerFormat>,
    policy: PeerPolicy,
    registration_policy: PeerPolicy,
    owner: crate::service::ServiceHandle,
}
impl RegistrationOwner {
    pub(crate) fn new(deployment: Deployment) -> Result<Self, Error> {
        let exists = deployment
            .state_root
            .join("communication/communication.sqlite")
            .exists();
        let client = deployment.communication(!exists)?;
        Ok(Self {
            network: deployment.network()?,
            installer_format: crate::installation::installer_format(&deployment).ok(),
            policy: deployment.clients.clone(),
            registration_policy: deployment.registration_clients.clone(),
            owner: deployment.activate(client)?,
        })
    }
}
impl Handler for RegistrationOwner {
    fn peer_policy(&self) -> Option<PeerPolicy> {
        let mut policy = self
            .owner
            .peer_policy()
            .unwrap_or_else(|| self.policy.clone());
        for image in &self.registration_policy.images {
            if !policy.images.iter().any(|v| {
                v.path == image.path && v.sha256 == image.sha256 && v.cdhash == image.cdhash
            }) {
                policy.images.push(image.clone());
            }
        }
        for subject in &self.registration_policy.subjects {
            if !policy.subjects.contains(subject) {
                policy.subjects.push(subject.clone());
            }
        }
        Some(policy)
    }
    fn register_helper(&mut self, peer: &Peer) -> Result<(), execution_app::Error> {
        self.owner.register_helper(peer)
    }
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply {
        if matches!(
            request,
            Request::RegistrationStatus {}
                | Request::RegistrationContext {}
                | Request::RequestRegistration {}
                | Request::ImportRegistration { .. }
        ) {
            if peer.authenticate(&self.registration_policy).is_err()
                || peer.session_binding().is_err()
            {
                return Reply::Rejected;
            }
            if let Request::RegistrationContext {} = request {
                return match self.owner.registration_status() {
                    Reply::RegistrationStatus { value } => Reply::RegistrationContext {
                        value: execution_ipc::host::RegistrationContext {
                            service_id: self.network.service_id.to_string(),
                            origin: self
                                .network
                                .connections
                                .configuration
                                .preferred_origin
                                .clone(),
                            tenant_id: self.network.tenant.to_string(),
                            status: value,
                        },
                    },
                    other => other,
                };
            }
            if let Request::RegistrationStatus {} = request {
                return self.owner.registration_status();
            }
            if let Request::ImportRegistration { file } = &request {
                use agent_client::Clock;
                if self.installer_format.is_none()
                    || SystemClock::new()
                        .and_then(|clock| clock.now())
                        .and_then(|now| {
                            agent_client::verify_configuration(file, &self.network, now)
                        })
                        .is_err()
                {
                    return Reply::Rejected;
                }
            }
            return self.owner.registration_request(request);
        }
        self.owner.handle(peer, request)
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        self.owner.tick()
    }
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        self.owner.stop()
    }
}
#[cfg(test)]
struct Snapshot {
    status: RegistrationStatus,
    readiness: Readiness,
}
#[cfg(test)]
enum Command {
    Request {
        prepared: Option<mpsc::SyncSender<Result<(), Error>>>,
    },
    Configuration {
        file: Box<rss_mdm_agent_wire::enroll::configuration::ConfigurationFile>,
        installer_format: rss_mdm_agent_wire::enroll::configuration::InstallerFormat,
    },
}
#[cfg(test)]
fn drive_fixture_enrollment<S: agent_client::IdentityProvider, C: agent_client::Clock, O>(
    mut client: Option<Client<S, C>>,
    capabilities: Vec<rss_mdm_agent_wire::runtime::Capability>,
    shared: Arc<Mutex<Snapshot>>,
    requests: mpsc::Receiver<Command>,
    stopping: Arc<AtomicBool>,
    mut open: impl FnMut() -> Result<Client<S, C>, Error>,
    activate: impl FnOnce(Client<S, C>) -> Result<O, Error>,
) -> Result<Option<O>, Error> {
    let runtime = crate::service::owner_runtime()?;
    loop {
        if stopping.load(Ordering::Acquire) {
            return Ok(None);
        }
        if let Some(value) = &client {
            let status = value.enrollment_status()?;
            let mut snapshot = shared.lock().map_err(|_| Error::Unavailable)?;
            let previous_failure = snapshot.status.failure;
            snapshot.status = project(status);
            if snapshot.status.failure.is_none() {
                snapshot.status.failure = previous_failure;
            }
            if snapshot.status.registered {
                drop(snapshot);
                let owner = activate(client.take().ok_or(Error::Storage)?)?;
                shared.lock().map_err(|_| Error::Unavailable)?.readiness = Readiness::NotReady;
                return Ok(Some(owner));
            }
        }
        let command = requests.recv_timeout(std::time::Duration::from_secs(2));
        match command {
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
            Ok(command) => {
                if client.is_none() {
                    client = Some(open()?);
                }
                let value = client.as_mut().ok_or(Error::Storage)?;
                let (prepared, acknowledgement) = match command {
                    Command::Request { prepared } => (
                        value
                            .prepare_authorization(capabilities.clone())
                            .map(|_| ()),
                        prepared,
                    ),
                    Command::Configuration {
                        file,
                        installer_format,
                    } => (
                        value.prepare_configuration(*file, installer_format, capabilities.clone()),
                        None,
                    ),
                };
                record_result(prepared, value, &shared)?;
                if let Some(reply) = acknowledgement {
                    let _ = reply.send(prepared);
                }
                let result = match prepared {
                    Ok(()) => runtime.block_on(until_stopped(value.drive_pki(), &stopping)),
                    Err(error) => Err(error),
                };
                record_result(result, value, &shared)?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(value) = &mut client {
                    let prepared = value.prepare_managed_enrollment(capabilities.clone());
                    if let Err(error) = prepared {
                        record_result(Err(error), value, &shared)?;
                    }
                    if value.has_pending_enrollment()? {
                        let result = runtime.block_on(until_stopped(value.drive_pki(), &stopping));
                        record_result(result, value, &shared)?;
                    }
                }
            }
        }
    }
}
#[cfg(test)]
async fn until_stopped<T>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    stopping: &AtomicBool,
) -> Result<T, Error> {
    tokio::select! {
        result = future => result,
        _ = async {
            while !stopping.load(Ordering::Acquire) {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        } => Err(Error::Unavailable),
    }
}
#[cfg(test)]
fn failure(error: Error) -> execution_ipc::host::RegistrationFailure {
    use execution_ipc::host::RegistrationFailure as F;
    match error {
        Error::Conflict => F::Conflict,
        Error::Denied => F::Denied,
        Error::Expired => F::Expired,
        Error::Capacity => F::Capacity,
        Error::Untrusted | Error::Identity | Error::Configuration | Error::Protocol => {
            F::InvalidConfiguration
        }
        _ => F::Unavailable,
    }
}
#[cfg(test)]
fn record_result<S: agent_client::IdentityProvider, C: agent_client::Clock>(
    result: Result<(), Error>,
    client: &Client<S, C>,
    shared: &Arc<Mutex<Snapshot>>,
) -> Result<(), Error> {
    if let Err(error) = result {
        eprintln!("agent_enrollment: {error}");
    }
    let mut status = project(client.enrollment_status()?);
    status.failure = result.err().map(failure).or(status.failure);
    shared.lock().map_err(|_| Error::Unavailable)?.status = status;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol_test_support::{Root, Server};
    use agent_client::{IdentityProvider, OpenMode};
    use rss_mdm_agent_wire::runtime::Capability;
    struct InterruptedSecret(crate::protocol_test_support::Secrets);
    impl agent_client::IdentityProvider for InterruptedSecret {
        fn resolve(&self, reference: &str) -> Result<rss_mdm_agent_wire::runtime::Secret, Error> {
            self.0.resolve(reference)
        }
        fn import_secret(
            &self,
            reference: &str,
            secret: &rss_mdm_agent_wire::runtime::Secret,
        ) -> Result<(), Error> {
            self.0.import_secret(reference, secret)
        }
        fn create_secret(&self, _: &str) -> Result<rss_mdm_agent_wire::runtime::Secret, Error> {
            Err(Error::Storage)
        }
        fn create_key(&self, reference: &str) -> Result<Arc<dyn rustls::sign::SigningKey>, Error> {
            self.0.create_key(reference)
        }
        fn key(&self, reference: &str) -> Result<Arc<dyn rustls::sign::SigningKey>, Error> {
            self.0.key(reference)
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn startup_prepares_interrupted_renewal_before_registered_handoff() {
        let _budget = crate::scenario_budget::enter();
        for missing_key in [false, true] {
            let server = Server::tls().await;
            let root = Root::new();
            let mut client = server.client(&root, OpenMode::Create);
            server.register(&mut client).await;
            let binding = client.registration().unwrap();
            drop(client);
            let mut interrupted = Client::open(
                &root.path,
                server.config(),
                OpenMode::Existing,
                InterruptedSecret(server.secrets.clone()),
                server.time.clone(),
            )
            .unwrap();
            // Fail at the real protected-secret boundary after pending + key + CSR are durable.
            assert_eq!(
                interrupted.renew_certificate().await.unwrap_err(),
                Error::Storage
            );
            let identity = || {
                rusqlite::Connection::open(root.path.join("communication.sqlite"))
                    .unwrap()
                    .query_row("SELECT body FROM state WHERE key='identity'", [], |row| {
                        row.get::<_, Vec<u8>>(0)
                    })
                    .unwrap()
            };
            let before = identity();
            let state: serde_json::Value = serde_json::from_slice(&before).unwrap();
            let pending = &state["pending"];
            let key = pending["key_reference"].as_str().unwrap();
            let secret = pending["secret_reference"].as_str().unwrap();
            assert!(pending["csr"].is_string());
            assert!(server.secrets.key(key).is_ok());
            assert!(server.secrets.resolve(secret).is_err());
            drop(interrupted);
            if missing_key {
                server.secrets.0.lock().unwrap().remove(key);
                let secrets = server.secrets.0.lock().unwrap().clone();
                assert!(matches!(
                    Client::open(
                        &root.path,
                        server.config(),
                        OpenMode::Existing,
                        server.secrets.clone(),
                        server.time.clone()
                    ),
                    Err(Error::Identity)
                ));
                assert_eq!(identity(), before);
                assert_eq!(*server.secrets.0.lock().unwrap(), secrets);
            } else {
                let client = server.client(&root, OpenMode::Existing);
                assert_eq!(identity(), before); // No operation, reference, profile or frozen CSR changes.
                assert!(server.secrets.resolve(secret).is_ok());
                let secrets = server.secrets.0.lock().unwrap().clone();
                let shared = snapshot();
                let published = shared.clone();
                let (_sender, receiver) = mpsc::sync_channel(8);
                let (completed, completion) = mpsc::channel();
                let worker = std::thread::spawn(move || {
                    let result = drive_fixture_enrollment(
                        Some(client),
                        capabilities(),
                        published,
                        receiver,
                        Arc::new(AtomicBool::new(false)),
                        || unreachable!(),
                        |client| {
                            client.output_policy()?; // The execution assembly's first pending-secret consumer.
                            Ok(client)
                        },
                    );
                    completed.send(result).unwrap();
                });
                let mut client = tokio::task::spawn_blocking(move || {
                    completion.recv_timeout(std::time::Duration::from_secs(8))
                })
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .unwrap();
                worker.join().unwrap();
                // Registered handoff publishes readiness only after the execution owner assembles.
                assert_eq!(shared.lock().unwrap().readiness, Readiness::NotReady);
                client.runtime_readiness().unwrap();
                assert!(shared.lock().unwrap().status.registered);
                assert_eq!(client.registration().unwrap(), binding);
                assert_eq!(server.data.lock().unwrap().certificate_calls.len(), 1);
                assert_eq!(server.data.lock().unwrap().activation_calls.len(), 1);
                client.drive_enrollment().await.unwrap();
                assert_eq!(client.registration().unwrap(), binding);
                assert_eq!(*server.secrets.0.lock().unwrap(), secrets);
                let data = server.data.lock().unwrap();
                assert_eq!(data.certificate_calls.len(), 2);
                assert_eq!(
                    data.certificate_calls[1]["operationId"],
                    pending["certificate_operation"]
                );
                assert_eq!(data.certificate_calls[1]["csr"], pending["csr"]);
                assert_eq!(
                    data.activation_calls[1]["operationId"],
                    pending["activation_operation"]
                );
            }
        }
    }
    fn snapshot() -> Arc<Mutex<Snapshot>> {
        Arc::new(Mutex::new(Snapshot {
            status: project(EnrollmentStatus {
                task_trust: None,
                pki_trust: None,
                task_trust_acknowledged: false,
                pki_trust_acknowledged: false,
                phase: EnrollmentPhase::Unregistered,
                grant_id: None,
                expires_at: None,
                public_key_fingerprint: None,
                registered: false,
                failure: None,
            }),
            readiness: Readiness::NotReady,
        }))
    }
    fn capabilities() -> Vec<Capability> {
        vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1]
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn production_native_capability_reaches_authorization_and_configuration_registrations() {
        let _budget = crate::scenario_budget::enter();
        for configuration in [false, true] {
            let server = Server::tls().await;
            if cfg!(windows) {
                server.data.lock().unwrap().native_collection();
            }
            let root = Root::new();
            let mut deployment = crate::deployment::tests::deployment();
            deployment.execution.interpreters.clear();
            deployment.execution.managers.clear();
            let capabilities = deployment.capabilities();
            let expected = capabilities.clone();
            let environment = crate::collection_environment::observe(&[]).await.unwrap();
            assert_eq!(
                capabilities.contains(&Capability::InventoryNativeV1),
                !environment.native_engines.is_empty()
            );
            let mut client = server.client(&root, OpenMode::Create);
            client.set_collection_environment(environment).unwrap();
            let command = if configuration {
                let installer_format = if cfg!(windows) {
                    rss_mdm_agent_wire::enroll::configuration::InstallerFormat::WindowsMsi
                } else {
                    rss_mdm_agent_wire::enroll::configuration::InstallerFormat::MacosPkg
                };
                let file = Box::new(server.configuration());
                {
                    let mut data = server.data.lock().unwrap();
                    data.packages[0].format = installer_format;
                    data.packages[0].filename = if cfg!(windows) {
                        "fixture.msi"
                    } else {
                        "fixture.pkg"
                    }
                    .into();
                }
                Command::Configuration {
                    file,
                    installer_format,
                }
            } else {
                Command::Request { prepared: None }
            };
            let (sender, receiver) = mpsc::sync_channel(8);
            sender.send(command).unwrap();
            let stopping = Arc::new(AtomicBool::new(false));
            let stop = stopping.clone();
            let shared = snapshot();
            let published = shared.clone();
            let (completed, completion) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let result = drive_fixture_enrollment(
                    Some(client),
                    capabilities,
                    published,
                    receiver,
                    stop,
                    || unreachable!("new registration already owns its client"),
                    |client| client.registration(),
                );
                let _ = completed.send(result);
            });
            let result = tokio::task::spawn_blocking(move || {
                completion.recv_timeout(std::time::Duration::from_secs(8))
            })
            .await
            .unwrap();
            stopping.store(true, Ordering::Release);
            worker.join().unwrap();
            let binding = result.unwrap().unwrap().unwrap();
            assert!(shared.lock().unwrap().status.registered);
            assert_eq!(binding.capabilities, expected);
            let data = server.data.lock().unwrap();
            assert_eq!(data.certificate_calls.len(), 1);
            let profile: rss_mdm_agent_wire::enroll::RuntimeProfile =
                serde_json::from_value(data.certificate_calls[0]["runtime"].clone()).unwrap();
            assert_eq!(profile.capabilities, expected);
            assert_eq!(data.activation.as_ref().unwrap().runtime, binding);
            assert_eq!(data.configuration_calls.len(), usize::from(configuration));
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn registration_owner_recovers_transient_failure_without_another_request() {
        let _budget = crate::scenario_budget::enter();
        for fault in ["registration", "trust"] {
            for restart in [false, true] {
                let server = Server::tls().await;
                let root = Root::new();
                match fault {
                    "registration" => server.data.lock().unwrap().registration_failure = true,
                    "trust" => server.data.lock().unwrap().trust_failure = true,
                    _ => unreachable!(),
                }
                let client = if restart {
                    let mut value = server.client(&root, OpenMode::Create);
                    value.prepare_authorization(capabilities()).unwrap();
                    assert!(value.drive_pki().await.is_err());
                    drop(value);
                    Some(server.client(&root, OpenMode::Existing))
                } else {
                    None
                };
                let moving_time = server.time.clone();
                let clock_task = tokio::spawn(async move {
                    let start = std::time::Instant::now();
                    loop {
                        moving_time.set(1 + start.elapsed().as_secs() as i64);
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                });
                let cfg = server.config();
                let secrets = server.secrets.clone();
                let time = server.time.clone();
                let shared = snapshot();
                let published = shared.clone();
                let (sender, receiver) = mpsc::sync_channel(8);
                let (completed, completion) = mpsc::channel();
                let worker = std::thread::spawn(move || {
                    let result = drive_fixture_enrollment(
                        client,
                        capabilities(),
                        published,
                        receiver,
                        Arc::new(AtomicBool::new(false)),
                        || {
                            Client::open(
                                &root.path,
                                cfg.clone(),
                                OpenMode::Create,
                                secrets.clone(),
                                time.clone(),
                            )
                        },
                        |client| client.registration(),
                    );
                    let _ = completed.send(result);
                });
                if !restart {
                    sender.send(Command::Request { prepared: None }).unwrap();
                }
                let result = tokio::task::spawn_blocking(move || {
                    completion.recv_timeout(std::time::Duration::from_secs(8))
                })
                .await
                .unwrap()
                .unwrap()
                .unwrap();
                assert!(result.is_some());
                worker.join().unwrap();
                clock_task.abort();
                assert!(shared.lock().unwrap().status.registered);
                assert_eq!(
                    shared.lock().unwrap().status.phase,
                    RegistrationPhase::Active
                );
                let data = server.data.lock().unwrap();
                assert_eq!(data.certificate_calls.len(), 1);
                assert_eq!(data.activation_calls.len(), 1);
            }
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn full_registration_queue_cannot_drop_stop() {
        let _budget = crate::scenario_budget::enter();
        let server = Server::tls().await;
        server.data.lock().unwrap().registration_failure = true;
        let root = Root::new();
        let cfg = server.config();
        let secrets = server.secrets.clone();
        let time = server.time.clone();
        let (commands, requests) = mpsc::sync_channel(8);
        let (started, start) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let (completed, completion) = mpsc::sync_channel(1);
        let shared = snapshot();
        let published = shared.clone();
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let thread = std::thread::spawn(move || {
            let result = drive_fixture_enrollment(
                None,
                capabilities(),
                published,
                requests,
                stop,
                || {
                    started.send(()).unwrap();
                    released
                        .recv_timeout(std::time::Duration::from_secs(4))
                        .unwrap();
                    Client::open(
                        &root.path,
                        cfg.clone(),
                        OpenMode::Create,
                        secrets.clone(),
                        time.clone(),
                    )
                },
                |_| -> Result<crate::service::ServiceHandle, Error> {
                    panic!("stop precedes activation")
                },
            );
            completed.send(result).unwrap();
        });
        commands.send(Command::Request { prepared: None }).unwrap();
        tokio::task::spawn_blocking(move || start.recv_timeout(std::time::Duration::from_secs(3)))
            .await
            .unwrap()
            .unwrap();
        for _ in 0..8 {
            commands
                .try_send(Command::Request { prepared: None })
                .unwrap();
        }
        assert!(commands
            .try_send(Command::Request { prepared: None })
            .is_err());
        stopping.store(true, Ordering::Release);
        release.send(()).unwrap();
        let result = tokio::task::spawn_blocking(move || {
            completion.recv_timeout(std::time::Duration::from_secs(4))
        })
        .await
        .unwrap()
        .unwrap();
        assert!(result.unwrap().is_none());
        thread.join().unwrap();
        assert_eq!(server.data.lock().unwrap().certificate_calls.len(), 0);
    }
}
