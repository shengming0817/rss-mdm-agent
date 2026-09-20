//! Real WebView -> AppKit secure entry -> Keychain -> native broker -> Host acceptance.
//! The supplied credential is synthetic and is never written to the report or stdout.
use rss_mdm_desktop::composition::{ipc, lifecycle::Lifecycle, runtime::DesktopRuntime};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::Manager;
#[path = "../src/navigation.rs"]
mod navigation;

struct Evidence {
    path: PathBuf,
    finished: AtomicBool,
    source: serde_json::Value,
}

#[cfg(target_os = "macos")]
fn fill_secure_field(view: &objc2_app_kit::NSView, value: &str) -> bool {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSSecureTextField;
    use objc2_foundation::NSString;
    if let Some(field) = (view as &AnyObject).downcast_ref::<NSSecureTextField>() {
        field.setStringValue(&NSString::from_str(value));
        return true;
    }
    view.subviews()
        .iter()
        .any(|child| fill_secure_field(&child, value))
}

#[cfg(target_os = "macos")]
fn automate_secure_entry(app: tauri::AppHandle, secret: String, evidence: Arc<Evidence>) {
    std::thread::spawn(move || {
        let entered = Arc::new(AtomicBool::new(false));
        let secret = Arc::new(secret);
        while !evidence.finished.load(Ordering::Acquire) && !entered.load(Ordering::Acquire) {
            let entered_on_main = entered.clone();
            let value = Arc::clone(&secret);
            let _ = app.run_on_main_thread(move || {
                use objc2::MainThreadMarker;
                use objc2_app_kit::{NSAlertFirstButtonReturn, NSApplication};
                let Some(mtm) = MainThreadMarker::new() else {
                    return;
                };
                let application = NSApplication::sharedApplication(mtm);
                let Some(window) = application.modalWindow() else {
                    return;
                };
                let Some(content) = window.contentView() else {
                    return;
                };
                if fill_secure_field(&content, &value) {
                    entered_on_main.store(true, Ordering::Release);
                    application.stopModalWithCode(NSAlertFirstButtonReturn);
                }
            });
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

fn window(app: &tauri::AppHandle, evidence: Arc<Evidence>) -> tauri::Result<()> {
    let events = evidence.clone();
    tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
        .on_navigation(navigation::allowed)
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .on_page_load(move |window, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let script = format!(
                    "window.__RSS_CUSTOM_CONNECTION__={};\n{}",
                    evidence.source,
                    include_str!("../../../../tests/desktop/custom-connection-flow.js")
                );
                let _ = window.eval(&script);
            }
        })
        .on_document_title_changed(move |window, title| {
            let Some(raw) = title.strip_prefix("RSS_CUSTOM_CONNECTION:") else {
                return;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
                return;
            };
            if value["step"] == "progress" || events.finished.swap(true, Ordering::AcqRel) {
                return;
            }
            let recorded = std::fs::write(
                &events.path,
                serde_json::to_vec_pretty(&value).unwrap_or_default(),
            )
            .is_ok();
            window
                .app_handle()
                .exit(if recorded && value["step"] == "passed" {
                    0
                } else {
                    1
                });
        })
        .build()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        return Err("acceptance is macOS arm64 only".into());
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: custom-connection-acceptance PRIVATE_RUN_DIRECTORY FIXED_ARTIFACT_DIRECTORY REPORT_FILE".into());
    }
    let root = PathBuf::from(&args[0]);
    let artifact = PathBuf::from(&args[1]);
    let report = PathBuf::from(&args[2]);
    if root.join("ai.sqlite").exists() || report.exists() {
        return Err("fresh acceptance directory required".into());
    }
    let secret_path = root.join("fixture-secret");
    let secret = std::fs::read_to_string(&secret_path)?;
    std::fs::remove_file(&secret_path)?;
    if secret.is_empty()
        || secret.len() > 256
        || secret.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
    {
        return Err("invalid synthetic fixture input".into());
    }
    let evidence = Arc::new(Evidence {
        path: report,
        finished: AtomicBool::new(false),
        source: serde_json::from_slice(&std::fs::read(root.join("acceptance.json"))?)?,
    });
    let setup = evidence.clone();
    let app = ipc::register(tauri::Builder::default())
        .setup(move |app| {
            app.manage(tauri::async_runtime::block_on(DesktopRuntime::start(
                &root, &artifact,
            ))?);
            window(app.handle(), setup.clone())?;
            automate_secure_entry(app.handle().clone(), secret.clone(), setup.clone());
            let handle = app.handle().clone();
            let timeout = setup.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(180)).await;
                if !timeout.finished.swap(true, Ordering::AcqRel) {
                    let _ = std::fs::write(
                        &timeout.path,
                        b"{\"step\":\"failed\",\"stage\":\"native_timeout\"}",
                    );
                    handle.exit(1);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())?;
    let mut lifecycle = Lifecycle::default();
    app.run(move |app, event| lifecycle.handle(app, event, |app| window(app, evidence.clone())));
    Ok(())
}
