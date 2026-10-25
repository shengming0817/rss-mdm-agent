use crate::{Error, HostUi, Operation, OperationKind, Operations, Result, Status};
use passkey_vault::{
    ceremony::{Authorizer, Context, Decision},
    UnlockMethod, Vault,
};
use std::{
    fs::File,
    path::Path,
    sync::{Arc, Mutex},
};
use windows_sys::Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*};
mod api;
mod com;
mod identity;
pub mod registration;
mod ui;
pub use ui::NativeUi;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
struct Handle(HANDLE);
// SAFETY: kernel handles are process-scoped; access is synchronized by the kernel.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Own one COM server in a current-user process. Never run in the system service.
pub struct ProviderRuntime {
    operations: Operations,
    ui: Arc<dyn HostUi>,
    cancel: Handle,
    stop_event: Handle,
    _lease: File,
    owner: String,
    root: std::path::PathBuf,
    stop: std::sync::atomic::AtomicBool,
}
impl ProviderRuntime {
    pub fn new(ui: Arc<dyn HostUi>) -> Result<Arc<Self>> {
        identity::platform()?;
        registration::require_identity()?;
        registration::verify_owner()?;
        let owner = passkey_vault::platform::owner()?;
        let root = passkey_vault::platform::default_root()?;
        let directory = platform_private_storage::PrivateDirectory::create(&root)
            .map_err(|_| Error::Storage)?;
        let lease = match directory.create_file(Path::new("provider.lock")) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => directory
                .open_file(Path::new("provider.lock"))
                .map_err(|_| Error::Owner)?,
            Err(_) => return Err(Error::Storage),
        };
        lease.try_lock().map_err(|_| Error::Busy)?;
        // Manual reset is only performed before accepting a new, explicitly interactive request.
        // SAFETY: current-user object name; default token DACL; named handle stays live.
        let event = unsafe {
            CreateEventW(
                std::ptr::null(),
                1,
                0,
                wide(&format!("Local\\RSS.PersonalPasskeys.Cancel.{owner}")).as_ptr(),
            )
        };
        if event.is_null() {
            return Err(Error::SystemUnavailable);
        }
        let stop_event = unsafe {
            CreateEventW(
                std::ptr::null(),
                1,
                0,
                wide(&format!("Local\\RSS.PersonalPasskeys.Stop.{owner}")).as_ptr(),
            )
        };
        if stop_event.is_null() {
            unsafe {
                CloseHandle(event);
            }
            return Err(Error::SystemUnavailable);
        }
        unsafe {
            ResetEvent(stop_event);
        }
        Ok(Arc::new(Self {
            stop_event: Handle(stop_event),
            operations: Operations::default(),
            ui,
            cancel: Handle(event),
            _lease: lease,
            owner,
            root,
            stop: std::sync::atomic::AtomicBool::new(false),
        }))
    }
    pub fn run(self: Arc<Self>) -> Result<()> {
        com::run(self)
    }
    pub fn shutdown(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        self.operations.shutdown();
        unsafe {
            SetEvent(self.cancel.0);
        }
    }
    pub fn status(&self) -> Result<Status> {
        let state = registration::state()?;
        Ok(Status {
            registered: state.is_some(),
            enabled: state == Some(true),
            locked: true,
        })
    }
    fn check(&self, op: &Operation) -> Result<()> {
        op.check()?;
        if unsafe { WaitForSingleObject(self.cancel.0, 0) } != WAIT_TIMEOUT
            || passkey_vault::platform::owner()? != self.owner
        {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    fn execute(&self, kind: OperationKind, request: &api::Request) -> Result<Vec<u8>> {
        // SAFETY: COM validates marshal buffers; additionally constrain before copying or decoding.
        if request.encoding != 1
            || request.encoded.is_null()
            || request.signature.is_null()
            || !(1..=131072).contains(&request.encoded_len)
            || !(1..=1024).contains(&request.signature_len)
        {
            return Err(Error::Input);
        }
        identity::caller(request.hwnd, &self.owner)?;
        let encoded =
            unsafe { std::slice::from_raw_parts(request.encoded, request.encoded_len as usize) }
                .to_vec();
        let signature = unsafe {
            std::slice::from_raw_parts(request.signature, request.signature_len as usize)
        };
        let api = api::Api::get()?;
        crate::signature::verify(&api.key(false)?, &encoded, signature)?;
        let decoded = crate::protocol::Request::decode(kind, &encoded)?;
        let id = identity::guid_bytes(&request.transaction);
        let op = self.operations.begin(id, kind, &encoded)?;
        struct Finish<'a>(&'a Operations, Arc<Operation>);
        impl Drop for Finish<'_> {
            fn drop(&mut self) {
                let _ = self.0.finish(&self.1);
            }
        }
        let _finish = Finish(&self.operations, op.clone());
        // Always finish and lock on error; RAII prevents a forgotten return retaining admission.
        let result = (|| {
            unsafe {
                ResetEvent(self.cancel.0);
            }
            self.check(&op)?;
            if !self.status()?.enabled {
                return Err(Error::Authentication);
            }
            let uv_key = api.key(true)?;
            let uv = api.verify_user(request.hwnd, &request.transaction, decoded.rp())?;
            self.check(&op)?;
            crate::signature::verify(&uv_key, &encoded, &uv)?;
            let grant = Arc::new(Mutex::new(Some(())));
            let approval = grant.clone();
            let system = Arc::new(passkey_vault::platform::OsProtection::new(move || {
                approval
                    .lock()
                    .map_err(|_| Error::Busy)?
                    .take()
                    .ok_or(Error::Authentication)
            }));
            let vault = Vault::open(&self.root, system)?;
            let token = vault.begin_interaction();
            let status = vault.status()?;
            if !status.initialized {
                self.ui.open_management()?;
                return Err(Error::NotFound);
            } else {
                vault.unlock(UnlockMethod::System, None, &token)?;
            }
            let transaction = Approval {
                runtime: self,
                op: &op,
                kind,
                encoded: &encoded,
            };
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|_| Error::SystemUnavailable)?;
            let response = runtime.block_on(async {
                match decoded {
                    crate::protocol::Request::Create(r) => {
                        crate::protocol::encode(&vault.register(r, &transaction).await?)
                    }
                    crate::protocol::Request::Assert(r) => {
                        crate::protocol::encode(&vault.assert(r, &transaction).await?)
                    }
                }
            });
            // Cache is derived. Its failure must not roll back an already published credential.
            if crate::cache::update(|| {
                let snapshot = vault.index_snapshot()?;
                registration::sync_cache(&snapshot.entries)?;
                vault.acknowledge_index(&snapshot)
            })
            .is_err()
            {
                eprintln!("passkey cache synchronization pending");
            }
            self.check(&op)?;
            // Drop zeroizes the session. Explicit lock would broadcast cancellation to this request.
            drop(vault);
            response
        })();
        let finish = self.operations.finish(&op);
        finish?;
        result
    }
}
impl Drop for ProviderRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}
struct Approval<'a> {
    runtime: &'a ProviderRuntime,
    op: &'a Operation,
    kind: OperationKind,
    encoded: &'a [u8],
}
impl Authorizer for Approval<'_> {
    fn authorize(&self, request: &Context<'_>) -> Result<Decision> {
        self.check()?;
        if !self.op.matches(self.kind, self.encoded) {
            return Err(Error::Authentication);
        }
        let selected = self.runtime.ui.choose(request, self.op)?;
        self.check()?;
        Ok(Decision {
            selected,
            verified: true,
        })
    }
    fn check(&self) -> Result<()> {
        self.runtime.check(self.op)
    }
}
