//! Export only the generated closed status projection; never raw stderr, paths or AI payloads.
// ref: objc2-app-kit src/generated/NSSavePanel.rs@0.3.2
use crate::self_service::{error, Result};
fn failed() -> crate::self_service::ServiceError {
    error(
        "diagnostics_unavailable",
        "诊断未导出，请检查保存位置后重试",
    )
}
pub async fn export<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    status: ai_session_contract::HostStatus,
) -> Result<bool> {
    let bytes = serde_json::to_vec_pretty(&status).map_err(|_| failed())?;
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
    tokio::task::spawn_blocking(move || {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| failed())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| failed())
    })
    .await
    .map_err(|_| failed())??;
    Ok(true)
}
