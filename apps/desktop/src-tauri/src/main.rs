// ref: Tauri crates/tauri/src/webview/webview_window.rs@tauri-v2.11.2
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod navigation;
mod startup;

fn main() -> std::process::ExitCode {
    let result = tauri::Builder::default()
        .setup(|app| {
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
                .on_navigation(navigation::allowed)
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!());
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            startup::report(&error);
            std::process::ExitCode::FAILURE
        }
    }
}
