//! Authenticated local ingress, native login facts and bounded service/helper transport.
#![deny(missing_docs)]
mod diagnostics;
pub use diagnostics::record_startup_failure;
/// Fixed helper protocol and authenticated per-login client. Only runner executes its commands.
#[allow(missing_docs)]
pub mod helper;
/// Current service wire, installation pins, authenticated peer and ingress port.
pub mod host;
#[cfg(test)]
mod host_tests;
#[cfg(target_os = "macos")]
#[doc(hidden)]
#[allow(missing_docs)]
pub mod macos_identity;
#[cfg(target_os = "macos")]
/// Native macOS XPC transport.
pub mod macos_service;
#[cfg(windows)]
#[path = "windows/identity.rs"]
#[doc(hidden)]
#[allow(missing_docs)]
pub mod windows_identity;
#[cfg(windows)]
#[path = "windows/service.rs"]
/// Native Windows SCM and authenticated named-pipe transport.
pub mod windows_service;
