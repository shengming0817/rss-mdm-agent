//! One bounded IPC owner driving the existing application. Peer identity is never a wire field.
use execution_contract::{Digest, Id, RequestId, VersionedRef};
use serde::{Deserialize, Serialize};
/// Current desktop/system IPC. No negotiation or legacy parser exists.
pub const IPC_VERSION: u8 = 6;
/// Current protected installation document format.
pub const DEPLOYMENT_VERSION: u32 = 6;

/// Read-only native state, independent from Agent identity and policy authorization.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MdmStatus {
    /// Local OS observation; Unknown does not mean absent.
    pub state: rss_mdm_agent_wire::runtime::MdmEnrollmentState,
    /// UTC observation time, absent before the first successful owner drive.
    pub observed_at: Option<i64>,
}
/// Safe readiness facts; connecting never grants execution authority.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(tag = "phase", rename_all = "camelCase", deny_unknown_fields)]
pub enum Readiness {
    /// Existing device owner is open and driving recovery/work.
    Ready,
    /// Existing state must be inspected; it must not be reset or re-registered.
    NotReady,
}
/// Safe statement from the authenticated production owner.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceStatus {
    /// Bounded local telemetry health; diagnostic values grant no authority.
    pub telemetry: Option<serde_json::Value>,
    /// Actual binary build version.
    pub build: String,
    /// Actual local IPC version.
    pub protocol: u8,
    /// Native service platform.
    pub platform: String,
    /// Independent readiness, not authorization or an effect receipt.
    pub readiness: Readiness,
}
impl ServiceStatus {
    /// Construct a diagnostic without sensitive device/storage fields.
    pub fn new(readiness: Readiness) -> Self {
        Self {
            build: env!("CARGO_PKG_VERSION").into(),
            protocol: IPC_VERSION,
            telemetry: None,
            platform: std::env::consts::OS.into(),
            readiness,
        }
    }
}
/// Public registration phases, independent of enterprise execution permissions.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RegistrationPhase {
    /// No explicit machine registration request exists.
    Unregistered,
    /// Preparing the exact persisted request.
    Preparing,
    /// The original configuration allocation is not yet confirmed.
    ConfigurationUnknown,
    /// User authorization is pending.
    AwaitingAuthorization,
    /// Original grant is authorized.
    Authorized,
    /// Candidate activation is pending.
    Activating,
    /// Active binding is persisted.
    Active,
    /// Original CA issuance needs recovery.
    IssuanceUnknown,
    /// Original activation needs recovery.
    ActivationUnknown,
    /// New registration admission is paused; retain original material and expiry.
    Paused,
    /// Original request or credential is rejected.
    Rejected,
}
/// Local interpretation of the retained device credential; never an authorization grant.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CredentialStatus {
    /// No activated certificate exists.
    Absent,
    /// Current certificate and binding are retained.
    Active,
    /// Certificate expired; retain identity and journal.
    Expired,
    /// Server revoked the credential or registration.
    Revoked,
    /// A later activation retired the certificate.
    Retired,
}
/// Closed, credential-free registration diagnostics.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RegistrationFailure {
    /// Signed file or installed coordinates do not match.
    InvalidConfiguration,
    /// An existing operation must be resumed instead of replaced.
    Conflict,
    /// Authorization is denied or revoked.
    Denied,
    /// Frozen authorization expired.
    Expired,
    /// Server-side registration capacity is exhausted.
    Capacity,
    /// Transport or native storage is temporarily unavailable.
    Unavailable,
}
/// A bounded machine request preview; contains no key handles, secrets or certificate bodies.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrationStatus {
    /// Last task trust observation; absence means unknown.
    pub task_trust: Option<TrustInstallationStatus>,
    /// Last Agent PKI installation observation; not a CRL freshness proof.
    pub pki_trust: Option<TrustInstallationStatus>,
    /// Classified failure without untrusted server text or secrets.
    pub failure: Option<RegistrationFailure>,
    /// Closed lifecycle phase.
    pub phase: RegistrationPhase,
    /// Server grant selected by the machine owner.
    pub grant_id: Option<String>,
    /// Original authorization deadline.
    pub expires_at: Option<i64>,
    /// SHA256 of the exact request public key.
    pub public_key_fingerprint: Option<String>,
    /// A durable registration exists, independent of runtime readiness.
    pub registered: bool,
    /// Exact retained credential state, independent of a pending authorization.
    pub credential: CredentialStatus,
}
/// Non-secret installation progress from the sole communication owner.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustInstallationStatus {
    /// Last locally observed result; not current connectivity.
    pub state: TrustInstallationState,
    /// Target public revision when known.
    pub target_revision: Option<u64>,
    /// Last read-back installed public revision when known.
    pub actual_revision: Option<u64>,
    /// Local observation time, not ordering authority.
    pub observed_at: i64,
    /// Separate successful ACK confirmation.
    pub acknowledged: bool,
}
/// Closed installation result projection.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustInstallationState {
    /// Received but not yet applied.
    Received,
    /// Durable installation and readback succeeded.
    Applied,
    /// Installation or refresh failed.
    Failed,
    /// Actual state cannot be determined.
    Unknown,
}
/// Authenticated machine-owned coordinates for native user authorization.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrationContext {
    /// Stable installed service identity; addresses never authorize a user credential.
    pub service_id: String,
    /// Selected address for display only.
    pub origin: String,
    /// Installed tenant UUID.
    pub tenant_id: String,
    /// Safe snapshot of the original machine operation.
    pub status: RegistrationStatus,
}
/// Bounded local device facts for the authenticated desktop session; never an execution grant.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceOverview {
    /// Extensible device facts; OS peer identity is held separately.
    pub attributes: DeviceAttributes,
}
/// Known device presentation keys; new ordinary keys do not change IPC identity.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAttributes {
    /// OS computer name, absent when the platform read failed.
    pub computer_name: Option<String>,
    /// Up-interface unicast addresses; empty means no address, absent means read failed.
    pub ip_addresses: Option<Vec<String>>,
    /// OS login name of the requesting interactive session, never a product account.
    pub login_user: Option<String>,
    /// Ordinary unrecognized facts preserved without becoming authority.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
