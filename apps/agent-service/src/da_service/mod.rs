//! The existing system service drives DA on its own bounded worker. An unavailable DA
//! endpoint cannot stall authenticated IPC, MDM enrollment, or the ordinary task owner.
#[cfg(target_os = "macos")]
mod macos;
pub(crate) mod policy;
use crate::{deployment::Deployment, DeviceSecrets, SystemClock};
use agent_client::{da, Error};
use execution_ipc::host::{
    Handler, Peer, PeerPolicy, Readiness, Reply, Request, ServiceStatus, SessionFence,
};
use std::{
    path::Path,
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};
use uuid::Uuid;

pub(crate) fn preflight(d: &Deployment) -> Result<(), Error> {
    if let Ok(backend) = d.modules.da.backend() {
        da::inspect(&d.da_root()?, backend, d.installation_id)?;
    }
    Ok(())
}
pub(crate) fn status(d: &Deployment) -> Result<Option<da::Status>, Error> {
    let Ok(backend) = d.modules.da.backend() else {
        return Ok(None);
    };
    da::inspect(&d.da_root()?, backend, d.installation_id)
}
fn open(d: &Deployment) -> Result<da::Client<DeviceSecrets, SystemClock>, Error> {
    let root = d.da_root()?;
    let backend = d.modules.da.backend()?;
    let namespace = crate::installation::hash(
        &serde_json::to_vec(&("da", d.installation_id, backend.instance, backend.tenant))
            .map_err(|_| Error::Configuration)?,
    );
    platform_private_storage::directory(&root)?;
    let secrets = root.join("secrets");
    platform_private_storage::directory(&secrets)?;
    let store = DeviceSecrets::open(&secrets, &namespace)?;
    if root.join("initialized").try_exists()? {
        store.verify_storage(&root)?;
    } else {
        store.bind_storage(&root)?;
    }
    da::Client::open(
        &root,
        backend.clone(),
        d.installation_id,
        store,
        SystemClock::new()?,
    )
}

pub(crate) fn prepare(d: &Deployment, operation: Uuid, file: &Path) -> Result<(), Error> {
    d.require_service()?;
    let request = open(d)?.prepare(operation)?;
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&request).map_err(|_| Error::Protocol)?);
    match platform_private_storage::write_new(file, &bytes) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let old = zeroize::Zeroizing::new(platform_private_storage::read(file, 65536)?);
            if *old == *bytes {
                Ok(())
            } else {
                Err(Error::Conflict)
            }
        }
        Err(e) => Err(e.into()),
    }
}
pub(crate) fn import(d: &Deployment, file: &Path) -> Result<(), Error> {
    d.require_service()?;
    let bytes = installation_security::read_protected(file).map_err(|_| Error::Denied)?;
    open(d)?.import(serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?)
}
pub(crate) fn renew(d: &Deployment) -> Result<(), Error> {
    d.require_service()?;
    open(d)?.start_renewal()?;
    Ok(())
}

pub(crate) fn assemble(d: Deployment) -> Result<Box<dyn Handler>, Error> {
    if !d.modules.da.enabled() {
        return d.mdm_owner();
    }
    let mdm = if d.modules.mdm.enabled() {
        Some(d.clone().mdm_owner()?)
    } else {
        None
    };
    let policy = d.clients.clone();
    let (stop, rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("device-access".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| Error::Unavailable)?;
            loop {
                let run = || -> Result<(), Error> {
                    let da = open(&d)?;
                    runtime.block_on(async {
                        let mut policy = policy::Machine::observe(d.da_root()?).await;
                        da.drive(&mut policy).await
                    })
                };
                let delay = match run() {
                    Ok(()) => 30,
                    Err(Error::Conflict) => 2,
                    Err(error) => {
                        eprintln!("device_access: {error}");
                        10
                    }
                };
                if rx.recv_timeout(Duration::from_secs(delay))
                    != Err(mpsc::RecvTimeoutError::Timeout)
                {
                    return Ok(());
                }
            }
        })?;
    Ok(Box::new(Service {
        mdm,
        policy,
        stop: Some(stop),
        worker: Some(worker),
    }))
}
struct Service {
    mdm: Option<Box<dyn Handler>>,
    policy: PeerPolicy,
    stop: Option<mpsc::Sender<()>>,
    worker: Option<thread::JoinHandle<Result<(), Error>>>,
}
impl Handler for Service {
    fn session_fence(&self) -> Option<Arc<dyn SessionFence>> {
        self.mdm.as_ref().and_then(|owner| owner.session_fence())
    }
    fn peer_policy(&self) -> Option<PeerPolicy> {
        self.mdm
            .as_ref()
            .and_then(|owner| owner.peer_policy())
            .or_else(|| Some(self.policy.clone()))
    }
    fn register_helper(&mut self, peer: &Peer) -> Result<(), execution_app::Error> {
        self.mdm
            .as_mut()
            .ok_or(execution_app::Error::Denied)?
            .register_helper(peer)
    }
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply {
        if let Some(mdm) = &mut self.mdm {
            return mdm.handle(peer, request);
        }
        if peer.authenticate(&self.policy).is_err() {
            return Reply::Rejected;
        }
        match request {
            Request::ServiceStatus {} => Reply::ServiceStatus {
                value: ServiceStatus::new(
                    if self.worker.as_ref().is_some_and(|w| !w.is_finished()) {
                        Readiness::Ready
                    } else {
                        Readiness::NotReady
                    },
                ),
            },
            _ => Reply::Rejected,
        }
    }
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        if let Some(mdm) = &mut self.mdm {
            mdm.tick()?;
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        drop(self.stop.take());
        let mdm = self.mdm.as_mut().map(|owner| owner.stop()).transpose();
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| execution_app::Error::Unavailable)?
                .map_err(|_| execution_app::Error::Unavailable)?;
        }
        mdm.map(|_| ())
    }
}
