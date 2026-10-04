//! Production window/process lifetime, shared by the native acceptance carrier.
// ref: Tauri crates/tauri/src/app.rs@tauri-v2.11.2 (RunEvent / ExitRequestApi).
#[cfg(not(feature = "dev-fixture"))]
use super::runtime::DesktopRuntime;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[cfg(not(feature = "dev-fixture"))]
use tauri::Manager;

#[derive(Default)]
pub struct Lifecycle {
    quitting: bool,
    closing_view: bool,
    shutdown_complete: Arc<AtomicBool>,
    shutdown_failed: Arc<AtomicBool>,
}

#[derive(Debug, PartialEq, Eq)]
enum Exit {
    Allow,
    KeepAlive,
    Shutdown(i32),
}

impl Lifecycle {
    fn exit_requested(&mut self, code: Option<i32>) -> Exit {
        if self.shutdown_failed.swap(false, Ordering::AcqRel) {
            self.quitting = false;
        }
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
                    #[cfg(not(feature = "dev-fixture"))]
                    app.state::<DesktopRuntime>().detach_views().await;
                    #[cfg(feature = "dev-fixture")]
                    let _ = app;
                });
            }
            tauri::RunEvent::ExitRequested { api, code, .. } => match self.exit_requested(code) {
                Exit::Allow => (),
                Exit::KeepAlive => api.prevent_exit(),
                Exit::Shutdown(code) => {
                    api.prevent_exit();
                    let app = app.clone();
                    let completed = self.shutdown_complete.clone();
                    let failed = self.shutdown_failed.clone();
                    tauri::async_runtime::spawn(async move {
                        #[cfg(not(feature = "dev-fixture"))]
                        {
                            let runtime = app.state::<DesktopRuntime>();
                            runtime.shutdown().await;
                            if !runtime.closed() {
                                failed.store(true, Ordering::Release);
                                return;
                            }
                        }
                        #[cfg(feature = "dev-fixture")]
                        let _ = failed;
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
    fn unconfirmed_shutdown_blocks_exit_and_allows_an_explicit_retry() {
        let mut lifecycle = Lifecycle::default();
        assert_eq!(lifecycle.exit_requested(Some(0)), Exit::Shutdown(0));
        assert_eq!(lifecycle.exit_requested(Some(0)), Exit::KeepAlive);
        lifecycle.shutdown_failed.store(true, Ordering::Release);
        assert_eq!(lifecycle.exit_requested(Some(0)), Exit::Shutdown(0));
        assert_eq!(lifecycle.exit_requested(Some(0)), Exit::KeepAlive);
        lifecycle.shutdown_complete.store(true, Ordering::Release);
        assert_eq!(lifecycle.exit_requested(Some(0)), Exit::Allow);
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
