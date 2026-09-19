use std::error::Error;

pub(super) fn diagnostic(error: &dyn Error) -> String {
    let mut message = format!("RSS MDM Agent 启动失败：{error}");
    let mut cause = error.source();
    while let Some(error) = cause {
        message.push_str(&format!("\n原因：{error}"));
        cause = error.source();
    }
    message
}

pub fn report(error: &dyn Error) {
    let message = diagnostic(error);
    eprintln!("{message}");
    #[cfg(windows)]
    show_error(&message);
}

#[cfg(windows)]
fn show_error(message: &str) {
    // ref: Win32 winuser.h MessageBoxW; available independently of WebView startup.
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut std::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            flags: u32,
        ) -> i32;
    }
    let text: Vec<u16> = message
        .replace('\0', "�")
        .encode_utf16()
        .chain([0])
        .collect();
    let caption: Vec<u16> = "RSS MDM Agent".encode_utf16().chain([0]).collect();
    // SAFETY: both UTF-16 buffers are NUL terminated and live throughout this
    // synchronous call; no owner window is required. MB_OK | MB_ICONERROR.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct StartupError(std::io::Error);
    impl std::fmt::Display for StartupError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "window initialization")
        }
    }
    impl Error for StartupError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn diagnostic_preserves_the_underlying_startup_error() {
        let message = diagnostic(&StartupError(std::io::Error::other("WebView unavailable")));
        assert!(message.contains("window initialization"));
        assert!(message.contains("WebView unavailable"));
    }
}
