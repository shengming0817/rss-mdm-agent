//! Pure execution decisions with explicit trusted host boundaries.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod model;
mod state;
mod validation;
pub use model::*;
pub use state::*;