/// Closed presentation combining local installation checks and authenticated service facts.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(tag = "phase", rename_all = "camelCase", deny_unknown_fields)]
pub enum ServiceView {
    /// Configuration and OS registration are both absent.
    NotInstalled,
    /// OS registration exists without the required deployment.
    ConfigurationRequired,
    /// Installation or peer identity is rejected.
    Rejected,
    /// Connection/registration inspection could not be established.
    Unavailable,
    /// Current deployment or response does not match this client's contract.
    Mismatch,
    /// Authenticated statement, independent of execution permissions.
    Connected {
        /// Safe system owner facts.
        status: ServiceStatus,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    version: u8,
    reply: Reply,
}
/// Serialize the single current response envelope for native transport and operator probes.
pub fn encode_reply(reply: Reply) -> Vec<u8> {
    let bytes = serde_json::to_vec(&Response {
        version: IPC_VERSION,
        reply,
    })
    .expect("service reply serialization");
    if valid_reply_shape(&bytes) {
        bytes
    } else {
        serde_json::to_vec(&Response {
            version: IPC_VERSION,
            reply: Reply::Unavailable,
        })
        .expect("unavailable reply serialization")
    }
}
// All reply content, including unknown display keys, shares the native frame budget.
// This is transport validation only: JSON contents never establish peer authority.
fn valid_reply_shape(bytes: &[u8]) -> bool {
    fn visit(value: &serde_json::Value, depth: usize, remaining: &mut usize) -> bool {
        if depth > 64 || *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        match value {
            serde_json::Value::Array(values) => {
                values.iter().all(|v| visit(v, depth + 1, remaining))
            }
            serde_json::Value::Object(values) => {
                values.values().all(|v| visit(v, depth + 1, remaining))
            }
            _ => true,
        }
    }
    bytes.len() <= FRAME_LIMIT
        && serde_json::from_slice::<serde_json::Value>(bytes)
            .is_ok_and(|value| visit(&value, 0, &mut 131072))
}
fn decode_reply(bytes: &[u8]) -> Result<Reply, execution_app::Error> {
    if !valid_reply_shape(bytes) {
        return Err(execution_app::Error::InvalidInput);
    }
    let response: Response =
        serde_json::from_slice(bytes).map_err(|_| execution_app::Error::InvalidInput)?;
    if response.version != IPC_VERSION {
        return Err(execution_app::Error::Unsupported);
    }
    Ok(response.reply)
}
fn registration_present() -> Result<bool, execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/bin/launchctl")
            .args(["print", "system/com.rss-mdm.agent.execution"])
            .output()
            .map_err(|_| execution_app::Error::Unavailable)?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(113) => Ok(false),
            _ => Err(execution_app::Error::Unavailable),
        }
    }
    #[cfg(windows)]
    {
        crate::windows_service::registration_present()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(execution_app::Error::Unsupported)
    }
}
#[cfg(target_os = "macos")]
use std::sync::atomic::AtomicBool;

/// Current actual OS service subject, never an enterprise identity or a request claim.
pub fn current_subject() -> Result<String, execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        Ok(unsafe { libc::geteuid() }.to_string())
    }
    #[cfg(windows)]
    {
        crate::windows_identity::token_identity().map(|v| v.0)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(execution_app::Error::Unsupported)
    }
}

/// Actual kernel login session of this process, independent of product identity.
pub fn current_session() -> Result<u32, execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn rss_execution_audit_session() -> i64;
        }
        let session = unsafe { rss_execution_audit_session() };
        u32::try_from(session).map_err(|_| execution_app::Error::Unbound)
    }
    #[cfg(windows)]
    {
        crate::windows_identity::token_identity().map(|v| v.1)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(execution_app::Error::Unsupported)
    }
}
/// Login identity includes boot generation and the native login token, so a reused numeric
/// session after reboot/logoff cannot inherit an earlier task's run context.
pub fn current_session_binding() -> Result<Id, execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        Id::new(format!(
            "{}/{}",
            crate::macos_identity::boot_generation()?.as_str(),
            current_session()?
        ))
        .map_err(|_| execution_app::Error::Unbound)
    }
    #[cfg(windows)]
    {
        crate::windows_identity::current_session_binding()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(execution_app::Error::Unsupported)
    }
}

/// Current kernel boot generation, independent of adjustable wall-clock time.
pub fn boot_generation() -> Result<Id, execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        crate::macos_identity::boot_generation()
    }
    #[cfg(windows)]
    {
        crate::windows_identity::boot_generation()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(execution_app::Error::Unsupported)
    }
}

/// Device-wide monotonic time, shared across service/helper processes and including sleep.
/// Used for the original first-start window; it is never renewed by an IPC retry.
pub fn monotonic_millis() -> Result<u64, execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn rss_execution_continuous_millis() -> u64;
        }
        let value = unsafe { rss_execution_continuous_millis() };
        if value == u64::MAX {
            Err(execution_app::Error::Clock)
        } else {
            Ok(value)
        }
    }
    #[cfg(windows)]
    {
        Ok(unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() })
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(execution_app::Error::Unsupported)
    }
}

/// Select the actual console account before its user helper has been installed.
/// This grants no session or IPC authority; callers still enforce the configured peer policy.
pub fn active_user_subject() -> Result<String, execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::macos_identity::console_account()?.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        active_user_session().map(|(subject, _)| subject)
    }
}

/// Select an actual active OS login. Ambiguous or absent sessions are unavailable.
pub fn active_user_session() -> Result<(String, u32), execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        let uid = crate::macos_identity::console_account()?;
        Ok((uid.to_string(), crate::macos_service::helper_session(uid)?))
    }
    #[cfg(windows)]
    {
        crate::windows_identity::active_user_session()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(execution_app::Error::Unsupported)
    }
}

