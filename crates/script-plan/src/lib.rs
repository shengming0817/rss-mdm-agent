//! Native script planning only. Compilation and freezing never grant authority or execute code.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod model;
mod planner;
pub use model::*;
pub use planner::compile;
