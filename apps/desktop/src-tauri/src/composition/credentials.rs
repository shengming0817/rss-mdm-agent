//! Native API-key input and one app master key. Official CLI credentials are never inspected.
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
    pub fn get(&self, create: bool) -> Result<Vec<u8>> {
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
/// Returns input only to the native save operation, never to the WebView.
#[cfg(target_os = "macos")]
pub async fn enter<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSSecureTextField};
        use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
        let result = (|| {
            let mtm = MainThreadMarker::new().ok_or_else(unavailable)?;
            let alert = NSAlert::new(mtm);
            alert.setMessageText(&NSString::from_str("输入 API 密钥"));
            alert.setInformativeText(&NSString::from_str(
                "验证成功后与连接一起保存。取消或验证失败不会保存密钥。",
            ));
            alert.addButtonWithTitle(&NSString::from_str("验证并保存"));
            alert.addButtonWithTitle(&NSString::from_str("取消"));
            let field = NSSecureTextField::new(mtm);
            field.setFrame(NSRect::new(NSPoint::new(0., 0.), NSSize::new(360., 24.)));
            alert.setAccessoryView(Some(&field));
            if alert.runModal() != NSAlertFirstButtonReturn {
                return Err(error("cancelled", "未保存连接"));
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
#[cfg(windows)]
pub async fn enter<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let result = native_process::private_storage::enter_secret()
            .map_err(|_| unavailable())
            .and_then(|value| value.ok_or_else(|| error("cancelled", "未保存连接")))
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
