//! Export only the generated closed status projection; never raw stderr, paths or AI payloads.
// ref: objc2-app-kit src/generated/NSSavePanel.rs@0.3.2
use crate::self_service::{error, Result};
fn failed() -> crate::self_service::ServiceError {
    error(
        "diagnostics_unavailable",
        "诊断未导出，请检查保存位置后重试",
    )
}
/// Explicit projection: adding a status field never adds it to an exported diagnostic.
pub fn snapshot(status: &ai_session_contract::HostStatus) -> Result<Vec<u8>> {
    let recent: Vec<_> = status
        .recent
        .iter()
        .map(|row| serde_json::json!({"stage":row.stage,"code":row.code,"atMs":row.at_ms}))
        .collect();
    serde_json::to_vec_pretty(
        &serde_json::json!({"version":status.version,"source":status.source,"recent":recent}),
    )
    .map_err(|_| failed())
}
/// The native dialog owns path selection. Acceptance reuses the same file writer in a private directory.
pub fn save(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
        if path.is_symlink() {
            return Err(failed());
        }
    }
    let mut file = options.open(path).map_err(|_| failed())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| failed())
}
#[cfg(target_os = "macos")]
pub async fn export<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    status: ai_session_contract::HostStatus,
) -> Result<bool> {
    let bytes = snapshot(&status)?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSModalResponseOK, NSSavePanel};
        use objc2_foundation::NSString;
        let selected: Result<Option<std::path::PathBuf>> = (|| {
            let panel = NSSavePanel::savePanel(MainThreadMarker::new().ok_or_else(failed)?);
            panel.setNameFieldStringValue(&NSString::from_str("rss-ai-diagnostics.json"));
            panel.setCanCreateDirectories(true);
            if panel.runModal() != NSModalResponseOK {
                return Ok(None);
            }
            let path = panel.URL().and_then(|url| url.path()).ok_or_else(failed)?;
            Ok(Some(std::path::PathBuf::from(path.to_string())))
        })();
        let _ = sender.send(selected);
    })
    .map_err(|_| failed())?;
    let Some(path) = receiver.await.map_err(|_| failed())?? else {
        return Ok(false);
    };
    tokio::task::spawn_blocking(move || save(&path, &bytes))
        .await
        .map_err(|_| failed())??;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_is_an_explicit_projection_and_rejects_secret_fields() {
        let value = serde_json::json!({"schemaVersion":5,"kind":"hostStatus","generation":99,"phase":"failed","source":"development_override","version":"0.1.0","recent":[{"stage":"host_process","code":"host_exited","action":"restart_host","atMs":1}]});
        let status: ai_session_contract::HostStatus =
            serde_json::from_value(value.clone()).unwrap();
        let output: serde_json::Value =
            serde_json::from_slice(&snapshot(&status).unwrap()).unwrap();
        assert_eq!(
            output,
            serde_json::json!({"version":"0.1.0","source":"development_override","recent":[{"stage":"host_process","code":"host_exited","atMs":1}]})
        );
        let mut poisoned = value;
        poisoned["secret"] = serde_json::json!("CANARY_SECRET");
        assert!(ai_session_contract::decode(
            &serde_json::to_vec(&poisoned).unwrap(),
            &ai_session_contract::Limits {
                max_bytes: 4096,
                max_text_bytes: 2048,
                max_depth: 8,
                max_nodes: 128
            }
        )
        .is_err());
    }
}

#[cfg(windows)]
pub async fn export<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    status: ai_session_contract::HostStatus,
) -> Result<bool> {
    let bytes = snapshot(&status)?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(native_process::private_storage::save_dialog());
    })
    .map_err(|_| failed())?;
    let Some(path) = receiver
        .await
        .map_err(|_| failed())?
        .map_err(|_| failed())?
    else {
        return Ok(false);
    };
    tokio::task::spawn_blocking(move || save(&path, &bytes))
        .await
        .map_err(|_| failed())??;
    Ok(true)
}
