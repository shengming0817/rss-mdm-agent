//! One bounded IPC owner driving the existing application. Peer identity is never a wire field.
use execution_contract::{Digest, Id, RequestId, VersionedRef};
use serde::{Deserialize, Serialize};
/// Current desktop/system IPC. No negotiation or legacy parser exists.
pub const IPC_VERSION: u8 = 6;
/// Current protected installation document format.
pub const DEPLOYMENT_VERSION: u32 = 2;

/// Safe readiness facts; connecting never grants execution authority.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(tag = "phase", rename_all = "camelCase", deny_unknown_fields)]
pub enum Readiness {
    /// Existing device owner is open and driving recovery/work.
    Ready,
    /// A trusted installation is provably never initialized.
    RegistrationRequired,
    /// Existing state must be inspected; it must not be reset or re-registered.
    NotReady,
}
/// Safe statement from the authenticated production owner.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceStatus {
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
            platform: std::env::consts::OS.into(),
            readiness,
        }
    }
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
    serde_json::to_vec(&Response {
        version: IPC_VERSION,
        reply,
    })
    .expect("closed service reply serialization")
}
fn decode_reply(bytes: &[u8]) -> Result<Reply, execution_app::Error> {
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
        crate::windows::token_identity().map(|v| v.0)
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
        crate::windows::token_identity().map(|v| v.1)
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
            crate::platform::boot_generation()?.as_str(),
            current_session()?
        ))
        .map_err(|_| execution_app::Error::Unbound)
    }
    #[cfg(windows)]
    {
        crate::windows::current_session_binding()
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

/// Select an actual active OS login. Ambiguous or absent sessions are unavailable.
pub fn active_user_session() -> Result<(String, u32), execution_app::Error> {
    #[cfg(target_os = "macos")]
    {
        let uid = crate::macos::console_account()?;
        Ok((uid.to_string(), crate::macos_service::helper_session(uid)?))
    }
    #[cfg(windows)]
    {
        crate::windows::active_user_session()
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
        let Ok((subject, _)) = crate::windows::token_identity() else {
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
        crate::windows::identity(
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
    pub(crate) native: usize,
}
impl Peer {
    /// Authenticate the live connection against administrator-owned image and OS account pins.
    /// This is local ingress identity, never an enterprise task permission.
    pub fn authenticate(&self, policy: &PeerPolicy) -> Result<String, execution_app::Error> {
        policy.validate()?;
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
    pub fn native_handle(&self) -> usize {
        self.native
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
                crate::platform::boot_generation()?.as_str(),
                self.session
            ))
            .map_err(|_| execution_app::Error::Unbound)
        }
        #[cfg(windows)]
        {
            crate::windows::service::session_binding(self)
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            Err(execution_app::Error::Unsupported)
        }
    }
}

/// A live native connection authenticated as the pinned system service. This permits only
/// delegated process handling; enterprise authorization and the journal remain in that service.
pub(crate) struct SystemConnection<'a> {
    _peer: &'a Peer,
}
impl Peer {
    /// Authenticate a system-service call before decoding its delegated process input.
    pub(crate) fn system_connection<'a>(
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
    /// Exact allowed OS subjects; these are not enterprise principals.
    pub subjects: Vec<String>,
    /// Require a non-system interactive session.
    pub interactive: bool,
}
impl PeerPolicy {
    /// Verify bounded pins and every protected image before exposing a native endpoint.
    pub fn validate(&self) -> Result<(), execution_app::Error> {
        if self.images.is_empty()
            || self.images.len() > 8
            || self.subjects.is_empty()
            || self.subjects.len() > 64
            || self.subjects.iter().any(|s| s.is_empty() || s.len() > 256)
        {
            return Err(execution_app::Error::Configuration);
        }
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
    },
}
/// Local IPC V6 carries backend task references, never executable plans or authority claims.
#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    /// Read safe installation and execution readiness; never execution permission.
    ServiceStatus {},
    /// List authorized local projections and the current offered task.
    Tasks {
        /// Exclusive journal page cursor.
        after: Option<RequestId>,
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
    /// Safe readiness projection from the authenticated system owner.
    ServiceStatus {
        /// Closed service diagnostic.
        value: ServiceStatus,
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
    /// Send a bounded V6 request over a mutually authenticated native connection.
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
pub trait Handler: Send {
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
    /// service uses this default, which accepts only task-reference IPC V6.
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
    use super::*;
    #[test]
    fn client_never_decodes_legacy_or_wrong_version_replies() {
        for bytes in [
            br#"{"kind":"unavailable"}"#.as_slice(),
            br#"{"version":5,"reply":{"kind":"unavailable"}}"#,
            br#"{"version":6,"reply":{"kind":"unavailable"},"authority":"fake"}"#,
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
