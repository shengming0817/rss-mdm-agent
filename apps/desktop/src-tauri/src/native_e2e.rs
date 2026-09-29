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
    // Fail closed on an occupied port; runner additionally verifies listener ownership.
    let socket = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))?;
    drop(socket);
    eprintln!(
        "RSS_NATIVE_E2E {}",
        serde_json::json!({
            "pid": std::process::id(), "nonce": nonce, "port": port,
            "dataRootSha256": format!("{:x}", Sha256::digest(root.as_os_str().as_encoded_bytes()))
        })
    );
    Ok(builder.plugin(tauri_plugin_wdio_webdriver::init_with_port(port)))
}
