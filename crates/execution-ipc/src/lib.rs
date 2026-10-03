//! Authenticated local ingress, native login facts and bounded service/helper transport.
#![deny(missing_docs)]
mod diagnostics;
pub use diagnostics::record_startup_failure;
/// Fixed helper protocol and authenticated per-login client. Only runner executes its commands.
pub mod helper;
/// Current service wire, installation pins, authenticated peer and ingress port.
pub mod host;
#[cfg(test)]
mod host_tests;
#[cfg(target_os = "macos")]
/// Native console-login facts shared with the macOS runner.
pub mod macos_identity;
#[cfg(target_os = "macos")]
/// Native macOS XPC transport.
pub mod macos_service;
#[cfg(windows)]
#[path = "windows/identity.rs"]
/// Native identity and resource ownership shared with the Windows runner.
pub mod windows_identity;
#[cfg(windows)]
#[path = "windows/service.rs"]
/// Native Windows SCM and authenticated named-pipe transport.
pub mod windows_service;
