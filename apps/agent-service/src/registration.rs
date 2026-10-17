//! Registration runs on one communication owner before the execution owner is assembled.
use crate::{deployment::Deployment, DeviceSecrets, SystemClock};
use agent_client::{Client, EnrollmentPhase, EnrollmentStatus, Error};
use execution_ipc::host::{
    CredentialStatus, Handler, Peer, PeerPolicy, Readiness, RegistrationPhase, RegistrationStatus,
    Reply, Request, ServiceStatus,
};
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
struct Snapshot {
    status: RegistrationStatus,
    readiness: Readiness,
}
enum Command {
    Request {
        prepared: Option<mpsc::SyncSender<Result<(), Error>>>,
    },
    Configuration {
        file: rss_mdm_agent_wire::enroll::configuration::ConfigurationFile,
        candidate_sha256: [u8; 32],
    },
}
/// Native callbacks read a public snapshot and enqueue an explicit fixed-context request.
pub(crate) struct RegistrationOwner {
    network: agent_client::Config,
    candidate_sha256: Option<[u8; 32]>,
    origin: String,
    tenant_id: String,
    policy: PeerPolicy,
    registration_policy: PeerPolicy,
    snapshot: Arc<Mutex<Snapshot>>,
    commands: mpsc::SyncSender<Command>,
    completion: mpsc::Receiver<Result<Option<crate::service::ServiceHandle>, Error>>,
    owner: Option<crate::service::ServiceHandle>,
    thread: Option<std::thread::JoinHandle<()>>,
    failed: bool,
    stopping: Arc<AtomicBool>,
}
impl RegistrationOwner {
    pub(crate) fn new(deployment: Deployment) -> Result<Self, Error> {
        let snapshot = Arc::new(Mutex::new(Snapshot {
            status: RegistrationStatus {
                failure: None,
                phase: RegistrationPhase::Unregistered,
                grant_id: None,
                expires_at: None,
                public_key_fingerprint: None,
                registered: false,
                credential: CredentialStatus::Absent,
            },
            readiness: Readiness::RegistrationRequired,
        }));
        let (commands, requests) = mpsc::sync_channel(8);
        let (completed, completion) = mpsc::sync_channel(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let shared = snapshot.clone();
        let network = deployment.network()?;
        let candidate_sha256 = crate::installation::publication_digest(&deployment).ok();
        let origin = deployment.origin.clone();
        let tenant_id = deployment.tenant.to_string();
        let policy = deployment.clients.clone();
        let registration_policy = deployment.registration_clients.clone();
        let thread = std::thread::Builder::new()
            .name("rss-registration-owner".into())
            .spawn(move || {
                let result = drive(deployment, shared.clone(), requests, stop);
                if let Err(error) = &result {
                    execution_ipc::record_startup_failure(super::deployment::startup_failure(
                        *error,
                    ));
                    if let Ok(mut snapshot) = shared.lock() {
                        snapshot.readiness = Readiness::NotReady;
                        snapshot.status.phase = RegistrationPhase::Rejected;
                    }
                }
                let _ = completed.send(result);
            })?;
        Ok(Self {
            network,
            candidate_sha256,
            origin,
            tenant_id,
            policy,
            registration_policy,
            snapshot,
            commands,
            completion,
            owner: None,
            thread: Some(thread),
            failed: false,
            stopping,
        })
    }
}
fn drive(
    deployment: Deployment,
    shared: Arc<Mutex<Snapshot>>,
    requests: mpsc::Receiver<Command>,
    stopping: Arc<AtomicBool>,
) -> Result<Option<crate::service::ServiceHandle>, Error> {
    let exists = deployment
        .state_root
        .join("communication/communication.sqlite")
        .exists();
    let client: Option<Client<DeviceSecrets, SystemClock>> = if exists {
        Some(deployment.communication(false)?)
    } else {
        None
    };
    run(
        client,
        deployment.capabilities(),
        shared,
        requests,
        stopping,
        || deployment.communication(true),
        |client| deployment.activate(client),
    )
}
fn run<S: agent_client::IdentityProvider, C: agent_client::Clock, O>(
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
                        candidate_sha256,
                    } => (
                        value.prepare_configuration(file, candidate_sha256, capabilities.clone()),
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
                    if value.has_pending_enrollment()? {
                        let result = runtime.block_on(until_stopped(value.drive_pki(), &stopping));
                        record_result(result, value, &shared)?;
                    }
                }
            }
        }
    }
}
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
impl Handler for RegistrationOwner {
    fn peer_policy(&self) -> Option<PeerPolicy> {
        let mut policy = self
            .owner
            .as_ref()
            .and_then(Handler::peer_policy)
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
        self.owner
            .as_mut()
            .ok_or(execution_app::Error::Unavailable)?
            .register_helper(peer)
    }
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply {
        match request {
            Request::ImportRegistration { file } => {
                use agent_client::Clock;
                if peer.authenticate(&self.registration_policy).is_err()
                    || self.owner.is_some()
                    || self.failed
                {
                    return Reply::Rejected;
                }
                let Some(candidate_sha256) = self.candidate_sha256 else {
                    return Reply::Rejected;
                };
                let valid = SystemClock::new()
                    .and_then(|clock| clock.now())
                    .and_then(|now| agent_client::verify_configuration(&file, &self.network, now));
                if valid.is_err() {
                    return Reply::Rejected;
                }
                if self
                    .commands
                    .try_send(Command::Configuration {
                        file,
                        candidate_sha256,
                    })
                    .is_err()
                {
                    return Reply::Unavailable;
                }
                match self.snapshot.lock() {
                    Ok(v) => Reply::RegistrationStatus {
                        value: v.status.clone(),
                    },
                    Err(_) => Reply::Unavailable,
                }
            }
            Request::RegistrationContext {} => {
                if peer.authenticate(&self.registration_policy).is_err() {
                    return Reply::Rejected;
                }
                let status = if let Some(owner) = &self.owner {
                    match owner.registration_status() {
                        Reply::RegistrationStatus { value } => value,
                        reply => return reply,
                    }
                } else {
                    match self.snapshot.lock() {
                        Ok(value) => value.status.clone(),
                        Err(_) => return Reply::Unavailable,
                    }
                };
                Reply::RegistrationContext {
                    value: execution_ipc::host::RegistrationContext {
                        origin: self.origin.clone(),
                        tenant_id: self.tenant_id.clone(),
                        status,
                    },
                }
            }
            Request::RegistrationStatus {} | Request::RequestRegistration {} => {
                if peer.authenticate(&self.registration_policy).is_err() {
                    return Reply::Rejected;
                }
                if matches!(request, Request::RequestRegistration {}) {
                    if self.owner.is_some() || self.failed {
                        return Reply::Rejected;
                    }
                    let (prepared, completion) = mpsc::sync_channel(1);
                    if self
                        .commands
                        .try_send(Command::Request {
                            prepared: Some(prepared),
                        })
                        .is_err()
                    {
                        return Reply::Unavailable;
                    }
                    // Acknowledge durable local preparation before the caller interprets a stale failure.
                    // HTTP/PKI continues on the communication owner, outside this callback.
                    if completion
                        .recv_timeout(std::time::Duration::from_secs(2))
                        .is_err()
                    {
                        return Reply::Unavailable;
                    }
                }
                if let Some(owner) = &self.owner {
                    return owner.registration_status();
                }
                match self.snapshot.lock() {
                    Ok(v) => Reply::RegistrationStatus {
                        value: v.status.clone(),
                    },
                    Err(_) => Reply::Unavailable,
                }
            }
            Request::ServiceStatus {} => {
                if peer.authenticate(&self.policy).is_err()
                    && peer.authenticate(&self.registration_policy).is_err()
                {
                    return Reply::Rejected;
                }
                if let Some(owner) = &mut self.owner {
                    return owner.service_status();
                }
                match self.snapshot.lock() {
                    Ok(v) => Reply::ServiceStatus {
                        value: ServiceStatus::new(v.readiness.clone()),
                    },
                    Err(_) => Reply::Unavailable,
                }
            }
            request => self
                .owner
                .as_mut()
                .map_or(Reply::Rejected, |owner| owner.handle(peer, request)),
        }
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        if self.owner.is_none() && !self.failed {
            match self.completion.try_recv() {
                Ok(Ok(Some(owner))) => self.owner = Some(owner),
                Ok(Err(_)) | Ok(Ok(None)) => self.failed = true,
                Err(mpsc::TryRecvError::Disconnected) => self.failed = true,
                Err(mpsc::TryRecvError::Empty) => (),
            }
        }
        self.owner.as_mut().map_or(Ok(()), Handler::tick)
    }
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        self.stopping.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| execution_app::Error::OutcomeUnknown)?;
        }
        if self.owner.is_none() {
            if let Ok(Ok(Some(owner))) = self.completion.try_recv() {
                self.owner = Some(owner);
            }
        }
        self.owner.as_mut().map_or(Ok(()), Handler::stop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol_test_support::{Root, Server};
    use agent_client::{IdentityProvider, OpenMode};
    use rss_mdm_agent_wire::runtime::Capability;
    struct InterruptedSecret(crate::protocol_test_support::Secrets);
    impl agent_client::MachineTrust for InterruptedSecret {
        fn certificate_present(
            &self,
            store: agent_client::MachineStore,
            der: &[u8],
        ) -> Result<bool, Error> {
            self.0.certificate_present(store, der)
        }
        fn read_certificate(
            &self,
            store: agent_client::MachineStore,
            der: &[u8],
        ) -> Result<bool, Error> {
            self.0.read_certificate(store, der)
        }
        fn add_certificate(
            &self,
            store: agent_client::MachineStore,
            der: &[u8],
        ) -> Result<agent_client::CertificateAdd, Error> {
            self.0.add_certificate(store, der)
        }
        fn configure_certificate(
            &self,
            store: agent_client::MachineStore,
            der: &[u8],
        ) -> Result<(), Error> {
            self.0.configure_certificate(store, der)
        }
        fn delete_certificate(
            &self,
            store: agent_client::MachineStore,
            der: &[u8],
        ) -> Result<(), Error> {
            self.0.delete_certificate(store, der)
        }
    }
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
                    let result = run(
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
                phase: EnrollmentPhase::Unregistered,
                grant_id: None,
                expires_at: None,
                public_key_fingerprint: None,
                registered: false,
                failure: None,
            }),
            readiness: Readiness::RegistrationRequired,
        }))
    }
    fn capabilities() -> Vec<Capability> {
        vec![Capability::InventoryCollectionV1, Capability::TaskExecuteV1]
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn registration_owner_recovers_transient_failure_without_another_request() {
        let _budget = crate::scenario_budget::enter();
        for fault in ["registration", "trust", "machine"] {
            for restart in [false, true] {
                let server = Server::tls().await;
                let root = Root::new();
                match fault {
                    "registration" => server.data.lock().unwrap().registration_failure = true,
                    "trust" => server.data.lock().unwrap().trust_failure = true,
                    "machine" => {
                        server
                            .secrets
                            .0
                            .lock()
                            .unwrap()
                            .insert("machine/configure-failure".into(), "injected".into());
                    }
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
                let native = server.secrets.clone();
                let clock_task = tokio::spawn(async move {
                    let start = std::time::Instant::now();
                    loop {
                        moving_time.set(1 + start.elapsed().as_secs() as i64);
                        if start.elapsed() >= std::time::Duration::from_secs(1) {
                            native.0.lock().unwrap().remove("machine/configure-failure");
                        }
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
                    let result = run(
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
            let result = run(
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
        let policy = PeerPolicy {
            images: vec![],
            subjects: vec![],
            interactive: true,
        };
        let mut owner = RegistrationOwner {
            network: server.config(),
            candidate_sha256: None,
            origin: server.url.origin().ascii_serialization(),
            tenant_id: server.config().tenant.to_string(),
            policy: policy.clone(),
            registration_policy: policy,
            snapshot: shared,
            commands,
            completion,
            owner: None,
            thread: Some(thread),
            failed: false,
            stopping,
        };
        owner
            .commands
            .send(Command::Request { prepared: None })
            .unwrap();
        tokio::task::spawn_blocking(move || start.recv_timeout(std::time::Duration::from_secs(3)))
            .await
            .unwrap()
            .unwrap();
        for _ in 0..8 {
            owner
                .commands
                .try_send(Command::Request { prepared: None })
                .unwrap();
        }
        assert!(owner
            .commands
            .try_send(Command::Request { prepared: None })
            .is_err());
        let observed = owner.stopping.clone();
        let shutdown = tokio::task::spawn_blocking(move || owner.stop());
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !observed.load(Ordering::Acquire) {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        release.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(4), shutdown)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(server.data.lock().unwrap().certificate_calls.len(), 0);
    }
}
