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
    /// Open the current user's native sign-in settings; never accepts a URL, PIN or account.
    OpenWindowsHello {},
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
    /// Shell accepted the fixed native settings URI; this is not registration success.
    WindowsHelloSettingsRequested {},
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
    /// Desired signed policy, not proof of native profile application.
    pub configuration: Option<rss_mdm_agent_wire::platform_sso::LoginConfiguration>,
    /// Server password exchange only; never proof of local password or keychain synchronization.
    pub password_exchange: Option<rss_mdm_agent_wire::platform_sso::PasswordExchangeStatus>,
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
    /// Actual LoginManager readback matches the retained registration.
    pub native_configuration_matches: Option<bool>,
    /// Current user login-keychain accessibility only; never a synchronization fact.
    pub login_keychain: KeychainStatus,
    /// System user-registration observation.
    pub native_user_registered: Option<bool>,
    /// Native observation UTC milliseconds.
    pub observed_at: Option<u64>,
}

/// Read-only current-user login-keychain diagnostic, never a login gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum KeychainStatus {
    /// Login keychain is unlocked.
    Unlocked,
    /// Login keychain is locked; RSS does not unlock it.
    Locked,
    /// No login keychain was found.
    Missing,
    /// The current helper cannot establish the state.
    Unavailable,
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
    /// Native Hello observations are independent of the RSS credential provider.
    pub hello: WindowsHelloStatus,
}
/// Read-only current-user facts from the authenticated helper, not a device-key assertion.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowsHelloUser {
    /// Current user's native registration, never inferred from policy.
    pub registration: w::AccessObservation,
    /// Current user's Hello key identifier; no private material.
    pub key_id: Option<Uuid>,
    /// Exact user-key hardware evidence; Unknown when unavailable.
    pub hardware_key: w::AccessObservation,
    /// UTC seconds sampled in the user helper.
    pub observed_at: i64,
}
/// Configuration prerequisites; not a platform certification or login claim.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum WindowsHelloAvailability {
    /// Windows/Entra/TPM prerequisites are present.
    Eligible,
    /// This account or platform combination is outside the selected profile.
    Unsupported,
    /// Another policy authority conflicts with RSS management.
    Conflict,
    /// Native evidence could not be obtained.
    Unavailable,
}
/// Desired state and independently observed native facts for the current user.
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowsHelloStatus {
    /// Current native configuration prerequisites.
    pub availability: WindowsHelloAvailability,
    /// Exact retained desired policy; absent before a signed task.
    pub desired_enabled: Option<bool>,
    /// The RSS binding has been retired; native keys may still remain.
    pub retired: bool,
    /// A previously observed key cannot satisfy the new credential generation.
    pub replacement_required: bool,
    /// Current observations, not proof of an actual login.
    pub facts: Option<w::AccessWhfbFacts>,
}