/// Whether this process belongs to the active graphical login of its actual OS account.
pub fn active_login() -> bool {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn rss_execution_gui_active() -> i32;
        }
        unsafe { rss_execution_gui_active() == 1 }
    }
    #[cfg(windows)]
    {
        let Ok((subject, _)) = crate::windows_identity::token_identity() else {
            return false;
        };
        let Ok(account) = Id::new(subject).map(|subject| execution_contract::OsAccountRef {
            platform: execution_contract::Platform::Windows,
            subject,
        }) else {
            return false;
        };
        let Ok(session) = current_session_binding() else {
            return false;
        };
        crate::windows_identity::identity(
            &execution_contract::RunAs::User {
                account: account.clone(),
            },
            &execution_contract::SessionRequirement::ActiveUser { account, session },
        )
        .is_ok()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        false
    }
}

/// Native connection facts, borrowed while the native transport retains the connection.
/// Fields are private and no deserializer exists; only native transports construct them.
pub struct Peer {
    pub(crate) pid: u32,
    pub(crate) uid: Option<u32>,
    pub(crate) session: u32,
    #[cfg(windows)]
    pub(crate) native: usize,
}
impl Peer {
    /// Authenticate the live connection against administrator-owned image and OS account pins.
    /// This is local ingress identity, never an enterprise task permission.
    pub fn authenticate(&self, policy: &PeerPolicy) -> Result<String, execution_app::Error> {
        // Endpoint assembly validates every installed candidate before listening. Each
        // message checks the bounded policy and the actual live peer's protected image;
        // hashing unrelated candidates again does not authenticate this connection.
        policy.validate_shape()?;
        #[cfg(target_os = "macos")]
        {
            crate::macos_service::authenticate(self, policy)
        }
        #[cfg(windows)]
        {
            crate::windows_service::authenticate(self, policy)
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            Err(execution_app::Error::Unsupported)
        }
    }
    /// Borrowed native connection identity; valid only during this handler call.
    #[cfg(windows)]
    pub(crate) fn native_handle(&self) -> usize {
        self.native
    }
    /// Verify a pinned extension image for a system-selected role or user account.
    /// This admits no Desktop/helper operation; the caller must restrict the request class first.
    pub fn authenticate_extension(
        &self,
        image: &installation_security::Artifact,
    ) -> Result<String, execution_app::Error> {
        #[cfg(target_os = "macos")]
        {
            let uid = self.uid.ok_or(execution_app::Error::Denied)?;
            self.authenticate(&PeerPolicy {
                images: vec![image.clone()],
                subjects: vec![uid.to_string()],
                interactive: false,
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = image;
            Err(execution_app::Error::Unsupported)
        }
    }
    /// Kernel peer process identity, never accepted from request bytes.
    pub fn pid(&self) -> u32 {
        self.pid
    }
    /// Unix peer UID; absent on Windows, where native token verification is required.
    pub fn uid(&self) -> Option<u32> {
        self.uid
    }
    /// Native peer session identifier, not product authentication.
    pub fn session(&self) -> u32 {
        self.session
    }
    /// Native original connection login, independent of every request-body claim.
    pub fn session_binding(&self) -> Result<Id, execution_app::Error> {
        #[cfg(target_os = "macos")]
        {
            Id::new(format!(
                "{}/{}",
                crate::macos_identity::boot_generation()?.as_str(),
                self.session
            ))
            .map_err(|_| execution_app::Error::Unbound)
        }
        #[cfg(windows)]
        {
            crate::windows_service::session_binding(self)
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            Err(execution_app::Error::Unsupported)
        }
    }
}

/// A live native connection authenticated as the pinned system service. This permits only
/// delegated process handling; enterprise authorization and the journal remain in that service.
pub struct SystemConnection<'a> {
    _peer: &'a Peer,
}
impl Peer {
    /// Authenticate a system-service call before decoding its delegated process input.
    pub fn system_connection<'a>(
        &'a self,
        policy: &PeerPolicy,
    ) -> Result<SystemConnection<'a>, execution_app::Error> {
        let expected = if cfg!(windows) { "S-1-5-18" } else { "0" };
        if policy.interactive
            || policy.subjects != [expected]
            || self.authenticate(policy)? != expected
        {
            return Err(execution_app::Error::Denied);
        }
        Ok(SystemConnection { _peer: self })
    }
}

