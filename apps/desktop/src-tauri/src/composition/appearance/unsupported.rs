use super::Preferences;
use tauri::{Runtime, WebviewWindow};
pub(super) fn probe<R: Runtime>(_: &WebviewWindow<R>) -> Result<Preferences, ()> {
    Err(())
}
pub(super) fn apply<R: Runtime>(_: &WebviewWindow<R>, _: bool) -> Result<(), ()> {
    Err(())
}
pub(super) fn clear<R: Runtime>(_: &WebviewWindow<R>) -> Result<(), ()> {
    Ok(())
}
