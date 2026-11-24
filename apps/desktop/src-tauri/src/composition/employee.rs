//! Browser-owned proof cookies stay on the registered organization origin. The main window
//! only selects a change to a binding returned by protected native IPC.
//! ref: tauri 2.11.2 WebviewWindowBuilder::{incognito,on_navigation,on_new_window}.
use super::runtime::DesktopRuntime;
use crate::self_service::{error, Result};
use execution_ipc::device_access::{Operation, Response};
use rss_mdm_agent_wire::runtime as wire;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

#[derive(Clone, Copy, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    ResetFactors,
    RepairBinding,
}

#[derive(Clone, Copy, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Open,
    Verify,
    Submit,
    Status,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub binding: Uuid,
    pub revision: i64,
    pub change: Change,
    pub stage: Stage,
}

fn intent(binding: Uuid, device: Uuid, principal: &str, revision: i64, change: Change) -> Uuid {
    // RFC 9562 Appendix B.2. Same operation survives a lost response or a desktop restart.
    let name = serde_json::to_vec(&(
        "rss.employee.binding",
        binding,
        device,
        principal,
        revision,
        change,
    ))
    .expect("infallible scalar encoding");
    let hash = Sha256::digest(name);
    uuid::Builder::from_custom_bytes(hash[..16].try_into().unwrap()).into_uuid()
}

pub async fn dispatch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    generation: String,
    input: Input,
) -> Result<()> {
    let denied = || error("binding_changed", "当前账号或设备绑定已更改，请刷新");
    let unavailable = || error("employee_browser", "请打开本人验证窗口并完成组织登录");
    let state = app.state::<DesktopRuntime>();
    let session = state.employee_session(&generation)?;
    let (origin, tenant, principal) = session.employee_scope();
    let handle = state.execution_for(&generation).await?;
    let Response::OwnBindings { device, bindings } =
        handle.device_access(Operation::OwnBindings {}).await?
    else {
        return Err(denied());
    };
    state.current(&generation)?;
    let b = bindings
        .iter()
        .find(|b| b.id == input.binding)
        .ok_or_else(denied)?;
    let op = intent(
        b.id,
        device.into_uuid(),
        principal,
        input.revision,
        input.change,
    );
    let status = matches!(input.stage, Stage::Status);
    if input.revision < 1 || (!status && b.revision != input.revision && op != b.operation_id) {
        return Err(denied());
    }
    if !status
        && matches!(input.change, Change::ResetFactors)
        && !(matches!(
            b.account,
            Some(wire::AccessAccount::Windows {
                authority: wire::AccessWindowsAuthority::Local,
                ..
            })
        ) && matches!(
            b.configuration.windows,
            wire::AccessWindowsConfiguration::RssFactor { .. }
        ))
    {
        return Err(error("factor", "此登录机制由系统或身份提供方管理"));
    }
    let label = format!(
        "employee-{}",
        Uuid::parse_str(&generation).map_err(|_| denied())?
    );
    let origin_url = url::Url::parse(origin).map_err(|_| denied())?;
    let window = match app.get_webview_window(&label) {
        Some(w) => w,
        None if matches!(input.stage, Stage::Open | Stage::Status) => {
            let start = origin_url
                .join(&format!("tenants/{tenant}"))
                .map_err(|_| denied())?;
            let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(start))
                .title("RSS 本人设备验证")
                .incognito(true)
                .on_navigation(|url| {
                    url.scheme() == "https"
                        && url.username().is_empty()
                        && url.password().is_none()
                        && !matches!(url.host_str(), Some("tauri.localhost" | "ipc.localhost"))
                })
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .build()
                .map_err(|_| unavailable())?;
            let watched = window.clone();
            let app = app.clone();
            let generation = generation.clone();
            tauri::async_runtime::spawn(async move {
                for _ in 0..600 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    if watched.url().is_err() {
                        return;
                    }
                    if app.state::<DesktopRuntime>().current(&generation).is_err() {
                        break;
                    }
                }
                let _ = watched.destroy();
            });
            window
        }
        _ => return Err(unavailable()),
    };
    window.set_focus().map_err(|_| unavailable())?;
    if matches!(input.stage, Stage::Open) {
        return Ok(());
    }
    if window.url().map_err(|_| unavailable())?.origin() != origin_url.origin() {
        return Err(unavailable());
    }
    let config = serde_json::json!({
        "origin":origin,"tenant":tenant,"principal":principal,"device":device,"binding":b.id,
        "operation": if status { b.operation_id } else { op }, "stage":input.stage,
        "change":{"operationId":op,"expectedRevision":input.revision,"action":{"kind":input.change}}
    });
    state.current(&generation)?;
    let script = include_str!("employee-browser.js");
    window
        .eval(format!("({script})({config})"))
        .map_err(|_| unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_uses_one_operation_per_subject_revision_and_purpose() {
        let b = Uuid::new_v4();
        let d = Uuid::new_v4();
        let reset = |p, rev| intent(b, d, p, rev, Change::ResetFactors);
        assert_eq!(reset("a", 1), reset("a", 1));
        assert_ne!(reset("a", 1), reset("a", 2));
        assert_ne!(reset("a", 1), reset("b", 1));
        assert_ne!(reset("a", 1), intent(b, d, "a", 1, Change::RepairBinding));
    }
}
