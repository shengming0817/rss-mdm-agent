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
