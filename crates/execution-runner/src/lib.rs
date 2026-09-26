//! Product OS adapters. No production authority, transport credential or automatic retry.
pub mod host;
#[cfg(target_os = "macos")]
mod macos;
mod materialize;
mod output;
mod runner;
#[cfg(target_os = "macos")]
use macos as platform;
pub use materialize::Artifacts;
pub use runner::NativeRunner;

#[cfg(target_os = "macos")]
pub mod macos_service;

#[cfg(not(target_os = "macos"))]
#[path = "unsupported.rs"]
mod platform;
