// ref: window-vibrancy src/macos/internal.rs@526926490c20252d7086b0ad432c6f9d57e90f19;
// objc2-app-kit src/generated/NSAccessibility.rs@0.3.2.
use super::Preferences;
use tauri::{Runtime, WebviewWindow};
pub(super) fn probe<R: Runtime>(window: &WebviewWindow<R>) -> Result<Preferences, ()> {
    objc2::MainThreadMarker::new().ok_or(())?;
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    Ok(Preferences {
        supported: true,
        reduce_transparency: workspace.accessibilityDisplayShouldReduceTransparency(),
        reduced_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        high_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
        dark: window.theme().map_err(|_| ())? == tauri::Theme::Dark,
    })
}
pub(super) fn apply<R: Runtime>(window: &WebviewWindow<R>, _dark: bool) -> Result<(), ()> {
    clear(window)?;
    window_vibrancy::apply_vibrancy(
        window,
        window_vibrancy::NSVisualEffectMaterial::Sidebar,
        Some(window_vibrancy::NSVisualEffectState::FollowsWindowActiveState),
        None,
    )
    .map_err(|_| ())
}
pub(super) fn clear<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), ()> {
    objc2::MainThreadMarker::new().ok_or(())?;
    window_vibrancy::clear_vibrancy(window)
        .map(|_| ())
        .map_err(|_| ())
}
