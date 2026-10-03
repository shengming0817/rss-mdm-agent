// ref: Tauri crates/tauri/src/app.rs@tauri-v2.11.2
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(all(feature = "native-e2e", not(debug_assertions)))]
compile_error!("native-e2e must never be included in a release build");
#[cfg(feature = "native-e2e")]
mod native_e2e;
#[cfg(all(feature = "dev-fixture", not(debug_assertions)))]
compile_error!("dev-fixture must never be included in a release build");
mod navigation;
mod startup;
#[cfg(not(feature = "dev-fixture"))]
use rss_mdm_desktop::composition::runtime::DesktopRuntime;
use rss_mdm_desktop::composition::{appearance, ipc, lifecycle::Lifecycle};
use tauri::Manager;

fn window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.set_focus()?;
    } else {
        let created = tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
            .on_navigation(navigation::allowed)
            .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
            .build()?;
        appearance::bind(&created);
    }
    Ok(())
}
fn run(_data_root: Option<std::path::PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "dev-fixture"))]
    let default = rss_mdm_desktop::composition::account::bundled_organization()?.ok_or(
        rss_mdm_desktop::organization_config::ConfigurationError(
            "the backend connection before startup",
        ),
    )?;
    if (std::env::var("RSS_DESKTOP_ASSEMBLY").as_deref() == Ok("fixture"))
        != cfg!(feature = "dev-fixture")
    {
        return Err("desktop fixture feature and startup mode must match".into());
    }
    #[cfg(not(feature = "dev-fixture"))]
    let builder = ipc::register(tauri::Builder::default());
    #[cfg(feature = "dev-fixture")]
    let builder = ipc::register_fixture(tauri::Builder::default());
    #[cfg(feature = "native-e2e")]
    let builder = native_e2e::configure(builder, _data_root.as_deref())?;
    let app = builder
        .setup(move |app| {
            app.manage(appearance::Appearance::default());
            eprintln!("RSS_DESKTOP_ASSEMBLY {}", serde_json::json!({"mode": if cfg!(feature = "dev-fixture") { "fixture" } else { "production" }, "productionIPC": !cfg!(feature = "dev-fixture")}));
            #[cfg(not(feature = "dev-fixture"))]
            {
            let root = match &_data_root {
                Some(root) => root.clone(),
                None => app.path().app_data_dir()?.join("desktop"),
            };
            let override_path = if cfg!(debug_assertions) {
                std::env::var_os("RSS_AI_HOST_RUNTIME")
            } else {
                None
            };
            let source = if override_path.is_some() {
                ai_session_contract::HostStatusSource::DevelopmentOverride
            } else {
                ai_session_contract::HostStatusSource::BundledResource
            };
            let artifact = match override_path {
                Some(path) => std::path::PathBuf::from(path),
                None => app.path().resource_dir()?.join("ai-host-runtime"),
            };
            #[cfg(all(feature = "native-e2e", target_os = "macos"))]
            let runtime = tauri::async_runtime::block_on(DesktopRuntime::start_with_key_backend(
                &root,
                &artifact,
                source,
                native_e2e::keychain(&root)?,
            ))?;
            #[cfg(not(all(feature = "native-e2e", target_os = "macos")))]
            let runtime =
                tauri::async_runtime::block_on(DesktopRuntime::start(&root, &artifact, source))?;
            eprintln!(
                "RSS_AI_HOST_STATUS {}",
                serde_json::to_string(&runtime.status())?
            );
            eprintln!(
                "RSS_DEFAULT_ORGANIZATION {}",
                serde_json::to_string(&default)?
            );
            app.manage(runtime);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    let runtime = handle.state::<DesktopRuntime>();
                    if runtime.closed() {
                        break;
                    }
                    let _ = runtime.verify_account().await;
                }
            });
            }
            use tauri::menu::{Menu, MenuItem, Submenu};
            let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit =
                MenuItem::with_id(app, "quit", "退出 RSS MDM Agent", true, Some("CmdOrCtrl+Q"))?;
            let application = Submenu::with_items(app, "RSS MDM Agent", true, &[&show, &quit])?;
            app.set_menu(Menu::with_items(app, &[&application])?)?;
            window(app.handle())?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "show" => {
                let _ = window(app);
            }
            _ => (),
        })
        .build(tauri::generate_context!())?;
    let mut lifecycle = Lifecycle::default();
    app.run(move |app, event| lifecycle.handle(app, event, window));
    Ok(())
}
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(not(feature = "dev-fixture"))]
    if args.len() == 1 && args[0] == "--service-probe" {
        let status = execution_ipc::host::ServiceClient::inspect();
        println!(
            "{}",
            serde_json::to_string(&status).expect("closed service projection")
        );
        return if matches!(status, execution_ipc::host::ServiceView::Connected { .. }) {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::FAILURE
        };
    }
    let data_root = match args.as_slice() {
        [] => None,
        [flag, path]
            if cfg!(feature = "native-e2e")
                && flag == "--test-data-dir"
                && std::path::Path::new(path).is_absolute() =>
        {
            Some(std::path::PathBuf::from(path))
        }
        _ => return std::process::ExitCode::FAILURE,
    };
    match run(data_root) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            startup::report(error.as_ref());
            std::process::ExitCode::FAILURE
        }
    }
}
