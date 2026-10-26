use super::appearance::AppearanceSnapshot;
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
fn decode_connection(
    input: serde_json::Value,
    expected: Option<u64>,
) -> Result<ai_session_contract::ConnectionDraft> {
    let bytes = serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"kind":"saveConnectionRequest","connection":input,"expectedRevision":expected})).map_err(|_| error("input", "无效连接"))?;
    let record = ai_session_contract::decode(
        &bytes,
        &ai_session_contract::Limits {
            max_bytes: 65536,
            max_text_bytes: 32768,
            max_depth: 16,
            max_nodes: 4096,
        },
    )
    .map_err(|_| error("input", "连接结构无效"))?;
    match record {
        ai_session_contract::WireRecord::SaveConnectionRequest(request) => Ok(request.connection),
        _ => Err(error("input", "连接结构无效")),
    }
}
// Production commands and generated UI contract share these signatures.
macro_rules! commands {
    ($($name:ident($input:ty) -> $output:ty = $method:ident),+ $(,)?) => {
        $(#[tauri::command]
        pub async fn $name(state: State<'_, DesktopRuntime>, generation: String, input: serde_json::Value) -> Result<$output> {
            let output: Result<$output> = async {
                let handle = state.execution_for(&generation).await?;
                let output = handle.$method(decode::<$input>(input)?).await;
                state.current(&generation)?;
                output
            }.await;
            if let Err(error) = &output {
                eprintln!("RSS_NATIVE_COMMAND_FAILURE command={} code={}", stringify!($name), error.code);
            }
            output
        })+
        pub fn wire_schema() -> schemars::Schema {
            #[derive(schemars::JsonSchema)]
            #[allow(dead_code)]
            struct Command<I: schemars::JsonSchema, O: schemars::JsonSchema> { input: I, output: O }
            #[derive(schemars::JsonSchema)]
            #[allow(dead_code)]
            struct SelfServiceCommands { personal_vault: Command<super::personal_vault::Operation, super::personal_vault::Status>, $($name: Command<$input, $output>),+, diagnostics_prepare: Command<super::diagnostics::Scopes, super::diagnostics::Preview>, diagnostics_export: Command<super::diagnostics::Selection,bool>, diagnostics_discard: Command<super::diagnostics::Selection,()>, appearance_snapshot: Command<(), AppearanceSnapshot>, device_overview: Command<(), execution_ipc::host::DeviceOverview> }
            schemars::generate::SchemaSettings::draft07().for_serialize().into_generator().into_root_schema_for::<SelfServiceCommands>()
        }
    }
}
commands! {
    self_service_offers(()) -> Vec<execution_contract::BackendTask> = offers_ui,
    task_list(execution_app::tasks::TaskQuery) -> execution_app::tasks::TaskPage = task_list,
    task_detail(execution_app::tasks::TaskRef) -> execution_app::tasks::TaskDetail = task_detail,
    task_control(execution_app::tasks::TaskControl) -> execution_app::tasks::TaskControlReceipt = task_control,
    task_control_status(execution_app::tasks::TaskControlQuery) -> execution_app::tasks::TaskControlReceipt = task_control_status,
    network_check(execution_contract::NetworkCheckOperation) -> execution_app::NetworkCheckResponse = network_check,
    system_settings(execution_contract::SettingsOperation) -> execution_contract::SettingsResponse = system_settings,
    device_access(execution_ipc::device_access::Operation) -> execution_ipc::device_access::Response = device_access,
    software_prepare(execution_contract::LocalSoftwareChoice) -> execution_contract::LocalSoftwarePrepared = software_prepare,
    software_submit(execution_contract::LocalSoftwareSubmission) -> execution_app::LocalSoftwareStatus = software_submit,
    software_status(execution_contract::QueryRequest) -> execution_app::LocalSoftwareStatus = software_status,
    software_history(execution_contract::SoftwareHistoryQuery) -> execution_app::LocalSoftwareHistory = software_history,
    software_read(execution_contract::SoftwareRead) -> execution_contract::SoftwarePage = software_read,
    query_directory(execution_contract::QueryDirectory) -> execution_contract::QueryPage = query_directory,
    data_read(execution_contract::DataRead) -> execution_contract::CollectedData = data_read,
    query_submit(execution_contract::QuerySubmit) -> execution_contract::QueryStatus = query_submit,
    query_status(execution_contract::QueryRequest) -> execution_contract::QueryStatus = query_status,
    self_service_observe_software(execution_contract::SoftwareReadChoice) -> execution_contract::BackendRequest = observe_software,
    self_service_policy_resolve(rss_mdm_agent_wire::runtime::CapabilitySelector) -> rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem = policy_resolve,
    self_service_policy_directory(PolicyDirectoryQuery) -> rss_mdm_agent_wire::runtime::SelfServiceDirectoryResponse = policy_directory,
    self_service_policy_invoke(execution_contract::PolicyInvocation) -> execution_contract::BackendRequest = invoke_policy,
    ai_operation_submit(execution_contract::PolicySubmission) -> execution_contract::BackendRequest = submit_policy,
    self_service_policy_confirm(ActionRef) -> execution_contract::BackendRequest = confirm_policy,
    self_service_policy_status(ActionRef) -> PolicyStatus = policy_status,
    self_service_execute(execution_contract::BackendSelection) -> execution_contract::TaskSubmission = execute_ui,
    self_service_confirm(execution_contract::BackendSelection) -> execution_contract::TaskSubmission = confirm_ui,
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
pub async fn select_device(
    state: State<'_, DesktopRuntime>,
) -> Result<ai_session_contract::UserContext> {
    state.select_device().await
}
#[tauri::command]
pub async fn account_logout(state: State<'_, DesktopRuntime>) -> Result<()> {
    state.logout().await
}
#[tauri::command]
pub async fn account_status(
    state: State<'_, DesktopRuntime>,
) -> Result<ai_session_contract::AccountStatus> {
    state.account_status().await
}
#[tauri::command]
pub fn account_organizations(
    state: State<'_, DesktopRuntime>,
) -> Result<ai_session_contract::AccountSettings> {
    let settings = state
        .organizations
        .lock()
        .map_err(|_| error("users_unavailable", "组织配置不可用"))?;
    Ok(ai_session_contract::AccountSettings {
        kind: ai_session_contract::AccountSettingsKind::AccountSettings,
        schema_version: ai_session_contract::AccountSettingsSchemaVersion::VALUE,
        organizations: settings.list(),
        selected: settings.selected(),
    })
}
#[tauri::command]
pub fn account_save_organization(
    state: State<'_, DesktopRuntime>,
    input: serde_json::Value,
) -> Result<ai_session_contract::AccountSettings> {
    let mut settings = state
        .organizations
        .lock()
        .map_err(|_| error("users_unavailable", "组织配置不可用"))?;
    let saved = settings.save(decode(input)?)?;
    Ok(ai_session_contract::AccountSettings {
        kind: ai_session_contract::AccountSettingsKind::AccountSettings,
        schema_version: ai_session_contract::AccountSettingsSchemaVersion::VALUE,
        organizations: settings.list(),
        selected: Some(saved.id),
    })
}
#[tauri::command]
pub async fn account_login(
    state: State<'_, DesktopRuntime>,
    input: super::account::EnterpriseCredentials,
) -> Result<ai_session_contract::UserContext> {
    input.validate()?;
    let organization = state
        .organizations
        .lock()
        .map_err(|_| error("users_unavailable", "组织配置不可用"))?
        .get(&input.organization_id)?;
    state
        .login(organization, &input.login, input.password)
        .await
}
#[tauri::command]
pub async fn account_recover(
    state: State<'_, DesktopRuntime>,
    clear: bool,
) -> Result<Option<ai_session_contract::UserContext>> {
    state.recover_account(clear).await
}

#[tauri::command]
pub async fn save_connection(
    state: State<'_, DesktopRuntime>,
    generation: String,
    input: serde_json::Value,
    expected: Option<u64>,
    secret: Option<String>,
) -> Result<serde_json::Value> {
    let secret = secret.map(zeroize::Zeroizing::new);
    state.current(&generation)?;
    let connection = decode_connection(input, expected)?;
    state
        .save_connection(&generation, connection, expected, secret)
        .await
}

#[tauri::command]
pub fn ai_host_status(state: State<'_, DesktopRuntime>) -> ai_session_contract::HostStatus {
    state.status()
}
#[tauri::command]
pub async fn ai_restart_host(
    state: State<'_, DesktopRuntime>,
    generation: u64,
) -> Result<ai_session_contract::HostStatus> {
    Ok(state.restart(generation).await)
}
#[tauri::command]
pub async fn diagnostics_prepare(
    state: State<'_, DesktopRuntime>,
    input: super::diagnostics::Scopes,
) -> Result<super::diagnostics::Preview> {
    super::diagnostics::prepare(&state, input).await
}
#[tauri::command]
pub async fn diagnostics_export<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, DesktopRuntime>,
    input: super::diagnostics::Selection,
) -> Result<bool> {
    super::diagnostics::export(app, &state, input).await
}
#[tauri::command]
pub fn diagnostics_discard(
    state: State<'_, DesktopRuntime>,
    input: super::diagnostics::Selection,
) -> Result<()> {
    super::diagnostics::discard(&state, &input)
}

