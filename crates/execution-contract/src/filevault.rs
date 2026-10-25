//! Public local FileVault custody diagnostics, independent of login and PSSO state.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
/// Collection/transport facts only; none proves that a PRK can unlock the current volume.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FilevaultState {
    /// No explicit server authorization or recipient configuration.
    #[default]
    NotConfigured,
    /// The fixed system CMS file is absent.
    MissingMaterial,
    /// One frozen upload is waiting for its original durable receipt.
    Pending,
    /// The CMS does not name the authorized recipient certificate.
    RecipientMismatch,
    /// The server persisted the material; recovery validity remains unverified.
    EscrowedUnverified,
    /// File ownership, links, permissions, size or snapshot checks failed.
    UnsafeMaterial,
    /// The exact operation was definitively rejected; await material/configuration change.
    Rejected,
    /// Collection or authenticated communication is temporarily unavailable.
    Unavailable,
    /// This installation is retiring and must not collect new material.
    Retired,
}
