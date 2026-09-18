// @generated from packages/ai-contract/schema/runtime.schema.json. Do not edit.
#[doc = "`Binding`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    #[serde(rename = "accountRef")]
    pub account_ref: Id,
    #[serde(rename = "adapterVersion")]
    pub adapter_version: Id,
    pub config: ConfigRef,
    pub generation: Id,
    #[serde(
        rename = "nativeRequestId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_request_id: ::std::option::Option<Id>,
    #[serde(
        rename = "nativeRunId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_run_id: ::std::option::Option<Id>,
    #[serde(rename = "nativeSessionId")]
    pub native_session_id: Id,
    pub provider: Id,
    #[serde(rename = "providerVersion")]
    pub provider_version: Id,
}
#[doc = "`Capabilities`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub cancellation: CapabilitiesCancellation,
    pub continuation: CapabilitiesContinuation,
    pub fork: CapabilityState,
    pub multimodal: CapabilityState,
    pub steer: CapabilityState,
    #[serde(rename = "structuredQuestion")]
    pub structured_question: CapabilityState,
    pub subagent: CapabilityState,
    pub terminal: CapabilityState,
    pub tools: CapabilitiesTools,
}
#[doc = "`CapabilitiesCancellation`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum CapabilitiesCancellation {
    #[serde(rename = "request_only")]
    RequestOnly,
    #[serde(rename = "terminal_acknowledged")]
    TerminalAcknowledged,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::fmt::Display for CapabilitiesCancellation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::RequestOnly => f.write_str("request_only"),
            Self::TerminalAcknowledged => f.write_str("terminal_acknowledged"),
            Self::Unsupported => f.write_str("unsupported"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for CapabilitiesCancellation {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "request_only" => Ok(Self::RequestOnly),
            "terminal_acknowledged" => Ok(Self::TerminalAcknowledged),
            "unsupported" => Ok(Self::Unsupported),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CapabilitiesCancellation {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CapabilitiesCancellation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`CapabilitiesContinuation`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum CapabilitiesContinuation {
    #[serde(rename = "same_process")]
    SameProcess,
    #[serde(rename = "across_processes")]
    AcrossProcesses,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::fmt::Display for CapabilitiesContinuation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SameProcess => f.write_str("same_process"),
            Self::AcrossProcesses => f.write_str("across_processes"),
            Self::Unsupported => f.write_str("unsupported"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for CapabilitiesContinuation {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "same_process" => Ok(Self::SameProcess),
            "across_processes" => Ok(Self::AcrossProcesses),
            "unsupported" => Ok(Self::Unsupported),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CapabilitiesContinuation {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CapabilitiesContinuation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`CapabilitiesTools`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum CapabilitiesTools {
    #[serde(rename = "host_mediated")]
    HostMediated,
    #[serde(rename = "provider_managed")]
    ProviderManaged,
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::fmt::Display for CapabilitiesTools {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::HostMediated => f.write_str("host_mediated"),
            Self::ProviderManaged => f.write_str("provider_managed"),
            Self::Disabled => f.write_str("disabled"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for CapabilitiesTools {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "host_mediated" => Ok(Self::HostMediated),
            "provider_managed" => Ok(Self::ProviderManaged),
            "disabled" => Ok(Self::Disabled),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CapabilitiesTools {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CapabilitiesTools {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`CapabilityState`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum CapabilityState {
    #[serde(rename = "supported")]
    Supported,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::fmt::Display for CapabilityState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Supported => f.write_str("supported"),
            Self::Unsupported => f.write_str("unsupported"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for CapabilityState {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "supported" => Ok(Self::Supported),
            "unsupported" => Ok(Self::Unsupported),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CapabilityState {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CapabilityState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Command`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Command {
    #[serde(rename = "commandId")]
    pub command_id: Id,
    #[serde(rename = "expiresAtMs")]
    pub expires_at_ms: Counter,
    pub input: Input,
    pub kind: ::std::string::String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[serde(rename = "sessionId")]
    pub session_id: Id,
}
#[doc = "`CommandRecord`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CommandRecord {
    pub command: Command,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub dispatch: ::std::option::Option<Dispatch>,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub failure: ::std::option::Option<Failure>,
    pub kind: ::std::string::String,
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub outcome: ::std::option::Option<Outcome>,
    pub receipt: Receipt,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    pub state: CommandState,
}
#[doc = "`CommandState`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum CommandState {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "dispatching")]
    Dispatching,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "terminal")]
    Terminal,
    #[serde(rename = "reconciliation_required")]
    ReconciliationRequired,
}
impl ::std::fmt::Display for CommandState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Accepted => f.write_str("accepted"),
            Self::Dispatching => f.write_str("dispatching"),
            Self::Running => f.write_str("running"),
            Self::Terminal => f.write_str("terminal"),
            Self::ReconciliationRequired => f.write_str("reconciliation_required"),
        }
    }
}
impl ::std::str::FromStr for CommandState {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "accepted" => Ok(Self::Accepted),
            "dispatching" => Ok(Self::Dispatching),
            "running" => Ok(Self::Running),
            "terminal" => Ok(Self::Terminal),
            "reconciliation_required" => Ok(Self::ReconciliationRequired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandState {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ConfigRef`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ConfigRef {
    pub id: Id,
    pub revision: Id,
}
#[doc = "`Counter`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct Counter(pub i64);
impl ::std::ops::Deref for Counter {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<Counter> for i64 {
    fn from(value: Counter) -> Self {
        value.0
    }
}
impl ::std::convert::From<i64> for Counter {
    fn from(value: i64) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Counter {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Counter {
    type Err = <i64 as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for Counter {
    type Error = <i64 as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for Counter {
    type Error = <i64 as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
#[doc = "`Delivery`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub attempts: Counter,
    #[serde(rename = "contentHash")]
    pub content_hash: DeliveryContentHash,
    #[serde(rename = "eventId")]
    pub event_id: Id,
    pub kind: ::std::string::String,
    pub namespace: Namespace,
    #[serde(rename = "nextAttemptAtMs")]
    pub next_attempt_at_ms: Counter,
    #[serde(rename = "operationId")]
    pub operation_id: Id,
    pub retry: DeliveryRetry,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    pub status: DeliveryStatus,
    pub target: Id,
}
#[doc = "`DeliveryContentHash`"]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DeliveryContentHash(::std::string::String);
impl ::std::ops::Deref for DeliveryContentHash {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<DeliveryContentHash> for ::std::string::String {
    fn from(value: DeliveryContentHash) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for DeliveryContentHash {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[a-f0-9]{64}$").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-f0-9]{64}$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for DeliveryContentHash {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DeliveryContentHash {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for DeliveryContentHash {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`DeliveryRetry`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DeliveryRetry {
    #[serde(rename = "receiver_idempotent")]
    ReceiverIdempotent,
    #[serde(rename = "reconcile_first")]
    ReconcileFirst,
    #[serde(rename = "never")]
    Never,
}
impl ::std::fmt::Display for DeliveryRetry {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ReceiverIdempotent => f.write_str("receiver_idempotent"),
            Self::ReconcileFirst => f.write_str("reconcile_first"),
            Self::Never => f.write_str("never"),
        }
    }
}
impl ::std::str::FromStr for DeliveryRetry {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "receiver_idempotent" => Ok(Self::ReceiverIdempotent),
            "reconcile_first" => Ok(Self::ReconcileFirst),
            "never" => Ok(Self::Never),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DeliveryRetry {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DeliveryRetry {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`DeliveryStatus`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DeliveryStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "delivered")]
    Delivered,
    #[serde(rename = "reconciliation_required")]
    ReconciliationRequired,
}
impl ::std::fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("pending"),
            Self::Delivered => f.write_str("delivered"),
            Self::ReconciliationRequired => f.write_str("reconciliation_required"),
        }
    }
}
impl ::std::str::FromStr for DeliveryStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pending" => Ok(Self::Pending),
            "delivered" => Ok(Self::Delivered),
            "reconciliation_required" => Ok(Self::ReconciliationRequired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DeliveryStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DeliveryStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Dispatch`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Dispatch {
    pub certainty: DispatchCertainty,
    pub generation: Id,
    #[serde(
        rename = "nativeRequestId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_request_id: ::std::option::Option<Id>,
    #[serde(
        rename = "nativeRunId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_run_id: ::std::option::Option<Id>,
    #[serde(rename = "nativeSessionId")]
    pub native_session_id: Id,
}
#[doc = "`DispatchCertainty`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum DispatchCertainty {
    #[serde(rename = "not_sent")]
    NotSent,
    #[serde(rename = "submitted")]
    Submitted,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::fmt::Display for DispatchCertainty {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::NotSent => f.write_str("not_sent"),
            Self::Submitted => f.write_str("submitted"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for DispatchCertainty {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "not_sent" => Ok(Self::NotSent),
            "submitted" => Ok(Self::Submitted),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DispatchCertainty {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DispatchCertainty {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ErrorCode`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ErrorCode {
    #[serde(rename = "invalid_input")]
    InvalidInput,
    #[serde(rename = "unsupported_version")]
    UnsupportedVersion,
    #[serde(rename = "unsupported_capability")]
    UnsupportedCapability,
    #[serde(rename = "permission_denied")]
    PermissionDenied,
    #[serde(rename = "content_conflict")]
    ContentConflict,
    #[serde(rename = "revision_conflict")]
    RevisionConflict,
    #[serde(rename = "stale_binding")]
    StaleBinding,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "reconciliation_required")]
    ReconciliationRequired,
    #[serde(rename = "limit_exceeded")]
    LimitExceeded,
    #[serde(rename = "cursor_expired")]
    CursorExpired,
    #[serde(rename = "session_gone")]
    SessionGone,
    #[serde(rename = "already_answered")]
    AlreadyAnswered,
}
impl ::std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::InvalidInput => f.write_str("invalid_input"),
            Self::UnsupportedVersion => f.write_str("unsupported_version"),
            Self::UnsupportedCapability => f.write_str("unsupported_capability"),
            Self::PermissionDenied => f.write_str("permission_denied"),
            Self::ContentConflict => f.write_str("content_conflict"),
            Self::RevisionConflict => f.write_str("revision_conflict"),
            Self::StaleBinding => f.write_str("stale_binding"),
            Self::Expired => f.write_str("expired"),
            Self::Unavailable => f.write_str("unavailable"),
            Self::ReconciliationRequired => f.write_str("reconciliation_required"),
            Self::LimitExceeded => f.write_str("limit_exceeded"),
            Self::CursorExpired => f.write_str("cursor_expired"),
            Self::SessionGone => f.write_str("session_gone"),
            Self::AlreadyAnswered => f.write_str("already_answered"),
        }
    }
}
impl ::std::str::FromStr for ErrorCode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "invalid_input" => Ok(Self::InvalidInput),
            "unsupported_version" => Ok(Self::UnsupportedVersion),
            "unsupported_capability" => Ok(Self::UnsupportedCapability),
            "permission_denied" => Ok(Self::PermissionDenied),
            "content_conflict" => Ok(Self::ContentConflict),
            "revision_conflict" => Ok(Self::RevisionConflict),
            "stale_binding" => Ok(Self::StaleBinding),
            "expired" => Ok(Self::Expired),
            "unavailable" => Ok(Self::Unavailable),
            "reconciliation_required" => Ok(Self::ReconciliationRequired),
            "limit_exceeded" => Ok(Self::LimitExceeded),
            "cursor_expired" => Ok(Self::CursorExpired),
            "session_gone" => Ok(Self::SessionGone),
            "already_answered" => Ok(Self::AlreadyAnswered),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ErrorCode {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ErrorCode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Event`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub body: EventBody,
    #[serde(rename = "commandId")]
    pub command_id: Id,
    #[serde(rename = "eventId")]
    pub event_id: Id,
    pub generation: Id,
    pub kind: ::std::string::String,
    pub namespace: Namespace,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    pub sequence: Counter,
}
#[doc = "`EventBody`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum EventBody {
    #[serde(rename = "text")]
    Text {
        #[serde(rename = "messageId")]
        message_id: Id,
        text: EventBodyText,
    },
    #[serde(rename = "status")]
    Status { state: CommandState },
    #[serde(rename = "terminal")]
    Terminal { outcome: Outcome },
    #[serde(rename = "cancel_dispatched")]
    CancelDispatched { confirmation: EventBodyConfirmation },
    #[serde(rename = "tool_proposal")]
    ToolProposal {
        arguments: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        name: Id,
        #[serde(rename = "proposalId")]
        proposal_id: Id,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        disposition: EventBodyDisposition,
        #[serde(rename = "proposalId")]
        proposal_id: Id,
        text: EventBodyText,
    },
    #[serde(rename = "interaction")]
    Interaction {
        #[serde(rename = "interactionId")]
        interaction_id: Id,
        status: EventBodyStatus,
    },
    #[serde(rename = "error")]
    Error { failure: Failure },
}
#[doc = "`EventBodyConfirmation`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum EventBodyConfirmation {
    #[serde(rename = "request_only")]
    RequestOnly,
    #[serde(rename = "already_terminal")]
    AlreadyTerminal,
    #[serde(rename = "unsupported")]
    Unsupported,
}
impl ::std::fmt::Display for EventBodyConfirmation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::RequestOnly => f.write_str("request_only"),
            Self::AlreadyTerminal => f.write_str("already_terminal"),
            Self::Unsupported => f.write_str("unsupported"),
        }
    }
}
impl ::std::str::FromStr for EventBodyConfirmation {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "request_only" => Ok(Self::RequestOnly),
            "already_terminal" => Ok(Self::AlreadyTerminal),
            "unsupported" => Ok(Self::Unsupported),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventBodyConfirmation {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventBodyConfirmation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`EventBodyDisposition`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum EventBodyDisposition {
    #[serde(rename = "returned")]
    Returned,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "unavailable")]
    Unavailable,
}
impl ::std::fmt::Display for EventBodyDisposition {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Returned => f.write_str("returned"),
            Self::Rejected => f.write_str("rejected"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}
impl ::std::str::FromStr for EventBodyDisposition {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "returned" => Ok(Self::Returned),
            "rejected" => Ok(Self::Rejected),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventBodyDisposition {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventBodyDisposition {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`EventBodyStatus`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum EventBodyStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "answered")]
    Answered,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "unavailable")]
    Unavailable,
}
impl ::std::fmt::Display for EventBodyStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("pending"),
            Self::Answered => f.write_str("answered"),
            Self::Expired => f.write_str("expired"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}
impl ::std::str::FromStr for EventBodyStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pending" => Ok(Self::Pending),
            "answered" => Ok(Self::Answered),
            "expired" => Ok(Self::Expired),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventBodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventBodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`EventBodyText`"]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct EventBodyText(::std::string::String);
impl ::std::ops::Deref for EventBodyText {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<EventBodyText> for ::std::string::String {
    fn from(value: EventBodyText) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for EventBodyText {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() > 65536usize {
            return Err("longer than 65536 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for EventBodyText {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventBodyText {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for EventBodyText {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`Failure`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub code: ErrorCode,
    pub retry: Retry,
}
#[doc = "`Id`"]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct Id(::std::string::String);
impl ::std::ops::Deref for Id {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Id> for ::std::string::String {
    fn from(value: Id) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for Id {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() > 128usize {
            return Err("longer than 128 characters".into());
        }
        if value.chars().count() < 1usize {
            return Err("shorter than 1 characters".into());
        }
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| {
                ::regress::Regex::new("^[A-Za-z0-9][A-Za-z0-9._:/+-]*$").unwrap()
            });
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^[A-Za-z0-9][A-Za-z0-9._:/+-]*$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for Id {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Id {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`Input`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Input {
    #[serde(rename = "prompt")]
    Prompt {
        policy: InputPolicy,
        #[serde(
            rename = "targetRunId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        target_run_id: ::std::option::Option<Id>,
        text: InputText,
    },
    #[serde(rename = "cancel")]
    Cancel {
        generation: Id,
        #[serde(
            rename = "nativeRunId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        native_run_id: ::std::option::Option<Id>,
        #[serde(rename = "targetCommandId")]
        target_command_id: Id,
    },
    #[serde(rename = "respond")]
    Respond {
        answer: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        generation: Id,
        #[serde(rename = "interactionId")]
        interaction_id: Id,
        #[serde(
            rename = "nativeRunId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        native_run_id: ::std::option::Option<Id>,
    },
}
#[doc = "`InputPolicy`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum InputPolicy {
    #[serde(rename = "queue_next")]
    QueueNext,
    #[serde(rename = "steer")]
    Steer,
}
impl ::std::fmt::Display for InputPolicy {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::QueueNext => f.write_str("queue_next"),
            Self::Steer => f.write_str("steer"),
        }
    }
}
impl ::std::str::FromStr for InputPolicy {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "queue_next" => Ok(Self::QueueNext),
            "steer" => Ok(Self::Steer),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for InputPolicy {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InputPolicy {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`InputText`"]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct InputText(::std::string::String);
impl ::std::ops::Deref for InputText {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<InputText> for ::std::string::String {
    fn from(value: InputText) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for InputText {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() > 65536usize {
            return Err("longer than 65536 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for InputText {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InputText {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for InputText {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`Interaction`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    #[serde(rename = "callbackLifetime")]
    pub callback_lifetime: InteractionCallbackLifetime,
    #[serde(rename = "commandId")]
    pub command_id: Id,
    #[serde(rename = "expiresAtMs")]
    pub expires_at_ms: Counter,
    pub generation: Id,
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    pub kind: ::std::string::String,
    pub namespace: Namespace,
    #[serde(rename = "nativeRequestId")]
    pub native_request_id: Id,
    #[serde(
        rename = "nativeRunId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_run_id: ::std::option::Option<Id>,
    #[serde(
        rename = "responseCommandId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub response_command_id: ::std::option::Option<Id>,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    pub status: InteractionStatus,
}
#[doc = "`InteractionCallbackLifetime`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum InteractionCallbackLifetime {
    #[serde(rename = "generation_bound")]
    GenerationBound,
    #[serde(rename = "provider_resumable")]
    ProviderResumable,
}
impl ::std::fmt::Display for InteractionCallbackLifetime {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::GenerationBound => f.write_str("generation_bound"),
            Self::ProviderResumable => f.write_str("provider_resumable"),
        }
    }
}
impl ::std::str::FromStr for InteractionCallbackLifetime {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "generation_bound" => Ok(Self::GenerationBound),
            "provider_resumable" => Ok(Self::ProviderResumable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for InteractionCallbackLifetime {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InteractionCallbackLifetime {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`InteractionStatus`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum InteractionStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "answered")]
    Answered,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "unavailable")]
    Unavailable,
}
impl ::std::fmt::Display for InteractionStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("pending"),
            Self::Answered => f.write_str("answered"),
            Self::Expired => f.write_str("expired"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}
impl ::std::str::FromStr for InteractionStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pending" => Ok(Self::Pending),
            "answered" => Ok(Self::Answered),
            "expired" => Ok(Self::Expired),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for InteractionStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InteractionStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Namespace`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Namespace {
    #[serde(rename = "authorityId")]
    pub authority_id: Id,
    #[serde(rename = "principalId")]
    pub principal_id: Id,
    #[serde(rename = "sessionId")]
    pub session_id: Id,
    #[serde(rename = "tenantId")]
    pub tenant_id: Id,
}
#[doc = "`Outcome`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum Outcome {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "interrupted")]
    Interrupted,
    #[serde(rename = "refused")]
    Refused,
    #[serde(rename = "limit_reached")]
    LimitReached,
    #[serde(rename = "failed")]
    Failed,
}
impl ::std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Completed => f.write_str("completed"),
            Self::Interrupted => f.write_str("interrupted"),
            Self::Refused => f.write_str("refused"),
            Self::LimitReached => f.write_str("limit_reached"),
            Self::Failed => f.write_str("failed"),
        }
    }
}
impl ::std::str::FromStr for Outcome {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "completed" => Ok(Self::Completed),
            "interrupted" => Ok(Self::Interrupted),
            "refused" => Ok(Self::Refused),
            "limit_reached" => Ok(Self::LimitReached),
            "failed" => Ok(Self::Failed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Outcome {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Outcome {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Receipt`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    #[serde(rename = "acceptedAtMs")]
    pub accepted_at_ms: Counter,
    #[serde(rename = "acceptedRevision")]
    pub accepted_revision: Counter,
    #[serde(rename = "commandId")]
    pub command_id: Id,
    #[serde(rename = "contentHash")]
    pub content_hash: ReceiptContentHash,
    pub kind: ::std::string::String,
    pub namespace: Namespace,
    #[serde(rename = "receiptUntilMs")]
    pub receipt_until_ms: Counter,
    #[serde(rename = "retryUntilMs")]
    pub retry_until_ms: Counter,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
}
#[doc = "`ReceiptContentHash`"]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ReceiptContentHash(::std::string::String);
impl ::std::ops::Deref for ReceiptContentHash {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<ReceiptContentHash> for ::std::string::String {
    fn from(value: ReceiptContentHash) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for ReceiptContentHash {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        static PATTERN: ::std::sync::LazyLock<::regress::Regex> =
            ::std::sync::LazyLock::new(|| ::regress::Regex::new("^[a-f0-9]{64}$").unwrap());
        if PATTERN.find(value).is_none() {
            return Err("doesn't match pattern \"^[a-f0-9]{64}$\"".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for ReceiptContentHash {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ReceiptContentHash {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for ReceiptContentHash {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        ::std::string::String::deserialize(deserializer)?
            .parse()
            .map_err(|e: self::error::ConversionError| {
                <D::Error as ::serde::de::Error>::custom(e.to_string())
            })
    }
}
#[doc = "`Retry`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum Retry {
    #[serde(rename = "same_command")]
    SameCommand,
    #[serde(rename = "reconcile_first")]
    ReconcileFirst,
    #[serde(rename = "never")]
    Never,
}
impl ::std::fmt::Display for Retry {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SameCommand => f.write_str("same_command"),
            Self::ReconcileFirst => f.write_str("reconcile_first"),
            Self::Never => f.write_str("never"),
        }
    }
}
impl ::std::str::FromStr for Retry {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "same_command" => Ok(Self::SameCommand),
            "reconcile_first" => Ok(Self::ReconcileFirst),
            "never" => Ok(Self::Never),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Retry {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Retry {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Session`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub binding: Binding,
    pub capabilities: Capabilities,
    pub kind: ::std::string::String,
    #[serde(rename = "lastSequence")]
    pub last_sequence: Counter,
    pub namespace: Namespace,
    pub revision: Counter,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    pub status: SessionStatus,
}
#[doc = "`SessionStatus`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum SessionStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "retired")]
    Retired,
}
impl ::std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Active => f.write_str("active"),
            Self::Retired => f.write_str("retired"),
        }
    }
}
impl ::std::str::FromStr for SessionStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "active" => Ok(Self::Active),
            "retired" => Ok(Self::Retired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SessionStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SessionStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SurfaceAction`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SurfaceAction {
    #[serde(rename = "commandId")]
    pub command_id: Id,
    pub generation: Id,
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    pub kind: ::std::string::String,
    #[serde(rename = "nativeRunId")]
    pub native_run_id: Id,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[serde(rename = "sessionId")]
    pub session_id: Id,
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
    #[serde(rename = "surfaceRevision")]
    pub surface_revision: Counter,
}
#[doc = "`SurfaceBinding`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SurfaceBinding {
    #[serde(rename = "a2uiVersion")]
    pub a2ui_version: ::std::string::String,
    #[serde(rename = "catalogId")]
    pub catalog_id: Id,
    #[serde(rename = "catalogVersion")]
    pub catalog_version: Id,
    #[serde(rename = "eventName")]
    pub event_name: Id,
    pub generation: Id,
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    pub kind: ::std::string::String,
    pub namespace: Namespace,
    #[serde(rename = "nativeRunId")]
    pub native_run_id: Id,
    pub revision: Counter,
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[serde(rename = "sourceComponentId")]
    pub source_component_id: Id,
    #[serde(rename = "surfaceId")]
    pub surface_id: Id,
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
}
#[doc = "Product reliability records only. No record authenticates a caller, grants approval or proves business execution. Standard ACP/A2UI schemas retain their upstream owners."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(untagged)]
pub enum WireRecord {
    Command(Command),
    Receipt(Receipt),
    CommandRecord(CommandRecord),
    Event(Event),
    Session(Session),
    Interaction(Interaction),
    Delivery(Delivery),
    SurfaceBinding(SurfaceBinding),
    SurfaceAction(SurfaceAction),
}
impl ::std::convert::From<Command> for WireRecord {
    fn from(value: Command) -> Self {
        Self::Command(value)
    }
}
impl ::std::convert::From<Receipt> for WireRecord {
    fn from(value: Receipt) -> Self {
        Self::Receipt(value)
    }
}
impl ::std::convert::From<CommandRecord> for WireRecord {
    fn from(value: CommandRecord) -> Self {
        Self::CommandRecord(value)
    }
}
impl ::std::convert::From<Event> for WireRecord {
    fn from(value: Event) -> Self {
        Self::Event(value)
    }
}
impl ::std::convert::From<Session> for WireRecord {
    fn from(value: Session) -> Self {
        Self::Session(value)
    }
}
impl ::std::convert::From<Interaction> for WireRecord {
    fn from(value: Interaction) -> Self {
        Self::Interaction(value)
    }
}
impl ::std::convert::From<Delivery> for WireRecord {
    fn from(value: Delivery) -> Self {
        Self::Delivery(value)
    }
}
impl ::std::convert::From<SurfaceBinding> for WireRecord {
    fn from(value: SurfaceBinding) -> Self {
        Self::SurfaceBinding(value)
    }
}
impl ::std::convert::From<SurfaceAction> for WireRecord {
    fn from(value: SurfaceAction) -> Self {
        Self::SurfaceAction(value)
    }
}
#[doc = " Error types."]
pub mod error {
    #[doc = r" Error from a `TryFrom` or `FromStr` implementation."]
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Binding), "([redacted])"))
    }
}
impl std::fmt::Debug for Capabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Capabilities), "([redacted])"))
    }
}
impl std::fmt::Debug for CapabilitiesCancellation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CapabilitiesCancellation),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CapabilitiesContinuation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CapabilitiesContinuation),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CapabilitiesTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(CapabilitiesTools), "([redacted])"))
    }
}
impl std::fmt::Debug for CapabilityState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(CapabilityState), "([redacted])"))
    }
}
impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Command), "([redacted])"))
    }
}
impl std::fmt::Debug for CommandRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(CommandRecord), "([redacted])"))
    }
}
impl std::fmt::Debug for CommandState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(CommandState), "([redacted])"))
    }
}
impl std::fmt::Debug for ConfigRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ConfigRef), "([redacted])"))
    }
}
impl std::fmt::Debug for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Counter), "([redacted])"))
    }
}
impl std::fmt::Debug for Delivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Delivery), "([redacted])"))
    }
}
impl std::fmt::Debug for DeliveryContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DeliveryContentHash), "([redacted])"))
    }
}
impl std::fmt::Debug for DeliveryRetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DeliveryRetry), "([redacted])"))
    }
}
impl std::fmt::Debug for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DeliveryStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for Dispatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Dispatch), "([redacted])"))
    }
}
impl std::fmt::Debug for DispatchCertainty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DispatchCertainty), "([redacted])"))
    }
}
impl std::fmt::Debug for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ErrorCode), "([redacted])"))
    }
}
impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Event), "([redacted])"))
    }
}
impl std::fmt::Debug for EventBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventBodyConfirmation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventBodyConfirmation), "([redacted])"))
    }
}
impl std::fmt::Debug for EventBodyDisposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventBodyDisposition), "([redacted])"))
    }
}
impl std::fmt::Debug for EventBodyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventBodyStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for EventBodyText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventBodyText), "([redacted])"))
    }
}
impl std::fmt::Debug for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Failure), "([redacted])"))
    }
}
impl std::fmt::Debug for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Id), "([redacted])"))
    }
}
impl std::fmt::Debug for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Input), "([redacted])"))
    }
}
impl std::fmt::Debug for InputPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(InputPolicy), "([redacted])"))
    }
}
impl std::fmt::Debug for InputText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(InputText), "([redacted])"))
    }
}
impl std::fmt::Debug for Interaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Interaction), "([redacted])"))
    }
}
impl std::fmt::Debug for InteractionCallbackLifetime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(InteractionCallbackLifetime),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for InteractionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(InteractionStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for Namespace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Namespace), "([redacted])"))
    }
}
impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Outcome), "([redacted])"))
    }
}
impl std::fmt::Debug for Receipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Receipt), "([redacted])"))
    }
}
impl std::fmt::Debug for ReceiptContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ReceiptContentHash), "([redacted])"))
    }
}
impl std::fmt::Debug for Retry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Retry), "([redacted])"))
    }
}
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Session), "([redacted])"))
    }
}
impl std::fmt::Debug for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SessionStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceAction), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceBinding), "([redacted])"))
    }
}
impl std::fmt::Debug for WireRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(WireRecord), "([redacted])"))
    }
}
