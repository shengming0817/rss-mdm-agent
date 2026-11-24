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
    Close,
}

/// Public progress for the exact employee operation, with no account or proof material.
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub revision: i64,
    pub received: bool,
    pub execution: String,
    pub cancellation: String,
    pub cleanup_pending: bool,
    pub effect_observed: bool,
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
) -> Result<Option<Progress>> {
    let denied = || error("binding_changed", "当前账号或设备绑定已更改，请刷新");
    let unavailable = || error("employee_browser", "请打开本人验证窗口并完成组织登录");
    let state = app.state::<DesktopRuntime>();
    let session = state.employee_session(&generation)?;
    let (origin, tenant, principal) = session.employee_scope();
    let label = format!(
        "employee-{}",
        Uuid::parse_str(&generation).map_err(|_| denied())?
    );
    if matches!(input.stage, Stage::Close) {
        let _guard = state.employee_guard(&generation).await?;
        state.close_employee_window()?;
        return Ok(None);
    }
    let handle = state.execution_for(&generation).await?;
    let Response::OwnBindings {
        device, bindings, ..
    } = handle.device_access(Operation::OwnBindings {}).await?
    else {
        return Err(denied());
    };
    let _guard = state.employee_guard(&generation).await?;
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
    let change = serde_json::json!({"operationId":op,"expectedRevision":input.revision,"action":{"kind":input.change}});
    if status {
        return session
            .employee_operation(device.into_uuid(), b.id, op)
            .await
            .map(Some);
    }
    let origin_url = url::Url::parse(origin).map_err(|_| denied())?;
    let window = match app.get_webview_window(&label) {
        Some(w) => w,
        None if matches!(input.stage, Stage::Open) => {
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
            let owned = window.clone();
            state.own_employee_window(move || {
                let _ = owned.destroy();
            })?;
            window
        }
        _ => return Err(unavailable()),
    };
    window.set_focus().map_err(|_| unavailable())?;
    if matches!(input.stage, Stage::Open) {
        return Ok(None);
    }
    if window.url().map_err(|_| unavailable())?.origin() != origin_url.origin() {
        return Err(unavailable());
    }
    if matches!(input.stage, Stage::Submit) {
        let cookies = window
            .cookies_for_url(origin_url.clone())
            .map_err(|_| unavailable())?;
        let mut pairs = Vec::new();
        for name in ["__Host-identity-session", "__Host-identity-operation"] {
            let mut found = cookies.iter().filter(|c| c.name() == name);
            if let Some(cookie) = found.next() {
                let value = cookie.value();
                if found.next().is_some()
                    || value.is_empty()
                    || value.len() > 4096
                    || !value
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
                {
                    return Err(unavailable());
                }
                pairs.push(zeroize::Zeroizing::new(format!("{name}={value}")));
            } else if name == "__Host-identity-session" {
                return Err(unavailable());
            }
        }
        let cookie = zeroize::Zeroizing::new(
            pairs
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        );
        drop(_guard);
        let csrf = session.employee_browser_csrf(&cookie).await?;
        let _admitted = state.employee_guard(&generation).await?;
        if window.url().map_err(|_| unavailable())?.origin() != origin_url.origin() {
            return Err(unavailable());
        }
        session
            .submit_employee_change(&cookie, &csrf, device.into_uuid(), b.id, &change)
            .await?;
        return Ok(None);
    }
    let config = serde_json::json!({
        "origin":origin,"tenant":tenant,"principal":principal,"device":device,"binding":b.id,
        "change":change,"returnTarget":session.employee_return_target()
    });
    let script = include_str!("employee-browser.js");
    window
        .eval(format!("({script})({config})"))
        .map_err(|_| unavailable())?;
    Ok(None)
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
