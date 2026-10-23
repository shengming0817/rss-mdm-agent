//! Native device/user relation progress; this is not an execution permission.
use rss_mdm_agent_wire::runtime as wire;
use serde::{Deserialize, Serialize};
/// Non-secret progress for the original operation.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CurrentUserSyncPhase {
    /// No relation has been published by this owner.
    #[default]
    Unbound,
    /// An original login write has no confirmed response.
    BindingUnknown,
    /// The original login receipt was verified and persisted.
    Bound,
    /// An original cleanup write has no confirmed response.
    CleanupPending,
    /// CAS or operation identity conflicts; explicit recovery is required.
    Conflict,
    /// The retained registration or credential is no longer current.
    Invalidated,
}
/// Public local progress, independent from network presence.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentUserSync {
    /// Local settlement phase.
    pub phase: CurrentUserSyncPhase,
    /// Last confirmed public server projection.
    pub current: Option<wire::CurrentUser>,
}
