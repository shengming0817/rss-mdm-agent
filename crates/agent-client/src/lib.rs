//! Agent V5 communication and bounded content delivery; execution remains journal-owned.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod bridge;
mod client;
mod config;
mod content;
mod store;
pub use bridge::*;
pub use client::*;
pub use config::*;
pub use content::{ContentFile, Materials};
/// The protocol's single producer.
pub use rss_mdm_agent_wire as wire;
pub use store::inspect_registration;
/// Closed errors, without secrets, response bodies or backend paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Invalid trusted configuration.
    #[error("invalid communication configuration")]
    Configuration,
    /// Invalid protocol value or remote response.
    #[error("invalid Agent protocol")]
    Protocol,
    /// Current identity is absent or rejected.
    #[error("Agent identity unavailable")]
    Identity,
    /// Current authorization was rejected.
    #[error("Agent permission denied")]
    Denied,
    /// An immutable request conflicts.
    #[error("Agent operation conflict")]
    Conflict,
    /// Retry the same operation in a later bounded drive.
    #[error("Agent transport unavailable")]
    Unavailable,
    /// A budget was exhausted before new work.
    #[error("Agent capacity exhausted")]
    Capacity,
    /// Protected storage failed or is corrupt.
    #[error("Agent storage unavailable")]
    Storage,
    /// Preserve an unsupported database.
    #[error("unsupported communication database")]
    Schema,
    /// Trusted time is unavailable or moved backwards.
    #[error("reliable Agent time unavailable")]
    Clock,
    /// A permit expired.
    #[error("Agent permit expired")]
    Expired,
    /// Signature or frozen coordinates do not match.
    #[error("untrusted Agent task")]
    Untrusted,
    /// The trusted host cannot represent this task.
    #[error("Agent task unsupported")]
    Unsupported,
}
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Storage
    }
}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}
impl From<wire::WireError> for Error {
    fn from(_: wire::WireError) -> Self {
        Self::Protocol
    }
}
impl From<execution_app::Error> for Error {
    fn from(v: execution_app::Error) -> Self {
        use execution_app::Error as E;
        match v {
            E::Denied => Self::Denied,
            E::Unbound => Self::Identity,
            E::Unsupported => Self::Unsupported,
            E::Clock => Self::Clock,
            E::Conflict => Self::Conflict,
            E::Capacity => Self::Capacity,
            E::Configuration => Self::Configuration,
            _ => Self::Storage,
        }
    }
}

#[cfg(test)]
extern crate self as agent_client;
#[cfg(test)]
#[path = "../tests/unit/chunk_recovery.rs"]
mod chunk_recovery;
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;

mod software;
pub use software::{software_commands, SoftwareCommands};
