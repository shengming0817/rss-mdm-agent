//! Rust-owned credential cryptography and one application master key.
use crate::self_service::{error, Result};
#[cfg(target_os = "macos")]
use security_framework::passwords::{get_generic_password, set_generic_password};
use std::sync::Mutex;
#[cfg(target_os = "macos")]
const SERVICE: &str = "RSS MDM Agent";
#[cfg(target_os = "macos")]
const ACCOUNT: &str = "connection-master-key";
fn unavailable() -> crate::self_service::ServiceError {
    error(
        "authentication_required",
        "连接密钥不可用，请检查应用钥匙串访问权限",
    )
}
#[derive(Debug)]
pub struct KeyUnavailable;
/// Injected in deterministic tests; tests must never use the current user's Keychain.
pub trait KeyBackend: Send + Sync {
    fn read(&self) -> std::result::Result<Option<Vec<u8>>, KeyUnavailable>;
    fn create(&self, key: &[u8]) -> std::result::Result<(), KeyUnavailable>;
}
#[cfg(target_os = "macos")]
pub struct Keychain;
#[cfg(target_os = "macos")]
impl KeyBackend for Keychain {
    fn read(&self) -> std::result::Result<Option<Vec<u8>>, KeyUnavailable> {
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.code() == -25300 => Ok(None),
            Err(_) => Err(KeyUnavailable),
        }
    }
    fn create(&self, key: &[u8]) -> std::result::Result<(), KeyUnavailable> {
        set_generic_password(SERVICE, ACCOUNT, key).map_err(|_| KeyUnavailable)
    }
}
pub struct MasterKey {
    backend: Box<dyn KeyBackend>,
    pub permit: std::sync::Arc<tokio::sync::Semaphore>,
    key: Mutex<Option<Vec<u8>>>,
}
impl Default for MasterKey {
    fn default() -> Self {
        Self::new(platform_backend())
    }
}
impl MasterKey {
    pub fn new(backend: impl KeyBackend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
            permit: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            key: Mutex::new(None),
        }
    }
    fn get(&self, create: bool) -> Result<Vec<u8>> {
        let mut cached = self.key.lock().map_err(|_| unavailable())?;
        if let Some(key) = &*cached {
            return Ok(key.clone());
        }
        let key = match self.backend.read().map_err(|_| unavailable())? {
            Some(key) if key.len() == 32 => key,
            Some(_) => return Err(unavailable()),
            None if !create => return Err(unavailable()),
            None => {
                #[cfg(target_os = "macos")]
                let key = {
                    let mut key = vec![0; 32];
                    security_framework::random::SecRandom::default()
                        .copy_bytes(&mut key)
                        .map_err(|_| unavailable())?;
                    key
                };
                #[cfg(windows)]
                let key =
                    native_process::private_storage::random_key().map_err(|_| unavailable())?;
                self.backend.create(&key).map_err(|_| unavailable())?;
                key
            }
        };
        *cached = Some(key.clone());
        Ok(key)
    }
}
/// Bind encryption to the exact native product caller and provider target.
pub fn credential_owner(
    context: &ai_session_contract::UserContext,
    connection: &ai_session_contract::ConnectionDraft,
) -> Result<ai_session_contract::CredentialOwner> {
    let value = serde_json::to_value(connection).map_err(|_| error("input", "无效连接"))?;
    if value["source"]["type"] != "custom_api" {
        return Err(error("input", "该连接不接受凭据"));
    }
    let endpoint = url::Url::parse(value["source"]["apiUrl"].as_str().ok_or_else(unavailable)?)
        .map_err(|_| error("input", "API 地址无效"))?
        .to_string();
    let identity = serde_json::to_value(&context.identity).map_err(|_| unavailable())?;
    serde_json::from_value(serde_json::json!({
        "tenantId": identity["tenantId"].as_str().unwrap_or("test-users"),
        "principalId": identity["principalId"].as_str().unwrap_or(context.user.user_id.as_str()),
        "authorityId": identity["authorityId"].as_str().unwrap_or("desktop-fixture"),
        "connectionId": connection.connection_id,
        "provider": value["provider"],
        "endpoint": endpoint,
        "credentialType": value["source"]["credentialType"].as_str().unwrap_or("api_key"),
    }))
    .map_err(|_| error("input", "凭据目标无效"))
}

