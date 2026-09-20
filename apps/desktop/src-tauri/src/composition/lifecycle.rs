//! Production window/process lifetime, shared by the native acceptance carrier.
// ref: Tauri crates/tauri/src/app.rs@tauri-v2.11.2 (RunEvent / ExitRequestApi).
use super::runtime::DesktopRuntime;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Manager;

#[derive(Default)]
pub struct Lifecycle {
    quitting: bool,
    closing_view: bool,
    shutdown_complete: Arc<AtomicBool>,
}

#[derive(Debug, PartialEq, Eq)]
enum Exit {
    Allow,
    KeepAlive,
    Shutdown(i32),
}

impl Lifecycle {
    fn exit_requested(&mut self, code: Option<i32>) -> Exit {
        if self.quitting {
            return if self.shutdown_complete.load(Ordering::Acquire) {
                Exit::Allow
            } else {
                Exit::KeepAlive
            };
        }
        if code.is_none() && std::mem::take(&mut self.closing_view) {
            return Exit::KeepAlive;
        }
        self.quitting = true;
        Exit::Shutdown(code.unwrap_or(0))
    }

    fn reopen(&mut self) -> bool {
        self.closing_view = false;
        !self.quitting
    }

    pub fn handle(
        &mut self,
        app: &tauri::AppHandle,
        event: tauri::RunEvent,
        show: impl FnOnce(&tauri::AppHandle) -> tauri::Result<()>,
    ) {
        match event {
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Destroyed,
                ..
            } if label == "main" => {
                self.closing_view = true;
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    app.state::<DesktopRuntime>().detach_views().await;
                });
            }
            tauri::RunEvent::ExitRequested { api, code, .. } => match self.exit_requested(code) {
                Exit::Allow => (),
                Exit::KeepAlive => api.prevent_exit(),
                Exit::Shutdown(code) => {
                    api.prevent_exit();
                    let app = app.clone();
                    let completed = self.shutdown_complete.clone();
                    tauri::async_runtime::spawn(async move {
                        app.state::<DesktopRuntime>().shutdown().await;
                        completed.store(true, Ordering::Release);
                        app.exit(code);
                    });
                }
            },
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } if self.reopen() => {
                let _ = show(app);
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_and_reopening_a_view_keeps_host_alive_until_explicit_quit() {
        let mut lifecycle = Lifecycle {
            closing_view: true,
            ..Default::default()
        };
        assert_eq!(lifecycle.exit_requested(None), Exit::KeepAlive);
        assert!(lifecycle.reopen());
        assert_eq!(lifecycle.exit_requested(Some(0)), Exit::Shutdown(0));
        assert_eq!(lifecycle.exit_requested(Some(0)), Exit::KeepAlive);
        lifecycle.shutdown_complete.store(true, Ordering::Release);
        assert_eq!(lifecycle.exit_requested(Some(0)), Exit::Allow);
        assert!(!lifecycle.reopen());
    }

    #[test]
    fn explicit_quit_after_close_cannot_be_swallowed_and_preserves_exit_status() {
        let mut lifecycle = Lifecycle {
            closing_view: true,
            ..Default::default()
        };
        assert_eq!(lifecycle.exit_requested(Some(1)), Exit::Shutdown(1));
        assert_eq!(lifecycle.exit_requested(None), Exit::KeepAlive);
        lifecycle.shutdown_complete.store(true, Ordering::Release);
        assert_eq!(lifecycle.exit_requested(Some(1)), Exit::Allow);
    }

    #[test]
    fn reopening_clears_a_close_event_that_did_not_request_exit() {
        let mut lifecycle = Lifecycle {
            closing_view: true,
            ..Default::default()
        };
        assert!(lifecycle.reopen());
        assert_eq!(lifecycle.exit_requested(None), Exit::Shutdown(0));
    }
}
