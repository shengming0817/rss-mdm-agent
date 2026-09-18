// ref: Tauri crates/tauri/src/test/mod.rs@tauri-v2.11.2
use super::*;
use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::State;
pub struct FixtureState(pub Mutex<FixtureService>);
fn now() -> Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| error("clock", "无法读取服务时钟"))?
            .as_millis(),
    )
    .map_err(|_| error("clock", "服务时钟超出范围"))
}
impl FixtureState {
    pub fn new() -> Result<Self> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| error("clock", "无法读取服务时钟"))?;
        Ok(Self(Mutex::new(FixtureService::new(
            format!("fixture-{}", time.as_nanos()),
            now()?,
        )?)))
    }
}
fn with_service<T>(
    state: State<'_, FixtureState>,
    f: impl FnOnce(&mut FixtureService, u64) -> Result<T>,
) -> Result<T> {
    let mut service = state
        .0
        .lock()
        .map_err(|_| error("unavailable", "测试服务不可用，请退出后重新启动"))?;
    f(&mut service, now()?)
}
#[tauri::command]
pub fn self_service_snapshot(state: State<'_, FixtureState>) -> Result<Snapshot> {
    with_service(state, |service, now| service.snapshot(now))
}
#[tauri::command]
pub fn self_service_preview(
    state: State<'_, FixtureState>,
    input: serde_json::Value,
) -> Result<PlanView> {
    with_service(state, |service, now| service.preview(decode(input)?, now))
}
#[tauri::command]
pub fn self_service_submit(
    state: State<'_, FixtureState>,
    input: serde_json::Value,
) -> Result<RequestView> {
    with_service(state, |service, now| service.submit(decode(input)?, now))
}
#[tauri::command]
pub fn self_service_respond(
    state: State<'_, FixtureState>,
    input: serde_json::Value,
) -> Result<RequestView> {
    with_service(state, |service, now| service.respond(decode(input)?, now))
}

fn decode<T: serde::de::DeserializeOwned>(input: serde_json::Value) -> Result<T> {
    if serde_json::to_vec(&input)
        .map_err(|_| error("input", "无效请求"))?
        .len()
        > 16384
    {
        return Err(error("limit", "请求超过输入预算"));
    }
    serde_json::from_value(input)
        .map_err(|_| error("input", "请求结构无效；不支持未知字段或回答类型"))
}
pub fn register<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    state: FixtureState,
) -> tauri::Builder<R> {
    builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            self_service_snapshot,
            self_service_preview,
            self_service_submit,
            self_service_respond
        ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{get_ipc_response, mock_builder, INVOKE_KEY};
    fn app() -> tauri::App<tauri::test::MockRuntime> {
        register(mock_builder(), FixtureState::new().unwrap())
            .build(tauri::generate_context!())
            .expect("actual app ACL context")
    }
    fn call(
        window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        command: &str,
        url: &str,
        body: serde_json::Value,
    ) -> std::result::Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        get_ipc_response(
            window,
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: url.parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.into(),
            },
        )
    }
    #[test]
    fn actual_manifest_limits_fixture_ipc_to_local_main() {
        let app = app();
        let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let other = tauri::WebviewWindowBuilder::new(&app, "other", Default::default())
            .build()
            .unwrap();
        let snapshot = call(
            &main,
            "self_service_snapshot",
            "tauri://localhost",
            serde_json::json!({}),
        )
        .unwrap()
        .deserialize::<serde_json::Value>()
        .unwrap();
        assert_eq!(snapshot["mode"], "fixture");
        assert!(call(
            &other,
            "self_service_snapshot",
            "tauri://localhost",
            serde_json::json!({})
        )
        .is_err());
        assert!(call(
            &main,
            "self_service_snapshot",
            "https://example.invalid",
            serde_json::json!({})
        )
        .is_err());
        assert!(call(
            &main,
            "run_shell",
            "tauri://localhost",
            serde_json::json!({})
        )
        .is_err());
    }
    #[test]
    fn malformed_ipc_does_not_echo_sensitive_values_or_mint_authority() {
        let app = app();
        let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        for command in [
            "self_service_preview",
            "self_service_submit",
            "self_service_respond",
        ] {
            let err = call(
                &main,
                command,
                "tauri://localhost",
                serde_json::json!({"input":{"approved":true,"secret":"sensitive-canary"}}),
            )
            .err()
            .unwrap();
            assert!(!err.to_string().contains("sensitive-canary"));
            assert_eq!(err["code"], "input");
        }
    }
    #[test]
    fn all_four_commands_roundtrip_the_real_wire() {
        let app = app();
        let main = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let invoke = |command: &str, body: serde_json::Value| {
            call(&main, command, "tauri://localhost", body)
                .unwrap()
                .deserialize::<serde_json::Value>()
                .unwrap()
        };
        let snapshot = invoke("self_service_snapshot", serde_json::json!({}));
        let item = snapshot["catalog"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["itemId"] == "diagnostics")
            .unwrap();
        let plan = invoke(
            "self_service_preview",
            serde_json::json!({"input":{"instanceId":snapshot["instanceId"],"requestId":"ipc-request","revision":1,"catalog":item["catalog"],"itemId":"diagnostics","variantId":"test","fields":{"host":{"kind":"text","value":"example.invalid"}}}}),
        );
        let task = invoke(
            "self_service_submit",
            serde_json::json!({"input":{"instanceId":snapshot["instanceId"],"requestId":plan["requestId"],"planId":plan["planId"],"digest":plan["digest"]}}),
        );
        assert_eq!(task["status"], "waiting");
        let task = invoke(
            "self_service_respond",
            serde_json::json!({"input":{"instanceId":snapshot["instanceId"],"requestId":plan["requestId"],"interactionId":task["interactions"][0]["id"],"commandId":"ipc-answer","answer":{"kind":"confirmation","accepted":true}}}),
        );
        assert_eq!(task["interactions"][1]["kind"]["kind"], "privacyConsent");
    }
}
