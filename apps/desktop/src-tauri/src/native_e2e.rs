// ref: webdriverio/desktop-mobile packages/tauri-plugin-webdriver/src/lib.rs
// @wdio-tauri-service@v1.4.0 (aef40049a9c566e72de4ffd08e08197ff32386ed)
// Standard WebDriver only. No product IPC, fake accounts or lifecycle overrides.
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn configure(
    builder: tauri::Builder<tauri::Wry>,
    data_root: Option<&Path>,
) -> Result<tauri::Builder<tauri::Wry>, Box<dyn std::error::Error>> {
    let root = data_root.ok_or("native-e2e requires an isolated --test-data-dir")?;
    if !root.is_dir() || std::fs::read_dir(root)?.next().is_some() {
        return Err("native-e2e requires a fresh empty data directory".into());
    }
    let nonce = std::env::var("RSS_NATIVE_E2E_NONCE")?;
    if nonce.len() != 32 || !nonce.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("native-e2e requires an explicit run nonce".into());
    }
    let port: u16 = std::env::var("RSS_NATIVE_E2E_PORT")?.parse()?;
    if port == 0 {
        return Err("native-e2e requires an explicit port".into());
    }
    let mut capability = [0_u8; 16];
    for (index, byte) in capability.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&nonce[index * 2..index * 2 + 2], 16)?;
    }
    // Keep ownership across plugin setup; there is no check-then-bind race.
    let socket = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))?;
    socket.set_nonblocking(true)?;
    eprintln!(
        "RSS_NATIVE_E2E {}",
        serde_json::json!({
            "pid": std::process::id(), "nonceSha256": format!("{:x}", Sha256::digest(nonce.as_bytes())), "port": port,
            "dataRootSha256": format!("{:x}", Sha256::digest(root.as_os_str().as_encoded_bytes()))
        })
    );
    Ok(builder.plugin(tauri_plugin_wdio_webdriver::init(socket, capability)))
}

// ref: security-framework 3.5.1 os/macos/passwords.rs temp_keychain_setup.
// The real OS credential store belongs to this run, never to the developer's login keychain.
// The harness deletes it through Security.framework after the owned process has exited.
#[cfg(all(target_os = "macos", not(feature = "dev-fixture")))]
pub fn keychain(
    root: &Path,
) -> Result<impl rss_mdm_desktop::composition::credentials::KeyBackend, Box<dyn std::error::Error>>
{
    use platform_credentials::{Keychain, NonInteractiveGuard};
    use rss_mdm_desktop::composition::credentials::{KeyBackend, KeyUnavailable};
    struct Isolated {
        keychain: Keychain,
        _noninteractive: NonInteractiveGuard,
    }
    const SERVICE: &str = "RSS MDM Agent native-e2e";
    const ACCOUNT: &str = "connection-master-key";
    impl KeyBackend for Isolated {
        fn read(&self) -> Result<Option<Vec<u8>>, KeyUnavailable> {
            match self.keychain.read(SERVICE, ACCOUNT) {
                Ok(key) => Ok(Some(key)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(_) => Err(KeyUnavailable),
            }
        }
        fn create(&self, key: &[u8]) -> Result<(), KeyUnavailable> {
            self.keychain
                .create_new(SERVICE, ACCOUNT, key)
                .map_err(|_| KeyUnavailable)
        }
    }
    platform_private_storage::directory(root)?;
    let noninteractive = Keychain::disable_interaction()?;
    let password = zeroize::Zeroizing::new(uuid::Uuid::new_v4().to_string());
    let keychain = Keychain::create_file(&root.join("native-e2e.keychain"), &password)?;
    Ok(Isolated {
        keychain,
        _noninteractive: noninteractive,
    })
}