#[tauri::command]
pub fn development_assembly() -> &'static str {
    if cfg!(feature = "dev-fixture") {
        "fixture"
    } else {
        "production"
    }
}
#[tauri::command]
pub async fn execution_service_status() -> execution_ipc::host::ServiceView {
    tokio::task::spawn_blocking(execution_ipc::host::ServiceClient::inspect)
        .await
        .unwrap_or(execution_ipc::host::ServiceView::Unavailable)
}
async fn registration_request(
    request: execution_ipc::host::Request,
) -> Result<execution_ipc::host::RegistrationStatus> {
    tokio::task::spawn_blocking(move || {
        let client = execution_ipc::host::ServiceClient::installed()
            .map_err(|_| error("service_unavailable", "本机服务不可用"))?;
        match client
            .request(request)
            .map_err(|_| error("service_unavailable", "本机服务不可用"))?
        {
            execution_ipc::host::Reply::RegistrationStatus { value } => Ok(value),
            execution_ipc::host::Reply::Rejected => {
                Err(error("registration_denied", "当前桌面没有注册权限"))
            }
            _ => Err(error("service_unavailable", "本机服务不可用")),
        }
    })
    .await
    .map_err(|_| error("service_unavailable", "本机服务不可用"))?
}
#[tauri::command]
pub async fn agent_registration_status() -> Result<execution_ipc::host::RegistrationStatus> {
    registration_request(execution_ipc::host::Request::RegistrationStatus {}).await
}
#[tauri::command]
pub async fn agent_request_registration(
    state: State<'_, DesktopRuntime>,
    input: Option<super::account::EnterpriseCredentials>,
) -> Result<execution_ipc::host::RegistrationStatus> {
    if let Some(input) = &input {
        input.validate()?;
    }
    state.register_device(input).await
}

