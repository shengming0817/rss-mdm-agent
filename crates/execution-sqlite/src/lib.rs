//! Protected SQLite execution journal; no runner, model, UI or background worker.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod database;
mod execution;
mod interaction;
mod journal;
mod model;
mod process;
mod software;
mod software_progress;
mod trust;
pub use database::{OpenOutcome, Store};
pub use model::*;
pub use software_progress::CommittedSoftwareProgress;

pub use interaction::execution_confirmation;
