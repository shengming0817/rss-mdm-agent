// ref: Tauri crates/tauri/src/webview/webview_window.rs@tauri-v2.11.2
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod navigation;
mod startup;
use rss_mdm_desktop::self_service::ipc::{self, FixtureState};

fn main() -> std::process::ExitCode {
    match run(FixtureState::new) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            startup::report(error.as_ref());
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(
    initialize: impl FnOnce() -> rss_mdm_desktop::self_service::Result<FixtureState>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = initialize()?;
    ipc::register(tauri::Builder::default(), state)
        .setup(|app| {
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
                .on_navigation(navigation::allowed)
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn initialization_failure_reaches_the_common_reporter_before_creating_a_window() {
        let error = super::run(|| {
            Err(rss_mdm_desktop::self_service::error(
                "clock",
                "无法读取服务时钟",
            ))
        })
        .expect_err("initialization must fail without launching Tauri");
        let message = super::startup::diagnostic(error.as_ref());
        assert!(message.contains("RSS MDM Agent 启动失败"));
        assert!(message.contains("无法读取服务时钟"));
    }
}
