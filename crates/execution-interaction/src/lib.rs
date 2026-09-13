//! One-shot interactions. Answers are data, never authorization or task cancellation.
//! Persist transitions with compare-and-swap on `(interaction id, expected_revision)`;
//! on conflict reload and reevaluate with the host's current time. Dropping a value has no effects.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod model;
mod state;
pub use model::*;
pub use state::*;
