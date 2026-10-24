//! Explicit scoped diagnostic snapshots. No raw service or session data is exported.
//! ref: tauri-apps/plugins-workspace plugins/dialog/src/desktop.rs@c75459d8996b8e7097d556de4e9948bd730f6e9c.
use super::runtime::DesktopRuntime;
use crate::self_service::{self as ui, error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scopes {
    pub host: bool,
    pub service: bool,
    pub device: bool,
    pub execution: bool,
}
#[derive(Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub snapshot: String,
    pub content: Value,
    pub bytes: usize,
}
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub snapshot: String,
}
struct Snapshot {
    id: String,
    generation: Option<String>,
    created: Instant,
    bytes: Vec<u8>,
}
#[derive(Default)]
pub struct Store {
    epoch: AtomicU64,
    snapshot: Mutex<Option<Snapshot>>,
}
impl Store {
    fn read(&self, id: &str, owner: &Option<String>) -> Result<Vec<u8>> {
        let snapshot = self.snapshot.lock().map_err(|_| failed())?;
        let s = snapshot
            .as_ref()
            .filter(|s| {
                s.id == id
                    && &s.generation == owner
                    && s.created.elapsed() < Duration::from_secs(300)
            })
            .ok_or_else(failed)?;
        Ok(s.bytes.clone())
    }
    pub(super) fn invalidate(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut snapshot) = self.snapshot.lock() {
            *snapshot = None;
        }
    }
}
fn failed() -> ui::ServiceError {
    error(
        "diagnostics_unavailable",
        "诊断未导出，请刷新预览或检查保存位置",
    )
}
fn generation(state: &DesktopRuntime) -> Result<Option<String>> {
    Ok(state
        .users
        .lock()
        .map_err(|_| failed())?
        .current()
        .ok()
        .map(|u| u.generation.to_string()))
}
fn host_projection(status: &ai_session_contract::HostStatus) -> Value {
    json!({"version":status.version.chars().take(128).collect::<String>(),"source":status.source,"phase":status.phase,
        "recent":status.recent.iter().rev().take(64).map(|row| json!({"stage":row.stage,"code":row.code,"atMs":row.at_ms})).collect::<Vec<_>>()})
}
fn service_projection(view: execution_ipc::host::ServiceView) -> Value {
    use execution_ipc::host::ServiceView;
    match view {
        ServiceView::Connected { status } => {
            json!({"phase":"connected","build":status.build.chars().take(128).collect::<String>(),"platform":status.platform.chars().take(32).collect::<String>(),"protocol":status.protocol,"readiness":status.readiness})
        }
        ServiceView::NotInstalled => json!({"phase":"notInstalled"}),
        ServiceView::ConfigurationRequired => json!({"phase":"configurationRequired"}),
        ServiceView::Rejected => json!({"phase":"rejected"}),
        ServiceView::Mismatch => json!({"phase":"mismatch"}),
        ServiceView::Unavailable => json!({"phase":"unavailable"}),
    }
}
async fn execution_projection(handle: &super::execution::ExecutionHandle) -> Result<Value> {
    let mut after = None;
    let mut rows = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut truncated = false;
    for index in 0..4 {
        let page = handle
            .snapshot(ui::SnapshotQuery {
                after: after.clone(),
                selected: None,
            })
            .await?;
        let count = page.requests.len();
        for task in page.requests {
            if rows.len() == 20 {
                truncated = true;
                break;
            }
            let action = match task.action.operation.action.as_str() {
                "network.check" => "networkCheck",
                "local-query" => "query",
                "settings.read" => "settingsRead",
                "settings.change" => "settingsChange",
                "settings.open" => "settingsOpen",
                _ => "publishedTask",
            };
            rows.push(json!({"action":action,"phase":task.status.phase,"cancelRequested":task.status.cancel_requested,"assessment":task.status.assessment}));
        }
        if rows.len() == 20 {
            truncated |= count > 20 || page.next.is_some();
            break;
        }
        let Some(next) = page.next else {
            return Ok(json!({"available":true,"items":rows,"truncated":truncated}));
        };
        if !seen.insert(next.clone()) {
            return Err(failed());
        }
        after = Some(next);
        truncated = index == 3;
    }
    Ok(json!({"available":true,"items":rows,"truncated":truncated}))
}
pub async fn prepare(state: &DesktopRuntime, scopes: Scopes) -> Result<Preview> {
    let ticket = state.diagnostics.epoch.fetch_add(1, Ordering::AcqRel) + 1;
    *state.diagnostics.snapshot.lock().map_err(|_| failed())? = None;
    let owner = generation(state)?;
    let at = super::execution::now().map_err(|_| failed())?;
    let mut content = json!({"schemaVersion":1,"generatedAtUnixMs":at,"scopes":scopes});
    if scopes.host {
        content["host"] = host_projection(&state.status());
    }
    if scopes.service {
        content["service"] = match tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(execution_ipc::host::ServiceClient::inspect),
        )
        .await
        {
            Ok(Ok(view)) => service_projection(view),
            _ => json!({"phase":"unavailable"}),
        };
    }
    if scopes.device {
        content["device"] = json!({"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"available":false});
        if let Some(owner) = &owner {
            if let Ok(handle) = state.execution_for(owner) {
                if let Ok(Ok(value)) =
                    tokio::time::timeout(Duration::from_secs(3), handle.device_diagnostics()).await
                {
                    content["device"] = value;
                }
            }
        }
    }
    if scopes.execution {
        content["execution"] = json!({"available":false,"reason":"unboundOrUnavailable"});
        if let Some(owner) = &owner {
            if let Ok(handle) = state.execution_for(owner) {
                if let Ok(Ok(value)) =
                    tokio::time::timeout(Duration::from_secs(5), execution_projection(&handle))
                        .await
                {
                    content["execution"] = value;
                }
            }
        }
    }
    if generation(state)? != owner || state.diagnostics.epoch.load(Ordering::Acquire) != ticket {
        return Err(failed());
    }
    let bytes = serde_json::to_vec_pretty(&content).map_err(|_| failed())?;
    if bytes.len() > 1024 * 1024 {
        return Err(failed());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let preview = Preview {
        snapshot: id.clone(),
        content,
        bytes: bytes.len(),
    };
    let mut retained = state.diagnostics.snapshot.lock().map_err(|_| failed())?;
    if state.diagnostics.epoch.load(Ordering::Acquire) != ticket {
        return Err(failed());
    }
    *retained = Some(Snapshot {
        id,
        generation: owner,
        created: Instant::now(),
        bytes,
    });
    Ok(preview)
}
pub fn discard(state: &DesktopRuntime, input: &Selection) -> Result<()> {
    let mut snapshot = state.diagnostics.snapshot.lock().map_err(|_| failed())?;
    if snapshot.as_ref().is_some_and(|s| s.id == input.snapshot) {
        state.diagnostics.epoch.fetch_add(1, Ordering::AcqRel);
        *snapshot = None;
    }
    Ok(())
}
fn frozen(state: &DesktopRuntime, input: &Selection) -> Result<(Vec<u8>, Option<String>)> {
    let owner = generation(state)?;
    Ok((state.diagnostics.read(&input.snapshot, &owner)?, owner))
}
pub async fn export<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: &DesktopRuntime,
    input: Selection,
) -> Result<bool> {
    frozen(state, &input)?;
    let Some(path) = choose_path(app).await? else {
        return Ok(false);
    };
    let (bytes, owner) = frozen(state, &input)?;
    let users = state.users.clone();
    tokio::task::spawn_blocking(move || {
        let users = users.lock().map_err(|_| failed())?;
        if users.current().ok().map(|u| u.generation.to_string()) != owner {
            return Err(failed());
        }
        save(&path, &bytes)
    })
    .await
    .map_err(|_| failed())??;
    discard(state, &input)?;
    Ok(true)
}
/// Write privately and replace only after the complete snapshot is durable.
pub fn save(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    if path.is_symlink() || std::fs::symlink_metadata(path).is_ok_and(|m| !m.is_file()) {
        return Err(failed());
    }
    let parent = path.parent().ok_or_else(failed)?;
    let temporary = parent.join(format!(".rss-diagnostics-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&temporary).map_err(|_| failed())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| failed())?;
        drop(file);
        if path.is_symlink() {
            return Err(failed());
        }
        std::fs::rename(&temporary, path).map_err(|_| failed())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
#[cfg(target_os = "macos")]
async fn choose_path<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<std::path::PathBuf>> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSModalResponseOK, NSSavePanel};
        use objc2_foundation::NSString;
        let result = (|| {
            let panel = NSSavePanel::savePanel(MainThreadMarker::new().ok_or_else(failed)?);
            panel.setNameFieldStringValue(&NSString::from_str("rss-diagnostics.json"));
            panel.setCanCreateDirectories(true);
            if panel.runModal() != NSModalResponseOK {
                return Ok(None);
            }
            let path = panel.URL().and_then(|u| u.path()).ok_or_else(failed)?;
            Ok(Some(std::path::PathBuf::from(path.to_string())))
        })();
        let _ = sender.send(result);
    })
    .map_err(|_| failed())?;
    receiver.await.map_err(|_| failed())?
}
#[cfg(windows)]
async fn choose_path<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<std::path::PathBuf>> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(super::windows_dialogs::save_dialog().map_err(|_| failed()));
    })
    .map_err(|_| failed())?;
    receiver.await.map_err(|_| failed())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_snapshot_keeps_exact_bytes_and_rejects_expiry_other_users_and_replacement() {
        let store = Store::default();
        let owner = Some("alice-generation".into());
        *store.snapshot.lock().unwrap() = Some(Snapshot {
            id: "original".into(),
            generation: owner.clone(),
            created: Instant::now(),
            bytes: b"frozen content".to_vec(),
        });
        assert_eq!(store.read("original", &owner).unwrap(), b"frozen content");
        assert!(store
            .read("original", &Some("bob-generation".into()))
            .is_err());
        assert!(store.read("replaced", &owner).is_err());
        store.snapshot.lock().unwrap().as_mut().unwrap().created =
            Instant::now() - Duration::from_secs(301);
        assert!(store.read("original", &owner).is_err());
        store.invalidate();
        assert!(store.snapshot.lock().unwrap().is_none());
    }
    #[test]
    fn service_projection_drops_extensions() {
        let mut status =
            execution_ipc::host::ServiceStatus::new(execution_ipc::host::Readiness::Ready);
        status.telemetry = Some(json!({"secret":"CANARY_SECRET"}));
        assert!(
            !service_projection(execution_ipc::host::ServiceView::Connected { status })
                .to_string()
                .contains("CANARY")
        );
    }
    #[test]
    fn atomic_private_save_replaces_files_and_refuses_symlinks() {
        let root =
            std::env::temp_dir().join(format!("rss-diagnostics-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("report.json");
        std::fs::write(&path, b"old").unwrap();
        save(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        #[cfg(unix)]
        {
            use std::os::unix::fs::{symlink, PermissionsExt};
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let link = root.join("link");
            symlink(&path, &link).unwrap();
            assert!(save(&link, b"bad").is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"new");
        }
        assert!(save(&root, b"bad").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
