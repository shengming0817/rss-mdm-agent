//! Protected SQLite execution journal; no runner, model, UI or background worker.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod adapter;
mod backend_requests;
mod config;
mod database;
mod execution;
mod interaction;
mod journal;
mod model;
mod process;
mod software;
mod software_progress;
mod trust;
pub use config::test_store_limits;
pub use database::{OpenOutcome, Store};
use execution_app::*;
pub use model::{Error, Limits};
