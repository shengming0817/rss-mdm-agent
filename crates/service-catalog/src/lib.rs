//! Immutable directory and shared human/AI parameter rules. Data never grants execution authority.
//! Bounded decoding/freeze are the only validated entrypoints. Capability matching, resource
//! resolution, authority verification, authorization and execution belong to other owners.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![warn(clippy::cognitive_complexity)]
mod bounded;
mod catalog;
mod error;
mod model;
mod parameters;
pub use bounded::CatalogLimits;
pub use catalog::*;
pub use error::{ArgumentRule, CatalogError, DefinitionRule, Limit};
pub use model::*;
pub use parameters::{Parameter, ParameterLimits, ParameterProjection, ParameterRule};
/// Structural catalog schema generated from the Rust declaration; semantic checks remain mandatory.
pub fn catalog_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<CatalogSnapshot>()
}
