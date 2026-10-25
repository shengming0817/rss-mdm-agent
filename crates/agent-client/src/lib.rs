//! Agent V1 communication and bounded content delivery; execution remains journal-owned.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
mod collected;
pub mod protocol;
pub mod telemetry;
pub use collected::CollectedStore;

use rss_mdm_agent_wire::runtime::Secret;
mod credential_escrow;
mod delivery;
mod device_access;
pub use device_access::device_access_complete;
mod platform_sso;
mod unmanage;
pub use unmanage::{inspect_unmanage, UnmanagePermit, UnmanageTransport};
mod redaction;
pub use delivery::{AbandonmentAck, Association, FrozenAbandonment, FrozenResult, ResultAck};
pub use redaction::CredentialRedactor;
mod client;
mod configuration;
mod connections;
pub use connections::{verify_connections, ConnectionStatus};
mod enrollment;
mod managed;
mod native_enrollment;
pub use configuration::{verify_configuration, CONFIGURATION_BYTES, CONFIGURATION_FILENAME};
pub use managed::{bootstrap_coordinates, NativeIdentity};
pub use native_enrollment::NativeEnrollmentMaterial;
mod identity;
mod pki;
mod task_trust;

/// Device-bound enterprise relation and durable recovery.
pub mod current_user;
mod self_service;
mod tls;
mod trust;
pub use enrollment::{EnrollmentPhase, EnrollmentStatus, GrantImport};
pub use tls::validate_public_ca;
mod config;
mod content;
mod native_content;
/// Bounded verification of the standard native source documents, without execution authority.
pub mod native_source;
mod store;
pub use client::*;
pub use config::*;
pub use content::{ContentFile, Materials};
/// Generated values for the backend-owned HTTP API.
use rss_mdm_agent_wire::runtime as wire;
pub use store::{inspect_binding, validate_existing, validate_identity, validate_stored_identity};
/// Closed errors, without secrets, response bodies or backend paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Original employee invocation is not yet present.
    #[error("Agent operation not found")]
    NotFound,
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
    /// Classified network failure, including an uncertain response; never authorizes write replay.
    #[error("Agent network unavailable")]
    Network,
    /// The device certificate is expired; retain its registration and journal.
    #[error("Agent certificate expired")]
    CertificateExpired,
    /// The active credential was revoked.
    #[error("Agent credential revoked")]
    CredentialRevoked,
    /// A newer certificate retired this credential; recover its original activation.
    #[error("Agent credential retired")]
    CredentialRetired,
    /// Original signing result is unknown; never issue again.
    #[error("Agent certificate issuance unknown")]
    IssuanceUnknown,
    /// Original activation result is unknown.
    #[error("Agent activation unknown")]
    ActivationUnknown,
    /// Admission is paused; retain the original operation, key and deadline.
    #[error("new registration admission paused")]
    EnrollmentPaused,
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

#[cfg(test)]
extern crate self as agent_client;
#[cfg(test)]
#[path = "../tests/unit/chunk_recovery.rs"]
mod chunk_recovery;
#[cfg(test)]
#[path = "../../../tests/agent-protocol/mod.rs"]
mod test_support;

#[cfg(test)]
#[path = "../tests/unit/api_encoding.rs"]
mod api_encoding;

pub use pki::RuntimeGate;

impl From<rss_mdm_agent_wire::runtime::WireError> for Error {
    fn from(_: rss_mdm_agent_wire::runtime::WireError) -> Self {
        Self::Protocol
    }
}