/// Trusted installation pins shared by the native server and client. Never supplied by IPC data.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PeerPolicy {
    /// Allowed installed image identities.
    pub images: Vec<installation_security::Artifact>,
    /// Exact allowed OS subjects; empty denies every peer for unattended deployment.
    /// These are not enterprise principals.
    pub subjects: Vec<String>,
    /// Require a non-system interactive session.
    pub interactive: bool,
}
impl PeerPolicy {
    fn validate_shape(&self) -> Result<(), execution_app::Error> {
        if self.images.is_empty()
            || self.images.len() > 8
            || self.subjects.len() > 64
            || self.subjects.iter().any(|s| s.is_empty() || s.len() > 256)
        {
            return Err(execution_app::Error::Configuration);
        }
        Ok(())
    }
    /// Verify bounded pins and every protected image before exposing a native endpoint.
    pub fn validate(&self) -> Result<(), execution_app::Error> {
        self.validate_shape()?;
        for image in &self.images {
            image
                .verify(&image.path)
                .map_err(|_| execution_app::Error::Denied)?;
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn requirement(&self) -> Result<std::ffi::CString, execution_app::Error> {
        self.validate()?;
        let requirements = self
            .images
            .iter()
            .map(|i| {
                i.requirement()
                    .map(|r| format!("({})", r.to_string_lossy()))
                    .map_err(|_| execution_app::Error::Denied)
            })
            .collect::<Result<Vec<_>, _>>()?;
        std::ffi::CString::new(requirements.join(" or "))
            .map_err(|_| execution_app::Error::Configuration)
    }
}
/// Provenance supplied by the trusted native client; OS identity always comes from the connection.
#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientOrigin {
    /// User selected the exact backend task in the desktop.
    Desktop {},
    /// Native desktop accepted a user-confirmed AI request with its bound conversation identity.
    Ai {
        /// Native connection configuration.
        config: VersionedRef,
        /// Conversation correlation.
        conversation: Id,
        /// Exact tool-call correlation.
        tool_call: Id,
        /// Exact owner-issued execution context from the native bridge.
        context: execution_contract::NativeContextRef,
    },
}
/// Closed native credential handoff; never included in renderer, AI schema or journal.
#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NativeAccess {
    /// Device access has no enterprise credential or account coordinates.
    Device {},
    /// Native verified employee session; the backend owns its effective qualification.
    User {
        /// Only the employee secret, not the cookie or CSRF header.
        credential: zeroize::Zeroizing<String>,
        /// Verified product identity, distinct from the authenticated OS peer.
        user: rss_mdm_agent_wire::runtime::SelfServiceUser,
    },
}
/// Current native ingress context, optionally carrying an enterprise session.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeContextSession {
    /// Closed access shape; Device never carries a user secret.
    pub access: NativeAccess,
    /// Exact native product login generation.
    pub generation: Id,
    /// Current session expiry; never extended by an invocation.
    pub expires_at_unix_ms: u64,
}
impl NativeContextSession {
    /// Enterprise identity hint; effective admission remains backend-owned.
    pub fn user(&self) -> Option<&rss_mdm_agent_wire::runtime::SelfServiceUser> {
        match &self.access {
            NativeAccess::Device {} => None,
            NativeAccess::User { user, .. } => Some(user),
        }
    }
    /// Protected native credential, available only in Rust.
    pub fn credential(&self) -> Option<&zeroize::Zeroizing<String>> {
        match &self.access {
            NativeAccess::Device {} => None,
            NativeAccess::User { credential, .. } => Some(credential),
        }
    }
    /// Retire bearer memory without reclassifying a User context as Device.
    pub fn clear_credential(&mut self) {
        if let NativeAccess::User { credential, .. } = &mut self.access {
            zeroize::Zeroize::zeroize(credential);
        }
    }
}
/// Local IPC V1 carries backend task references, never executable plans or authority claims.
#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    /// Verify only this authenticated native peer's configured user helper.
    HelperStatus {},
    /// Finite desktop DNS/TCP diagnostics through the existing execution owner.
    NetworkCheck {
        /// Current native login context.
        context: execution_contract::NativeContextRef,
        /// Native-attested source, restricted to Desktop by the service.
        origin: ClientOrigin,
        /// One finite diagnostic operation.
        input: execution_contract::NetworkCheckOperation,
    },
    /// Desktop-only finite local settings operation.
    Settings {
        /// Current authenticated native login context.
        context: execution_contract::NativeContextRef,
        /// Attested source; AI cannot submit this interface.
        origin: ClientOrigin,
        /// Closed read, change or native navigation intent.
        input: execution_contract::SettingsOperation,
    },
    /// Only the separately pinned native extension may use this route.
    PlatformSso {
        /// Bounded registration operation; never accepts a caller-selected local account.
        input: crate::platform_sso::Operation,
    },
    /// Authenticated desktop-only Device Access setup; no AI equivalent.
    DeviceAccess {
        /// Exact current employee context.
        context: execution_contract::NativeContextRef,
        /// Native-attested origin.
        origin: ClientOrigin,
        /// Finite local setup operation.
        input: crate::device_access::Operation,
    },
    /// Typed local software intent, without a remote task or caller-selected command.
    LocalSoftware {
        /// Current authenticated native context.
        context: execution_contract::NativeContextRef,
        /// Native-attested source; confirmation is accepted only from the desktop.
        origin: ClientOrigin,
        /// Bounded read/prepare/submit/cancel operation.
        input: execution_contract::LocalSoftwareOperation,
    },
    /// Read user-visible software from an existing authorized data snapshot.
    SoftwareRead {
        /// Current authenticated native context.
        context: execution_contract::NativeContextRef,
        /// Native-attested source.
        origin: ClientOrigin,
        /// Bounded snapshot read.
        input: execution_contract::SoftwareRead,
    },
    /// Shared local data boundary, with a native-attested caller context.
    Data {
        /// Current native login binding.
        context: execution_contract::NativeContextRef,
        /// Native-attested caller origin.
        origin: ClientOrigin,
        /// Bounded data operation.
        input: execution_contract::DataOperation,
    },
    /// Finite native software observation; service selects only the exact signed detector.
    ObserveSoftware {
        /// Stable query and exact current signed selection.
        choice: execution_contract::SoftwareReadChoice,
        /// Origin attested by the native peer.
        origin: ClientOrigin,
    },
    /// Bind or revoke the native employee session for this authenticated login.
    BindContext {
        /// Device context, optionally augmented by a verified enterprise session.
        value: NativeContextSession,
        /// Existing binding for a refresh; absence requires a retired or absent context.
        expected: Option<Id>,
    },
    /// Read durable projection progress without changing the local execution context.
    CurrentUserStatus {},
    /// Drain the exact enterprise relation after immediate local fencing.
    RetireEnterpriseContext {
        /// Original native login incarnation, absent only for explicit crash recovery.
        generation: Option<Id>,
        /// Original enterprise proof, retained only in native memory through retirement.
        proof: Option<zeroize::Zeroizing<String>>,
        /// Explicitly abandon a lost enterprise proof and clear the observed device relation.
        recover: bool,
    },
    /// Retire this peer's current context using the independent control lane.
    RevokeContext {},
    /// Query the current Policy directory; origin is attested by the native bridge.
    PolicyDirectory {
        /// Exact current native context binding.
        context: execution_contract::NativeContextRef,
        /// Opaque backend page cursor.
        after: Option<String>,
        /// Trusted native origin.
        origin: ClientOrigin,
    },
    /// Resolve an exact authorized capability independently of directory pagination.
    ResolvePolicy {
        /// Current native context.
        context: execution_contract::NativeContextRef,
        /// Server-owned exact address.
        selector: rss_mdm_agent_wire::runtime::CapabilitySelector,
        /// Authenticated caller classification.
        origin: ClientOrigin,
    },
    /// Freeze a Policy choice in the unique journal; neither source nor confirmation is a model field.
    InvokePolicy {
        /// Exact current native context binding.
        context: execution_contract::NativeContextRef,
        /// Exact directory choice and Input values.
        choice: execution_contract::PolicyChoice,
        /// Actual discovery page, never an authorization claim.

        /// Trusted native origin.
        origin: ClientOrigin,
    },
    /// Prepare an editable AI operation, without sending or granting consent.
    PreparePolicy {
        /// Current authenticated native binding.
        context: execution_contract::NativeContextRef,
        /// Policy identity and non-authoritative initial suggestions.
        choice: execution_contract::PolicyChoice,
        /// Actual discovery page, re-fetched before freezing the draft.

        /// Attested AI origin; never renderer supplied.
        origin: ClientOrigin,
    },
    /// Freeze current parameters and record the authenticated user's save intent.
    SubmitPolicy {
        /// Current native binding, revalidated by the service.
        context: execution_contract::NativeContextRef,
        /// Original request, displayed revision and final values.
        input: execution_contract::PolicySubmission,
    },
    /// Sole employee response to the original risk-two Policy preparation.
    ConfirmPolicy {
        /// Exact current native context binding.
        context: execution_contract::NativeContextRef,
        /// Original stable request.
        request: RequestId,
    },
    /// Authorized result of the original Policy invocation.
    PolicyStatus {
        /// Exact current native context binding.
        context: execution_contract::NativeContextRef,
        /// Stable original request.
        request: RequestId,
    },
    /// Read safe installation and execution readiness; never execution permission.
    ServiceStatus {},
    /// Read local identity and network facts for the authenticated desktop peer.
    DeviceOverview {},
    /// Read the same native MDM state used for Inventory.
    MdmStatus {},
    /// Read public machine registration progress.
    RegistrationStatus {},
    /// Read trusted machine coordinates without disclosing credentials.
    RegistrationContext {},
    /// Native-only complete signed configuration; secrets never cross the renderer.
    ImportRegistration {
        /// Current server-owned complete configuration format.
        file: rss_mdm_agent_wire::enroll::configuration::ConfigurationFile,
    },
    /// Explicitly request browser authorization for this configured installation.
    RequestRegistration {},
    /// List authorized local projections and the current offered task.
    Tasks {
        /// Exclusive journal page cursor.
        after: Option<RequestId>,
    },
    /// Bounded latest activities from the existing journal, under current peer authorization.
    RecentTasks {
        /// Exclusive receipt sequence, decimal text from the previous page.
        before: Option<String>,
    },
    /// Select the precise backend offer. This is a user action, not an enterprise approval.
    StartTask {
        /// Deterministic request from the displayed offer.
        request: RequestId,
        /// Exact backend task.
        task: Id,
        /// Exact backend attempt.
        attempt: Id,
        /// Digest of the displayed offer.
        revision: Digest,
        /// Native origin correlation.
        origin: ClientOrigin,
    },
    /// Confirm exactly one pending risk-two AI action from the authenticated desktop login.
    /// AI MCP callers have no equivalent method.
    ConfirmTask {
        /// Original backend task reference; never executable content or an approval claim.
        selection: execution_contract::BackendSelection,
    },
    /// Read an existing execution.
    Status {
        /// Journal request identity.
        request: RequestId,
    },
    /// Read the safe immutable details of an existing execution.
    Details {
        /// Journal request identity.
        request: RequestId,
    },
    /// Request cancellation of the caller's user-triggered task.
    Cancel {
        /// Journal request identity.
        request: RequestId,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    request: Request,
}
/// Closed reply, preserving acceptance separately from execution and effect.
#[derive(Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Reply {
    /// Native helper facts verified by the system owner for the requesting peer.
    HelperStatus {
        /// Code identity, login binding and work root have been checked by the owner.
        value: crate::helper::UserContext,
    },
    /// Default destination selected from the installed backend configuration.
    NetworkDefaults {
        /// Absent when backend configuration is unavailable; manual entry remains possible.
        target: Option<execution_contract::NetworkTarget>,
    },
    /// Original diagnostic task and safe result.
    NetworkCheck {
        /// Existing lifecycle with bounded diagnostic evidence.
        value: execution_app::NetworkCheckStatus,
    },
    /// Observation and original journal status for a finite local settings request.
    Settings {
        /// Safe native projection; raw worker output remains private.
        value: execution_contract::SettingsResponse,
    },
    /// Reply only to an authenticated Platform SSO extension.
    PlatformSso {
        /// Bounded result containing no private keys.
        value: crate::platform_sso::Response,
    },
    /// Device Access response, including ephemeral local setup material when requested.
    DeviceAccess {
        /// Typed desktop-only result.
        value: crate::device_access::Response,
    },
    /// Immutable local preview retained in the existing execution journal.
    SoftwarePrepared {
        /// Safe summary; executable input remains private.
        value: execution_contract::LocalSoftwarePrepared,
    },
    /// Authorized bounded local software journal history.
    SoftwareHistory {
        /// Existing execution status and immutable previews.
        value: execution_app::LocalSoftwareHistory,
    },
    /// Existing journal status for the same local software request.
    SoftwareStatus {
        /// Typed preview and lifecycle facts.
        value: execution_app::LocalSoftwareStatus,
    },
    /// Typed software facts, independently of enterprise directory availability.
    SoftwarePage {
        /// Existing observations with precise snapshot references.
        value: execution_contract::SoftwarePage,
    },
    /// Closed data boundary failure.
    DataFailure {
        /// Static non-sensitive classification.
        reason: execution_contract::DataError,
    },
    /// Local collected data or original query state.
    Data {
        /// Shared typed projection.
        value: execution_contract::DataResponse,
    },
    /// Read-only native registration observation.
    MdmStatus {
        /// Current owner snapshot.
        value: MdmStatus,
    },
    /// Current bounded local facts; no secrets, stable IDs or enterprise identity.
    DeviceOverview {
        /// Projection owned by the installed system service.
        value: DeviceOverview,
    },
    /// Known pre-submission rejection, distinct from an ambiguous response loss.
    OperationRejected {
        /// Closed, non-sensitive diagnosis.
        reason: execution_contract::OperationRejection,
    },
    /// Native employee handoff accepted; no enterprise permission is granted.
    ContextBound {
        /// Nonsecret binding, attested by this native owner.
        value: execution_contract::NativeContextRef,
    },
    /// Relation settlement is separate from the immediate local fence.
    CurrentUserSync {
        /// Nonsecret, durable synchronization outcome.
        value: execution_contract::CurrentUserSync,
    },
    /// Retirement acknowledged by the independent control lane.
    ContextRevoked,
    /// Formal authorized Policy directory, not an execution permit.
    /// Exact current authorized interface.
    PolicyResolved {
        /// Server-owned capability descriptor and invocation interface.
        value: Box<rss_mdm_agent_wire::runtime::SelfServiceDirectoryItem>,
    },
    /// Current authorized discovery page.
    PolicyDirectory {
        /// Current server projection.
        value: rss_mdm_agent_wire::runtime::SelfServiceDirectoryResponse,
    },
    /// Original request state, including preparation before any backend Run exists.
    PolicyRequest {
        /// Same authoritative journal record.
        value: Box<execution_contract::BackendRequest>,
    },
    /// No request was persisted by this sole execution owner.
    PolicyAbsent,
    /// Structured server-authorized status/result.
    PolicyStatus {
        /// Original invocation's durable server observation.
        value: rss_mdm_agent_wire::runtime::SelfServiceStatus,
    },
    /// Safe readiness projection from the authenticated system owner.
    ServiceStatus {
        /// Closed service diagnostic.
        value: ServiceStatus,
    },
    /// Public registration projection, with no credentials or device key references.
    RegistrationContext {
        /// Trusted machine coordinates and progress.
        value: RegistrationContext,
    },
    /// Safe registration progress.
    RegistrationStatus {
        /// Closed public progress.
        value: RegistrationStatus,
    },
    /// Existing journal state.
    Status {
        /// Authorized projection.
        value: execution_app::ExecutionStatus,
    },
    /// Durable preparation, before an execution plan or attempt exists.
    Pending {
        /// Original authenticated local intent.
        value: Box<execution_contract::BackendRequest>,
    },
    /// Existing safe task details.
    Details {
        /// Authorized details without raw output.
        value: Box<execution_app::ExecutionTaskDetails>,
    },
    /// Bounded task page.
    Tasks {
        /// Existing executions.
        value: execution_app::TaskPage,
        /// Current offered task.
        available: Vec<execution_contract::BackendTask>,
        /// Recent preparation records retained in the same journal.
        preparations: Vec<execution_contract::BackendRequest>,
    },
    /// Authorized newest-per-request activities, with current safe task facts.
    RecentTasks {
        /// Bounded page; empty does not imply end of discovery.
        value: execution_app::RecentTaskPage,
    },
    /// User selection accepted; this does not assert dispatch or installation.
    Queued {
        /// Selected backend task.
        task: Id,
        /// Selected backend attempt.
        attempt: Id,
        /// Original journal request identity.
        request: RequestId,
        /// AI proposals require a subsequent human action.
        confirmation_required: bool,
    },
    /// Invalid or unauthorized operation.
    Rejected,
    /// Submission/response could not be confirmed; query the original identity.
    Unavailable,
}
/// Encode the current protocol only; no legacy plan endpoint or compatibility negotiation.
pub fn encode(request: Request) -> Result<Vec<u8>, execution_app::Error> {
    serde_json::to_vec(&Envelope {
        version: IPC_VERSION,
        request,
    })
    .map_err(|_| execution_app::Error::InvalidInput)
}
/// Pinned native service client shared by desktop and AI composition. Owns no journal or runner.
#[derive(Clone)]
pub struct ServiceClient {
    server: PeerPolicy,
}
impl ServiceClient {
    /// Load only the system-service pin from the protected deployment document.
    /// Desktop consumers do not open the communication credential or journal.
    pub fn installed() -> Result<Self, execution_app::Error> {
        #[derive(Deserialize)]
        struct Pin {
            version: u32,
            ipc_version: u8,
            service: installation_security::Artifact,
        }
        let path = installation_security::deployment_path()
            .map_err(|_| execution_app::Error::Configuration)?;
        let bytes = installation_security::read_protected(&path)
            .map_err(|_| execution_app::Error::Unavailable)?;
        let pin: Pin =
            serde_json::from_slice(&bytes).map_err(|_| execution_app::Error::Configuration)?;
        if pin.version != DEPLOYMENT_VERSION || pin.ipc_version != IPC_VERSION {
            return Err(execution_app::Error::Configuration);
        }
        // The native request validates this protected image immediately before connecting.
        // Avoid hashing the same installed image again while assembling each short-lived client.
        Ok(Self {
            server: PeerPolicy {
                images: vec![pin.service],
                subjects: vec![if cfg!(windows) { "S-1-5-18" } else { "0" }.into()],
                interactive: false,
            },
        })
    }

