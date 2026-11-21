//! Closed installation diagnostics: no arguments, paths, native output or enrollment material.
use super::*;
use std::cell::RefCell;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Report {
    pub version: u8,
    pub process_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_operation: Option<uuid::Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub terminal: bool,
    pub state: String,
    pub phase: String,
    pub failure_phase: Option<String>,
    pub error: Option<String>,
    pub native_exit_code: Option<i32>,
    pub compensation: String,
}
thread_local! {
    static REPORT: RefCell<Report> = RefCell::new(Report {
        version: 1, application_operation: None, details:None, process_id: std::process::id(), terminal: false, state: "running".into(), phase: "preflight".into(),
        failure_phase: None, error: None, native_exit_code: None,
        compensation: "not-required".into(),
    });
}
pub(super) fn snapshot() -> Report {
    REPORT.with(|value| value.borrow().clone())
}
pub(super) fn phase(name: &'static str) -> Result<(), Error> {
    REPORT.with(|value| value.borrow_mut().phase = name.into());
    #[cfg(windows)]
    super::windows::record(&snapshot())?;
    Ok(())
}
pub(super) fn application(id: uuid::Uuid) -> Result<(), Error> {
    REPORT.with(|value| value.borrow_mut().application_operation = Some(id));
    #[cfg(windows)]
    super::windows::record(&snapshot())?;
    Ok(())
}
pub(super) fn details(value: serde_json::Value) -> Result<(), Error> {
    println!("{value}");
    REPORT.with(|report| report.borrow_mut().details = Some(value));
    #[cfg(windows)]
    super::windows::record(&snapshot())?;
    Ok(())
}
pub(super) fn native_failure(code: Option<i32>) {
    REPORT.with(|value| value.borrow_mut().native_exit_code = code);
}
pub(super) fn profile_failure(phase: super::apply::Phase, error: Error) {
    REPORT.with(|value| {
        let mut value = value.borrow_mut();
        value.failure_phase = serde_json::to_value(phase)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned));
        value.error = Some(format!("{error:?}"));
    });
}
pub(super) fn failed(error: Error) {
    REPORT.with(|value| {
        let mut value = value.borrow_mut();
        if value.failure_phase.is_none() {
            value.failure_phase = Some(value.phase.clone());
            value.error = Some(format!("{error:?}"));
        }
        value.state = "failed".into();
    });
}
pub(super) fn compensation(state: &'static str) {
    REPORT.with(|value| value.borrow_mut().compensation = state.into());
}
pub(super) fn succeeded() {
    REPORT.with(|value| value.borrow_mut().state = "succeeded".into());
}
pub(super) fn terminal() {
    REPORT.with(|value| value.borrow_mut().terminal = true);
}
