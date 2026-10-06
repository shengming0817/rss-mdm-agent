//! Protected SQLite execution journal; no runner, model, UI or background worker.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod adapter;
mod backend_requests;
#[cfg(any(test, feature = "test-support"))]
mod config;
mod database;
mod execution;
mod interaction;
mod journal;
mod material;
mod model;
pub use material::MaterialCheckpoint;
mod process;
mod software;
mod software_progress;
mod trust;
#[cfg(any(test, feature = "test-support"))]
pub use config::test_store_limits;
pub use database::{OpenOutcome, Store};
use execution_app::*;
pub use model::{Error, Limits};
