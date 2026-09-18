//! V2 product AI reliability records generated from one JSON Schema owner.
//! Records are untrusted data: they neither authenticate a caller nor grant
//! approval or establish execution facts. TS owns Host, Provider and Store ports.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod codec;
#[allow(missing_docs, clippy::all)]
mod generated;
pub use codec::{decode, encode, fingerprint, ContractError, Diagnostic, Limits};
pub use generated::*;
