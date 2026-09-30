//! Window appearance is product presentation, never execution authority.
// ref: tauri crates/tauri/src/vibrancy/mod.rs@tauri-v2.11.2; explicit apply/clear failures.
use schemars::JsonSchema;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{Manager, Runtime, WebviewWindow};

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[path = "unsupported.rs"]
mod platform;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSnapshot {
    pub material_enabled: bool,
    pub reduced_motion: bool,
    pub high_contrast: bool,
}
#[derive(Clone, Copy)]
struct Preferences {
    supported: bool,
    reduce_transparency: bool,
    reduced_motion: bool,
    high_contrast: bool,
    dark: bool,
}
impl Preferences {
    fn enabled(self) -> bool {
        self.supported && !self.reduce_transparency && !self.reduced_motion && !self.high_contrast
    }
}
#[derive(Default)]
struct Owner {
    applied: Option<(bool, bool)>,
    snapshot: AppearanceSnapshot,
}
impl Owner {
    fn update(
        &mut self,
        preferences: Option<Preferences>,
        mut apply: impl FnMut(bool, bool) -> Result<(), ()>,
    ) {
        let enabled = preferences.is_some_and(Preferences::enabled);
        let dark = preferences.is_some_and(|p| p.dark);
        self.snapshot.reduced_motion = preferences.is_none_or(|p| p.reduced_motion);
        self.snapshot.high_contrast = preferences.is_some_and(|p| p.high_contrast);
        if self.applied == Some((enabled, dark)) {
            return;
        }
        self.snapshot.material_enabled = false;
        match apply(enabled, dark) {
            Ok(()) => {
                self.applied = Some((enabled, dark));
                self.snapshot.material_enabled = enabled;
            }
            Err(()) => {
                // A partially applied effect is cleared; failed cleanup remains retryable.
                eprintln!("RSS_APPEARANCE effect_failed; presenting solid");
                if enabled {
                    let _ = apply(false, dark);
                }
                self.applied = None;
            }
        }
    }
}
/// App has one current window. Each binding has its own owner so late events cannot mutate a replacement.
#[derive(Default)]
pub struct Appearance {
    current: Mutex<Option<Arc<Mutex<Owner>>>>,
}
fn refresh<R: Runtime>(window: &WebviewWindow<R>, owner: &Arc<Mutex<Owner>>) -> AppearanceSnapshot {
    let state = window.state::<super::runtime::DesktopRuntime>();
    let Ok(current) = state.appearance.current.lock() else {
        return AppearanceSnapshot::default();
    };
    if !current
        .as_ref()
        .is_some_and(|value| Arc::ptr_eq(value, owner))
    {
        return AppearanceSnapshot::default();
    }
    let Ok(mut value) = owner.lock() else {
        return AppearanceSnapshot::default();
    };
    let preferences = platform::probe(window).ok();
    value.update(preferences, |enabled, dark| {
        if enabled {
            platform::apply(window, dark)
        } else {
            platform::clear(window)
        }
    });
    value.snapshot
}
pub fn bind<R: Runtime>(window: &WebviewWindow<R>) {
    let owner = Arc::new(Mutex::new(Owner::default()));
    let state = window.state::<super::runtime::DesktopRuntime>();
    if let Ok(mut current) = state.appearance.current.lock() {
        *current = Some(owner.clone());
    }
    refresh(window, &owner);
    let bound = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let state = bound.state::<super::runtime::DesktopRuntime>();
            if let Ok(mut current) = state.appearance.current.lock() {
                if current
                    .as_ref()
                    .is_some_and(|value| Arc::ptr_eq(value, &owner))
                {
                    *current = None;
                }
            };
        } else if matches!(
            event,
            tauri::WindowEvent::Focused(true) | tauri::WindowEvent::ThemeChanged(_)
        ) {
            let window = bound.clone();
            let value = owner.clone();
            let _ = bound.run_on_main_thread(move || {
                refresh(&window, &value);
            });
        }
    });
}
pub async fn snapshot<R: Runtime>(
    window: WebviewWindow<R>,
) -> crate::self_service::Result<AppearanceSnapshot> {
    let owner = window
        .state::<super::runtime::DesktopRuntime>()
        .appearance
        .current
        .lock()
        .ok()
        .and_then(|value| value.clone());
    let Some(owner) = owner else {
        return Ok(AppearanceSnapshot::default());
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let bound = window.clone();
    window
        .run_on_main_thread(move || {
            let _ = sender.send(refresh(&bound, &owner));
        })
        .map_err(|_| crate::self_service::error("appearance_unavailable", "窗口外观不可用"))?;
    tokio::time::timeout(std::time::Duration::from_secs(2), receiver)
        .await
        .map_err(|_| crate::self_service::error("appearance_unavailable", "窗口外观读取超时"))?
        .map_err(|_| crate::self_service::error("appearance_unavailable", "窗口外观不可用"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_policy_requires_every_system_condition() {
        let normal = Preferences {
            supported: true,
            reduce_transparency: false,
            reduced_motion: false,
            high_contrast: false,
            dark: false,
        };
        assert!(normal.enabled());
        for restricted in [
            Preferences {
                supported: false,
                ..normal
            },
            Preferences {
                reduce_transparency: true,
                ..normal
            },
            Preferences {
                reduced_motion: true,
                ..normal
            },
            Preferences {
                high_contrast: true,
                ..normal
            },
        ] {
            assert!(!restricted.enabled());
        }
    }
    #[test]
    fn failed_apply_or_clear_never_reports_material_and_can_retry() {
        let normal = Preferences {
            supported: true,
            reduce_transparency: false,
            reduced_motion: false,
            high_contrast: false,
            dark: false,
        };
        let mut owner = Owner::default();
        let mut calls = Vec::new();
        owner.update(Some(normal), |enabled, _| {
            calls.push(enabled);
            Err(())
        });
        assert!(!owner.snapshot.material_enabled);
        owner.update(Some(normal), |enabled, _| {
            calls.push(enabled);
            Ok(())
        });
        assert!(owner.snapshot.material_enabled);
        owner.update(Some(normal), |_, _| panic!("no duplicate apply"));
        owner.update(None, |enabled, _| {
            calls.push(enabled);
            Err(())
        });
        assert!(!owner.snapshot.material_enabled);
        owner.update(None, |enabled, _| {
            calls.push(enabled);
            Ok(())
        });
        assert_eq!(calls, [true, false, true, false, false]);
    }
}
