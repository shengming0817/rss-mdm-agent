use super::runtime::DesktopRuntime;
use crate::self_service::*;
use tauri::State;

fn decode<T: serde::de::DeserializeOwned>(input: serde_json::Value) -> Result<T> {
    if serde_json::to_vec(&input)
        .map_err(|_| error("input", "无效请求"))?
        .len()
        > 16384
    {
        return Err(error("limit", "请求超过输入预算"));
    }
    serde_json::from_value(input).map_err(|_| error("input", "请求结构无效"))
}
// Production commands and generated UI contract share these signatures.
macro_rules! commands {
    ($($name:ident($input:ty) -> $output:ty = $method:ident),+ $(,)?) => {
        $(#[tauri::command]
        pub async fn $name(state: State<'_, DesktopRuntime>, generation: String, input: serde_json::Value) -> Result<$output> {
            let handle = state.execution_for(&generation)?;
            let output = handle.$method(decode::<$input>(input)?).await;
            state.current(&generation)?;
            output
        })+
        pub fn wire_schema() -> schemars::Schema {
            #[derive(schemars::JsonSchema)]
            #[allow(dead_code)]
            struct Command<I: schemars::JsonSchema, O: schemars::JsonSchema> { input: I, output: O }
            #[derive(schemars::JsonSchema)]
            #[allow(dead_code)]
            struct SelfServiceCommands { $($name: Command<$input, $output>),+ }
            schemars::generate::SchemaSettings::draft07().for_serialize().into_generator().into_root_schema_for::<SelfServiceCommands>()
        }
    }
}
commands! {
    self_service_snapshot(SnapshotQuery) -> Snapshot = snapshot,
    self_service_preview(Draft) -> PlanView = preview_ui,
    self_service_submit(Submission) -> RequestView = submit_ui,
    self_service_cancel(Submission) -> RequestView = cancel_ui,
    self_service_approve(Submission) -> RequestView = approve_ui,
    self_service_respond(Reply) -> RequestView = respond_ui,
}
#[tauri::command]
pub async fn execution_task_details(
    state: State<'_, DesktopRuntime>,
    request_id: String,
    generation: String,
) -> Result<execution_app::ExecutionTaskDetails> {
    let request = execution_contract::RequestId::new(request_id)
        .map_err(|_| error("input", "无效任务编号"))?;
    let handle = state.execution_for(&generation)?;
    let result = handle
        .details(request)
        .await
        .map_err(|_| error("task_unavailable", "任务不存在或当前无权读取"));
    state.current(&generation)?;
    result
}
#[tauri::command]
pub async fn ai_connect(state: State<'_, DesktopRuntime>, generation: String) -> Result<String> {
    state.connect(&generation).await
}
#[tauri::command]
pub async fn ai_receive(
    state: State<'_, DesktopRuntime>,
    connection_id: String,
) -> Result<Option<serde_json::Value>> {
    state.receive(&connection_id).await
}
#[tauri::command]
pub async fn ai_send(
    state: State<'_, DesktopRuntime>,
    connection_id: String,
    message: serde_json::Value,
) -> Result<()> {
    state.send(&connection_id, message).await
}
#[tauri::command]
pub async fn ai_disconnect(state: State<'_, DesktopRuntime>, connection_id: String) -> Result<()> {
    state.disconnect(&connection_id).await;
    Ok(())
}

#[tauri::command]
pub fn test_users(state: State<'_, DesktopRuntime>) -> Result<ai_session_contract::TestUserPage> {
    Ok(state
        .users
        .lock()
        .map_err(|_| error("users_unavailable", "测试用户记录不可用"))?
        .page())
}
#[tauri::command]
pub async fn select_test_user(
    state: State<'_, DesktopRuntime>,
    name: String,
) -> Result<ai_session_contract::UserContext> {
    state.select_user(&name).await
}

#[tauri::command]
pub async fn enter_connection_credential<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, DesktopRuntime>,
    generation: String,
) -> Result<String> {
    super::credentials::enter(app, state.users.clone(), generation).await
}

pub fn register<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        enter_connection_credential,
        test_users,
        select_test_user,
        self_service_snapshot,
        self_service_preview,
        self_service_submit,
        self_service_cancel,
        self_service_approve,
        self_service_respond,
        execution_task_details,
        ai_connect,
        ai_receive,
        ai_send,
        ai_disconnect
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::{
        test::{get_ipc_response, mock_builder, INVOKE_KEY},
        Manager,
    };
    fn call(
        window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        command: &str,
        url: &str,
        mut body: serde_json::Value,
    ) -> std::result::Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        if let Some(object) = body.as_object_mut() {
            if let Ok(context) = window
                .state::<DesktopRuntime>()
                .users
                .lock()
                .unwrap()
                .current()
            {
                object.insert("generation".into(), serde_json::json!(context.generation));
            }
        }
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
    fn production_acl_and_decode_protect_the_actual_runtime() {
        let root = std::env::temp_dir().join(format!(
            "rss-ipc-{}-{}",
            std::process::id(),
            super::super::execution::now().unwrap()
        ));
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let root = root.canonicalize().unwrap();
        let runtime = tauri::async_runtime::block_on(DesktopRuntime::start(
            &root,
            &root.join("missing-artifact"),
        ))
        .unwrap();
        tauri::async_runtime::block_on(runtime.select_user("Alice")).unwrap();
        let app = register(mock_builder())
            .manage(runtime)
            .build(tauri::generate_context!())
            .unwrap();
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
            serde_json::json!({"input":{"after":null,"requestIds":[]}}),
        )
        .unwrap()
        .deserialize::<serde_json::Value>()
        .unwrap();
        assert_eq!(snapshot["catalog"].as_array().unwrap().len(), 9);
        for command in [
            "self_service_snapshot",
            "self_service_preview",
            "self_service_submit",
            "self_service_approve",
            "self_service_cancel",
            "self_service_respond",
            "execution_task_details",
            "ai_connect",
            "ai_send",
            "ai_receive",
            "ai_disconnect",
        ] {
            assert!(
                call(&other, command, "tauri://localhost", serde_json::json!({})).is_err(),
                "{command}"
            );
            assert!(
                call(
                    &main,
                    command,
                    "https://example.invalid",
                    serde_json::json!({})
                )
                .is_err(),
                "{command}"
            );
        }
        assert!(call(
            &main,
            "run_shell",
            "tauri://localhost",
            serde_json::json!({})
        )
        .is_err());
        for command in [
            "self_service_preview",
            "self_service_submit",
            "self_service_approve",
            "self_service_cancel",
            "self_service_respond",
        ] {
            let error = call(
                &main,
                command,
                "tauri://localhost",
                serde_json::json!({"input":{"approved":true,"secret":"sensitive-canary"}}),
            )
            .err()
            .unwrap();
            assert_eq!(error["code"], "input");
            assert!(!error.to_string().contains("sensitive-canary"));
        }
        tauri::async_runtime::block_on(app.state::<DesktopRuntime>().shutdown());
        std::fs::remove_dir_all(root).unwrap();
    }
}