    /// Inspect installation and the same authenticated service without opening device storage.
    pub fn inspect() -> ServiceView {
        let Ok(path) = installation_security::deployment_path() else {
            return ServiceView::Unavailable;
        };
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return match registration_present() {
                    Ok(false) => ServiceView::NotInstalled,
                    Ok(true) => ServiceView::ConfigurationRequired,
                    Err(_) => ServiceView::Unavailable,
                }
            }
            Err(_) => return ServiceView::Rejected,
            Ok(_) => (),
        }
        if installation_security::read_protected(&path).is_err() {
            return ServiceView::Rejected;
        }
        let client = match Self::installed() {
            Ok(v) => v,
            Err(execution_app::Error::Configuration | execution_app::Error::InvalidInput) => {
                return ServiceView::Mismatch
            }
            Err(execution_app::Error::Denied) => return ServiceView::Rejected,
            Err(_) => return ServiceView::Unavailable,
        };
        match client.request(Request::ServiceStatus {}) {
            Ok(Reply::ServiceStatus { value }) if value.protocol == IPC_VERSION => {
                ServiceView::Connected { status: value }
            }
            Ok(Reply::Rejected) | Err(execution_app::Error::Denied) => ServiceView::Rejected,
            Err(execution_app::Error::Unsupported | execution_app::Error::InvalidInput) => {
                ServiceView::Mismatch
            }
            Ok(Reply::ServiceStatus { .. }) => ServiceView::Mismatch,
            _ => ServiceView::Unavailable,
        }
    }
    /// Bind only to the installed system-service identity from protected deployment data.
    pub fn new(server: PeerPolicy) -> Result<Self, execution_app::Error> {
        server.validate()?;
        let expected = if cfg!(windows) { "S-1-5-18" } else { "0" };
        if server.interactive || server.subjects != [expected] {
            return Err(execution_app::Error::Denied);
        }
        Ok(Self { server })
    }
    /// Send a bounded V1 request over a mutually authenticated native connection.
    /// Transport failure does not mean a submitted selection or cancellation failed.
    pub fn request(&self, request: Request) -> Result<Reply, execution_app::Error> {
        let bytes = encode(request)?;
        #[cfg(target_os = "macos")]
        let response = crate::macos_service::query_trusted(&bytes, true, &self.server)?;
        #[cfg(windows)]
        let response = crate::windows_service::query_trusted(&bytes, true, &self.server)?;
        #[cfg(not(any(target_os = "macos", windows)))]
        let response: Vec<u8> = {
            let _ = bytes;
            return Err(execution_app::Error::Unsupported);
        };
        decode_reply(&response)
    }
}
/// The native OS service owns one handler; ticks and calls are serialized by the owner loop.
pub trait SessionFence: Send + Sync {
    /// Authenticate the native peer and invalidate its employee grants without business IO.
    fn revoke(&self, peer: &Peer) -> Reply;
}
/// Only a current, bounded native employee revocation bypasses the serial business handler.
#[cfg(windows)]
pub(crate) fn is_employee_revocation(bytes: &[u8]) -> bool {
    bytes.len() <= 65536
        && serde_json::from_slice::<Envelope>(bytes).is_ok_and(|e| {
            e.version == IPC_VERSION && matches!(e.request, Request::RevokeContext {})
        })
}
/// Decode a current revocation and dispatch only its authenticated ephemeral fence.
pub fn dispatch_fence(fence: &dyn SessionFence, peer: &Peer, bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() > 65536 {
        return None;
    }
    let envelope: Envelope = serde_json::from_slice(bytes).ok()?;
    if envelope.version != IPC_VERSION {
        return None;
    }
    match envelope.request {
        Request::RevokeContext {} => Some(encode_reply(fence.revoke(peer))),
        _ => None,
    }
}
/// Native lifecycle handler; all business work remains serialized by its owner.
pub trait Handler: Send {
    /// Optional authenticated revocation which performs no execution or journal IO.
    fn session_fence(&self) -> Option<std::sync::Arc<dyn SessionFence>> {
        None
    }

