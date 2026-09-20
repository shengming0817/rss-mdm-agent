// ref: Tauri crates/tauri/src/app.rs@tauri-v2.11.2
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod navigation;
mod startup;
use rss_mdm_desktop::composition::{ipc, lifecycle::Lifecycle, runtime::DesktopRuntime};
use tauri::Manager;

fn window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.set_focus()?;
    } else {
        tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
            .on_navigation(navigation::allowed)
            .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
            .build()?;
    }
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let app = ipc::register(tauri::Builder::default())
        .setup(|app| {
            let root = app.path().app_data_dir()?.join("test-users");
            let artifact = std::env::var_os("RSS_AI_HOST_RUNTIME")
                .map(std::path::PathBuf::from)
                .unwrap_or(app.path().resource_dir()?.join("ai-host-runtime"));
            app.manage(tauri::async_runtime::block_on(DesktopRuntime::start(
                &root, &artifact,
            ))?);
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
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            startup::report(error.as_ref());
            std::process::ExitCode::FAILURE
        }
    }
}
