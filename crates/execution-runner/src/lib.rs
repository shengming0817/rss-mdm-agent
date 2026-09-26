//! Product OS adapters. No production authority, transport credential or automatic retry.
pub mod host;
#[cfg(target_os = "macos")]
mod macos;
mod materialize;
mod output;
mod runner;
#[cfg(target_os = "macos")]
use macos as platform;
pub use materialize::{Artifacts, InputBytes, InputResolver};
pub use runner::NativeRunner;

#[cfg(target_os = "macos")]
pub mod macos_service;

#[cfg(not(any(target_os = "macos", windows)))]
#[path = "unsupported.rs"]
mod platform;

#[cfg(test)]
mod host_tests;

#[cfg(any(windows, test))]
mod windows_argv;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(windows)]
pub use windows::service as windows_service;
