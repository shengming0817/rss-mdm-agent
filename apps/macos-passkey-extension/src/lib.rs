//! In-process ABI for the OS-hosted extension. Never exposed over IPC or to WebView.
#[cfg(target_os = "macos")]
mod bridge;
