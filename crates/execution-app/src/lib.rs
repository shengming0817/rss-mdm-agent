//! Headless controlled execution application shared by production and explicit test assemblies.
//! Deterministic runners and fixture limits require the non-default `test-support` feature.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod config;
mod delivery;
mod details;
mod host;
mod journal;
mod journal_model;
mod model;
mod ports;
mod service;
#[cfg(any(test, feature = "test-support"))]
mod test_runner;
pub use config::*;
pub use details::*;
pub use journal::*;
pub use journal_model::*;
pub use model::*;
pub use ports::*;
pub use service::*;
#[cfg(any(test, feature = "test-support"))]
pub use test_runner::*;

/// Unified task projections over the original business owners.
pub mod tasks;
