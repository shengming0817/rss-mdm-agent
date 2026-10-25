//! Personal Vault has an OS-user owner, independent of enterprise/AI runtime state.
mod native;
use passkey_vault::{Error, UnlockMethod, Vault};

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "PersonalVaultStatus")]
pub struct Status {
    #[serde(flatten)]
    pub vault: passkey_vault::Status,
    pub provider: Option<ProviderStatus>,
}
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub enabled: Option<bool>,
    pub index_pending: bool,
}
fn status(vault: &Vault) -> Result<Status, Error> {
    #[cfg(target_os = "macos")]
    let provider = {
        use macos_passkey_provider::index::{self, Availability};
        let state = index::status(vault)?;
        Some(ProviderStatus {
            enabled: match state.availability {
                Availability::Unknown => None,
                Availability::Enabled => Some(true),
                Availability::Disabled => Some(false),
            },
            index_pending: state.index_pending,
        })
    };
    #[cfg(not(target_os = "macos"))]
    let provider = None;
    Ok(Status {
        vault: vault.status()?,
        provider,
    })
}
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Manager, Runtime, WebviewWindow};

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "PersonalVaultOperation")]
pub enum Operation {
    Status {},
    Create {},
    Unlock {
        method: UnlockMethod,
    },
    Lock {},
    SetPassword {
        authorization: UnlockMethod,
    },
    RemovePassword {},
    RepairSystem {},
    RepairIndex {},
    Delete {
        credential_id: Vec<u8>,
        revision: u64,
    },
    Reset {
        confirmation: String,
    },
}
pub(super) type SharedVault = Arc<Mutex<Option<Arc<Vault>>>>;
pub struct PersonalVault {
    root: Result<PathBuf, Error>,
    vault: SharedVault,
    busy: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    observing: AtomicBool,
}
impl PersonalVault {
    pub fn new(root: &Path) -> Self {
        Self::from_root(Ok(root.into()))
    }
    pub fn from_root(root: Result<PathBuf, Error>) -> Self {
        let state = Self {
            root,
            vault: Arc::new(Mutex::new(None)),
            busy: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "macos")]
            observing: AtomicBool::new(false),
        };
        let _ = state.open();
        state
    }
    // ref: Tokio sync/once_cell.rs get_or_try_init@tokio-1.48.0: only cache success.
    // Disk initialization is synchronous; a short mutex serializes attempts without
    // introducing an async initialization owner or retaining a transient error.
    fn open(&self) -> std::result::Result<Arc<Vault>, Error> {
        let mut slot = self.vault.lock().map_err(|_| Error::Busy)?;
        if let Some(vault) = &*slot {
            return Ok(vault.clone());
        }
        let protection = Arc::new(passkey_vault::platform::OsProtection::new(native::verify));
        let vault = Arc::new(Vault::open(
            self.root.as_ref().map_err(|e| *e)?,
            protection,
        )?);
        *slot = Some(vault.clone());
        Ok(vault)
    }
    pub fn lock(&self) {
        lock_slot(&self.vault);
        native::cancel();
    }
    pub fn bind<R: Runtime>(&self, window: &WebviewWindow<R>) {
        #[cfg(target_os = "macos")]
        if self.observing.swap(true, Ordering::AcqRel) {
            return;
        }
        // Bind the stable slot, even when the first disk-open attempt failed.
        native::observe(window, self.vault.clone());
    }
}
pub(super) fn lock_slot(slot: &Mutex<Option<Arc<Vault>>>) {
    if let Ok(slot) = slot.lock() {
        if let Some(vault) = &*slot {
            if vault.lock().is_err() {
                eprintln!("personal vault cancellation persistence failed");
            }
        }
    }
}
impl Drop for PersonalVault {
    fn drop(&mut self) {
        self.lock();
    }
}
struct Busy(Arc<AtomicBool>);
impl Drop for Busy {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub async fn execute<R: Runtime>(
    window: WebviewWindow<R>,
    input: Operation,
) -> std::result::Result<Status, Error> {
    if window.label() != "main" {
        return Err(Error::Owner);
    }
    let state = window.state::<PersonalVault>();
    let vault = state.open()?;
    if matches!(input, Operation::Lock {}) {
        native::cancel();
        vault.lock()?;
        return status(&vault);
    }
    if matches!(input, Operation::Status {}) {
        return status(&vault);
    }
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err(Error::Busy);
    }
    let busy = Busy(state.busy.clone());
    let token = vault.begin_interaction();
    // Renderer cannot send passwords, caller identity, OS paths, key references or UV claims.
    tauri::async_runtime::spawn_blocking(move || {
        let _busy = busy;
        #[cfg(windows)]
        let changes = !matches!(&input, Operation::Status {} | Operation::Unlock { .. });
        #[cfg(target_os = "macos")]
        let repair_index = matches!(input, Operation::RepairIndex {});
        match input {
            Operation::Create {} => vault.create(&token)?,
            Operation::Unlock { method } => {
                let password = match method {
                    UnlockMethod::Password => Some(native::password("输入个人 Vault 口令")?),
                    UnlockMethod::System => None,
                };
                vault.unlock(method, password.as_deref().map(String::as_str), &token)?;
            }
            Operation::SetPassword { authorization } => {
                let current = match authorization {
                    UnlockMethod::Password => Some(native::password("输入当前个人 Vault 口令")?),
                    UnlockMethod::System => None,
                };
                let new = native::password("设置新口令：至少 12 个字符；不会修改网站 Passkey")?;
                let confirmation = native::password("再次输入新口令")?;
                vault.set_password(
                    authorization,
                    current.as_deref().map(String::as_str),
                    &new,
                    &confirmation,
                    &token,
                )?;
            }
            Operation::RemovePassword {} => vault.remove_password(&token)?,
            Operation::RepairIndex {} => {
                #[cfg(target_os = "macos")]
                macos_passkey_provider::index::refresh(vault.clone())?;
                #[cfg(not(target_os = "macos"))]
                return Err(Error::Unsupported);
            }
            Operation::RepairSystem {} => {
                let password = native::password("输入个人口令，以重新建立此 OS 用户的系统解锁")?;
                vault.repair_system(&password, &token)?;
            }
            Operation::Delete {
                credential_id,
                revision,
            } => vault.delete(&credential_id, revision, &token)?,
            Operation::Reset { confirmation } => vault.reset(&confirmation, &token)?,
            Operation::Status {} | Operation::Lock {} => return Err(Error::Input),
        }
        #[cfg(windows)]
        if changes {
            passkey_vault::platform::notify_management_change();
            if windows_passkey_provider::registration::invalidate_index().is_err() {
                eprintln!("personal passkey index refresh pending");
            }
        }
        #[cfg(target_os = "macos")]
        if !repair_index && macos_passkey_provider::index::refresh(vault.clone()).is_err() {
            eprintln!("personal passkey index refresh pending");
        }
        status(&vault)
    })
    .await
    .map_err(|_| Error::Cancelled)?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transient_open_failure_retries_in_the_same_owner_and_keeps_the_event_slot() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-vault-open-{}", uuid::Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let held = platform_private_storage::create_new(&root.join("vault.lock")).unwrap();
        held.try_lock().unwrap();
        let state = PersonalVault::new(&root);
        let observed_slot = state.vault.clone();
        assert!(state.vault.lock().unwrap().is_none());
        assert_eq!(state.open().err(), Some(Error::Busy));
        drop(held);
        let opened = state.open().unwrap();
        assert!(Arc::ptr_eq(&opened, &state.open().unwrap()));
        assert!(Arc::ptr_eq(
            &opened,
            observed_slot.lock().unwrap().as_ref().unwrap()
        ));
        // A pre-existing observer sees the later successful initialization.
        let token = opened.begin_interaction();
        lock_slot(&observed_slot);
        assert_eq!(opened.create(&token), Err(Error::Cancelled));
        assert!(!opened.status().unwrap().initialized);
        drop(state);
        drop(opened);
        drop(observed_slot);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn personal_commands_work_without_an_enterprise_runtime() {
        use tauri::test::mock_builder;
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-personal-ipc-{}", uuid::Uuid::new_v4()));
        let app = super::super::ipc::register(mock_builder())
            .manage(PersonalVault::new(&root))
            .build(tauri::generate_context!())
            .unwrap();
        assert!(app
            .try_state::<super::super::runtime::DesktopRuntime>()
            .is_none());
        let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let status =
            tauri::async_runtime::block_on(execute(main.clone(), Operation::Status {})).unwrap();
        assert!(!status.vault.initialized);
        let status = tauri::async_runtime::block_on(execute(main, Operation::Lock {})).unwrap();
        assert!(!status.vault.unlocked);
        let other = tauri::WebviewWindowBuilder::new(&app, "other", Default::default())
            .build()
            .unwrap();
        assert_eq!(
            tauri::async_runtime::block_on(execute(other, Operation::Status {})).err(),
            Some(Error::Owner)
        );
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn management_contract_cannot_transport_secrets_or_identity() {
        for input in [
            serde_json::json!({"kind":"unlock","method":"password","password":"CANARY_SECRET"}),
            serde_json::json!({"kind":"unlock","method":"system","verified":true}),
            serde_json::json!({"kind":"create","owner":"someone-else"}),
            serde_json::json!({"kind":"sign","bytes":[1,2,3]}),
            serde_json::json!({"kind":"set_password","authorization":"system","rootKey":"CANARY_SECRET"}),
        ] {
            assert!(serde_json::from_value::<Operation>(input).is_err());
        }
        let schema = serde_json::to_string(&schemars::schema_for!(Operation)).unwrap();
        for field in [
            "rootKey",
            "privateKey",
            "currentPassword",
            "newPassword",
            "owner",
            "path",
        ] {
            assert!(!schema.contains(field));
        }
    }
}
