//! Native-only credential acquisition. The WebView receives an opaque reference, never a secret.
use super::users::Users;
use crate::self_service::{error, Result};
use futures_util::{SinkExt, StreamExt};
use security_framework::passwords::{get_generic_password, set_generic_password};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::net::{UnixListener, UnixStream};
use tokio_util::{
    codec::{Framed, LinesCodec},
    sync::CancellationToken,
};
mod vault;
pub use vault::Vault;
const SERVICE: &str = "RSS MDM Agent test-user connections";
fn unavailable() -> crate::self_service::ServiceError {
    error(
        "authentication_required",
        "凭据不可用，请更新所选连接的认证来源",
    )
}

pub async fn enter<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    users: Arc<Mutex<Users>>,
    vault: Arc<Mutex<Vault>>,
    generation: String,
) -> Result<String> {
    users
        .lock()
        .map_err(|_| unavailable())?
        .require(&generation)?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSSecureTextField};
        use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
        let result = (|| {
            let mtm = MainThreadMarker::new().ok_or_else(unavailable)?;
            let alert = NSAlert::new(mtm);
            alert.setMessageText(&NSString::from_str("为当前测试用户保存 API 凭据"));
            alert.setInformativeText(&NSString::from_str(
                "凭据将保存到 macOS 钥匙串。网页只接收凭据引用。",
            ));
            alert.addButtonWithTitle(&NSString::from_str("保存"));
            alert.addButtonWithTitle(&NSString::from_str("取消"));
            let field = NSSecureTextField::new(mtm);
            field.setFrame(NSRect::new(NSPoint::new(0., 0.), NSSize::new(360., 24.)));
            alert.setAccessoryView(Some(&field));
            if alert.runModal() != NSAlertFirstButtonReturn {
                return Err(error("cancelled", "未保存凭据"));
            }
            let mut secret = field.stringValue().to_string().into_bytes();
            field.setStringValue(&NSString::from_str(""));
            let result = (|| {
                if secret.is_empty()
                    || secret.len() > 16384
                    || secret.iter().any(|b| *b < 0x20 || *b == 0x7f)
                {
                    return Err(unavailable());
                }
                let users = users.lock().map_err(|_| unavailable())?;
                let context = users.require(&generation)?;
                let reference = uuid::Uuid::new_v4().to_string();
                vault
                    .lock()
                    .map_err(|_| unavailable())?
                    .stage(context.user.user_id.as_str(), &generation, &reference)
                    .map_err(|_| unavailable())?;
                if set_generic_password(
                    SERVICE,
                    &format!("{}:{reference}", context.user.user_id.as_str()),
                    &secret,
                )
                .is_err()
                {
                    let _ = vault
                        .lock()
                        .map_err(|_| unavailable())?
                        .discard(context.user.user_id.as_str(), &reference);
                    return Err(unavailable());
                }
                Ok(reference)
            })();
            secret.fill(0);
            result
        })();
        let _ = sender.send(result);
    })
    .map_err(|_| unavailable())?;
    receiver.await.map_err(|_| unavailable())?
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Activate {
        #[serde(rename = "userId")]
        user_id: String,
        generation: String,
        #[serde(rename = "credentialRef")]
        credential_ref: String,
    },
    Discard {
        #[serde(rename = "userId")]
        user_id: String,
        generation: String,
        #[serde(rename = "credentialRef")]
        credential_ref: String,
    },
    Collect {
        #[serde(rename = "userId")]
        user_id: String,
        generation: String,
        keep: Vec<String>,
    },
    Credential {
        #[serde(rename = "userId")]
        user_id: String,
        generation: String,
        #[serde(rename = "credentialRef")]
        credential_ref: String,
    },
    Codex {
        #[serde(rename = "userId")]
        user_id: String,
        generation: String,
        directory: PathBuf,
        storage: Storage,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Storage {
    File,
    Keyring,
    Auto,
    Ephemeral,
}
fn read_auth(directory: &Path) -> std::result::Result<Vec<u8>, ()> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    if !directory.is_absolute() || directory.canonicalize().map_err(|_| ())? != directory {
        return Err(());
    }
    let parent = std::fs::symlink_metadata(directory).map_err(|_| ())?;
    if !parent.is_dir() || parent.permissions().mode() & 0o022 != 0 {
        return Err(());
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(directory.join("auth.json"))
        .map_err(|_| ())?;
    let stat = file.metadata().map_err(|_| ())?;
    if !stat.is_file()
        || stat.permissions().mode() & 0o077 != 0
        || stat.uid() != parent.uid()
        || stat.len() > 262144
    {
        return Err(());
    }
    let mut data = Vec::new();
    file.take(262145).read_to_end(&mut data).map_err(|_| ())?;
    if data.len() > 262144 {
        return Err(());
    }
    Ok(data)
}
fn resolve(
    request: Request,
    users: &Mutex<Users>,
    vault: &Mutex<Vault>,
) -> std::result::Result<Value, ()> {
    let (user, generation) = match &request {
        Request::Credential {
            user_id,
            generation,
            ..
        }
        | Request::Codex {
            user_id,
            generation,
            ..
        }
        | Request::Activate {
            user_id,
            generation,
            ..
        }
        | Request::Discard {
            user_id,
            generation,
            ..
        }
        | Request::Collect {
            user_id,
            generation,
            ..
        } => (user_id, generation),
    };
    let users = users.lock().map_err(|_| ())?;
    if users
        .require(generation)
        .map_err(|_| ())?
        .user
        .user_id
        .as_str()
        != user
    {
        return Err(());
    }
    match request {
        Request::Discard {
            user_id,
            credential_ref,
            ..
        } => {
            vault
                .lock()
                .map_err(|_| ())?
                .discard(&user_id, &credential_ref)?;
            Ok(json!(null))
        }
        Request::Activate {
            user_id,
            generation,
            credential_ref,
        } => {
            vault
                .lock()
                .map_err(|_| ())?
                .activate(&user_id, &generation, &credential_ref)?;
            Ok(json!(null))
        }
        Request::Collect { user_id, keep, .. } => {
            if keep.len() > 16384 {
                return Err(());
            }
            vault.lock().map_err(|_| ())?.collect(&user_id, &keep)?;
            Ok(json!(null))
        }
        Request::Credential {
            user_id,
            generation,
            credential_ref,
        } => {
            if !vault
                .lock()
                .map_err(|_| ())?
                .readable(&user_id, &generation, &credential_ref)
                || uuid::Uuid::parse_str(&credential_ref).is_err()
            {
                return Err(());
            }
            let secret = get_generic_password(SERVICE, &format!("{user_id}:{credential_ref}"))
                .map_err(|_| ())?;
            let value = String::from_utf8(secret).map_err(|_| ())?;
            Ok(json!({"value":value}))
        }
        Request::Codex {
            user_id: _,
            generation: _,
            directory,
            storage,
        } => {
            if !directory.is_absolute() {
                return Err(());
            }
            let canonical = directory.canonicalize().map_err(|_| ())?;
            let key = format!(
                "cli|{}",
                &format!(
                    "{:x}",
                    Sha256::digest(canonical.to_string_lossy().as_bytes())
                )[..16]
            );
            let data = match storage {
                Storage::File => read_auth(&canonical)?,
                Storage::Keyring => get_generic_password("Codex Auth", &key).map_err(|_| ())?,
                Storage::Auto => match get_generic_password("Codex Auth", &key) {
                    Ok(data) => data,
                    Err(error) if error.code() == -25300 => read_auth(&canonical)?,
                    Err(_) => return Err(()),
                },
                Storage::Ephemeral => return Err(()),
            };
            if data.len() > 262144 {
                return Err(());
            }
            let auth: Value = serde_json::from_slice(&data).map_err(|_| ())?;
            if auth["auth_mode"] != "chatgpt" {
                return Err(());
            }
            let token = auth["tokens"]["access_token"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or(())?;
            let account = auth["tokens"]["account_id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or(())?;
            Ok(json!({"accessToken":token,"accountId":account}))
        }
    }
}
pub async fn serve(
    path: PathBuf,
    users: Arc<Mutex<Users>>,
    vault: Arc<Mutex<Vault>>,
    stop: CancellationToken,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(stat) = std::fs::symlink_metadata(&path) {
        use std::os::unix::fs::FileTypeExt;
        if !stat.file_type().is_socket() {
            return Err("credential endpoint occupied".into());
        }
        match UnixStream::connect(&path).await {
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                std::fs::remove_file(&path)?
            }
            _ => return Err("credential endpoint active".into()),
        }
    }
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    tokio::spawn(async move {
        loop {
            let stream = tokio::select! { _ = stop.cancelled() => break, next = listener.accept() => match next { Ok((stream, _)) => stream, Err(_) => break } };
            let users = users.clone();
            let vault = vault.clone();
            tokio::spawn(async move {
                let mut io = Framed::new(stream, LinesCodec::new_with_max_length(262144));
                if let Ok(Some(Ok(line))) =
                    tokio::time::timeout(Duration::from_secs(10), io.next()).await
                {
                    let result = serde_json::from_str::<Request>(&line)
                        .map_err(|_| ())
                        .and_then(|request| resolve(request, &users, &vault));
                    let response = match result {
                        Ok(value) => json!({"ok":true,"value":value}),
                        Err(_) => json!({"ok":false,"error":"authentication_required"}),
                    };
                    let _ = tokio::time::timeout(
                        Duration::from_secs(10),
                        io.send(response.to_string()),
                    )
                    .await;
                }
            });
        }
        let _ = std::fs::remove_file(path);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broker_rejects_old_generation_and_foreign_users_before_keychain_access() {
        let root = std::env::temp_dir().join(format!("rss-broker-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let mut users = Users::open(&root).unwrap();
        let alice = users.select("Alice").unwrap();
        let bob = users.select("Bob").unwrap();
        let users = Mutex::new(users);
        let vault = Mutex::new(Vault::open(&root).unwrap());
        for (user, generation) in [
            (alice.user.user_id.to_string(), alice.generation.to_string()),
            (alice.user.user_id.to_string(), bob.generation.to_string()),
        ] {
            assert!(resolve(
                Request::Credential {
                    user_id: user.clone(),
                    generation: generation.clone(),
                    credential_ref: uuid::Uuid::new_v4().to_string()
                },
                &users,
                &vault
            )
            .is_err());
            assert!(resolve(
                Request::Codex {
                    user_id: user,
                    generation,
                    directory: root.clone(),
                    storage: Storage::Keyring
                },
                &users,
                &vault
            )
            .is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
