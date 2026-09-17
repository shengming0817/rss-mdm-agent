//! One software decision from an explicit host snapshot; no authorization, workflow or installer.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod model;
mod planner;
pub use model::*;
pub use planner::decide;
