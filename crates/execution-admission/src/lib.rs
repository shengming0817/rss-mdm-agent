//! Deterministic authorization, not execution or approval consumption.
//! The host's verifier is a trust boundary, not a parser: it must independently
//! authenticate subjects, bind origin accounts, verify delegation issuer authority,
//! obtain current trusted rules, and establish reliable time/revocation freshness.
//! Only explicitly named test adapters may synthesize these facts in S1.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod decision;
mod evaluator;
mod model;
pub use decision::*;
pub use evaluator::decide;
pub use model::*;
