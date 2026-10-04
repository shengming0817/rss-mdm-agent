//! Registration runs on one communication owner before the execution owner is assembled.
use crate::{deployment::Deployment, DeviceSecrets, SystemClock};
use agent_client::{Client, EnrollmentPhase, EnrollmentStatus, Error};
use execution_ipc::host::{Handler, Peer, PeerPolicy, Readiness, RegistrationPhase, RegistrationStatus, Reply, Request, ServiceStatus};
use std::sync::{Arc, Mutex, mpsc};

pub(crate) fn project(value: EnrollmentStatus) -> RegistrationStatus {
    let phase = match value.phase {
        EnrollmentPhase::Unregistered => RegistrationPhase::Unregistered,
        EnrollmentPhase::Preparing => RegistrationPhase::Preparing,
        EnrollmentPhase::AwaitingAuthorization => RegistrationPhase::AwaitingAuthorization,
        EnrollmentPhase::Authorized => RegistrationPhase::Authorized,
        EnrollmentPhase::Activating => RegistrationPhase::Activating,
        EnrollmentPhase::Active => RegistrationPhase::Active,
        EnrollmentPhase::IssuanceUnknown => RegistrationPhase::IssuanceUnknown,
        EnrollmentPhase::ActivationUnknown => RegistrationPhase::ActivationUnknown,
        EnrollmentPhase::Rejected => RegistrationPhase::Rejected,
    };
    RegistrationStatus { phase, grant_id: value.grant_id.map(|id| id.to_string()), expires_at: value.expires_at, public_key_fingerprint: value.public_key_fingerprint, registered: value.registered }
}
struct Snapshot { status: RegistrationStatus, readiness: Readiness }
enum Command { Request, Stop }
/// Native callbacks read a public snapshot and enqueue an explicit fixed-context request.
pub(crate) struct RegistrationOwner {
    policy: PeerPolicy,
    registration_policy: PeerPolicy,
    snapshot: Arc<Mutex<Snapshot>>,
    commands: mpsc::SyncSender<Command>,
    completion: mpsc::Receiver<Result<Option<crate::service::ServiceHandle>, Error>>,
    owner: Option<crate::service::ServiceHandle>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl RegistrationOwner {
    pub(crate) fn new(deployment: Deployment) -> Result<Self, Error> {
        let snapshot = Arc::new(Mutex::new(Snapshot {
            status: RegistrationStatus { phase: RegistrationPhase::Unregistered, grant_id: None, expires_at: None, public_key_fingerprint: None, registered: false },
            readiness: Readiness::RegistrationRequired,
        }));
        let (commands, requests) = mpsc::sync_channel(8);
        let (completed, completion) = mpsc::sync_channel(1);
        let shared = snapshot.clone();
        let policy = deployment.clients.clone();
        let registration_policy = deployment.registration_clients.clone();
        let thread = std::thread::Builder::new().name("rss-registration-owner".into()).spawn(move || {
            let result = drive(deployment, shared.clone(), requests);
            if let Err(error) = &result {
                execution_ipc::record_startup_failure(super::deployment::startup_failure(*error));
                if let Ok(mut snapshot) = shared.lock() { snapshot.readiness = Readiness::NotReady; snapshot.status.phase = RegistrationPhase::Rejected; }
            }
            let _ = completed.send(result);
        })?;
        Ok(Self { policy, registration_policy, snapshot, commands, completion, owner: None, thread: Some(thread) })
    }
}
fn drive(deployment: Deployment, shared: Arc<Mutex<Snapshot>>, requests: mpsc::Receiver<Command>) -> Result<Option<crate::service::ServiceHandle>, Error> {
    let runtime = crate::service::owner_runtime()?;
    let exists = deployment.state_root.join("communication/communication.sqlite").exists();
    let mut client: Option<Client<DeviceSecrets, SystemClock>> = if exists { Some(deployment.communication(false)?) } else { None };
    loop {
        if let Some(value) = &client {
            let status = value.enrollment_status()?;
            let mut snapshot = shared.lock().map_err(|_| Error::Unavailable)?;
            snapshot.status = project(status);
            if snapshot.status.registered && snapshot.status.phase == RegistrationPhase::Active {
                drop(snapshot);
                let owner = deployment.activate(client.take().ok_or(Error::Storage)?)?;
                shared.lock().map_err(|_| Error::Unavailable)?.readiness = Readiness::Ready;
                return Ok(Some(owner));
            }
        }
        let command = requests.recv_timeout(std::time::Duration::from_secs(2));
        match command {
            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
            Ok(Command::Request) => {
                if client.is_none() { client = Some(deployment.communication(true)?); }
                let value = client.as_mut().ok_or(Error::Storage)?;
                let status = value.enrollment_status()?;
                let result = if status.phase == EnrollmentPhase::Unregistered {
                    runtime.block_on(value.request_authorization(deployment.capabilities())).map(|_| ())
                } else { runtime.block_on(value.drive_enrollment()) };
                record_result(result, value, &shared)?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(value) = &mut client {
                    let status = value.enrollment_status()?;
                    if matches!(status.phase, EnrollmentPhase::Preparing | EnrollmentPhase::AwaitingAuthorization | EnrollmentPhase::Authorized | EnrollmentPhase::Activating | EnrollmentPhase::IssuanceUnknown | EnrollmentPhase::ActivationUnknown) {
                        let result = runtime.block_on(value.drive_enrollment());
                        record_result(result, value, &shared)?;
                    }
                }
            }
        }
    }
}
fn record_result(result: Result<(), Error>, client: &Client<DeviceSecrets, SystemClock>, shared: &Arc<Mutex<Snapshot>>) -> Result<(), Error> {
    if let Err(error) = result { eprintln!("agent_enrollment: {error}"); }
    shared.lock().map_err(|_| Error::Unavailable)?.status = project(client.enrollment_status()?);
    Ok(())
}
impl Handler for RegistrationOwner {
    fn peer_policy(&self) -> Option<PeerPolicy> {
        let mut policy = self.owner.as_ref().and_then(Handler::peer_policy).unwrap_or_else(|| self.policy.clone());
        for image in &self.registration_policy.images { if !policy.images.iter().any(|v| v.path==image.path && v.sha256==image.sha256 && v.cdhash==image.cdhash) { policy.images.push(image.clone()); } }
        for subject in &self.registration_policy.subjects { if !policy.subjects.contains(subject) { policy.subjects.push(subject.clone()); } }
        Some(policy)
    }
    fn register_helper(&mut self, peer: &Peer) -> Result<(), execution_app::Error> {
        self.owner.as_mut().ok_or(execution_app::Error::Unavailable)?.register_helper(peer)
    }
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply {
        match request {
            Request::RegistrationStatus {} | Request::RequestRegistration {} => {
                if peer.authenticate(&self.registration_policy).is_err() { return Reply::Rejected; }
                if matches!(request, Request::RequestRegistration {}) {
                    if self.owner.is_some() { return Reply::Rejected; }
                    if self.commands.try_send(Command::Request).is_err() { return Reply::Unavailable; }
                }
                match self.snapshot.lock() { Ok(v) => Reply::RegistrationStatus { value: v.status.clone() }, Err(_) => Reply::Unavailable }
            }
            Request::ServiceStatus {} => {
                if peer.authenticate(&self.policy).is_err() && peer.authenticate(&self.registration_policy).is_err() { return Reply::Rejected; }
                if let Some(owner) = &mut self.owner { return owner.handle(peer, Request::ServiceStatus {}); }
                match self.snapshot.lock() { Ok(v) => Reply::ServiceStatus { value: ServiceStatus::new(v.readiness.clone()) }, Err(_) => Reply::Unavailable }
            }
            request => self.owner.as_mut().map_or(Reply::Rejected, |owner| owner.handle(peer, request)),
        }
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        if self.owner.is_none() {
            match self.completion.try_recv() {
                Ok(Ok(Some(owner))) => self.owner = Some(owner),
                Ok(Err(_)) | Ok(Ok(None)) => return Err(execution_app::Error::Unavailable),
                Err(mpsc::TryRecvError::Disconnected) => return Err(execution_app::Error::Unavailable),
                Err(mpsc::TryRecvError::Empty) => (),
            }
        }
        self.owner.as_mut().map_or(Ok(()), Handler::tick)
    }
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        let _ = self.commands.try_send(Command::Stop);
        if let Some(thread) = self.thread.take() { thread.join().map_err(|_| execution_app::Error::OutcomeUnknown)?; }
        if self.owner.is_none() { if let Ok(Ok(Some(owner))) = self.completion.try_recv() { self.owner = Some(owner); } }
        self.owner.as_mut().map_or(Ok(()), Handler::stop)
    }
}