/// ref: RustCrypto AEADs aes-gcm/src/lib.rs@aes-gcm-v0.10.3
impl MasterKey {
    pub fn seal(
        &self,
        owner: &ai_session_contract::CredentialOwner,
        secret: &str,
        create: bool,
    ) -> Result<Vec<u8>> {
        use aes_gcm::{
            aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
            Aes256Gcm,
        };
        if secret.is_empty() || secret.len() > 16384 || secret.chars().any(char::is_control) {
            return Err(error("input", "凭据格式无效"));
        }
        let key = zeroize::Zeroizing::new(self.get(create)?);
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| unavailable())?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let aad = serde_json::to_vec(owner).map_err(|_| unavailable())?;
        let mut output = nonce.to_vec();
        output.extend(
            cipher
                .encrypt(
                    &nonce,
                    Payload {
                        msg: secret.as_bytes(),
                        aad: &aad,
                    },
                )
                .map_err(|_| unavailable())?,
        );
        Ok(output)
    }
    pub fn open(
        &self,
        owner: &ai_session_contract::CredentialOwner,
        encrypted: &[u8],
    ) -> Result<String> {
        use aes_gcm::{
            aead::{Aead, KeyInit, Payload},
            Aes256Gcm, Nonce,
        };
        if !(29..=16412).contains(&encrypted.len()) {
            return Err(unavailable());
        }
        let key = zeroize::Zeroizing::new(self.get(false)?);
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| unavailable())?;
        let aad = serde_json::to_vec(owner).map_err(|_| unavailable())?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&encrypted[..12]),
                Payload {
                    msg: &encrypted[12..],
                    aad: &aad,
                },
            )
            .map_err(|_| unavailable())?;
        String::from_utf8(plaintext).map_err(|_| unavailable())
    }
}

