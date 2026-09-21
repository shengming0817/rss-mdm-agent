//! Real WebView + production IPC + fixed Node artifact + existing Codex login acceptance.
//! No model fixtures, IPC replacement, or OS executor. Inputs are private run/artifact paths.
use rss_mdm_desktop::composition::{ipc, lifecycle::Lifecycle, runtime::DesktopRuntime};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::Manager;
#[path = "../src/navigation.rs"]
mod navigation;
struct Evidence {
    path: PathBuf,
    phase: AtomicUsize,
    finished: AtomicBool,
    source: serde_json::Value,
}
fn window(app: &tauri::AppHandle, evidence: Arc<Evidence>) -> tauri::Result<()> {
    let events = evidence.clone();
    tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
        .on_navigation(navigation::allowed)
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .on_page_load(move |window, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let script = format!(
                    "window.__RSS_ACCEPTANCE_PHASE__={};window.__RSS_CONNECTION_SOURCE__={};\n{}",
                    evidence.phase.load(Ordering::Acquire),
                    evidence.source,
                    include_str!("../../../../tests/desktop/native-flow.js")
                );
                let _ = window.eval(&script);
            }
        })
        .on_document_title_changed(move |window, title| {
            let Some(raw) = title.strip_prefix("RSS_ACCEPTANCE:") else {
                return;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
                return;
            };
            eprintln!("Native acceptance: {}", value);
            if value["step"] == "progress" {
                return;
            }
            if value["step"] == "detach" {
                if events.phase.swap(1, Ordering::AcqRel) != 0 {
                    return;
                }
                let app = window.app_handle().clone();
                let next = events.clone();
                let _ = window.destroy();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    let ui = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        let _ = self::window(&ui, next);
                    });
                });
            } else if !events.finished.swap(true, Ordering::AcqRel) {
                let recorded =
                    std::fs::write(&events.path, serde_json::to_vec_pretty(&value).unwrap())
                        .is_ok();
                let app = window.app_handle().clone();
                app.exit(if recorded && value["step"] == "passed" {
                    0
                } else {
                    1
                });
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
        return Err(
            "usage: desktop-acceptance PRIVATE_RUN_DIRECTORY FIXED_ARTIFACT_DIRECTORY REPORT_FILE"
                .into(),
        );
    }
    let root = PathBuf::from(&args[0]);
    let artifact = PathBuf::from(&args[1]);
    if root.join("ai.sqlite").exists()
        || root.join("execution.sqlite").exists()
        || PathBuf::from(&args[2]).exists()
    {
        return Err("fresh acceptance directory required".into());
    }
    let evidence = Arc::new(Evidence {
        path: PathBuf::from(&args[2]),
        phase: AtomicUsize::new(0),
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
            let probe=app.handle().clone();
            let finished=setup.clone();
            tauri::async_runtime::spawn(async move {
                while !finished.finished.load(Ordering::Acquire) {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    if let Some(view)=probe.get_webview_window("main") { let _=view.eval("document.title='RSS_ACCEPTANCE:'+JSON.stringify({step:'progress',sessionRows:document.querySelectorAll('.assistant-sessions li button').length,messages:document.querySelectorAll('.assistant .message').length,facts:document.querySelector('.assistant-facts')?.textContent,alerts:[...document.querySelectorAll('[role=alert]')].map(e=>e.textContent)});"); }
                }
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(240)).await;
                if !setup.finished.swap(true, Ordering::AcqRel) {
                    let _ = std::fs::write(
                        &setup.path,
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