    /// Current installed client pins. Production handlers must supply these before listening.
    fn peer_policy(&self) -> Option<PeerPolicy> {
        None
    }
    /// Register a native user-helper endpoint after checking the actual connection identity.
    fn register_helper(&mut self, _: &Peer) -> Result<(), execution_app::Error> {
        Err(execution_app::Error::Denied)
    }
    /// Whether this handler is a helper in its still-active original GUI login.
    fn helper_active(&self) -> bool {
        false
    }
    /// Process one bounded request serially; a lost reply does not prove no mutation.
    fn handle(&mut self, peer: &Peer, request: Request) -> Reply;
    /// Native framing seam for the separately authenticated helper protocol. The system
    /// service uses this default, which accepts only task-reference IPC V1.
    fn handle_wire(&mut self, peer: &Peer, bytes: &[u8]) -> Vec<u8> {
        let reply = if bytes.len() > 65536 {
            Reply::Rejected
        } else {
            match serde_json::from_slice::<Envelope>(bytes) {
                Ok(e) if e.version == IPC_VERSION => self.handle(peer, e.request),
                _ => Reply::Rejected,
            }
        };
        encode_reply(reply)
    }
    /// Reconcile existing attempts without obtaining replacement dispatch authority.
    fn tick(&mut self) -> Result<(), execution_app::Error>;
    /// Stop owned work and persist available facts before returning.
    fn stop(&mut self) -> Result<(), execution_app::Error>;
}
/// Executable's default assembly: there is deliberately no environment/test authority fallback.
#[cfg(test)]
pub struct Unbound;
#[cfg(test)]
impl Handler for Unbound {
    /// Process one bounded request serially; a lost reply does not prove no mutation.
    fn handle(&mut self, _: &Peer, _: Request) -> Reply {
        Reply::Rejected
    }
    /// Reconcile existing attempts without obtaining replacement dispatch authority.
    fn tick(&mut self) -> Result<(), execution_app::Error> {
        Ok(())
    }
    /// Stop owned work and persist available facts before returning.
    fn stop(&mut self) -> Result<(), execution_app::Error> {
        Ok(())
    }
}
/// Decode only bounded, current envelopes; malformed input never invokes a handler.
pub fn dispatch(handler: &mut dyn Handler, peer: &Peer, bytes: &[u8]) -> Vec<u8> {
    if bytes.len() > FRAME_LIMIT {
        return encode_reply(Reply::Rejected);
    }
    let reply = handler.handle_wire(peer, bytes);
    if reply.len() > FRAME_LIMIT {
        encode_reply(Reply::Unavailable)
    } else {
        reply
    }
}
/// Native transport bound, including escaped bounded process output from a user helper.
pub const FRAME_LIMIT: usize = 8 * 1024 * 1024;
/// Enter the native lifecycle with the production application owner.
#[cfg(target_os = "macos")]
pub fn run(
    handler: Box<dyn Handler>,
    stop: &'static AtomicBool,
) -> Result<(), execution_app::Error> {
    crate::macos_service::run(handler, stop)
}

