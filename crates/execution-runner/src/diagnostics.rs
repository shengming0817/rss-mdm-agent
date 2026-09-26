//! Closed platform logging. No request bytes, paths, IDs, stderr or credentials enter this seam.
use execution_app::Error;
use execution_contract::ProcessFailureKind;
#[derive(Debug, Clone, Copy)]
pub(crate) enum Stage {
    Startup,
    Ingress,
    Reconcile,
    Shutdown,
}
pub(crate) fn record(stage: Stage, error: Error) {
    emit(stage, crate::runner::classify(error));
}
fn line(stage: Stage, kind: ProcessFailureKind, mode: &str) -> String {
    format!("rss-execution stage={stage:?} failure={kind:?} mode={mode}")
}
fn emit(stage: Stage, kind: ProcessFailureKind) {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn rss_execution_log(message: *const std::ffi::c_char);
        }
        let mode = if unsafe { libc::geteuid() } == 0 {
            "system"
        } else {
            "user"
        };
        let message = std::ffi::CString::new(line(stage, kind, mode))
            .expect("closed diagnostic strings have no NUL");
        unsafe {
            rss_execution_log(message.as_ptr());
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::EventLog::*;
        // ref: RegisterEventSourceW: an unregistered source is written to Application.
        // No registry provider or message DLL is installed; structured strings are the record.
        let mode = match crate::windows::token_identity() {
            Ok((sid, _)) if sid == "S-1-5-18" => "system",
            Ok(_) => "user",
            Err(_) => "unknown",
        };
        let message = crate::windows::wide(line(stage, kind, mode));
        let strings = [message.as_ptr()];
        unsafe {
            let source = RegisterEventSourceW(
                std::ptr::null(),
                crate::windows::wide("RSS Execution").as_ptr(),
            );
            if !source.is_null() {
                if ReportEventW(
                    source,
                    EVENTLOG_ERROR_TYPE,
                    0,
                    1,
                    std::ptr::null_mut(),
                    1,
                    0,
                    strings.as_ptr(),
                    std::ptr::null(),
                ) == 0
                {
                    eprintln!("execution platform diagnostic unavailable");
                }
                DeregisterEventSource(source);
            } else {
                eprintln!("execution platform diagnostic unavailable");
            }
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    eprintln!("{}", line(stage, kind, "unsupported"));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_record_contains_only_closed_categories() {
        assert_eq!(
            line(Stage::Ingress, ProcessFailureKind::Unbound, "user"),
            "rss-execution stage=Ingress failure=Unbound mode=user"
        );
    }
}
