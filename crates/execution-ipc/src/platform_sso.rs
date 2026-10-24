//! Extension-only registration control. The native peer, not these bytes, selects the account.
use rss_mdm_agent_wire::platform_sso as w;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
/// Finite extension-only operations.
pub enum Operation {
    /// Obtain a device- or current-user-scoped registration challenge.
    Bootstrap {
        #[doc = "Device or current native user scope."]
        scope: w::Kind,
    },
    /// Submit proof for a challenge issued to this native peer.
    Register {
        #[doc = "Server-owned registration proof."]
        input: Box<w::Registration>,
    },
    /// Inspect server state for the exact current native account.
    Status {
        #[doc = "Device or current native user scope."]
        scope: w::Kind,
    },
    /// Report observations from the system-provided LoginManager.
    Observe {
        /// System device or native user callback scope.
        scope: w::Kind,
        /// Exact source whose system state was observed.
        source: Uuid,
        /// Exact native source generation.
        source_version: i64,
        /// Native LoginManager device-registration observation.
        device_registered: bool,
        /// Native LoginManager user-registration observation.
        user_registered: bool,
    },
    /// Revoke the current user's source after configuration removal.
    Unregister {
        /// System device or native user teardown scope.
        scope: w::Kind,
        /// Exact native registration being retired; cannot select a newer source version.
        source: Uuid,
        /// Exact observed native source generation.
        source_version: i64,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
/// Finite extension-only results.
pub enum Response {
    /// The native account must first complete RSS association.
    NeedsAccount {},
    /// Trusted device/account coordinates and server bootstrap; never forwarded to the renderer.
    Bootstrap {
        /// Protected installation and native account coordinates.
        context: platform_sso::Context,
        /// One-use authenticated server challenge.
        challenge: w::BootstrapResponse,
    },
    /// Durable server result, separate from system callback completion.
    Registered {
        #[doc = "Durable server receipt."]
        receipt: w::RegistrationReceipt,
    },
    /// Current server observations; no login authority.
    Status {
        #[doc = "Current server observations."]
        value: w::Status,
    },
    /// The old source no longer exists at that version; no newer source was revoked.
    Retired {},
    /// Native observation was retained under the actual peer account.
    Observed {},
    /// Server revocation completed; native system cleanup is still separate.
    Revoked {
        #[doc = "Durable server receipt."]
        receipt: w::RevocationReceipt,
    },
}
