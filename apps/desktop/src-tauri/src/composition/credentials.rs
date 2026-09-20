//! Native API-key input and one app master key. Official CLI credentials are never inspected.
use crate::self_service::{error, Result};
use security_framework::passwords::{get_generic_password, set_generic_password};
use std::sync::Mutex;
const SERVICE: &str = "RSS MDM Agent";
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
pub struct Keychain;
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
pub struct MasterKey<B: KeyBackend = Keychain> {
    backend: B,
    key: Mutex<Option<Vec<u8>>>,
}
impl Default for MasterKey {
    fn default() -> Self {
        Self::new(Keychain)
    }
}
impl<B: KeyBackend> MasterKey<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
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
                let mut key = vec![0; 32];
                security_framework::random::SecRandom::default()
                    .copy_bytes(&mut key)
                    .map_err(|_| unavailable())?;
                self.backend.create(&key).map_err(|_| unavailable())?;
                key
            }
        };
        *cached = Some(key.clone());
        Ok(key)
    }
}
/// Returns input only to the native save operation, never to the WebView.
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
