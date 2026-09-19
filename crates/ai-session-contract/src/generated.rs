// @generated from packages/ai-contract/schema/runtime.schema.json. Do not edit.
#[doc = "Provider context identity. Version, configuration, account and generation bind every capability and callback."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    #[doc = "Opaque account reference; no token, key or account-directory contents."]
    #[serde(rename = "accountRef")]
    pub account_ref: Id,
    #[doc = "Adapter implementation version used for capability verification."]
    #[serde(rename = "adapterVersion")]
    pub adapter_version: Id,
    #[doc = "Exact immutable configuration identity and revision."]
    pub config: ConfigRef,
    #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
    pub generation: Id,
    #[doc = "Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction."]
    #[serde(
        rename = "nativeRequestId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_request_id: ::std::option::Option<Id>,
    #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
    #[serde(
        rename = "nativeRunId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_run_id: ::std::option::Option<Id>,
    #[doc = "Provider-owned context session identifier; history alone cannot recreate it."]
    #[serde(rename = "nativeSessionId")]
    pub native_session_id: Id,
    #[doc = "Provider adapter identity."]
    pub provider: Id,
    #[doc = "Pinned native provider implementation version."]
    #[serde(rename = "providerVersion")]
    pub provider_version: Id,
}
#[doc = "Capabilities established for one exact provider binding, never execution authorization."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    #[doc = "request_only confirms sending; terminal_acknowledged requires native terminal evidence; unsupported/unknown cannot promise cancellation."]
    pub cancellation: CapabilitiesCancellation,
    #[doc = "same_process resumes only a live context; across_processes requires verified provider restoration; unsupported/unknown cannot resume."]
    pub continuation: CapabilitiesContinuation,
    #[doc = "Whether the provider supports an explicit context fork."]
    pub fork: CapabilityState,
    #[doc = "Whether provider-specific multimodal input is available through an adapter extension."]
    pub multimodal: CapabilityState,
    #[doc = "Whether an active native run accepts targeted steering."]
    pub steer: CapabilityState,
    #[doc = "Whether a native structured callback can be represented and answered."]
    #[serde(rename = "structuredQuestion")]
    pub structured_question: CapabilityState,
    #[doc = "Whether the provider supports child agents; not execution authorization."]
    pub subagent: CapabilityState,
    #[doc = "Whether the provider exposes a terminal facility; not the command terminal state."]
    pub terminal: CapabilityState,
    #[doc = "host_mediated still requires containment evidence; provider_managed is not controlled execution; disabled/unknown cannot enable tools."]
    pub tools: CapabilitiesTools,
}
#[doc = "request_only confirms sending; terminal_acknowledged requires native terminal evidence; unsupported/unknown cannot promise cancellation."]
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
    #[doc = "`RequestOnly` alternative; see the parent type's schema contract."]
    RequestOnly,
    #[serde(rename = "terminal_acknowledged")]
    #[doc = "`TerminalAcknowledged` alternative; see the parent type's schema contract."]
    TerminalAcknowledged,
    #[serde(rename = "unsupported")]
    #[doc = "`Unsupported` alternative; see the parent type's schema contract."]
    Unsupported,
    #[serde(rename = "unknown")]
    #[doc = "`Unknown` alternative; see the parent type's schema contract."]
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
#[doc = "same_process resumes only a live context; across_processes requires verified provider restoration; unsupported/unknown cannot resume."]
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
    #[doc = "`SameProcess` alternative; see the parent type's schema contract."]
    SameProcess,
    #[serde(rename = "across_processes")]
    #[doc = "`AcrossProcesses` alternative; see the parent type's schema contract."]
    AcrossProcesses,
    #[serde(rename = "unsupported")]
    #[doc = "`Unsupported` alternative; see the parent type's schema contract."]
    Unsupported,
    #[serde(rename = "unknown")]
    #[doc = "`Unknown` alternative; see the parent type's schema contract."]
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
#[doc = "host_mediated still requires containment evidence; provider_managed is not controlled execution; disabled/unknown cannot enable tools."]
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
    #[doc = "`HostMediated` alternative; see the parent type's schema contract."]
    HostMediated,
    #[serde(rename = "provider_managed")]
    #[doc = "`ProviderManaged` alternative; see the parent type's schema contract."]
    ProviderManaged,
    #[serde(rename = "disabled")]
    #[doc = "`Disabled` alternative; see the parent type's schema contract."]
    Disabled,
    #[serde(rename = "unknown")]
    #[doc = "`Unknown` alternative; see the parent type's schema contract."]
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
#[doc = "Only supported enables an operation; unknown and unsupported fail closed."]
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
    #[doc = "`Supported` alternative; see the parent type's schema contract."]
    Supported,
    #[serde(rename = "unsupported")]
    #[doc = "`Unsupported` alternative; see the parent type's schema contract."]
    Unsupported,
    #[serde(rename = "unknown")]
    #[doc = "`Unknown` alternative; see the parent type's schema contract."]
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
#[doc = "Client command identity and complete canonical input; trusted namespace is supplied separately."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Command {
    #[doc = "Client-generated idempotency key; reuse only with identical canonical content."]
    #[serde(rename = "commandId")]
    pub command_id: Id,
    #[doc = "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected."]
    #[serde(rename = "expiresAtMs")]
    pub expires_at_ms: Counter,
    #[doc = "Complete command content included in its canonical fingerprint."]
    pub input: Input,
    #[doc = "Closed product record discriminator."]
    pub kind: ::std::string::String,
    #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[doc = "Logical session identifier, never reusable after retirement."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
}
#[doc = "Closed command lifecycle; acceptance is immutable, local invalidation does not assert a model terminal."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(tag = "state", deny_unknown_fields)]
pub enum CommandRecord {
    #[serde(rename = "accepted")]
    #[doc = "`Accepted` alternative; see the parent type's schema contract."]
    Accepted {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
    },
    #[serde(rename = "dispatching")]
    #[doc = "`Dispatching` alternative; see the parent type's schema contract."]
    Dispatching {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
    },
    #[serde(rename = "running")]
    #[doc = "`Running` alternative; see the parent type's schema contract."]
    Running {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
    },
    #[serde(rename = "terminal")]
    #[doc = "`Terminal` alternative; see the parent type's schema contract."]
    Terminal {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Explicitly observed model terminal outcome."]
        outcome: Outcome,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
    },
    #[serde(rename = "reconciliation_required")]
    #[doc = "`ReconciliationRequired` alternative; see the parent type's schema contract."]
    ReconciliationRequired {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
    },
    #[serde(rename = "invalidated")]
    #[doc = "`Invalidated` alternative; see the parent type's schema contract."]
    Invalidated {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Local failure without asserting a model terminal."]
        failure: Failure,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
    },
}
#[doc = "accepted persists intent; dispatching persists dispatch intent; running has native confirmation; terminal has a definite outcome; reconciliation_required forbids blind resubmission."]
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
    #[doc = "`Accepted` alternative; see the parent type's schema contract."]
    Accepted,
    #[serde(rename = "dispatching")]
    #[doc = "`Dispatching` alternative; see the parent type's schema contract."]
    Dispatching,
    #[serde(rename = "running")]
    #[doc = "`Running` alternative; see the parent type's schema contract."]
    Running,
    #[serde(rename = "terminal")]
    #[doc = "`Terminal` alternative; see the parent type's schema contract."]
    Terminal,
    #[serde(rename = "reconciliation_required")]
    #[doc = "`ReconciliationRequired` alternative; see the parent type's schema contract."]
    ReconciliationRequired,
    #[serde(rename = "invalidated")]
    #[doc = "`Invalidated` alternative; see the parent type's schema contract."]
    Invalidated,
}
impl ::std::fmt::Display for CommandState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Accepted => f.write_str("accepted"),
            Self::Dispatching => f.write_str("dispatching"),
            Self::Running => f.write_str("running"),
            Self::Terminal => f.write_str("terminal"),
            Self::ReconciliationRequired => f.write_str("reconciliation_required"),
            Self::Invalidated => f.write_str("invalidated"),
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
            "invalidated" => Ok(Self::Invalidated),
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
#[doc = "Immutable configuration identity and revision; contains no credentials."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ConfigRef {
    #[doc = "Configuration identifier; resolve credentials outside the wire."]
    pub id: Id,
    #[doc = "Immutable configuration revision; changing it invalidates prior capability evidence."]
    pub revision: Id,
}
#[doc = "Nonnegative integer in the shared JavaScript safe-integer range."]
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
#[doc = "Reliable cross-service outbox record bound to an immutable event and target."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    #[doc = "Monotonic count of delivery attempts."]
    pub attempts: Counter,
    #[doc = "SHA-256 of JCS({event, target}), including the full referenced stable event and exact destination."]
    #[serde(rename = "contentHash")]
    pub content_hash: DeliveryContentHash,
    #[doc = "Stable unique event identifier within the namespace."]
    #[serde(rename = "eventId")]
    pub event_id: Id,
    #[doc = "Closed product record discriminator."]
    pub kind: ::std::string::String,
    #[doc = "Trusted storage isolation scope; not copied from model or action content."]
    pub namespace: Namespace,
    #[doc = "UTC epoch-millisecond earliest eligible retry time."]
    #[serde(rename = "nextAttemptAtMs")]
    pub next_attempt_at_ms: Counter,
    #[doc = "Stable receiver idempotency key for this delivery; cannot be rebound to another event/target."]
    #[serde(rename = "operationId")]
    pub operation_id: Id,
    #[doc = "Explicit retry discipline; uncertainty never authorizes blind resubmission."]
    pub retry: DeliveryRetry,
    #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
    pub status: DeliveryStatus,
    #[doc = "Opaque reliable-delivery destination identifier."]
    pub target: Id,
}
#[doc = "SHA-256 of JCS({event, target}), including the full referenced stable event and exact destination."]
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
#[doc = "Explicit retry discipline; uncertainty never authorizes blind resubmission."]
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
    #[doc = "`ReceiverIdempotent` alternative; see the parent type's schema contract."]
    ReceiverIdempotent,
    #[serde(rename = "reconcile_first")]
    #[doc = "`ReconcileFirst` alternative; see the parent type's schema contract."]
    ReconcileFirst,
    #[serde(rename = "never")]
    #[doc = "`Never` alternative; see the parent type's schema contract."]
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
#[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
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
    #[doc = "`Pending` alternative; see the parent type's schema contract."]
    Pending,
    #[serde(rename = "delivered")]
    #[doc = "`Delivered` alternative; see the parent type's schema contract."]
    Delivered,
    #[serde(rename = "reconciliation_required")]
    #[doc = "`ReconciliationRequired` alternative; see the parent type's schema contract."]
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
#[doc = "One active dispatch attempt. Origin identity is immutable; unknown native coordinates may be filled once. Only verified rebind changes observerGeneration."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct DispatchAttempt {
    #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
    #[serde(rename = "attemptId")]
    pub attempt_id: Id,
    #[doc = "intent is stored before native submission; submitted has native acceptance; unknown requires reconciliation."]
    pub certainty: DispatchAttemptCertainty,
    #[doc = "Append-once native lookup key returned for ambiguous submission."]
    #[serde(
        rename = "correlationId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub correlation_id: ::std::option::Option<Id>,
    #[doc = "Provider-owned parent prompt/query request identifier; cannot be rebound after dispatch. Callback identities belong to Interaction."]
    #[serde(
        rename = "nativeRequestId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_request_id: ::std::option::Option<Id>,
    #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
    #[serde(
        rename = "nativeRunId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_run_id: ::std::option::Option<Id>,
    #[doc = "Provider-owned context session identifier; history alone cannot recreate it."]
    #[serde(rename = "nativeSessionId")]
    pub native_session_id: Id,
    #[doc = "Current verified provider incarnation permitted to observe this attempt."]
    #[serde(rename = "observerGeneration")]
    pub observer_generation: Id,
    #[doc = "Immutable provider incarnation that originated this attempt."]
    #[serde(rename = "originGeneration")]
    pub origin_generation: Id,
}
#[doc = "intent is stored before native submission; submitted has native acceptance; unknown requires reconciliation."]
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
pub enum DispatchAttemptCertainty {
    #[serde(rename = "intent")]
    #[doc = "`Intent` alternative; see the parent type's schema contract."]
    Intent,
    #[serde(rename = "submitted")]
    #[doc = "`Submitted` alternative; see the parent type's schema contract."]
    Submitted,
    #[serde(rename = "unknown")]
    #[doc = "`Unknown` alternative; see the parent type's schema contract."]
    Unknown,
}
impl ::std::fmt::Display for DispatchAttemptCertainty {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Intent => f.write_str("intent"),
            Self::Submitted => f.write_str("submitted"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for DispatchAttemptCertainty {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "intent" => Ok(Self::Intent),
            "submitted" => Ok(Self::Submitted),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DispatchAttemptCertainty {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DispatchAttemptCertainty {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed value-free error category; diagnostics never include model text or credentials."]
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
    #[doc = "`InvalidInput` alternative; see the parent type's schema contract."]
    InvalidInput,
    #[serde(rename = "unsupported_version")]
    #[doc = "`UnsupportedVersion` alternative; see the parent type's schema contract."]
    UnsupportedVersion,
    #[serde(rename = "unsupported_capability")]
    #[doc = "`UnsupportedCapability` alternative; see the parent type's schema contract."]
    UnsupportedCapability,
    #[serde(rename = "permission_denied")]
    #[doc = "`PermissionDenied` alternative; see the parent type's schema contract."]
    PermissionDenied,
    #[serde(rename = "content_conflict")]
    #[doc = "`ContentConflict` alternative; see the parent type's schema contract."]
    ContentConflict,
    #[serde(rename = "revision_conflict")]
    #[doc = "`RevisionConflict` alternative; see the parent type's schema contract."]
    RevisionConflict,
    #[serde(rename = "stale_binding")]
    #[doc = "`StaleBinding` alternative; see the parent type's schema contract."]
    StaleBinding,
    #[serde(rename = "expired")]
    #[doc = "`Expired` alternative; see the parent type's schema contract."]
    Expired,
    #[serde(rename = "unavailable")]
    #[doc = "`Unavailable` alternative; see the parent type's schema contract."]
    Unavailable,
    #[serde(rename = "reconciliation_required")]
    #[doc = "`ReconciliationRequired` alternative; see the parent type's schema contract."]
    ReconciliationRequired,
    #[serde(rename = "limit_exceeded")]
    #[doc = "`LimitExceeded` alternative; see the parent type's schema contract."]
    LimitExceeded,
    #[serde(rename = "cursor_expired")]
    #[doc = "`CursorExpired` alternative; see the parent type's schema contract."]
    CursorExpired,
    #[serde(rename = "session_gone")]
    #[doc = "`SessionGone` alternative; see the parent type's schema contract."]
    SessionGone,
    #[serde(rename = "already_answered")]
    #[doc = "`AlreadyAnswered` alternative; see the parent type's schema contract."]
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
#[doc = "Closed stable events. Session events have no command, attempt observations name their exact attempt."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(untagged, deny_unknown_fields)]
pub enum Event {
    #[doc = "`Variant0` alternative; see the parent type's schema contract."]
    Variant0 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant0Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant1` alternative; see the parent type's schema contract."]
    Variant1 {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant1Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant2` alternative; see the parent type's schema contract."]
    Variant2 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant2Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant3` alternative; see the parent type's schema contract."]
    Variant3 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant3Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant4` alternative; see the parent type's schema contract."]
    Variant4 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant4Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant5` alternative; see the parent type's schema contract."]
    Variant5 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant5Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant6` alternative; see the parent type's schema contract."]
    Variant6 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant6Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant7` alternative; see the parent type's schema contract."]
    Variant7 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant7Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant8` alternative; see the parent type's schema contract."]
    Variant8 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant8Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant9` alternative; see the parent type's schema contract."]
    Variant9 {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant9Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant10` alternative; see the parent type's schema contract."]
    Variant10 {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant10Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant11` alternative; see the parent type's schema contract."]
    Variant11 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant11Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant12` alternative; see the parent type's schema contract."]
    Variant12 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant12Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant13` alternative; see the parent type's schema contract."]
    Variant13 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant13Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant14` alternative; see the parent type's schema contract."]
    Variant14 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant14Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant15` alternative; see the parent type's schema contract."]
    Variant15 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant15Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant16` alternative; see the parent type's schema contract."]
    Variant16 {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant16Body,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant17` alternative; see the parent type's schema contract."]
    Variant17 {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant17Body,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Variant18` alternative; see the parent type's schema contract."]
    Variant18 {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventVariant18Body,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: ::std::string::String,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: i64,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
}
#[doc = "text variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant0Body {
    #[doc = "Stable product message correlation identifier."]
    #[serde(rename = "messageId")]
    pub message_id: Id,
    #[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
    pub text: EventVariant0BodyText,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct EventVariant0BodyText(::std::string::String);
impl ::std::ops::Deref for EventVariant0BodyText {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<EventVariant0BodyText> for ::std::string::String {
    fn from(value: EventVariant0BodyText) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for EventVariant0BodyText {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() > 65536usize {
            return Err("longer than 65536 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for EventVariant0BodyText {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventVariant0BodyText {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for EventVariant0BodyText {
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
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant10Body {
    #[doc = "Local failure without asserting a model terminal."]
    pub failure: Failure,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant11Body {
    #[doc = "Complete dispatch identity retained for replay and reconciliation."]
    pub attempt: DispatchAttempt,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant12Body {
    #[doc = "Complete dispatch identity retained for replay and reconciliation."]
    pub attempt: DispatchAttempt,
    #[doc = "Provider observation bound to this attempt and its current observer."]
    pub resolution: EventVariant12BodyResolution,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Provider observation bound to this attempt and its current observer."]
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
pub enum EventVariant12BodyResolution {
    #[serde(rename = "running")]
    #[doc = "`Running` alternative; see the parent type's schema contract."]
    Running,
    #[serde(rename = "terminal")]
    #[doc = "`Terminal` alternative; see the parent type's schema contract."]
    Terminal,
    #[serde(rename = "not_submitted")]
    #[doc = "`NotSubmitted` alternative; see the parent type's schema contract."]
    NotSubmitted,
    #[serde(rename = "unknown")]
    #[doc = "`Unknown` alternative; see the parent type's schema contract."]
    Unknown,
}
impl ::std::fmt::Display for EventVariant12BodyResolution {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Running => f.write_str("running"),
            Self::Terminal => f.write_str("terminal"),
            Self::NotSubmitted => f.write_str("not_submitted"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for EventVariant12BodyResolution {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "running" => Ok(Self::Running),
            "terminal" => Ok(Self::Terminal),
            "not_submitted" => Ok(Self::NotSubmitted),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventVariant12BodyResolution {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventVariant12BodyResolution {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant13Body {
    #[doc = "Upstream surface lifecycle operation paired with its projection."]
    pub operation: ::std::string::String,
    #[doc = "Original bounded upstream A2UI payload, preserved for display recovery."]
    pub payload: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[doc = "Surface revision advanced atomically with the event watermark."]
    pub revision: Counter,
    #[doc = "Product surface incarnation, never resurrected after removal."]
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant14Body {
    #[doc = "Upstream surface lifecycle operation paired with its projection."]
    pub operation: ::std::string::String,
    #[doc = "Original bounded upstream A2UI payload, preserved for display recovery."]
    pub payload: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[doc = "Surface revision advanced atomically with the event watermark."]
    pub revision: Counter,
    #[doc = "Product surface incarnation, never resurrected after removal."]
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant15Body {
    #[doc = "Upstream surface lifecycle operation paired with its projection."]
    pub operation: ::std::string::String,
    #[doc = "Original bounded upstream A2UI payload, preserved for display recovery."]
    pub payload: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[doc = "Surface revision advanced atomically with the event watermark."]
    pub revision: Counter,
    #[doc = "Product surface incarnation, never resurrected after removal."]
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant16Body {
    #[doc = "Surface revision advanced atomically with the event watermark."]
    pub revision: Counter,
    #[doc = "Product surface incarnation, never resurrected after removal."]
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant17Body {
    #[doc = "Prior provider incarnation invalidated by this verified handoff."]
    #[serde(rename = "previousGeneration")]
    pub previous_generation: Id,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant18Body {
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "status variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant1Body {
    #[doc = "Closed command lifecycle projection."]
    pub state: EventVariant1BodyState,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Closed command lifecycle projection."]
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
pub enum EventVariant1BodyState {
    #[serde(rename = "accepted")]
    #[doc = "`Accepted` alternative; see the parent type's schema contract."]
    Accepted,
}
impl ::std::fmt::Display for EventVariant1BodyState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Accepted => f.write_str("accepted"),
        }
    }
}
impl ::std::str::FromStr for EventVariant1BodyState {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "accepted" => Ok(Self::Accepted),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventVariant1BodyState {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventVariant1BodyState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "status variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant2Body {
    #[doc = "Closed command lifecycle projection."]
    pub state: EventVariant2BodyState,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Closed command lifecycle projection."]
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
pub enum EventVariant2BodyState {
    #[serde(rename = "dispatching")]
    #[doc = "`Dispatching` alternative; see the parent type's schema contract."]
    Dispatching,
    #[serde(rename = "running")]
    #[doc = "`Running` alternative; see the parent type's schema contract."]
    Running,
    #[serde(rename = "reconciliation_required")]
    #[doc = "`ReconciliationRequired` alternative; see the parent type's schema contract."]
    ReconciliationRequired,
}
impl ::std::fmt::Display for EventVariant2BodyState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Dispatching => f.write_str("dispatching"),
            Self::Running => f.write_str("running"),
            Self::ReconciliationRequired => f.write_str("reconciliation_required"),
        }
    }
}
impl ::std::str::FromStr for EventVariant2BodyState {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "dispatching" => Ok(Self::Dispatching),
            "running" => Ok(Self::Running),
            "reconciliation_required" => Ok(Self::ReconciliationRequired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventVariant2BodyState {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventVariant2BodyState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "terminal variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant3Body {
    #[doc = "Definite model-turn result; no implication about business side effects."]
    pub outcome: Outcome,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "cancel_dispatched variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant4Body {
    #[doc = "Cancellation request transport confirmation only; does not manufacture a model terminal."]
    pub confirmation: EventVariant4BodyConfirmation,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Cancellation request transport confirmation only; does not manufacture a model terminal."]
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
pub enum EventVariant4BodyConfirmation {
    #[serde(rename = "request_only")]
    #[doc = "`RequestOnly` alternative; see the parent type's schema contract."]
    RequestOnly,
    #[serde(rename = "already_terminal")]
    #[doc = "`AlreadyTerminal` alternative; see the parent type's schema contract."]
    AlreadyTerminal,
    #[serde(rename = "unsupported")]
    #[doc = "`Unsupported` alternative; see the parent type's schema contract."]
    Unsupported,
}
impl ::std::fmt::Display for EventVariant4BodyConfirmation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::RequestOnly => f.write_str("request_only"),
            Self::AlreadyTerminal => f.write_str("already_terminal"),
            Self::Unsupported => f.write_str("unsupported"),
        }
    }
}
impl ::std::str::FromStr for EventVariant4BodyConfirmation {
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
impl ::std::convert::TryFrom<&str> for EventVariant4BodyConfirmation {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventVariant4BodyConfirmation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "tool_proposal variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant5Body {
    #[doc = "Untrusted tool JSON arguments, including keys, count toward product budgets."]
    pub arguments: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[doc = "Provider tool name; not an approved execution action."]
    pub name: Id,
    #[doc = "Untrusted tool proposal correlation identifier."]
    #[serde(rename = "proposalId")]
    pub proposal_id: Id,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "tool_result variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant6Body {
    #[doc = "Protocol tool-result disposition, not authoritative business execution status."]
    pub disposition: EventVariant6BodyDisposition,
    #[doc = "Untrusted tool proposal correlation identifier."]
    #[serde(rename = "proposalId")]
    pub proposal_id: Id,
    #[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
    pub text: EventVariant6BodyText,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Protocol tool-result disposition, not authoritative business execution status."]
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
pub enum EventVariant6BodyDisposition {
    #[serde(rename = "returned")]
    #[doc = "`Returned` alternative; see the parent type's schema contract."]
    Returned,
    #[serde(rename = "rejected")]
    #[doc = "`Rejected` alternative; see the parent type's schema contract."]
    Rejected,
    #[serde(rename = "unavailable")]
    #[doc = "`Unavailable` alternative; see the parent type's schema contract."]
    Unavailable,
}
impl ::std::fmt::Display for EventVariant6BodyDisposition {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Returned => f.write_str("returned"),
            Self::Rejected => f.write_str("rejected"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}
impl ::std::str::FromStr for EventVariant6BodyDisposition {
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
impl ::std::convert::TryFrom<&str> for EventVariant6BodyDisposition {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventVariant6BodyDisposition {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct EventVariant6BodyText(::std::string::String);
impl ::std::ops::Deref for EventVariant6BodyText {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<EventVariant6BodyText> for ::std::string::String {
    fn from(value: EventVariant6BodyText) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for EventVariant6BodyText {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() > 65536usize {
            return Err("longer than 65536 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for EventVariant6BodyText {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventVariant6BodyText {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for EventVariant6BodyText {
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
#[doc = "Initial ordinary question publication; the matching Interaction is committed atomically."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant7Body {
    #[doc = "Single-use interaction identity within the namespace."]
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    #[doc = "Required for the first pending event and equal to the newly committed Interaction request; forbidden on later lifecycle events."]
    pub request: InteractionRequest,
    #[doc = "First publication of an ordinary user question."]
    pub status: ::std::string::String,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Question lifecycle transition; cannot republish or replace its request."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant8Body {
    #[doc = "Single-use interaction identity within the namespace."]
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    #[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
    pub status: EventVariant8BodyStatus,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
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
pub enum EventVariant8BodyStatus {
    #[serde(rename = "answered")]
    #[doc = "`Answered` alternative; see the parent type's schema contract."]
    Answered,
    #[serde(rename = "expired")]
    #[doc = "`Expired` alternative; see the parent type's schema contract."]
    Expired,
    #[serde(rename = "unavailable")]
    #[doc = "`Unavailable` alternative; see the parent type's schema contract."]
    Unavailable,
}
impl ::std::fmt::Display for EventVariant8BodyStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Answered => f.write_str("answered"),
            Self::Expired => f.write_str("expired"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}
impl ::std::str::FromStr for EventVariant8BodyStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "answered" => Ok(Self::Answered),
            "expired" => Ok(Self::Expired),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventVariant8BodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventVariant8BodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "error variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventVariant9Body {
    #[doc = "Closed failure category and retry discipline."]
    pub failure: Failure,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
#[doc = "Value-free failure and explicit retry discipline."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    #[doc = "Closed diagnostic category without input values."]
    pub code: ErrorCode,
    #[doc = "Explicit retry discipline; uncertainty never authorizes blind resubmission."]
    pub retry: Retry,
}
#[doc = "Opaque ASCII correlation identifier (1–128 characters); never an authentication credential."]
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
#[doc = "Product command inputs; provider-specific formats remain adapter-owned."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Input {
    #[doc = "prompt variant; all fields are data, never authentication or execution authority."]
    #[serde(rename = "prompt")]
    Prompt {
        #[doc = "queue_next serializes later work; steer must match the currently active native run."]
        policy: InputPolicy,
        #[doc = "Exact active native run required for steer; forbidden for queue_next."]
        #[serde(
            rename = "targetRunId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        target_run_id: ::std::option::Option<Id>,
        #[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
        text: InputText,
    },
    #[doc = "cancel variant; all fields are data, never authentication or execution authority."]
    #[serde(rename = "cancel")]
    Cancel {
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
        #[serde(
            rename = "nativeRunId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        native_run_id: ::std::option::Option<Id>,
        #[doc = "Original accepted command being cancelled."]
        #[serde(rename = "targetCommandId")]
        target_command_id: Id,
    },
    #[doc = "respond variant; all fields are data, never authentication or execution authority."]
    #[serde(rename = "respond")]
    Respond {
        #[doc = "Untrusted JSON response data; cannot carry approval authority."]
        answer: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Single-use interaction identity within the namespace."]
        #[serde(rename = "interactionId")]
        interaction_id: Id,
        #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
        #[serde(
            rename = "nativeRunId",
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        native_run_id: ::std::option::Option<Id>,
        #[doc = "Mandatory for an interaction associated with a surface; cannot bypass deleted/stale state."]
        #[serde(skip_serializing_if = "::std::option::Option::is_none")]
        surface: ::std::option::Option<SurfaceReference>,
    },
}
#[doc = "queue_next serializes later work; steer must match the currently active native run."]
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
    #[doc = "`QueueNext` alternative; see the parent type's schema contract."]
    QueueNext,
    #[serde(rename = "steer")]
    #[doc = "`Steer` alternative; see the parent type's schema contract."]
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
#[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
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
#[doc = "Single-use provider callback with immutable command/native correlation, expiry and lifetime."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    #[doc = "generation_bound cannot survive callback loss; provider_resumable requires verified native restoration."]
    #[serde(rename = "callbackLifetime")]
    pub callback_lifetime: InteractionCallbackLifetime,
    #[doc = "Ordinary user question only; permission and execution callbacks are forbidden in this lifecycle."]
    pub category: ::std::string::String,
    #[doc = "Client-generated idempotency key; reuse only with identical canonical content."]
    #[serde(rename = "commandId")]
    pub command_id: Id,
    #[doc = "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected."]
    #[serde(rename = "expiresAtMs")]
    pub expires_at_ms: Counter,
    #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
    pub generation: Id,
    #[doc = "Single-use interaction identity within the namespace."]
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    #[doc = "Closed product record discriminator."]
    pub kind: ::std::string::String,
    #[doc = "Trusted storage isolation scope; not copied from model or action content."]
    pub namespace: Namespace,
    #[doc = "Provider-owned callback identifier, immutable and unique within a session generation; distinct from the parent dispatch request."]
    #[serde(rename = "nativeCallbackId")]
    pub native_callback_id: Id,
    #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
    #[serde(
        rename = "nativeRunId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_run_id: ::std::option::Option<Id>,
    #[doc = "Immutable untrusted question payload retained for display; does not restore a lost native callback or grant approval."]
    pub request: InteractionRequest,
    #[doc = "Accepted response command which atomically consumed the interaction; present only when answered."]
    #[serde(
        rename = "responseCommandId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub response_command_id: ::std::option::Option<Id>,
    #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
    pub status: InteractionStatus,
}
#[doc = "generation_bound cannot survive callback loss; provider_resumable requires verified native restoration."]
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
    #[doc = "`GenerationBound` alternative; see the parent type's schema contract."]
    GenerationBound,
    #[serde(rename = "provider_resumable")]
    #[doc = "`ProviderResumable` alternative; see the parent type's schema contract."]
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
#[doc = "Immutable untrusted provider question payload. Subject to whole-record JSON budgets; never authentication, permission or execution approval."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct InteractionRequest(pub ::serde_json::Map<::std::string::String, ::serde_json::Value>);
impl ::std::ops::Deref for InteractionRequest {
    type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
    fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
        &self.0
    }
}
impl ::std::convert::From<InteractionRequest>
    for ::serde_json::Map<::std::string::String, ::serde_json::Value>
{
    fn from(value: InteractionRequest) -> Self {
        value.0
    }
}
impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
    for InteractionRequest
{
    fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
        Self(value)
    }
}
#[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
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
    #[doc = "`Pending` alternative; see the parent type's schema contract."]
    Pending,
    #[serde(rename = "answered")]
    #[doc = "`Answered` alternative; see the parent type's schema contract."]
    Answered,
    #[serde(rename = "expired")]
    #[doc = "`Expired` alternative; see the parent type's schema contract."]
    Expired,
    #[serde(rename = "unavailable")]
    #[doc = "`Unavailable` alternative; see the parent type's schema contract."]
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
#[doc = "Trusted tenant/principal/authority/logical-session storage scope supplied by authenticated ingress."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Namespace {
    #[doc = "Authenticated authority scope."]
    #[serde(rename = "authorityId")]
    pub authority_id: Id,
    #[doc = "Authenticated principal scope."]
    #[serde(rename = "principalId")]
    pub principal_id: Id,
    #[doc = "Logical session identifier, never reusable after retirement."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
    #[doc = "Authenticated tenant scope."]
    #[serde(rename = "tenantId")]
    pub tenant_id: Id,
}
#[doc = "Definite model-turn outcome; does not establish process exit or business-side-effect completion."]
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
    #[doc = "`Completed` alternative; see the parent type's schema contract."]
    Completed,
    #[serde(rename = "interrupted")]
    #[doc = "`Interrupted` alternative; see the parent type's schema contract."]
    Interrupted,
    #[serde(rename = "refused")]
    #[doc = "`Refused` alternative; see the parent type's schema contract."]
    Refused,
    #[serde(rename = "limit_reached")]
    #[doc = "`LimitReached` alternative; see the parent type's schema contract."]
    LimitReached,
    #[serde(rename = "failed")]
    #[doc = "`Failed` alternative; see the parent type's schema contract."]
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
#[doc = "Immutable acceptance fact. Only an actual committed store makes it durable; it is not a model terminal."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    #[doc = "UTC epoch milliseconds at committed acceptance."]
    #[serde(rename = "acceptedAtMs")]
    pub accepted_at_ms: Counter,
    #[doc = "Session revision which atomically accepted this command."]
    #[serde(rename = "acceptedRevision")]
    pub accepted_revision: Counter,
    #[doc = "Client-generated idempotency key; reuse only with identical canonical content."]
    #[serde(rename = "commandId")]
    pub command_id: Id,
    #[doc = "SHA-256 of JCS(command), including every command field; namespace is a separate storage key."]
    #[serde(rename = "contentHash")]
    pub content_hash: ReceiptContentHash,
    #[doc = "Closed product record discriminator."]
    pub kind: ::std::string::String,
    #[doc = "Trusted storage isolation scope; not copied from model or action content."]
    pub namespace: Namespace,
    #[doc = "Inclusive stored-receipt deadline; cannot be shorter than the retry deadline."]
    #[serde(rename = "receiptUntilMs")]
    pub receipt_until_ms: Counter,
    #[doc = "Inclusive same-command retry deadline, no later than command expiry."]
    #[serde(rename = "retryUntilMs")]
    pub retry_until_ms: Counter,
    #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
}
#[doc = "SHA-256 of JCS(command), including every command field; namespace is a separate storage key."]
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
#[doc = "same_command preserves identity/content; reconcile_first checks the original operation; never forbids retry."]
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
    #[doc = "`SameCommand` alternative; see the parent type's schema contract."]
    SameCommand,
    #[serde(rename = "reconcile_first")]
    #[doc = "`ReconcileFirst` alternative; see the parent type's schema contract."]
    ReconcileFirst,
    #[serde(rename = "never")]
    #[doc = "`Never` alternative; see the parent type's schema contract."]
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
#[doc = "Logical session state and stable event watermark committed at one revision."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Session {
    #[doc = "Exact provider incarnation and native context identity."]
    pub binding: Binding,
    #[doc = "Capabilities bound to this exact provider/configuration/account incarnation."]
    pub capabilities: Capabilities,
    #[doc = "Closed product record discriminator."]
    pub kind: ::std::string::String,
    #[doc = "Highest committed stable-event sequence at this session revision."]
    #[serde(rename = "lastSequence")]
    pub last_sequence: Counter,
    #[doc = "Trusted storage isolation scope; not copied from model or action content."]
    pub namespace: Namespace,
    #[doc = "Monotonic CAS revision of this product record."]
    pub revision: Counter,
    #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
    pub status: SessionStatus,
}
#[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
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
    #[doc = "`Active` alternative; see the parent type's schema contract."]
    Active,
    #[serde(rename = "retired")]
    #[doc = "`Retired` alternative; see the parent type's schema contract."]
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
#[doc = "Product metadata accompanying an unchanged upstream action; association does not grant permission."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SurfaceAction {
    #[doc = "Client-generated idempotency key; reuse only with identical canonical content."]
    #[serde(rename = "commandId")]
    pub command_id: Id,
    #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
    pub generation: Id,
    #[doc = "Single-use interaction identity within the namespace."]
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    #[doc = "Closed product record discriminator."]
    pub kind: ::std::string::String,
    #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
    #[serde(rename = "nativeRunId")]
    pub native_run_id: Id,
    #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[doc = "Logical session identifier, never reusable after retirement."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
    #[doc = "Fresh product identity for each surface creation; deletion permanently invalidates old actions."]
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
    #[doc = "Exact current surface revision required to accept this action."]
    #[serde(rename = "surfaceRevision")]
    pub surface_revision: Counter,
}
#[doc = "Product association for an upstream A2UI surface instance; catalog/renderer retain upstream ownership."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SurfaceBinding {
    #[doc = "Exact negotiated upstream A2UI version."]
    #[serde(rename = "a2uiVersion")]
    pub a2ui_version: ::std::string::String,
    #[doc = "Negotiated upstream catalog identity."]
    #[serde(rename = "catalogId")]
    pub catalog_id: Id,
    #[doc = "Negotiated fixed catalog version; not a renderer implementation claim."]
    #[serde(rename = "catalogVersion")]
    pub catalog_version: Id,
    #[doc = "Exact upstream action name associated with this interaction."]
    #[serde(rename = "eventName")]
    pub event_name: Id,
    #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
    pub generation: Id,
    #[doc = "Single-use interaction identity within the namespace."]
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    #[doc = "Closed product record discriminator."]
    pub kind: ::std::string::String,
    #[doc = "Trusted storage isolation scope; not copied from model or action content."]
    pub namespace: Namespace,
    #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
    #[serde(rename = "nativeRunId")]
    pub native_run_id: Id,
    #[doc = "Monotonic CAS revision of this product record."]
    pub revision: Counter,
    #[doc = "Exact product wire version; V1 is rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[doc = "Exact upstream source component allowed to emit this action."]
    #[serde(rename = "sourceComponentId")]
    pub source_component_id: Id,
    #[doc = "Deleted is an upstream deletion; invalidated is a product-side loss of action authority. Neither may reactivate."]
    pub status: SurfaceBindingStatus,
    #[doc = "Upstream A2UI surface identifier."]
    #[serde(rename = "surfaceId")]
    pub surface_id: Id,
    #[doc = "Fresh product identity for each surface creation; deletion permanently invalidates old actions."]
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
}
#[doc = "Deleted is an upstream deletion; invalidated is a product-side loss of action authority. Neither may reactivate."]
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
pub enum SurfaceBindingStatus {
    #[serde(rename = "active")]
    #[doc = "`Active` alternative; see the parent type's schema contract."]
    Active,
    #[serde(rename = "deleted")]
    #[doc = "`Deleted` alternative; see the parent type's schema contract."]
    Deleted,
    #[serde(rename = "invalidated")]
    #[doc = "`Invalidated` alternative; see the parent type's schema contract."]
    Invalidated,
}
impl ::std::fmt::Display for SurfaceBindingStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Active => f.write_str("active"),
            Self::Deleted => f.write_str("deleted"),
            Self::Invalidated => f.write_str("invalidated"),
        }
    }
}
impl ::std::str::FromStr for SurfaceBindingStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            "invalidated" => Ok(Self::Invalidated),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SurfaceBindingStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SurfaceBindingStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Surface revision checked atomically when accepting an action response."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SurfaceReference {
    #[doc = "Exact product surface instance associated with this response."]
    #[serde(rename = "instanceId")]
    pub instance_id: Id,
    #[doc = "Current surface revision checked atomically during response acceptance."]
    pub revision: Counter,
}
#[doc = "Product reliability records only. No record authenticates a caller, grants approval or proves business execution. Standard ACP/A2UI schemas retain their upstream owners."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(untagged)]
pub enum WireRecord {
    #[doc = "`Command` alternative; see the parent type's schema contract."]
    Command(#[doc = "`` member; see its generated type and parent schema."] Command),
    #[doc = "`Receipt` alternative; see the parent type's schema contract."]
    Receipt(#[doc = "`` member; see its generated type and parent schema."] Receipt),
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord(#[doc = "`` member; see its generated type and parent schema."] CommandRecord),
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event(#[doc = "`` member; see its generated type and parent schema."] Event),
    #[doc = "`Session` alternative; see the parent type's schema contract."]
    Session(#[doc = "`` member; see its generated type and parent schema."] Session),
    #[doc = "`Interaction` alternative; see the parent type's schema contract."]
    Interaction(#[doc = "`` member; see its generated type and parent schema."] Interaction),
    #[doc = "`Delivery` alternative; see the parent type's schema contract."]
    Delivery(#[doc = "`` member; see its generated type and parent schema."] Delivery),
    #[doc = "`SurfaceBinding` alternative; see the parent type's schema contract."]
    SurfaceBinding(#[doc = "`` member; see its generated type and parent schema."] SurfaceBinding),
    #[doc = "`SurfaceAction` alternative; see the parent type's schema contract."]
    SurfaceAction(#[doc = "`` member; see its generated type and parent schema."] SurfaceAction),
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
impl std::fmt::Debug for DispatchAttempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DispatchAttempt), "([redacted])"))
    }
}
impl std::fmt::Debug for DispatchAttemptCertainty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(DispatchAttemptCertainty),
            "([redacted])"
        ))
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
impl std::fmt::Debug for EventVariant0Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant0Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant0BodyText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant0BodyText), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant10Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant10Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant11Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant11Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant12Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant12Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant12BodyResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventVariant12BodyResolution),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventVariant13Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant13Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant14Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant14Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant15Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant15Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant16Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant16Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant17Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant17Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant18Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant18Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant1Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant1Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant1BodyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant1BodyState), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant2Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant2Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant2BodyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant2BodyState), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant3Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant3Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant4Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant4Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant4BodyConfirmation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventVariant4BodyConfirmation),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventVariant5Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant5Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant6Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant6Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant6BodyDisposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventVariant6BodyDisposition),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventVariant6BodyText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant6BodyText), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant7Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant7Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant8Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant8Body), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant8BodyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant8BodyStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for EventVariant9Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventVariant9Body), "([redacted])"))
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
impl std::fmt::Debug for InteractionRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(InteractionRequest), "([redacted])"))
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
impl std::fmt::Debug for SurfaceBindingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceBindingStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceReference), "([redacted])"))
    }
}
impl std::fmt::Debug for WireRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(WireRecord), "([redacted])"))
    }
}
