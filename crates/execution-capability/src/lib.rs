//! Pure matching of frozen plans against caller-supplied environment snapshots.
//! Supported is neither a permission nor proof of OS enforcement. The host validates
//! snapshot authenticity/freshness; the runner must actually enforce every constraint.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod matcher;
mod model;
pub use matcher::match_capabilities;
pub use model::*;
