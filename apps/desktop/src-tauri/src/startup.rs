use std::error::Error;

pub(super) fn diagnostic(_error: &dyn Error) -> String {
    "RSS MDM Agent 启动失败：桌面组件不可用。请检查安装与本地数据目录权限后重试。".into()
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
    fn diagnostic_never_exports_raw_error_or_cause() {
        let message = diagnostic(&StartupError(std::io::Error::other(
            "CANARY_SECRET /Users/private/account/token",
        )));
        assert!(!message.contains("window initialization"));
        assert!(!message.contains("CANARY_SECRET"));
        assert!(!message.contains("/Users/private"));
        assert!(message.contains("启动失败"));
    }
}