#[cfg(windows)]
pub struct Dpapi;
#[cfg(windows)]
impl KeyBackend for Dpapi {
    fn read(&self) -> std::result::Result<Option<Vec<u8>>, KeyUnavailable> {
        let path = native_process::private_storage::key_path().map_err(|_| KeyUnavailable)?;
        match native_process::private_storage::read(&path, 65536) {
            Ok(bytes) => native_process::private_storage::unprotect_key(&bytes)
                .map(Some)
                .map_err(|_| KeyUnavailable),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(KeyUnavailable),
        }
    }
    fn create(&self, key: &[u8]) -> std::result::Result<(), KeyUnavailable> {
        let path = native_process::private_storage::key_path().map_err(|_| KeyUnavailable)?;
        let encrypted =
            native_process::private_storage::protect_key(key).map_err(|_| KeyUnavailable)?;
        native_process::private_storage::write_new(&path, &encrypted).map_err(|_| KeyUnavailable)
    }
}
pub fn platform_backend() -> impl KeyBackend {
    #[cfg(target_os = "macos")]
    {
        Keychain
    }
    #[cfg(windows)]
    {
        Dpapi
    }
}
#[cfg(target_os = "macos")]
pub async fn enter_enterprise_password<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    target: String,
) -> Result<String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSSecureTextField};
        use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
        let result = (|| {
            let mtm = MainThreadMarker::new().ok_or_else(unavailable)?;
            let alert = NSAlert::new(mtm);
            alert.setMessageText(&NSString::from_str("输入企业账号密码"));
            alert.setInformativeText(&NSString::from_str(&target));
            alert.addButtonWithTitle(&NSString::from_str("登录"));
            alert.addButtonWithTitle(&NSString::from_str("取消"));
            let field = NSSecureTextField::new(mtm);
            field.setFrame(NSRect::new(NSPoint::new(0., 0.), NSSize::new(360., 24.)));
            alert.setAccessoryView(Some(&field));
            if alert.runModal() != NSAlertFirstButtonReturn {
                return Err(error("cancelled", "未完成登录"));
            }
            let secret = field.stringValue().to_string();
            field.setStringValue(&NSString::from_str(""));
            if secret.trim().is_empty()
                || secret.len() > 16384
                || secret.chars().any(char::is_control)
            {
                return Err(unavailable());
            }
            Ok(secret)
        })();
        let _ = sender.send(result);
    })
    .map_err(|_| unavailable())?;
    receiver.await.map_err(|_| unavailable())?
}
#[cfg(windows)]
pub async fn enter_enterprise_password<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    target: String,
) -> Result<String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let result = native_process::private_storage::enter_enterprise_password(&target)
            .map_err(|_| unavailable())
            .and_then(|value| value.ok_or_else(|| error("cancelled", "未完成登录")))
            .and_then(|value| {
                if value.trim().is_empty() || value.chars().any(char::is_control) {
                    Err(unavailable())
                } else {
                    Ok(value)
                }
            });
        let _ = sender.send(result);
    })
    .map_err(|_| unavailable())?;
    receiver.await.map_err(|_| unavailable())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Fake {
        stored: Mutex<Option<Vec<u8>>>,
        writes: Arc<AtomicUsize>,
    }
    impl KeyBackend for Fake {
        fn read(&self) -> std::result::Result<Option<Vec<u8>>, KeyUnavailable> {
            Ok(self.stored.lock().unwrap().clone())
        }
        fn create(&self, key: &[u8]) -> std::result::Result<(), KeyUnavailable> {
            *self.stored.lock().unwrap() = Some(key.to_vec());
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    fn owner() -> ai_session_contract::CredentialOwner {
        serde_json::from_value(serde_json::json!({"tenantId":"t","principalId":"p","authorityId":"a","connectionId":"c","provider":"codex","endpoint":"https://example.invalid/","credentialType":"api_key"})).unwrap()
    }
    #[test]
    fn credential_owners_follow_native_test_guest_and_enterprise_identity() {
        let draft = serde_json::from_value(serde_json::json!({"connectionId":"c","name":"Connection","provider":"codex","profile":"conversation","source":{"type":"custom_api","apiUrl":"https://example.invalid","model":"chosen"}})).unwrap();
        for identity in [
            serde_json::Value::Null,
            serde_json::json!({"mode":"guest","tenantId":"local-guest","principalId":"guest","authorityId":"desktop-guest"}),
            serde_json::json!({"mode":"enterprise","tenantId":"tenant","principalId":"principal","authorityId":"authority","organizationId":"org","expiresAtMs":1000}),
        ] {
            let context = serde_json::from_value(serde_json::json!({"schemaVersion":6,"kind":"userContext","generation":"generation","user":{"schemaVersion":6,"kind":"testUser","userId":"profile","nameKey":"name","displayName":"Name"},"identity":identity})).unwrap();
            let owner = serde_json::to_value(credential_owner(&context, &draft).unwrap()).unwrap();
            assert_eq!(
                owner["principalId"],
                identity["principalId"].as_str().unwrap_or("profile")
            );
            assert_eq!(
                owner["tenantId"],
                identity["tenantId"].as_str().unwrap_or("test-users")
            );
            assert_eq!(
                owner["authorityId"],
                identity["authorityId"]
                    .as_str()
                    .unwrap_or("desktop-fixture")
            );
            assert_eq!(owner["endpoint"], "https://example.invalid/");
        }
    }
    #[test]
    fn credentials_use_random_nonces_bind_the_target_and_preserve_secret_bytes() {
        let keys = MasterKey::new(Fake {
            stored: Mutex::new(Some(vec![7; 32])),
            writes: Arc::new(AtomicUsize::new(0)),
        });
        let target = owner();
        let secret = "  synthetic-secret  ";
        let encrypted = keys.seal(&target, secret, false).unwrap();
        assert_ne!(encrypted, keys.seal(&target, secret, false).unwrap());
        assert!(keys.open(&target, &encrypted).unwrap() == secret);
        for (field, value) in [
            ("tenantId", "other"),
            ("principalId", "other"),
            ("authorityId", "other"),
            ("connectionId", "other"),
            ("provider", "claude"),
            ("endpoint", "https://other.invalid/"),
            ("credentialType", "auth_token"),
        ] {
            let mut changed = serde_json::to_value(&target).unwrap();
            changed[field] = serde_json::Value::String(value.into());
            assert!(keys
                .open(&serde_json::from_value(changed).unwrap(), &encrypted)
                .is_err());
        }
        let mut altered = encrypted;
        altered[12] ^= 1;
        assert!(keys.open(&target, &altered).is_err());
        assert!(keys.seal(&target, "", false).is_err());
    }
    #[test]
    fn missing_master_with_ciphertext_never_creates_a_replacement() {
        let writes = Arc::new(AtomicUsize::new(0));
        let keys = MasterKey::new(Fake {
            stored: Mutex::new(None),
            writes: writes.clone(),
        });
        assert!(keys.get(false).is_err());
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        let first = keys.get(true).unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(keys.get(false).unwrap(), first);
        assert_eq!(writes.load(Ordering::SeqCst), 1);
    }
}