#[cfg(test)]
mod tests {
    #[test]
    fn helper_status_accepts_no_caller_coordinates_and_returns_closed_native_facts() {
        let bytes = super::encode(super::Request::HelperStatus {}).unwrap();
        let mut request: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for key in ["subject", "session", "binding", "workRoot"] {
            request["request"][key] = serde_json::json!("spoofed");
            assert!(serde_json::from_value::<super::Envelope>(request.clone()).is_err());
            request["request"].as_object_mut().unwrap().remove(key);
        }
        let bytes = super::encode_reply(super::Reply::HelperStatus {
            value: crate::helper::UserContext {
                subject: "501".into(),
                session: 42,
                binding: execution_contract::Id::new("current-login").unwrap(),
                work_root: "/private/helper".into(),
            },
        });
        assert!(
            matches!(super::decode_reply(&bytes), Ok(super::Reply::HelperStatus { value }) if value.subject == "501" && value.session == 42)
        );
        let mut reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        reply["reply"]["value"]["authority"] = serde_json::json!("spoofed");
        assert!(super::decode_reply(&serde_json::to_vec(&reply).unwrap()).is_err());
    }

    #[test]
    fn device_overview_request_cannot_supply_a_login_or_network_target() {
        let request = super::encode(super::Request::DeviceOverview {}).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&request).unwrap();
        assert_eq!(value["version"], super::IPC_VERSION);
        for field in ["subject", "session", "address"] {
            value["request"][field] = serde_json::json!("spoofed");
            assert!(serde_json::from_value::<super::Envelope>(value.clone()).is_err());
            value["request"].as_object_mut().unwrap().remove(field);
        }
        let overview = super::DeviceOverview {
            attributes: super::DeviceAttributes {
                computer_name: Some("Studio Mac".into()),
                ip_addresses: Some(vec!["192.0.2.24".into()]),
                login_user: Some("studio".into()),
                extra: std::collections::BTreeMap::from([(
                    "customFact".into(),
                    serde_json::json!({"site":"test"}),
                )]),
            },
        };
        let bytes = super::encode_reply(super::Reply::DeviceOverview {
            value: overview.clone(),
        });
        assert!(
            matches!(super::decode_reply(&bytes), Ok(super::Reply::DeviceOverview {value}) if value == overview)
        );
    }
    #[test]
    fn native_access_is_closed_and_retirement_does_not_downgrade_user() {
        let base =
            serde_json::json!({"access":{"kind":"device"},"generation":"g","expiresAtUnixMs":1});
        assert!(serde_json::from_value::<super::NativeContextSession>(base.clone()).is_ok());
        for field in ["credential", "user", "credentialOrigin"] {
            let mut invalid = base.clone();
            invalid["access"][field] = serde_json::json!("unexpected");
            assert!(serde_json::from_value::<super::NativeContextSession>(invalid).is_err());
        }
        let mut old = base.clone();
        old.as_object_mut().unwrap().remove("access");
        old["user"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<super::NativeContextSession>(old).is_err());
        let user: rss_mdm_agent_wire::runtime::SelfServiceUser =
            serde_json::from_value(serde_json::json!({
                "tenantId":"00000000-0000-0000-0000-000000000001",
                "instanceId":"00000000-0000-0000-0000-000000000002",
                "principalId":"00000000-0000-0000-0000-000000000003"
            }))
            .unwrap();
        let mut context = super::NativeContextSession {
            access: super::NativeAccess::User {
                credential: zeroize::Zeroizing::new("c".repeat(64)),

                user: user.clone(),
            },
            generation: execution_contract::Id::new("g").unwrap(),
            expires_at_unix_ms: 1,
        };
        context.clear_credential();
        assert_eq!(context.user(), Some(&user));
        assert!(context.credential().unwrap().is_empty());
    }
    use super::*;
    #[test]
    fn client_never_decodes_legacy_or_wrong_version_replies() {
        for bytes in [
            br#"{"kind":"unavailable"}"#.as_slice(),
            br#"{"version":8,"reply":{"kind":"unavailable"}}"#,
            br#"{"version":9,"reply":{"kind":"unavailable"},"authority":"fake"}"#,
        ] {
            assert!(decode_reply(bytes).is_err());
        }
        assert!(matches!(
            decode_reply(&encode_reply(Reply::Unavailable)),
            Ok(Reply::Unavailable)
        ));
        assert!(matches!(
            decode_reply(&encode_reply(Reply::Rejected)),
            Ok(Reply::Rejected)
        ));
    }
    #[test]
    fn malformed_versions_and_unbound_calls_never_execute() {
        let peer = Peer {
            pid: 1,
            uid: Some(0),
            session: 1,
            #[cfg(windows)]
            native: 0,
        };
        let mut handler = Unbound;
        for bytes in [
            br#"{"version":1,"request":{"method":"status","request":"r"}}"#.as_slice(),
            br#"{"version":4,"request":{"method":"status","request":"r"}}"#,
            br#"{"version":4,"actor":"root","request":{"method":"cancel","request":"r"}}"#,
        ] {
            assert_eq!(
                dispatch(&mut handler, &peer, bytes),
                encode_reply(Reply::Rejected)
            )
        }
    }
}
