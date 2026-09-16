//! Pure execution decisions with explicit trusted host boundaries.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod decision;
mod evaluate;
mod model;
pub use decision::*;
pub use evaluate::evaluate;
pub use model::*;