#[tauri::command]
pub async fn agent_cancel_registration(state: State<'_, DesktopRuntime>) -> Result<()> {
    state.cancel_registration_wait().await;
    Ok(())
}
#[tauri::command]
pub async fn agent_import_registration<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<execution_ipc::host::RegistrationStatus>> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        let selected = (|| -> std::io::Result<Option<std::path::PathBuf>> {
            use objc2::MainThreadMarker;
            use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
            let panel =
                NSOpenPanel::openPanel(MainThreadMarker::new().ok_or(std::io::ErrorKind::Other)?);
            panel.setCanChooseDirectories(false);
            panel.setCanChooseFiles(true);
            panel.setAllowsMultipleSelection(false);
            if panel.runModal() != NSModalResponseOK {
                return Ok(None);
            }
            Ok(Some(std::path::PathBuf::from(
                panel
                    .URL()
                    .and_then(|v| v.path())
                    .ok_or(std::io::ErrorKind::InvalidData)?
                    .to_string(),
            )))
        })();
        #[cfg(windows)]
        let selected = super::windows_dialogs::configuration_dialog();
        #[cfg(not(any(target_os = "macos", windows)))]
        let selected: std::io::Result<Option<std::path::PathBuf>> =
            Err(std::io::ErrorKind::Unsupported.into());
        let _ = sender.send(selected);
    })
    .map_err(|_| error("configuration_unavailable", "无法打开配置选择器"))?;
    let Some(path) = receiver
        .await
        .map_err(|_| error("configuration_unavailable", "配置选择未完成"))?
        .map_err(|_| error("configuration_unavailable", "配置选择未完成"))?
    else {
        return Ok(None);
    };
    let file = tokio::task::spawn_blocking(move || {
        let bytes = zeroize::Zeroizing::new(
            installation_security::read_external(&path, 65536)
                .map_err(|_| error("configuration_invalid", "配置文件不可读或不安全"))?,
        );
        serde_json::from_slice::<rss_mdm_agent_wire::enroll::configuration::ConfigurationFile>(
            &bytes,
        )
        .map_err(|_| error("configuration_invalid", "请选择当前版本的完整签名注册配置"))
    })
    .await
    .map_err(|_| error("configuration_unavailable", "读取配置未完成"))??;
    registration_request(execution_ipc::host::Request::ImportRegistration { file })
        .await
        .map(Some)
}
#[tauri::command]
pub async fn mdm_registration_status() -> Result<execution_ipc::host::MdmStatus> {
    tokio::task::spawn_blocking(|| {
        let client = execution_ipc::host::ServiceClient::installed()
            .map_err(|_| error("service_unavailable", "本机服务不可用"))?;
        match client.request(execution_ipc::host::Request::MdmStatus {}) {
            Ok(execution_ipc::host::Reply::MdmStatus { value }) => Ok(value),
            _ => Err(error("mdm_unavailable", "无法读取 MDM 注册状态")),
        }
    })
    .await
    .map_err(|_| error("service_unavailable", "本机服务不可用"))?
}
#[tauri::command]
pub async fn device_overview() -> Result<execution_ipc::host::DeviceOverview> {
    static READING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let permit = READING
        .try_acquire()
        .map_err(|_| error("device_busy", "设备信息读取中"))?;
    let read = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let client = execution_ipc::host::ServiceClient::installed()
            .map_err(|_| error("service_unavailable", "本机服务不可用"))?;
        match client.request(execution_ipc::host::Request::DeviceOverview {}) {
            Ok(execution_ipc::host::Reply::DeviceOverview { value }) => Ok(value),
            _ => Err(error("device_unavailable", "设备信息读取失败")),
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), read)
        .await
        .map_err(|_| error("device_timeout", "设备信息读取超时"))?
        .map_err(|_| error("device_unavailable", "设备信息读取失败"))?
}
#[tauri::command]
pub async fn appearance_snapshot<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
) -> Result<AppearanceSnapshot> {
    super::appearance::snapshot(window).await
}
#[tauri::command]
pub async fn personal_vault<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    input: super::personal_vault::Operation,
) -> std::result::Result<super::personal_vault::Status, passkey_vault::Error> {
    super::personal_vault::execute(window, input).await
}
pub fn register<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        personal_vault,
        device_overview,
        appearance_snapshot,
        development_assembly,
        select_device,
        account_logout,
        account_status,
        account_organizations,
        account_save_organization,
        account_login,
        account_recover,
        execution_service_status,
        agent_registration_status,
        mdm_registration_status,
        agent_request_registration,
        agent_import_registration,
        agent_cancel_registration,
        ai_host_status,
        ai_restart_host,
        diagnostics_prepare,
        diagnostics_export,
        diagnostics_discard,
        save_connection,
        test_users,
        select_test_user,
        query_directory,
        data_read,
        query_submit,
        query_status,
        self_service_observe_software,
        device_access,
        software_history,
        software_read,
        software_prepare,
        system_settings,
        network_check,
        software_submit,
        software_status,
        self_service_policy_directory,
        self_service_policy_resolve,
        self_service_policy_invoke,
        self_service_policy_confirm,
        ai_operation_submit,
        self_service_policy_status,
        task_list,
        task_detail,
        task_control,
        task_control_status,
        self_service_offers,
        self_service_execute,
        self_service_confirm,
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
        platform_private_storage::directory(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let mut runtime = tauri::async_runtime::block_on(DesktopRuntime::start(
            &root,
            &root.join("missing-artifact"),
            ai_session_contract::HostStatusSource::DevelopmentOverride,
        ))
        .unwrap();
        tauri::async_runtime::block_on(runtime.select_user("Alice")).unwrap();
        runtime.execution = runtime.execution.clone().with_context_transport(|request| {
            use execution_app::tasks::{TaskOperation, TaskPage, TaskResponse};
            match request {
                execution_ipc::host::Request::Task {
                    input: TaskOperation::List(query),
                    ..
                } => {
                    assert_eq!(query.limit, 32);
                    Ok(execution_ipc::host::Reply::Task {
                        value: TaskResponse::Page(TaskPage {
                            items: vec![],
                            next: None,
                            scope: "original-owner".into(),
                            as_of: 0,
                            last_sync: None,
                            remote_available: false,
                            coverage: vec![],
                            retained_from: None,
                            complete: true,
                        }),
                    })
                }
                _ => Err(execution_app::Error::Unavailable),
            }
        });
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
        let page = call(
            &main, "task_list", "tauri://localhost",
            serde_json::json!({"input":execution_app::tasks::TaskQuery { limit:32, ..Default::default() }}),
        ).unwrap().deserialize::<execution_app::tasks::TaskPage>().unwrap();
        assert_eq!(page.scope, "original-owner");
        assert!(page.items.is_empty());
        // Production has no private S1 catalog or journal fallback. The installed service
        // may be absent in this ACL test; either response must retain that boundary.
        match call(
            &main,
            "self_service_offers",
            "tauri://localhost",
            serde_json::json!({"input":null}),
        ) {
            Ok(reply) => {
                let snapshot = reply.deserialize::<serde_json::Value>().unwrap();
                assert!(snapshot.get("catalog").is_none());
                assert!(snapshot.is_array());
            }
            Err(error) => assert_ne!(error["code"], "input"),
        }
        let service = call(
            &main,
            "execution_service_status",
            "tauri://localhost",
            serde_json::json!({}),
        )
        .unwrap()
        .deserialize::<serde_json::Value>()
        .unwrap();
        assert!(service.get("phase").is_some());
        for command in [
            "select_device",
            "account_logout",
            "account_status",
            "account_login",
            "account_recover",
            "account_organizations",
            "account_save_organization",
            "execution_service_status",
            "agent_registration_status",
            "mdm_registration_status",
            "agent_request_registration",
            "agent_import_registration",
            "agent_cancel_registration",
            "appearance_snapshot",
            "device_overview",
            "ai_host_status",
            "ai_restart_host",
            "diagnostics_prepare",
            "diagnostics_export",
            "diagnostics_discard",
            "self_service_observe_software",
            "device_access",
            "software_history",
            "software_read",
            "software_prepare",
            "system_settings",
            "network_check",
            "software_submit",
            "software_status",
            "task_list",
            "task_detail",
            "task_control_status",
            "task_control",
            "self_service_policy_directory",
            "self_service_policy_resolve",
            "self_service_policy_invoke",
            "self_service_policy_confirm",
            "ai_operation_submit",
            "self_service_policy_status",
            "self_service_offers",
            "self_service_execute",
            "self_service_confirm",
            "task_control",
            "save_connection",
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
            "self_service_execute",
            "self_service_confirm",
            "task_control",
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

#[cfg(feature = "dev-fixture")]
pub fn register_fixture<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        appearance_snapshot,
        development_assembly
    ])
}
