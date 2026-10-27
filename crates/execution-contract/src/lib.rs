//! Local execution contracts; data never confers execution authority.
//! All public deserializable values are untrusted DTOs. Use bounded decoding and
//! freeze before comparison; identity verification, policy decisions, signing,
//! journal writes and runner enforcement belong to their host/adapter owners.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod audit;
mod query_schema;
mod resources;
pub use query_schema::*;
mod collected_data;
pub use collected_data::*;
mod current_user;
pub use current_user::*;
mod backend_task;
mod environment;
mod error;
mod input;
mod launch;
mod model;
mod network;
mod network_check;
pub use network_check::*;
mod process;
mod settings;
mod software;
mod software_management;
pub use settings::*;
pub use software_management::*;
mod software_format;
mod software_inputs;
mod software_program;
pub use software_inputs::*;
mod software_progress;
mod validation;
mod value;
pub use audit::*;
pub use backend_task::*;
pub use environment::EnvironmentKey;
pub use error::*;
pub use input::{decode_execution, FrozenExecution};
pub use launch::*;
pub use model::*;
pub use network::*;
pub use process::*;
pub use software::*;
pub use software_format::*;
pub use software_program::*;
pub use software_progress::*;
pub use validation::{ExecutionLimits, MAX_SAFE_INTEGER};
pub use value::*;

/// Schema projections of the single Rust declaration source, always Draft 2020-12.
pub fn execution_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<ExecutionInput>()
}
/// Generate Draft 2020-12 structural schema for AuditEvent from the Rust declaration source.
pub fn audit_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<AuditEvent>()
}

mod filevault;
pub use filevault::FilevaultState;
