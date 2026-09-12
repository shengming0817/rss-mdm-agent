//! Local execution contracts; data never confers execution authority.
//! All public deserializable values are untrusted DTOs. Use bounded decoding and
//! freeze before comparison; identity verification, policy decisions, signing,
//! journal writes and runner enforcement belong to their host/adapter owners.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod audit;
mod environment;
mod error;
mod model;
mod network;
mod plan;
mod validation;
mod value;
pub use audit::*;
pub use environment::EnvironmentKey;
pub use error::*;
pub use model::*;
pub use network::*;
pub use plan::{decode_plan, FrozenPlan};
pub use validation::PlanLimits;
pub use value::*;

/// Schema projections of the single Rust declaration source, always Draft 2020-12.
pub fn plan_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<PlanSpec>()
}
/// Generate Draft 2020-12 structural schema for AuditEvent from the Rust declaration source.
pub fn audit_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<AuditEvent>()
}
