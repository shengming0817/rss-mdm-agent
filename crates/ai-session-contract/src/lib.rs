//! Provider-independent conversation data. A model proposal, role, response or
//! capability claim never authenticates a user, grants approval or proves execution.
//! Only the current local V1 encoding is accepted. The host/adapter owns provider
//! conversion, capability verification, I/O and all stateful execution decisions.
#![forbid(unsafe_code)]
mod capability;
mod model;
mod validation;
mod value;
pub use capability::*;
pub use model::*;
pub use validation::{decode_command, decode_event, SessionLimits};
pub use value::*;

pub fn event_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<EventEnvelope>()
}
pub fn command_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<CommandEnvelope>()
}
