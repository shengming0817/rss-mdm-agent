use std::error::Error;

pub(super) fn diagnostic(error: &(dyn Error + 'static)) -> String {
    let mut current = Some(error);
    while let Some(cause) = current {
        if let Some(config) =
            cause.downcast_ref::<rss_mdm_desktop::organization_config::ConfigurationError>()
        {
            return format!("RSS MDM Agent 后端配置不可用：{config}。请填写构建配置并重新构建。");
        }
        if let Some(execution_app::Error::UnsupportedSchema { found, supported }) =
            cause.downcast_ref::<execution_app::Error>()
        {
            return format!("执行数据库格式 {found} 不受支持，当前要求格式 {supported}。旧数据库保持原样；执行已停用，请由部署管理员核查原任务与版本。不迁移、覆盖或清空旧数据，也不创建新账本绕过未决任务。");
        }
        current = cause.source();
    }
    "RSS MDM Agent 启动失败：桌面组件不可用。请检查安装与本地数据目录权限后重试。".into()
}

pub fn report(error: &(dyn Error + 'static)) {
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
    #[test]
    fn schema_diagnostic_preserves_original_journal_without_test_fallback() {
        let error = execution_app::Error::UnsupportedSchema {
            found: 4,
            supported: 6,
        };
        let message = diagnostic(&error);
        assert!(message.contains("格式 4"));
        assert!(message.contains("格式 6"));
        assert!(!message.contains("--test-data-dir"));
        assert!(message.contains("不创建新账本"));
        assert!(message.contains("旧数据库保持原样"));
    }
}
