//! Personal Vault has an OS-user owner, independent of enterprise/AI runtime state.
mod native;
use passkey_vault::{Error, Status, UnlockMethod, Vault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{Manager, Runtime, WebviewWindow};

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "PersonalVaultOperation")]
pub enum Operation {
    Status,
    Create,
    Unlock {
        method: UnlockMethod,
    },
    Lock,
    SetPassword {
        authorization: UnlockMethod,
    },
    RemovePassword,
    RepairSystem,
    Delete {
        credential_id: Vec<u8>,
        revision: u64,
    },
    Reset {
        confirmation: String,
    },
}
pub struct PersonalVault {
    pub vault: std::result::Result<Arc<Vault>, Error>,
    busy: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    observing: AtomicBool,
}
impl PersonalVault {
    pub fn new(root: &Path) -> Self {
        let protection = Arc::new(passkey_vault::platform::OsProtection::new(native::verify));
        Self {
            vault: Vault::open(root, protection).map(Arc::new),
            busy: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "macos")]
            observing: AtomicBool::new(false),
        }
    }
    pub fn lock(&self) {
        if let Ok(vault) = &self.vault {
            vault.lock();
        }
        native::cancel();
    }
    pub fn bind<R: Runtime>(&self, window: &WebviewWindow<R>) {
        #[cfg(target_os = "macos")]
        if self.observing.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(vault) = &self.vault {
            native::observe(window, vault.clone());
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
    let vault = state.vault.as_ref().map_err(|e| *e)?.clone();
    if matches!(input, Operation::Lock) {
        state.lock();
        return vault.status();
    }
    if matches!(input, Operation::Status) {
        return vault.status();
    }
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err(Error::Busy);
    }
    let busy = Busy(state.busy.clone());
    let token = vault.begin_interaction();
    // Renderer cannot send passwords, caller identity, OS paths, key references or UV claims.
    tauri::async_runtime::spawn_blocking(move || {
        let _busy = busy;
        match input {
            Operation::Create => vault.create(&token)?,
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
            Operation::RemovePassword => vault.remove_password(&token)?,
            Operation::RepairSystem => {
                let password = native::password("输入个人口令，以重新建立此 OS 用户的系统解锁")?;
                vault.repair_system(&password, &token)?;
            }
            Operation::Delete {
                credential_id,
                revision,
            } => vault.delete(&credential_id, revision, &token)?,
            Operation::Reset { confirmation } => vault.reset(&confirmation, &token)?,
            Operation::Status | Operation::Lock => return Err(Error::Input),
        }
        vault.status()
    })
    .await
    .map_err(|_| Error::Cancelled)?
}

#[cfg(test)]
mod tests {
    use super::*;
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
