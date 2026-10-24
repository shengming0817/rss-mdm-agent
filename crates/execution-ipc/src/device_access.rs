//! Desktop-only setup protocol. System passwords are transient and redact on Debug.
use rss_mdm_agent_wire::runtime as w;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;
/// Finite local operations; there is no role or account mutation authority in this protocol.
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Operation {
    /// Prepare a signed Mac account creation without Windows factor material.
    PrepareMac {
        /// Exact retained signed task.
        task: Uuid,
        /// New account password, held only until native consumption.
        #[schemars(with = "String")]
        password: Zeroizing<String>,
    },
    /// Read separate extension, server and native observations for the current account.
    MacStatus {},
    /// Current local component and factor observations, independent of historical receipts.
    WindowsStatus {},
    /// Discover eligible first-enrollment policies and own binding states.
    Directory {
        /// Policy cursor.
        cursor: Option<String>,
    },
    /// Begin an authenticated first association.
    Begin(w::AccessBegin),
    /// Confirm the same original native login and challenge retained by the service.
    Confirm {
        /// Original ceremony operation.
        operation: Uuid,
    },
    /// Discover pending signed tasks for this RSS person.
    Tasks {},
    /// Generate or redisplay this exact task's local TOTP candidate.
    Prepare {
        /// Signed task identity.
        task: Uuid,
    },
    /// Confirm possession before Start; optional password is only for a signed Create task.
    ConfirmFactor {
        /// Signed task identity.
        task: Uuid,
        /// Six-digit TOTP.
        #[schemars(with = "String")]
        code: Zeroizing<String>,
        /// New Windows account password; never persisted or sent remotely.
        #[schemars(with = "Option<String>")]
        password: Option<Zeroizing<String>>,
    },
}
/// Safe local task selection; all authority remains in the signed task.
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupTask {
    /// Exact backend task.
    pub task: Uuid,
    /// Business association.
    pub binding: Uuid,
    /// Name only when creating a new account.
    pub create_name: Option<String>,
}
/// Response for the authenticated desktop session only.
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Response {
    /// Diagnostic facts do not authorize login or prove a complete application SSO flow.
    MacStatus(MacStatus),
    /// Local observations do not prove a successful Windows login.
    WindowsStatus(WindowsStatus),
    /// Backend-owned safe discovery result.
    Directory(w::AccessDirectory),
    /// Challenge retained in native memory; nonce is not sent to the renderer.
    Challenge {
        /// Original operation.
        operation: Uuid,
        /// Expiry in UTC seconds.
        expires_at: i64,
    },
    /// Confirmed desired binding; not evidence of Applied.
    Binding(Box<w::AccessBindingStatus>),
    /// Pending signed initialization tasks.
    Tasks {
        /// Tasks for the current person.
        tasks: Vec<SetupTask>,
    },
    /// Local TOTP secret, shown only while the setup screen remains active.
    Setup {
        /// Exact task.
        task: Uuid,
        /// RFC 4648 base32 secret for an authenticator.
        #[schemars(with = "String")]
        secret: Zeroizing<String>,
    },
    /// Setup saved; the existing task runner must still apply and report its effect.
    Prepared {},
}

/// Independent PSSO observations; absent values mean unavailable, never successful.
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MacStatus {
    /// Installed extension passed image and signing identity checks.
    pub extension_installed: bool,
    /// Server shared-device registration fact.
    pub device_registered: Option<bool>,
    /// Server user-registration fact.
    pub user_registered: Option<bool>,
    /// Current server login admission, not a completed login.
    pub login_ready: Option<bool>,
    /// System observation from the authenticated extension.
    pub native_device_registered: Option<bool>,
    /// System user-registration observation.
    pub native_user_registered: Option<bool>,
    /// Native observation UTC milliseconds.
    pub observed_at: Option<u64>,
}

/// Current credential-provider installation observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum WindowsComponentState {
    /// Candidate, storage and boot barrier verified.
    Ready,
    /// Candidate registered during this boot; activation must retry after reboot.
    RebootRequired,
    /// No RSS component installed.
    NotInstalled,
    /// Explicit administrative maintenance disable.
    MaintenanceDisabled,
    /// Local checks failed; never interpreted as ready.
    Unavailable,
}
/// Current native account's observations; no authority to mutate it.
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowsStatus {
    /// Component check at this observation.
    pub component: WindowsComponentState,
    /// Current bound factor is enabled; null means observation unavailable.
    pub factor_active: Option<bool>,
    /// First Entra binding needs a Windows session newer than the observed join.
    pub sign_in_required: bool,
}
