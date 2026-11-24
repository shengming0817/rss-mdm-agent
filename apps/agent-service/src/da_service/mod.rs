//! The existing system service drives DA on its own bounded worker. An unavailable DA
//! endpoint cannot stall authenticated IPC, MDM enrollment, or the ordinary task owner.
#[cfg(any(windows, target_os = "macos", test))]
mod attempt;
#[cfg(windows)]
mod delivery;
#[cfg(windows)]
mod login;
#[cfg(target_os = "macos")]
pub(crate) mod mac_accounts;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use crate::service::platform_sso::da as native;
pub(crate) mod policy;
#[cfg(windows)]
mod windows;
use crate::{deployment::Deployment, DeviceSecrets, SystemClock};
use agent_client::{da, Clock, Error};
use execution_ipc::host::{
    Handler, Peer, PeerPolicy, Readiness, Reply, Request, ServiceStatus, SessionFence,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use uuid::Uuid;

fn channel_initialized(d: &Deployment) -> Result<bool, Error> {
    let root = d.da_root()?;
    match root.symlink_metadata() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
        Ok(meta) if meta.is_dir() => Ok(std::fs::read_dir(root)?.next().transpose()?.is_some()),
        Ok(_) => Err(Error::Storage),
    }
}
pub(crate) fn has_retained_identity(d: &Deployment) -> Result<bool, Error> {
    da::has_live_credentials(&d.da_root()?, d.installation_id)
}
pub(crate) fn preflight(d: &Deployment) -> Result<(), Error> {
    if let Ok(backend) = d.modules.da.backend() {
        let root = d.da_root()?;
        if da::inspect(
            &root,
            backend,
            d.installation_id,
            SystemClock::new()?.now()?,
        )?
        .is_some()
        {
            let provider = provider(d, &root, backend)?;
            provider.verify_storage(&root)?;
            da::validate_keys(&root, backend, d.installation_id, &provider)?;
        }
    }
    Ok(())
}
pub(crate) fn status(d: &Deployment) -> Result<Option<da::Status>, Error> {
    let Ok(backend) = d.modules.da.backend() else {
        return Ok(None);
    };
    da::inspect(
        &d.da_root()?,
        backend,
        d.installation_id,
        SystemClock::new()?.now()?,
    )
}
fn provider(d: &Deployment, root: &Path, backend: &da::Backend) -> Result<DeviceSecrets, Error> {
    let namespace = crate::installation::hash(
        &serde_json::to_vec(&("da", d.installation_id, backend.instance, backend.tenant))
            .map_err(|_| Error::Configuration)?,
    );
    let secrets = root.join("secrets");
    DeviceSecrets::open(&secrets, &namespace)
}
#[cfg(target_os = "macos")]
pub(crate) fn release_installation(d: &Deployment) -> Result<bool, Error> {
    let root = d.da_root()?;
    let store = provider(d, &root, d.modules.da.backend()?)?;
    Ok(store.0.retire(&[])?)
}
fn open(d: &Deployment) -> Result<da::Client<DeviceSecrets, SystemClock>, Error> {
    let root = d.da_root()?;
    let backend = d.modules.da.backend()?;
    platform_private_storage::directory(root.parent().ok_or(Error::Configuration)?)?;
    platform_private_storage::directory(&root)?;
    platform_private_storage::directory(&root.join("secrets"))?;
    let store = provider(d, &root, backend)?;
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
    open(d)?.prepare(operation, |request| {
        let bytes =
            zeroize::Zeroizing::new(serde_json::to_vec(&request).map_err(|_| Error::Protocol)?);
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
    })
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
    #[cfg(windows)]
    if let Err(e) = crate::access_component::prepare_store(&d) {
        return Ok(Deployment::startup_diagnostic(d.clients.clone(), e));
    }
    if !d.modules.da.enabled() {
        return d.mdm_owner();
    }
    let mdm = if d.modules.mdm.enabled() {
        Some(d.clone().mdm_owner()?)
    } else {
        None
    };
    #[cfg(windows)]
    let login = Arc::new(login::Login::new(
        &d,
        mdm.as_ref().and_then(|owner| owner.session_fence()),
    )?);
    #[cfg(windows)]
    let worker_login = login.clone();
    #[cfg(target_os = "macos")]
    let (endpoint, mut native) =
        native::Endpoint::new(&d, mdm.as_ref().and_then(|h| h.session_fence()));
    let policy = d.clients.clone();
    let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
    let ready = Arc::new(AtomicBool::new(false));
    let available = ready.clone();
    let root = d.da_root()?;
    let worker = thread::Builder::new()
        .name("device-access".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| Error::Unavailable)?;
            runtime.block_on(async move {
                let work = async {
                    loop {
                        // Only explicit provisioning may initialize a DA identity namespace.
                        let result = match channel_initialized(&d).and_then(|retained| {
                            if retained {
                                open(&d).map(Some)
                            } else {
                                Ok(None)
                            }
                        }) {
                            Ok(None) => {
                                available.store(true, Ordering::Release);
                                tokio::time::sleep(Duration::from_secs(2)).await;
                                continue;
                            }
                            Ok(Some(client)) => {
                                available.store(true, Ordering::Release);
                                let mut policy = policy::Machine::observe(
                                    root.clone(),
                                    #[cfg(windows)]
                                    crate::access_worker::root(&d.execution),
                                    #[cfg(windows)]
                                    d.installation_id,
                                )
                                .await;
                                async {
                                    #[cfg(not(target_os="macos"))]
                                    let result = client.drive(&mut policy).await;
                                    #[cfg(target_os="macos")]
                                    let result = {
                                        let drive=client.drive(&mut policy);
                                        tokio::pin!(drive);
                                        loop {
                                            tokio::select! {
                                                result = &mut drive => break result,
                                                Some(cmd) = native.rx.recv() => native.handle(&d,&client,cmd).await,
                                            }
                                        }
                                    };
                                    #[cfg(target_os="macos")]
                                    {
                                        native.maintenance(&d,&client).await?;
                                        if client.registered()? {
                                            let setup=native.accounts();
                                            let delivery=mac_accounts::drive(&d,&client,&setup);
                                            tokio::pin!(delivery);
                                            let operations=loop {tokio::select! {
                                                r=&mut delivery=>break r,
                                                Some(cmd)=native.rx.recv()=>native.handle(&d,&client,cmd).await,
                                            }};
                                            client.record_delivery(operations)?;
                                        }
                                    }

                                    #[cfg(windows)]
                                    {
                                        let operations = async {
                                            if let Some(r) = client.revocation().await? {
                                                let native =
                                                    crate::access_worker::root(&d.execution);
                                                device_access::Store::open(
                                                    &native,
                                                    d.installation_id,
                                                )
                                                .map_err(|_| Error::Storage)?
                                                .retire_registration(device_access::Channel::Da)
                                                .map_err(|_| Error::Storage)?;
                                                worker_login.reset()?;
                                                client.retire(&r)?;
                                            } else if cfg!(target_arch = "x86_64") {
                                                let operations = delivery::drive(
                                                    &d,
                                                    &client,
                                                    &worker_login,
                                                    policy.local_rss,
                                                )
                                                .await;
                                                operations?;
                                            }
                                            Ok(())
                                        }
                                        .await;
                                        client.record_delivery(operations)?;
                                        result
                                    }
                                    #[cfg(not(windows))]
                                    result
                                }
                                .await
                            }
                            Err(e) => {
                                available.store(false, Ordering::Release);
                                Err(e)
                            }
                        };
                        let delay = match result {
                            Ok(()) => 30,
                            Err(Error::Conflict) => 2,
                            Err(e) => {
                                eprintln!("device_access: {e}");
                                10
                            }
                        };
                        #[cfg(windows)]
                        tokio::select! {_=worker_login.wake.notified()=>(),_=tokio::time::sleep(Duration::from_secs(delay))=>()}
                        #[cfg(target_os="macos")]
                        {
                            let setup = native.accounts();
                            let wait = tokio::time::sleep(Duration::from_secs(delay));
                            tokio::pin!(wait);
                            loop {
                                tokio::select! {
                                    _=&mut wait => break,
                                    _=setup.wake.notified()=>break,
                                    Some(cmd)=native.rx.recv() => {
                                        match channel_initialized(&d).and_then(|live| if live { open(&d) } else {Err(Error::Identity)}) {
                                            Ok(client) => native.handle(&d, &client, cmd).await,
                                            Err(_) => drop(cmd),
                                        }
                                    }
                                }
                            }
                        }
                        #[cfg(not(any(target_os="macos",windows)))]
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                    }
                };
                tokio::select! {biased; _=&mut stopped=>Ok(()), result=work=>result}
            })
        })?;
    Ok(Box::new(Service {
        mdm,
        #[cfg(target_os = "macos")]
        endpoint,
        #[cfg(windows)]
        login,
        ready,
        policy,
        stop: Some(stop),
        worker: Some(worker),
    }))
}
#[cfg(target_os = "macos")]
fn settle_mac_request(
    request: Request,
    native: impl FnOnce() -> Reply + Send + 'static,
    mdm: impl FnOnce(Request) -> Option<execution_ipc::host::DeferredReply>,
) -> execution_ipc::host::DeferredReply {
    let other = if matches!(
        request,
        Request::BindContext { .. }
            | Request::DeviceAccess {
                input: execution_ipc::device_access::Operation::Tasks {},
                ..
            }
    ) {
        mdm(request)
    } else {
        None
    };
    Box::new(move || {
        let result = native();
        match (other.map(|wait| wait()), result) {
            (Some(bound @ Reply::ContextBound { .. }), _) => bound,
            (
                Some(Reply::DeviceAccess {
                    value: execution_ipc::device_access::Response::Tasks { mut tasks },
                }),
                native,
            ) => {
                if let Reply::DeviceAccess {
                    value: execution_ipc::device_access::Response::Tasks { tasks: da },
                } = native
                {
                    tasks.extend(da);
                }
                Reply::DeviceAccess {
                    value: execution_ipc::device_access::Response::Tasks { tasks },
                }
            }
            (_, result) => result,
        }
    })
}
#[cfg(all(test, target_os = "macos"))]
#[test]
fn shared_login_preserves_mdm_context_and_native_rejection_never_retries_with_mdm() {
    let id = execution_contract::Id::new("login-1").unwrap();
    let login = Request::BindContext {
        value: execution_ipc::host::NativeContextSession::device(id.clone()),
        expected: None,
    };
    let bound = settle_mac_request(
        login,
        || Reply::Unavailable,
        |_| {
            Some(Box::new(move || Reply::ContextBound {
                value: execution_contract::NativeContextRef {
                    id: id.clone(),
                    generation: id,
                },
            }))
        },
    );
    assert!(matches!(bound(), Reply::ContextBound { .. }));
    let callback = Request::PlatformSso {
        input: execution_ipc::platform_sso::Operation::Status {
            operation: uuid::Uuid::new_v4(),
            scope: rss_mdm_agent_wire::platform_sso::Kind::Device,
        },
    };
    assert!(matches!(
        settle_mac_request(callback, || Reply::Rejected, |_| panic!("second issuer"))(),
        Reply::Rejected
    ));
}
#[cfg(all(test, target_os = "macos"))]
#[test]
fn account_tasks_merge_after_enqueue_and_a_denied_da_input_is_never_forwarded() {
    use execution_ipc::device_access::{Operation, Response, SetupTask};
    let id = execution_contract::Id::new("login").unwrap();
    let request = |input| Request::DeviceAccess {
        context: execution_contract::NativeContextRef {
            id: id.clone(),
            generation: id.clone(),
        },
        origin: execution_ipc::host::ClientOrigin::Desktop {},
        input,
        mac_password: None,
    };
    let tasks = || Reply::DeviceAccess {
        value: Response::Tasks {
            tasks: vec![SetupTask {
                task: Uuid::new_v4(),
                attempt: Uuid::new_v4(),
                binding: Uuid::new_v4(),
                create_name: Some("employee".into()),
            }],
        },
    };
    let called = Arc::new(AtomicBool::new(false));
    let read = called.clone();
    let waiter = settle_mac_request(request(Operation::Tasks {}), tasks, move |_| {
        Some(Box::new(move || {
            read.store(true, Ordering::Release);
            tasks()
        }))
    });
    assert!(!called.load(Ordering::Acquire));
    assert!(matches!(waiter(),Reply::DeviceAccess{value:Response::Tasks{tasks}} if tasks.len()==2));
    assert!(called.load(Ordering::Acquire));
    assert!(matches!(
        settle_mac_request(
            request(Operation::PrepareMac {
                task: Uuid::new_v4(),
                attempt: Uuid::new_v4()
            }),
            || Reply::Rejected,
            |_| panic!("password forwarded")
        )(),
        Reply::Rejected
    ));
}
struct Service {
    #[cfg(target_os = "macos")]
    endpoint: native::Endpoint,
    #[cfg(windows)]
    login: Arc<login::Login>,
    mdm: Option<Box<dyn Handler>>,
    policy: PeerPolicy,
    ready: Arc<AtomicBool>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    worker: Option<thread::JoinHandle<Result<(), Error>>>,
}
#[cfg(target_os = "macos")]
impl Service {
    fn begin_native(
        &mut self,
        peer: &Peer,
        request: &Request,
    ) -> Option<execution_ipc::host::DeferredReply> {
        let pending = self.endpoint.start(peer, request)?;
        Some(settle_mac_request(
            request.clone(),
            move || pending.map_or_else(|e| e, |call| call.wait()),
            |r| {
                let mdm = self.mdm.as_mut()?;
                let bytes = execution_ipc::host::encode(r.clone()).ok()?;
                Some(mdm.deferred_reply(peer, &bytes).unwrap_or_else(|| {
                    let immediate = mdm.handle(peer, r);
                    Box::new(move || immediate)
                }))
            },
        ))
    }
}
impl Service {
    fn handle_immediate(&mut self, peer: &Peer, request: Request) -> Reply {
        let da_ready = self.ready.load(Ordering::Acquire)
            && self
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished());
        #[cfg(windows)]
        if let Some(reply) = self.login.handle(peer, &request) {
            use execution_ipc::device_access::Response;
            match (&mut self.mdm, reply) {
                (None, reply) => return reply,
                (Some(mdm), Reply::ContextBound { value }) => {
                    let original = mdm.handle(peer, request);
                    return if matches!(original, Reply::ContextBound { .. }) {
                        original
                    } else {
                        Reply::ContextBound { value }
                    };
                }
                (
                    Some(mdm),
                    Reply::DeviceAccess {
                        value: Response::Tasks { mut tasks },
                    },
                ) => {
                    tasks.extend(self.login.mdm_tasks(peer, &request, mdm.as_mut(), true));
                    return Reply::DeviceAccess {
                        value: Response::Tasks { tasks },
                    };
                }
                (Some(mdm), Reply::DaVerificationPending {}) => {
                    self.login.mdm_tasks(peer, &request, mdm.as_mut(), false);
                    return Reply::DaVerificationPending {};
                }
                (Some(mdm), Reply::Rejected)
                    if matches!(
                        request,
                        Request::DeviceAccess {
                            input: execution_ipc::device_access::Operation::Tasks {},
                            ..
                        }
                    ) =>
                {
                    return Reply::DeviceAccess {
                        value: Response::Tasks {
                            tasks: self.login.mdm_tasks(peer, &request, mdm.as_mut(), true),
                        },
                    };
                }
                (Some(_), Reply::Rejected) if matches!(request, Request::BindContext { .. }) => (),
                (_, reply) => return reply,
            }
        }
        if let Some(mdm) = &mut self.mdm {
            let mut reply = mdm.handle(peer, request);
            if let Reply::ServiceStatus { value } = &mut reply {
                if !da_ready {
                    value.readiness = Readiness::NotReady;
                }
            }
            return reply;
        }
        if peer.authenticate(&self.policy).is_err() {
            return Reply::Rejected;
        }
        match request {
            Request::ServiceStatus {} => Reply::ServiceStatus {
                value: ServiceStatus::new(if da_ready {
                    Readiness::Ready
                } else {
                    Readiness::NotReady
                }),
            },
            _ => Reply::Rejected,
        }
    }
}
impl Handler for Service {
    #[cfg(target_os = "macos")]
    fn deferred_reply(
        &mut self,
        peer: &Peer,
        bytes: &[u8],
    ) -> Option<execution_ipc::host::DeferredReply> {
        let request = execution_ipc::host::decode_request(bytes)?;
        if let Some(wait) = self.begin_native(peer, &request) {
            return Some(wait);
        }
        let reply = self.handle_immediate(peer, request);
        Some(Box::new(move || reply))
    }
    fn session_fence(&self) -> Option<Arc<dyn SessionFence>> {
        #[cfg(windows)]
        {
            Some(self.login.clone())
        }
        #[cfg(target_os = "macos")]
        {
            Some(self.endpoint.fence.clone())
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            self.mdm.as_ref().and_then(|owner| owner.session_fence())
        }
    }
    fn peer_policy(&self) -> Option<PeerPolicy> {
        let policy = self
            .mdm
            .as_ref()
            .and_then(|h| h.peer_policy())
            .unwrap_or_else(|| self.policy.clone());
        #[cfg(target_os = "macos")]
        let policy = {
            let mut policy = policy;
            self.endpoint.policy(&mut policy);
            policy
        };
        Some(policy)
    }
    fn register_helper(&mut self, peer: &Peer) -> Result<(), execution_app::Error> {
        #[cfg(target_os = "macos")]
        if self.mdm.is_none() {
            return self.endpoint.helper(peer);
        }
        self.mdm
            .as_mut()
            .ok_or(execution_app::Error::Denied)?
            .register_helper(peer)
    }
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply {
        #[cfg(target_os = "macos")]
        if let Some(wait) = self.begin_native(peer, &request) {
            return wait();
        }
        self.handle_immediate(peer, request)
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
