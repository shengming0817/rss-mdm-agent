//! Real WebView -> AppKit secure entry -> private Host channel -> encrypted SQLite.
//! The master-key backend is injected; this test never accesses the user Keychain.
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

use rss_mdm_desktop::composition::credentials::KeyUnavailable;
struct TestKey;
impl rss_mdm_desktop::composition::credentials::KeyBackend for TestKey {
    fn read(&self) -> Result<Option<Vec<u8>>, KeyUnavailable> {
        Ok(Some(vec![7; 32]))
    }
    fn create(&self, _: &[u8]) -> Result<(), KeyUnavailable> {
        Err(KeyUnavailable)
    }
}
struct Evidence {
    path: PathBuf,
    finished: AtomicBool,
    source: serde_json::Value,
    secret: String,
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
struct SecureEntryAttempt {
    secret: Arc<String>,
    entered: Arc<AtomicBool>,
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn attempt_secure_entry(context: *mut std::ffi::c_void) {
    let attempt = unsafe { Box::from_raw(context.cast::<SecureEntryAttempt>()) };
    if attempt.entered.load(Ordering::Acquire) {
        return;
    }
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
    if fill_secure_field(&content, &attempt.secret) {
        attempt.entered.store(true, Ordering::Release);
        application.stopModalWithCode(NSAlertFirstButtonReturn);
    }
}

#[cfg(target_os = "macos")]
fn automate_secure_entry(secret: String, evidence: Arc<Evidence>) {
    unsafe extern "C" {
        static _dispatch_main_q: u8;
        fn dispatch_async_f(
            queue: *mut std::ffi::c_void,
            context: *mut std::ffi::c_void,
            work: unsafe extern "C" fn(*mut std::ffi::c_void),
        );
    }
    std::thread::spawn(move || {
        let entered = Arc::new(AtomicBool::new(false));
        let secret = Arc::new(secret);
        while !evidence.finished.load(Ordering::Acquire) && !entered.load(Ordering::Acquire) {
            let attempt = Box::new(SecureEntryAttempt {
                secret: Arc::clone(&secret),
                entered: Arc::clone(&entered),
            });
            unsafe {
                dispatch_async_f(
                    std::ptr::addr_of!(_dispatch_main_q)
                        .cast_mut()
                        .cast::<std::ffi::c_void>(),
                    Box::into_raw(attempt).cast(),
                    attempt_secure_entry,
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

fn finish(
    window: &tauri::WebviewWindow,
    evidence: &Evidence,
    mut value: serde_json::Value,
    outputs: Option<&str>,
) {
    let outputs_clean = outputs.is_some_and(|raw| {
        !raw.contains(&evidence.secret)
            && serde_json::from_str::<serde_json::Value>(raw).is_ok_and(|v| {
                v["dom"].is_string() && v["ipc"].is_array() && v["status"].is_object()
            })
    });
    let runtime = window.state::<DesktopRuntime>();
    let diagnostics =
        rss_mdm_desktop::composition::diagnostics::snapshot(&runtime.status()).unwrap();
    let diagnostics_clean = !String::from_utf8_lossy(&diagnostics).contains(&evidence.secret);
    let exported = rss_mdm_desktop::composition::diagnostics::save(
        &evidence.path.with_file_name("diagnostics.json"),
        &diagnostics,
    )
    .is_ok();
    value["secretOutputsClean"] = serde_json::json!(outputs_clean && diagnostics_clean && exported);
    // A failed flow has no snapshot; retain its original failure stage.
    if value["step"] == "passed" && (!outputs_clean || !diagnostics_clean || !exported) {
        value["step"] = serde_json::json!("failed");
        value["stage"] = serde_json::json!("secret_outputs");
    }
    let recorded = std::fs::write(
        &evidence.path,
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
            if value["step"] == "progress" {
                eprintln!("Credential acceptance stage: {}", value["stage"]);
                return;
            }
            if events.finished.swap(true, Ordering::AcqRel) {
                return;
            }
            if value["step"] != "passed" {
                finish(&window, &events, value, None);
                return;
            }
            // Keep large/sensitive snapshots out of the window title and logs.
            let completed = window.clone();
            let evidence = events.clone();
            let result = value.clone();
            if window
                .eval_with_callback("window.__RSS_OBSERVED_OUTPUTS__", move |raw| {
                    finish(&completed, &evidence, result.clone(), Some(&raw));
                })
                .is_err()
            {
                finish(&window, &events, value, None);
            }
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
        secret: secret.clone(),
        finished: AtomicBool::new(false),
        source: serde_json::from_slice(&std::fs::read(root.join("acceptance.json"))?)?,
    });
    let setup = evidence.clone();
    let app = ipc::register(tauri::Builder::default())
        .setup(move |app| {
            app.manage(tauri::async_runtime::block_on(
                DesktopRuntime::start_with_key_backend(
                    &root,
                    &artifact,
                    ai_session_contract::HostStatusSource::DevelopmentOverride,
                    TestKey,
                ),
            )?);
            window(app.handle(), setup.clone())?;
            automate_secure_entry(secret.clone(), setup.clone());
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
