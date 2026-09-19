// @generated from packages/ai-contract/schema/runtime.schema.json. Do not edit.
#[doc = "Explicitly selected upstream version and product catalog."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct A2uiNegotiation {
    #[doc = "Fixed product catalog identity."]
    #[serde(rename = "catalogId")]
    pub catalog_id: A2uiNegotiationCatalogId,
    #[doc = "Fixed product catalog revision."]
    #[serde(rename = "catalogVersion")]
    pub catalog_version: A2uiNegotiationCatalogVersion,
    #[doc = "Fixed upstream protocol version."]
    pub version: A2uiNegotiationVersion,
}
#[doc = "Fixed product catalog identity."]
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
pub enum A2uiNegotiationCatalogId {
    #[serde(rename = "urn:rss-mdm-agent:a2ui:interaction")]
    #[doc = "`UrnRssMdmAgentA2uiInteraction` alternative; see the parent type's schema contract."]
    UrnRssMdmAgentA2uiInteraction,
}
impl ::std::fmt::Display for A2uiNegotiationCatalogId {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::UrnRssMdmAgentA2uiInteraction => {
                f.write_str("urn:rss-mdm-agent:a2ui:interaction")
            }
        }
    }
}
impl ::std::str::FromStr for A2uiNegotiationCatalogId {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "urn:rss-mdm-agent:a2ui:interaction" => Ok(Self::UrnRssMdmAgentA2uiInteraction),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for A2uiNegotiationCatalogId {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for A2uiNegotiationCatalogId {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Fixed product catalog revision."]
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
pub enum A2uiNegotiationCatalogVersion {
    #[serde(rename = "1")]
    #[doc = "`X1` alternative; see the parent type's schema contract."]
    X1,
}
impl ::std::fmt::Display for A2uiNegotiationCatalogVersion {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::X1 => f.write_str("1"),
        }
    }
}
impl ::std::str::FromStr for A2uiNegotiationCatalogVersion {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "1" => Ok(Self::X1),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for A2uiNegotiationCatalogVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for A2uiNegotiationCatalogVersion {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Fixed upstream protocol version."]
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
pub enum A2uiNegotiationVersion {
    #[serde(rename = "v0.9.1")]
    #[doc = "`V091` alternative; see the parent type's schema contract."]
    V091,
}
impl ::std::fmt::Display for A2uiNegotiationVersion {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::V091 => f.write_str("v0.9.1"),
        }
    }
}
impl ::std::str::FromStr for A2uiNegotiationVersion {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "v0.9.1" => Ok(Self::V091),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for A2uiNegotiationVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for A2uiNegotiationVersion {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`AccessUpdate`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AccessUpdate {
    #[doc = "Connection-local attachment identity, echoed on every update."]
    #[serde(rename = "attachmentId")]
    pub attachment_id: Id,
    #[doc = "Closed record discriminator."]
    pub kind: AccessUpdateKind,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: AccessUpdateSchemaVersion,
    #[doc = "Product session identity within the authenticated caller namespace."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
    #[doc = "Stable event, ephemeral delta, or explicit resynchronization signal."]
    pub update: Subscription,
}
#[doc = "Closed record discriminator."]
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
pub enum AccessUpdateKind {
    #[serde(rename = "accessUpdate")]
    #[doc = "`AccessUpdate` alternative; see the parent type's schema contract."]
    AccessUpdate,
}
impl ::std::fmt::Display for AccessUpdateKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::AccessUpdate => f.write_str("accessUpdate"),
        }
    }
}
impl ::std::str::FromStr for AccessUpdateKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "accessUpdate" => Ok(Self::AccessUpdate),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AccessUpdateKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AccessUpdateKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct AccessUpdateSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for AccessUpdateSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<AccessUpdateSchemaVersion> for i64 {
    fn from(value: AccessUpdateSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for AccessUpdateSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for AccessUpdateSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Native control acknowledgement; never a model turn outcome."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(tag = "type", content = "confirmation")]
pub enum Acknowledgement {
    #[serde(rename = "cancel")]
    #[doc = "`Cancel` alternative; see the parent type's schema contract."]
    Cancel(
        #[doc = "`` member; see its generated type and parent schema."] AcknowledgementConfirmation,
    ),
    #[serde(rename = "respond")]
    #[doc = "`Respond` alternative; see the parent type's schema contract."]
    Respond,
    #[serde(rename = "steer")]
    #[doc = "`Steer` alternative; see the parent type's schema contract."]
    Steer,
}
impl ::std::convert::From<AcknowledgementConfirmation> for Acknowledgement {
    fn from(value: AcknowledgementConfirmation) -> Self {
        Self::Cancel(value)
    }
}
#[doc = "Provider confirms the control request, not a model terminal."]
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
pub enum AcknowledgementConfirmation {
    #[serde(rename = "request_only")]
    #[doc = "`RequestOnly` alternative; see the parent type's schema contract."]
    RequestOnly,
    #[serde(rename = "already_terminal")]
    #[doc = "`AlreadyTerminal` alternative; see the parent type's schema contract."]
    AlreadyTerminal,
}
impl ::std::fmt::Display for AcknowledgementConfirmation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::RequestOnly => f.write_str("request_only"),
            Self::AlreadyTerminal => f.write_str("already_terminal"),
        }
    }
}
impl ::std::str::FromStr for AcknowledgementConfirmation {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "request_only" => Ok(Self::RequestOnly),
            "already_terminal" => Ok(Self::AlreadyTerminal),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AcknowledgementConfirmation {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AcknowledgementConfirmation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ActionRequest`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    #[doc = "Command expiry in Unix milliseconds."]
    #[serde(rename = "expiresAtMs")]
    pub expires_at_ms: Counter,
    #[doc = "Closed record discriminator."]
    pub kind: ActionRequestKind,
    #[doc = "Unchanged upstream A2UI client message, validated against the negotiated schema."]
    pub message: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[doc = "Product action association, checked independently of untrusted upstream context."]
    pub metadata: SurfaceAction,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: ActionRequestSchemaVersion,
}
#[doc = "Closed record discriminator."]
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
pub enum ActionRequestKind {
    #[serde(rename = "actionRequest")]
    #[doc = "`ActionRequest` alternative; see the parent type's schema contract."]
    ActionRequest,
}
impl ::std::fmt::Display for ActionRequestKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ActionRequest => f.write_str("actionRequest"),
        }
    }
}
impl ::std::str::FromStr for ActionRequestKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "actionRequest" => Ok(Self::ActionRequest),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ActionRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ActionRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct ActionRequestSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for ActionRequestSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<ActionRequestSchemaVersion> for i64 {
    fn from(value: ActionRequestSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for ActionRequestSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for ActionRequestSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "`AttachReceipt`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AttachReceipt {
    #[doc = "Last stable sequence consumed before attaching."]
    pub after: Counter,
    #[doc = "Connection-local attachment identity, echoed on every update."]
    #[serde(rename = "attachmentId")]
    pub attachment_id: Id,
    #[doc = "Closed record discriminator."]
    pub kind: AttachReceiptKind,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: AttachReceiptSchemaVersion,
    #[doc = "Product session identity within the authenticated caller namespace."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
}
#[doc = "Closed record discriminator."]
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
pub enum AttachReceiptKind {
    #[serde(rename = "attachReceipt")]
    #[doc = "`AttachReceipt` alternative; see the parent type's schema contract."]
    AttachReceipt,
}
impl ::std::fmt::Display for AttachReceiptKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::AttachReceipt => f.write_str("attachReceipt"),
        }
    }
}
impl ::std::str::FromStr for AttachReceiptKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "attachReceipt" => Ok(Self::AttachReceipt),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AttachReceiptKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AttachReceiptKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct AttachReceiptSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for AttachReceiptSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<AttachReceiptSchemaVersion> for i64 {
    fn from(value: AttachReceiptSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for AttachReceiptSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for AttachReceiptSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "`AttachRequest`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AttachRequest {
    #[doc = "Last stable sequence consumed before attaching."]
    pub after: Counter,
    #[doc = "Connection-local attachment identity, echoed on every update."]
    #[serde(rename = "attachmentId")]
    pub attachment_id: Id,
    #[doc = "Closed record discriminator."]
    pub kind: AttachRequestKind,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: AttachRequestSchemaVersion,
    #[doc = "Product session identity within the authenticated caller namespace."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
}
#[doc = "Closed record discriminator."]
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
pub enum AttachRequestKind {
    #[serde(rename = "attachRequest")]
    #[doc = "`AttachRequest` alternative; see the parent type's schema contract."]
    AttachRequest,
}
impl ::std::fmt::Display for AttachRequestKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::AttachRequest => f.write_str("attachRequest"),
        }
    }
}
impl ::std::str::FromStr for AttachRequestKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "attachRequest" => Ok(Self::AttachRequest),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for AttachRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for AttachRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct AttachRequestSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for AttachRequestSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<AttachRequestSchemaVersion> for i64 {
    fn from(value: AttachRequestSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for AttachRequestSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for AttachRequestSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
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
    #[doc = "Provider-owned thread identity within the native session tree; required when the provider exposes distinct threads."]
    #[serde(
        rename = "nativeThreadId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_thread_id: ::std::option::Option<Id>,
    #[doc = "Provider adapter identity."]
    pub provider: Id,
    #[doc = "Pinned native provider implementation version."]
    #[serde(rename = "providerVersion")]
    pub provider_version: Id,
    #[doc = "SHA-256 identity of the normalized absolute workspace path. Filesystem containment remains owned by the provider adapter and composition root."]
    #[serde(rename = "workspaceId")]
    pub workspace_id: Id,
}
#[doc = "A live-generation callback. Restore preserves display history but always makes the previous callback unavailable."]
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
pub enum CallbackLifetime {
    #[serde(rename = "generation_bound")]
    #[doc = "`GenerationBound` alternative; see the parent type's schema contract."]
    GenerationBound,
}
impl ::std::fmt::Display for CallbackLifetime {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::GenerationBound => f.write_str("generation_bound"),
        }
    }
}
impl ::std::str::FromStr for CallbackLifetime {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "generation_bound" => Ok(Self::GenerationBound),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CallbackLifetime {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CallbackLifetime {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
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
    pub kind: CommandKind,
    #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: CommandSchemaVersion,
    #[doc = "Logical session identifier, never reusable after retirement."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandKind {
    #[serde(rename = "command")]
    #[doc = "`Command` alternative; see the parent type's schema contract."]
    Command,
}
impl ::std::fmt::Display for CommandKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Command => f.write_str("command"),
        }
    }
}
impl ::std::str::FromStr for CommandKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "command" => Ok(Self::Command),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed command lifecycle; acceptance is immutable, local invalidation does not assert a model terminal."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(untagged, deny_unknown_fields)]
pub enum CommandRecord {
    #[doc = "`Variant0` alternative; see the parent type's schema contract."]
    Variant0 {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant0Kind,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant0SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant0State,
    },
    #[doc = "`Variant1` alternative; see the parent type's schema contract."]
    Variant1 {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant1Kind,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant1SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant1State,
    },
    #[doc = "`Variant2` alternative; see the parent type's schema contract."]
    Variant2 {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant2Kind,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant2SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant2State,
    },
    #[doc = "`Variant3` alternative; see the parent type's schema contract."]
    Variant3 {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant3Kind,
        #[doc = "Explicitly observed model terminal outcome."]
        outcome: Outcome,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant3SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant3State,
    },
    #[doc = "`Variant4` alternative; see the parent type's schema contract."]
    Variant4 {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant4Kind,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant4SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant4State,
    },
    #[doc = "`Variant5` alternative; see the parent type's schema contract."]
    Variant5 {
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Local failure without asserting a model terminal."]
        failure: Failure,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant5Kind,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant5SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant5State,
    },
    #[doc = "`Variant6` alternative; see the parent type's schema contract."]
    Variant6 {
        #[doc = "Accepted local cancellation command identity."]
        #[serde(rename = "cancelledBy")]
        cancelled_by: Id,
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant6Kind,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant6SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant6State,
    },
    #[doc = "`Variant7` alternative; see the parent type's schema contract."]
    Variant7 {
        #[doc = "Closed control acknowledgement; never a model-turn outcome."]
        acknowledgement: Acknowledgement,
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Original attempt and append-once native correlation coordinates."]
        dispatch: DispatchAttempt,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant7Kind,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant7SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant7State,
    },
    #[doc = "`Variant8` alternative; see the parent type's schema contract."]
    Variant8 {
        #[doc = "`acknowledgement` member; see its generated type and parent schema."]
        acknowledgement: CommandRecordVariant8Acknowledgement,
        #[doc = "Immutable original command."]
        command: Command,
        #[doc = "Closed product record discriminator."]
        kind: CommandRecordVariant8Kind,
        #[doc = "Immutable original committed acceptance fact."]
        receipt: Receipt,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: CommandRecordVariant8SchemaVersion,
        #[doc = "Closed command lifecycle projection."]
        state: CommandRecordVariant8State,
    },
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant0Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant0Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant0Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant0Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant0Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant0SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant0SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant0SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant0SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant0SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant0SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant0State {
    #[serde(rename = "accepted")]
    #[doc = "`Accepted` alternative; see the parent type's schema contract."]
    Accepted,
}
impl ::std::fmt::Display for CommandRecordVariant0State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Accepted => f.write_str("accepted"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant0State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "accepted" => Ok(Self::Accepted),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant0State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant0State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant1Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant1Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant1Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant1Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant1Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant1SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant1SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant1SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant1SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant1SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant1SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant1State {
    #[serde(rename = "dispatching")]
    #[doc = "`Dispatching` alternative; see the parent type's schema contract."]
    Dispatching,
}
impl ::std::fmt::Display for CommandRecordVariant1State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Dispatching => f.write_str("dispatching"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant1State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "dispatching" => Ok(Self::Dispatching),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant1State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant1State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant2Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant2Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant2Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant2Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant2Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant2SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant2SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant2SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant2SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant2SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant2SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant2State {
    #[serde(rename = "running")]
    #[doc = "`Running` alternative; see the parent type's schema contract."]
    Running,
}
impl ::std::fmt::Display for CommandRecordVariant2State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Running => f.write_str("running"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant2State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "running" => Ok(Self::Running),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant2State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant2State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant3Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant3Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant3Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant3Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant3Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant3SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant3SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant3SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant3SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant3SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant3SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant3State {
    #[serde(rename = "terminal")]
    #[doc = "`Terminal` alternative; see the parent type's schema contract."]
    Terminal,
}
impl ::std::fmt::Display for CommandRecordVariant3State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Terminal => f.write_str("terminal"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant3State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "terminal" => Ok(Self::Terminal),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant3State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant3State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant4Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant4Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant4Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant4Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant4Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant4SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant4SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant4SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant4SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant4SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant4SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant4State {
    #[serde(rename = "reconciliation_required")]
    #[doc = "`ReconciliationRequired` alternative; see the parent type's schema contract."]
    ReconciliationRequired,
}
impl ::std::fmt::Display for CommandRecordVariant4State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ReconciliationRequired => f.write_str("reconciliation_required"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant4State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "reconciliation_required" => Ok(Self::ReconciliationRequired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant4State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant4State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant5Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant5Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant5Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant5Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant5Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant5SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant5SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant5SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant5SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant5SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant5SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant5State {
    #[serde(rename = "invalidated")]
    #[doc = "`Invalidated` alternative; see the parent type's schema contract."]
    Invalidated,
}
impl ::std::fmt::Display for CommandRecordVariant5State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Invalidated => f.write_str("invalidated"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant5State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "invalidated" => Ok(Self::Invalidated),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant5State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant5State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant6Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant6Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant6Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant6Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant6Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant6SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant6SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant6SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant6SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant6SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant6SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant6State {
    #[serde(rename = "cancelled")]
    #[doc = "`Cancelled` alternative; see the parent type's schema contract."]
    Cancelled,
}
impl ::std::fmt::Display for CommandRecordVariant6State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant6State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "cancelled" => Ok(Self::Cancelled),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant6State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant6State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant7Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant7Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant7Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant7Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant7Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant7SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant7SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant7SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant7SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant7SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant7SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant7State {
    #[serde(rename = "acknowledged")]
    #[doc = "`Acknowledged` alternative; see the parent type's schema contract."]
    Acknowledged,
}
impl ::std::fmt::Display for CommandRecordVariant7State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Acknowledged => f.write_str("acknowledged"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant7State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "acknowledged" => Ok(Self::Acknowledged),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant7State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant7State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed control acknowledgement; never a model-turn outcome."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CommandRecordVariant8Acknowledgement {
    #[doc = "Queued prompt cancelled without a native dispatch."]
    #[serde(rename = "targetCommandId")]
    pub target_command_id: Id,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: CommandRecordVariant8AcknowledgementType,
}
#[doc = "Closed variant discriminator."]
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
pub enum CommandRecordVariant8AcknowledgementType {
    #[serde(rename = "queued_cancelled")]
    #[doc = "`QueuedCancelled` alternative; see the parent type's schema contract."]
    QueuedCancelled,
}
impl ::std::fmt::Display for CommandRecordVariant8AcknowledgementType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::QueuedCancelled => f.write_str("queued_cancelled"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant8AcknowledgementType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "queued_cancelled" => Ok(Self::QueuedCancelled),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant8AcknowledgementType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant8AcknowledgementType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum CommandRecordVariant8Kind {
    #[serde(rename = "commandRecord")]
    #[doc = "`CommandRecord` alternative; see the parent type's schema contract."]
    CommandRecord,
}
impl ::std::fmt::Display for CommandRecordVariant8Kind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandRecord => f.write_str("commandRecord"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant8Kind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "commandRecord" => Ok(Self::CommandRecord),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant8Kind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant8Kind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandRecordVariant8SchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandRecordVariant8SchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandRecordVariant8SchemaVersion> for i64 {
    fn from(value: CommandRecordVariant8SchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandRecordVariant8SchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandRecordVariant8SchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum CommandRecordVariant8State {
    #[serde(rename = "acknowledged")]
    #[doc = "`Acknowledged` alternative; see the parent type's schema contract."]
    Acknowledged,
}
impl ::std::fmt::Display for CommandRecordVariant8State {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Acknowledged => f.write_str("acknowledged"),
        }
    }
}
impl ::std::str::FromStr for CommandRecordVariant8State {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "acknowledged" => Ok(Self::Acknowledged),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for CommandRecordVariant8State {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for CommandRecordVariant8State {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct CommandSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for CommandSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<CommandSchemaVersion> for i64 {
    fn from(value: CommandSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for CommandSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for CommandSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
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
pub struct Counter(#[doc = "`` member; see its generated type and parent schema."] pub i64);
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
    pub kind: DeliveryKind,
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
    #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: DeliverySchemaVersion,
    #[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
    pub status: DeliveryStatus,
    #[doc = "Opaque reliable-delivery destination identifier."]
    pub target: Id,
}
#[doc = "SHA-256 of JCS({event, target}), including the full referenced stable event and exact destination."]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct DeliveryContentHash(
    #[doc = "`` member; see its generated type and parent schema."] ::std::string::String,
);
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
#[doc = "Closed product record discriminator."]
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
pub enum DeliveryKind {
    #[serde(rename = "delivery")]
    #[doc = "`Delivery` alternative; see the parent type's schema contract."]
    Delivery,
}
impl ::std::fmt::Display for DeliveryKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Delivery => f.write_str("delivery"),
        }
    }
}
impl ::std::str::FromStr for DeliveryKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "delivery" => Ok(Self::Delivery),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DeliveryKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DeliveryKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
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
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct DeliverySchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for DeliverySchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<DeliverySchemaVersion> for i64 {
    fn from(value: DeliverySchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for DeliverySchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for DeliverySchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
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
#[doc = "`DetachRequest`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct DetachRequest {
    #[doc = "Connection-local attachment identity, echoed on every update."]
    #[serde(rename = "attachmentId")]
    pub attachment_id: Id,
    #[doc = "Closed record discriminator."]
    pub kind: DetachRequestKind,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: DetachRequestSchemaVersion,
    #[doc = "Product session identity within the authenticated caller namespace."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
}
#[doc = "Closed record discriminator."]
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
pub enum DetachRequestKind {
    #[serde(rename = "detachRequest")]
    #[doc = "`DetachRequest` alternative; see the parent type's schema contract."]
    DetachRequest,
}
impl ::std::fmt::Display for DetachRequestKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::DetachRequest => f.write_str("detachRequest"),
        }
    }
}
impl ::std::str::FromStr for DetachRequestKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "detachRequest" => Ok(Self::DetachRequest),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for DetachRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for DetachRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct DetachRequestSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for DetachRequestSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<DetachRequestSchemaVersion> for i64 {
    fn from(value: DetachRequestSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for DetachRequestSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for DetachRequestSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
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
    #[doc = "Provider-owned thread identity within the native session tree; required when the provider exposes distinct threads."]
    #[serde(
        rename = "nativeThreadId",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub native_thread_id: ::std::option::Option<Id>,
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
    #[serde(rename = "storage_corrupt")]
    #[doc = "`StorageCorrupt` alternative; see the parent type's schema contract."]
    StorageCorrupt,
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
            Self::StorageCorrupt => f.write_str("storage_corrupt"),
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
            "storage_corrupt" => Ok(Self::StorageCorrupt),
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
    #[doc = "`Text` alternative; see the parent type's schema contract."]
    Text {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventTextBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventTextKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventTextSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`CommandAccepted` alternative; see the parent type's schema contract."]
    CommandAccepted {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventCommandAcceptedBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventCommandAcceptedKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventCommandAcceptedSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`StatusDispatchingRunningReconciliationRequired` alternative; see the parent type's schema contract."]
    StatusDispatchingRunningReconciliationRequired {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventStatusDispatchingRunningReconciliationRequiredBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventStatusDispatchingRunningReconciliationRequiredKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventStatusDispatchingRunningReconciliationRequiredSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Terminal` alternative; see the parent type's schema contract."]
    Terminal {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventTerminalBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventTerminalKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventTerminalSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`CancelDispatched` alternative; see the parent type's schema contract."]
    CancelDispatched {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventCancelDispatchedBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventCancelDispatchedKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventCancelDispatchedSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`ToolProposal` alternative; see the parent type's schema contract."]
    ToolProposal {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventToolProposalBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventToolProposalKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventToolProposalSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`ToolResult` alternative; see the parent type's schema contract."]
    ToolResult {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventToolResultBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventToolResultKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventToolResultSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`InteractionPending` alternative; see the parent type's schema contract."]
    InteractionPending {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventInteractionPendingBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventInteractionPendingKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventInteractionPendingSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`InteractionAnswered` alternative; see the parent type's schema contract."]
    InteractionAnswered {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventInteractionAnsweredBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventInteractionAnsweredKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventInteractionAnsweredSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`InteractionExpiredUnavailable` alternative; see the parent type's schema contract."]
    InteractionExpiredUnavailable {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventInteractionExpiredUnavailableBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventInteractionExpiredUnavailableKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventInteractionExpiredUnavailableSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Error` alternative; see the parent type's schema contract."]
    Error {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventErrorBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventErrorKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventErrorSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Invalidated` alternative; see the parent type's schema contract."]
    Invalidated {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventInvalidatedBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventInvalidatedKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventInvalidatedSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Dispatch` alternative; see the parent type's schema contract."]
    Dispatch {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventDispatchBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventDispatchKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventDispatchSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Reconciled` alternative; see the parent type's schema contract."]
    Reconciled {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventReconciledBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventReconciledKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventReconciledSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Surface` alternative; see the parent type's schema contract."]
    Surface {
        #[doc = "Stable identity of one dispatch attempt; never reused after positive non-submission proof."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: ::std::boxed::Box<EventSurfaceBody>,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventSurfaceKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventSurfaceSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`SessionRebound` alternative; see the parent type's schema contract."]
    SessionRebound {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventSessionReboundBody,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventSessionReboundKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventSessionReboundSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`SessionRetired` alternative; see the parent type's schema contract."]
    SessionRetired {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventSessionRetiredBody,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventSessionRetiredKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventSessionRetiredSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Acknowledged` alternative; see the parent type's schema contract."]
    Acknowledged {
        #[doc = "Exact native dispatch attempt identity."]
        #[serde(rename = "attemptId")]
        attempt_id: Id,
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventAcknowledgedBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventAcknowledgedKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventAcknowledgedSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`Cancelled` alternative; see the parent type's schema contract."]
    Cancelled {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventCancelledBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventCancelledKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventCancelledSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`SessionRecoveryUnavailable` alternative; see the parent type's schema contract."]
    SessionRecoveryUnavailable {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventSessionRecoveryUnavailableBody,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventSessionRecoveryUnavailableKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventSessionRecoveryUnavailableSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
    #[doc = "`AcknowledgedQueuedCancelled` alternative; see the parent type's schema contract."]
    AcknowledgedQueuedCancelled {
        #[doc = "`body` member; see its generated type and parent schema."]
        body: EventAcknowledgedQueuedCancelledBody,
        #[doc = "Original command identity within the trusted namespace."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Stable unique event identifier within the namespace."]
        #[serde(rename = "eventId")]
        event_id: Id,
        #[doc = "Live provider incarnation token; rejects callbacks from previous incarnations."]
        generation: Id,
        #[doc = "Closed product record discriminator."]
        kind: EventAcknowledgedQueuedCancelledKind,
        #[doc = "Trusted storage isolation scope; not copied from model or action content."]
        namespace: Namespace,
        #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
        #[serde(rename = "schemaVersion")]
        schema_version: EventAcknowledgedQueuedCancelledSchemaVersion,
        #[doc = "Strictly increasing stable-event counter; attach cursors are exclusive."]
        sequence: Counter,
    },
}
#[doc = "Stable product contract field."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventAcknowledgedBody {
    #[doc = "Closed control acknowledgement; never a model-turn outcome."]
    pub acknowledgement: Acknowledgement,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventAcknowledgedBodyType,
}
#[doc = "Closed variant discriminator."]
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
pub enum EventAcknowledgedBodyType {
    #[serde(rename = "acknowledged")]
    #[doc = "`Acknowledged` alternative; see the parent type's schema contract."]
    Acknowledged,
}
impl ::std::fmt::Display for EventAcknowledgedBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Acknowledged => f.write_str("acknowledged"),
        }
    }
}
impl ::std::str::FromStr for EventAcknowledgedBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "acknowledged" => Ok(Self::Acknowledged),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventAcknowledgedBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventAcknowledgedBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventAcknowledgedKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventAcknowledgedKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventAcknowledgedKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventAcknowledgedKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventAcknowledgedKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Stable product contract field."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventAcknowledgedQueuedCancelledBody {
    #[doc = "`acknowledgement` member; see its generated type and parent schema."]
    pub acknowledgement: EventAcknowledgedQueuedCancelledBodyAcknowledgement,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventAcknowledgedQueuedCancelledBodyType,
}
#[doc = "Closed control acknowledgement; never a model-turn outcome."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventAcknowledgedQueuedCancelledBodyAcknowledgement {
    #[doc = "Queued prompt cancelled without a native dispatch."]
    #[serde(rename = "targetCommandId")]
    pub target_command_id: Id,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventAcknowledgedQueuedCancelledBodyAcknowledgementType,
}
#[doc = "Closed variant discriminator."]
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
pub enum EventAcknowledgedQueuedCancelledBodyAcknowledgementType {
    #[serde(rename = "queued_cancelled")]
    #[doc = "`QueuedCancelled` alternative; see the parent type's schema contract."]
    QueuedCancelled,
}
impl ::std::fmt::Display for EventAcknowledgedQueuedCancelledBodyAcknowledgementType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::QueuedCancelled => f.write_str("queued_cancelled"),
        }
    }
}
impl ::std::str::FromStr for EventAcknowledgedQueuedCancelledBodyAcknowledgementType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "queued_cancelled" => Ok(Self::QueuedCancelled),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventAcknowledgedQueuedCancelledBodyAcknowledgementType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for EventAcknowledgedQueuedCancelledBodyAcknowledgementType
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed variant discriminator."]
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
pub enum EventAcknowledgedQueuedCancelledBodyType {
    #[serde(rename = "acknowledged")]
    #[doc = "`Acknowledged` alternative; see the parent type's schema contract."]
    Acknowledged,
}
impl ::std::fmt::Display for EventAcknowledgedQueuedCancelledBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Acknowledged => f.write_str("acknowledged"),
        }
    }
}
impl ::std::str::FromStr for EventAcknowledgedQueuedCancelledBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "acknowledged" => Ok(Self::Acknowledged),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventAcknowledgedQueuedCancelledBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventAcknowledgedQueuedCancelledBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventAcknowledgedQueuedCancelledKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventAcknowledgedQueuedCancelledKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventAcknowledgedQueuedCancelledKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventAcknowledgedQueuedCancelledKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventAcknowledgedQueuedCancelledKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventAcknowledgedQueuedCancelledSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventAcknowledgedQueuedCancelledSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventAcknowledgedQueuedCancelledSchemaVersion> for i64 {
    fn from(value: EventAcknowledgedQueuedCancelledSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventAcknowledgedQueuedCancelledSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventAcknowledgedQueuedCancelledSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventAcknowledgedSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventAcknowledgedSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventAcknowledgedSchemaVersion> for i64 {
    fn from(value: EventAcknowledgedSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventAcknowledgedSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventAcknowledgedSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "cancel_dispatched variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventCancelDispatchedBody {
    #[doc = "Cancellation request transport confirmation only; does not manufacture a model terminal."]
    pub confirmation: EventCancelDispatchedBodyConfirmation,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventCancelDispatchedBodyType,
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
pub enum EventCancelDispatchedBodyConfirmation {
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
impl ::std::fmt::Display for EventCancelDispatchedBodyConfirmation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::RequestOnly => f.write_str("request_only"),
            Self::AlreadyTerminal => f.write_str("already_terminal"),
            Self::Unsupported => f.write_str("unsupported"),
        }
    }
}
impl ::std::str::FromStr for EventCancelDispatchedBodyConfirmation {
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
impl ::std::convert::TryFrom<&str> for EventCancelDispatchedBodyConfirmation {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventCancelDispatchedBodyConfirmation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed variant discriminator."]
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
pub enum EventCancelDispatchedBodyType {
    #[serde(rename = "cancel_dispatched")]
    #[doc = "`CancelDispatched` alternative; see the parent type's schema contract."]
    CancelDispatched,
}
impl ::std::fmt::Display for EventCancelDispatchedBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CancelDispatched => f.write_str("cancel_dispatched"),
        }
    }
}
impl ::std::str::FromStr for EventCancelDispatchedBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "cancel_dispatched" => Ok(Self::CancelDispatched),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventCancelDispatchedBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventCancelDispatchedBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventCancelDispatchedKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventCancelDispatchedKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventCancelDispatchedKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventCancelDispatchedKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventCancelDispatchedKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventCancelDispatchedSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventCancelDispatchedSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventCancelDispatchedSchemaVersion> for i64 {
    fn from(value: EventCancelDispatchedSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventCancelDispatchedSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventCancelDispatchedSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Stable product contract field."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventCancelledBody {
    #[doc = "Accepted local cancellation command identity."]
    #[serde(rename = "cancelledBy")]
    pub cancelled_by: Id,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventCancelledBodyType,
}
#[doc = "Closed variant discriminator."]
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
pub enum EventCancelledBodyType {
    #[serde(rename = "cancelled")]
    #[doc = "`Cancelled` alternative; see the parent type's schema contract."]
    Cancelled,
}
impl ::std::fmt::Display for EventCancelledBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}
impl ::std::str::FromStr for EventCancelledBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "cancelled" => Ok(Self::Cancelled),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventCancelledBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventCancelledBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventCancelledKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventCancelledKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventCancelledKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventCancelledKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventCancelledKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventCancelledSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventCancelledSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventCancelledSchemaVersion> for i64 {
    fn from(value: EventCancelledSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventCancelledSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventCancelledSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Immutable command accepted atomically with its receipt; never a model terminal."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventCommandAcceptedBody {
    #[doc = "Complete immutable original command, committed atomically with its receipt."]
    pub command: Command,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: EventCommandAcceptedBodyType,
}
#[doc = "Closed event discriminator."]
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
pub enum EventCommandAcceptedBodyType {
    #[serde(rename = "command_accepted")]
    #[doc = "`CommandAccepted` alternative; see the parent type's schema contract."]
    CommandAccepted,
}
impl ::std::fmt::Display for EventCommandAcceptedBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::CommandAccepted => f.write_str("command_accepted"),
        }
    }
}
impl ::std::str::FromStr for EventCommandAcceptedBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "command_accepted" => Ok(Self::CommandAccepted),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventCommandAcceptedBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventCommandAcceptedBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventCommandAcceptedKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventCommandAcceptedKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventCommandAcceptedKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventCommandAcceptedKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventCommandAcceptedKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventCommandAcceptedSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventCommandAcceptedSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventCommandAcceptedSchemaVersion> for i64 {
    fn from(value: EventCommandAcceptedSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventCommandAcceptedSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventCommandAcceptedSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventDispatchBody {
    #[doc = "Complete dispatch identity retained for replay and reconciliation."]
    pub attempt: DispatchAttempt,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: EventDispatchBodyType,
}
#[doc = "Closed event discriminator."]
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
pub enum EventDispatchBodyType {
    #[serde(rename = "dispatch")]
    #[doc = "`Dispatch` alternative; see the parent type's schema contract."]
    Dispatch,
}
impl ::std::fmt::Display for EventDispatchBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Dispatch => f.write_str("dispatch"),
        }
    }
}
impl ::std::str::FromStr for EventDispatchBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "dispatch" => Ok(Self::Dispatch),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventDispatchBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventDispatchBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventDispatchKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventDispatchKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventDispatchKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventDispatchKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventDispatchKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventDispatchSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventDispatchSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventDispatchSchemaVersion> for i64 {
    fn from(value: EventDispatchSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventDispatchSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventDispatchSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "error variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventErrorBody {
    #[doc = "Closed failure category and retry discipline."]
    pub failure: Failure,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventErrorBodyType,
}
#[doc = "Closed variant discriminator."]
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
pub enum EventErrorBodyType {
    #[serde(rename = "error")]
    #[doc = "`Error` alternative; see the parent type's schema contract."]
    Error,
}
impl ::std::fmt::Display for EventErrorBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Error => f.write_str("error"),
        }
    }
}
impl ::std::str::FromStr for EventErrorBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "error" => Ok(Self::Error),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventErrorBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventErrorBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventErrorKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventErrorKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventErrorKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventErrorKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventErrorKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventErrorSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventErrorSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventErrorSchemaVersion> for i64 {
    fn from(value: EventErrorSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventErrorSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventErrorSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "First accepted response identity, committed atomically with the receipt and Interaction."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventInteractionAnsweredBody {
    #[doc = "Single-use interaction identity within the namespace."]
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    #[doc = "Accepted response command which atomically consumed the interaction; present only when answered."]
    #[serde(rename = "responseCommandId")]
    pub response_command_id: Id,
    #[doc = "The first accepted response consumed this interaction."]
    pub status: EventInteractionAnsweredBodyStatus,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventInteractionAnsweredBodyType,
}
#[doc = "The first accepted response consumed this interaction."]
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
pub enum EventInteractionAnsweredBodyStatus {
    #[serde(rename = "answered")]
    #[doc = "`Answered` alternative; see the parent type's schema contract."]
    Answered,
}
impl ::std::fmt::Display for EventInteractionAnsweredBodyStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Answered => f.write_str("answered"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionAnsweredBodyStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "answered" => Ok(Self::Answered),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionAnsweredBodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInteractionAnsweredBodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed variant discriminator."]
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
pub enum EventInteractionAnsweredBodyType {
    #[serde(rename = "interaction")]
    #[doc = "`Interaction` alternative; see the parent type's schema contract."]
    Interaction,
}
impl ::std::fmt::Display for EventInteractionAnsweredBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Interaction => f.write_str("interaction"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionAnsweredBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "interaction" => Ok(Self::Interaction),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionAnsweredBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInteractionAnsweredBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventInteractionAnsweredKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventInteractionAnsweredKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionAnsweredKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionAnsweredKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInteractionAnsweredKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventInteractionAnsweredSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventInteractionAnsweredSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventInteractionAnsweredSchemaVersion> for i64 {
    fn from(value: EventInteractionAnsweredSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventInteractionAnsweredSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventInteractionAnsweredSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Question lifecycle transition; cannot republish or replace its request."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventInteractionExpiredUnavailableBody {
    #[doc = "Single-use interaction identity within the namespace."]
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    #[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
    pub status: EventInteractionExpiredUnavailableBodyStatus,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventInteractionExpiredUnavailableBodyType,
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
pub enum EventInteractionExpiredUnavailableBodyStatus {
    #[serde(rename = "expired")]
    #[doc = "`Expired` alternative; see the parent type's schema contract."]
    Expired,
    #[serde(rename = "unavailable")]
    #[doc = "`Unavailable` alternative; see the parent type's schema contract."]
    Unavailable,
}
impl ::std::fmt::Display for EventInteractionExpiredUnavailableBodyStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Expired => f.write_str("expired"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionExpiredUnavailableBodyStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "expired" => Ok(Self::Expired),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionExpiredUnavailableBodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for EventInteractionExpiredUnavailableBodyStatus
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed variant discriminator."]
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
pub enum EventInteractionExpiredUnavailableBodyType {
    #[serde(rename = "interaction")]
    #[doc = "`Interaction` alternative; see the parent type's schema contract."]
    Interaction,
}
impl ::std::fmt::Display for EventInteractionExpiredUnavailableBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Interaction => f.write_str("interaction"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionExpiredUnavailableBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "interaction" => Ok(Self::Interaction),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionExpiredUnavailableBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInteractionExpiredUnavailableBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventInteractionExpiredUnavailableKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventInteractionExpiredUnavailableKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionExpiredUnavailableKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionExpiredUnavailableKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInteractionExpiredUnavailableKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventInteractionExpiredUnavailableSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventInteractionExpiredUnavailableSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventInteractionExpiredUnavailableSchemaVersion> for i64 {
    fn from(value: EventInteractionExpiredUnavailableSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventInteractionExpiredUnavailableSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventInteractionExpiredUnavailableSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Initial ordinary question publication; the matching Interaction is committed atomically."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventInteractionPendingBody {
    #[doc = "A live-generation callback. Restore preserves display history but always makes the previous callback unavailable."]
    #[serde(rename = "callbackLifetime")]
    pub callback_lifetime: CallbackLifetime,
    #[doc = "Inclusive UTC epoch-millisecond deadline; later first acceptance is rejected."]
    #[serde(rename = "expiresAtMs")]
    pub expires_at_ms: Counter,
    #[doc = "Single-use interaction identity within the namespace."]
    #[serde(rename = "interactionId")]
    pub interaction_id: Id,
    #[doc = "Required for the first pending event and equal to the newly committed Interaction request; forbidden on later lifecycle events."]
    pub request: InteractionRequest,
    #[doc = "First publication of an ordinary user question."]
    pub status: EventInteractionPendingBodyStatus,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventInteractionPendingBodyType,
}
#[doc = "First publication of an ordinary user question."]
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
pub enum EventInteractionPendingBodyStatus {
    #[serde(rename = "pending")]
    #[doc = "`Pending` alternative; see the parent type's schema contract."]
    Pending,
}
impl ::std::fmt::Display for EventInteractionPendingBodyStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("pending"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionPendingBodyStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pending" => Ok(Self::Pending),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionPendingBodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInteractionPendingBodyStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed variant discriminator."]
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
pub enum EventInteractionPendingBodyType {
    #[serde(rename = "interaction")]
    #[doc = "`Interaction` alternative; see the parent type's schema contract."]
    Interaction,
}
impl ::std::fmt::Display for EventInteractionPendingBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Interaction => f.write_str("interaction"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionPendingBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "interaction" => Ok(Self::Interaction),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionPendingBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInteractionPendingBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventInteractionPendingKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventInteractionPendingKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventInteractionPendingKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInteractionPendingKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInteractionPendingKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventInteractionPendingSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventInteractionPendingSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventInteractionPendingSchemaVersion> for i64 {
    fn from(value: EventInteractionPendingSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventInteractionPendingSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventInteractionPendingSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventInvalidatedBody {
    #[doc = "Local failure without asserting a model terminal."]
    pub failure: Failure,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: EventInvalidatedBodyType,
}
#[doc = "Closed event discriminator."]
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
pub enum EventInvalidatedBodyType {
    #[serde(rename = "invalidated")]
    #[doc = "`Invalidated` alternative; see the parent type's schema contract."]
    Invalidated,
}
impl ::std::fmt::Display for EventInvalidatedBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Invalidated => f.write_str("invalidated"),
        }
    }
}
impl ::std::str::FromStr for EventInvalidatedBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "invalidated" => Ok(Self::Invalidated),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInvalidatedBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInvalidatedBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventInvalidatedKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventInvalidatedKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventInvalidatedKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventInvalidatedKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventInvalidatedKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventInvalidatedSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventInvalidatedSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventInvalidatedSchemaVersion> for i64 {
    fn from(value: EventInvalidatedSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventInvalidatedSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventInvalidatedSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventReconciledBody {
    #[doc = "Complete dispatch identity retained for replay and reconciliation."]
    pub attempt: DispatchAttempt,
    #[doc = "Provider observation bound to this attempt and its current observer."]
    pub resolution: EventReconciledBodyResolution,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: EventReconciledBodyType,
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
pub enum EventReconciledBodyResolution {
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
    #[serde(rename = "submitted")]
    #[doc = "`Submitted` alternative; see the parent type's schema contract."]
    Submitted,
    #[serde(rename = "acknowledged")]
    #[doc = "`Acknowledged` alternative; see the parent type's schema contract."]
    Acknowledged,
}
impl ::std::fmt::Display for EventReconciledBodyResolution {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Running => f.write_str("running"),
            Self::Terminal => f.write_str("terminal"),
            Self::NotSubmitted => f.write_str("not_submitted"),
            Self::Unknown => f.write_str("unknown"),
            Self::Submitted => f.write_str("submitted"),
            Self::Acknowledged => f.write_str("acknowledged"),
        }
    }
}
impl ::std::str::FromStr for EventReconciledBodyResolution {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "running" => Ok(Self::Running),
            "terminal" => Ok(Self::Terminal),
            "not_submitted" => Ok(Self::NotSubmitted),
            "unknown" => Ok(Self::Unknown),
            "submitted" => Ok(Self::Submitted),
            "acknowledged" => Ok(Self::Acknowledged),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventReconciledBodyResolution {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventReconciledBodyResolution {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed event discriminator."]
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
pub enum EventReconciledBodyType {
    #[serde(rename = "reconciled")]
    #[doc = "`Reconciled` alternative; see the parent type's schema contract."]
    Reconciled,
}
impl ::std::fmt::Display for EventReconciledBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Reconciled => f.write_str("reconciled"),
        }
    }
}
impl ::std::str::FromStr for EventReconciledBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "reconciled" => Ok(Self::Reconciled),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventReconciledBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventReconciledBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventReconciledKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventReconciledKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventReconciledKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventReconciledKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventReconciledKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventReconciledSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventReconciledSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventReconciledSchemaVersion> for i64 {
    fn from(value: EventReconciledSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventReconciledSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventReconciledSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventSessionReboundBody {
    #[doc = "Prior provider incarnation invalidated by this verified handoff."]
    #[serde(rename = "previousGeneration")]
    pub previous_generation: Id,
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: EventSessionReboundBodyType,
}
#[doc = "Closed event discriminator."]
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
pub enum EventSessionReboundBodyType {
    #[serde(rename = "session_rebound")]
    #[doc = "`SessionRebound` alternative; see the parent type's schema contract."]
    SessionRebound,
}
impl ::std::fmt::Display for EventSessionReboundBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SessionRebound => f.write_str("session_rebound"),
        }
    }
}
impl ::std::str::FromStr for EventSessionReboundBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "session_rebound" => Ok(Self::SessionRebound),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventSessionReboundBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventSessionReboundBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventSessionReboundKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventSessionReboundKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventSessionReboundKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventSessionReboundKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventSessionReboundKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventSessionReboundSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventSessionReboundSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventSessionReboundSchemaVersion> for i64 {
    fn from(value: EventSessionReboundSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventSessionReboundSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventSessionReboundSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Stable product contract field."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventSessionRecoveryUnavailableBody {
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventSessionRecoveryUnavailableBodyType,
}
#[doc = "Closed variant discriminator."]
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
pub enum EventSessionRecoveryUnavailableBodyType {
    #[serde(rename = "session_recovery_unavailable")]
    #[doc = "`SessionRecoveryUnavailable` alternative; see the parent type's schema contract."]
    SessionRecoveryUnavailable,
}
impl ::std::fmt::Display for EventSessionRecoveryUnavailableBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SessionRecoveryUnavailable => f.write_str("session_recovery_unavailable"),
        }
    }
}
impl ::std::str::FromStr for EventSessionRecoveryUnavailableBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "session_recovery_unavailable" => Ok(Self::SessionRecoveryUnavailable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventSessionRecoveryUnavailableBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventSessionRecoveryUnavailableBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventSessionRecoveryUnavailableKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventSessionRecoveryUnavailableKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventSessionRecoveryUnavailableKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventSessionRecoveryUnavailableKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventSessionRecoveryUnavailableKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventSessionRecoveryUnavailableSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventSessionRecoveryUnavailableSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventSessionRecoveryUnavailableSchemaVersion> for i64 {
    fn from(value: EventSessionRecoveryUnavailableSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventSessionRecoveryUnavailableSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventSessionRecoveryUnavailableSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Stable event data; never execution or authentication authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventSessionRetiredBody {
    #[doc = "Closed event discriminator."]
    #[serde(rename = "type")]
    pub type_: EventSessionRetiredBodyType,
}
#[doc = "Closed event discriminator."]
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
pub enum EventSessionRetiredBodyType {
    #[serde(rename = "session_retired")]
    #[doc = "`SessionRetired` alternative; see the parent type's schema contract."]
    SessionRetired,
}
impl ::std::fmt::Display for EventSessionRetiredBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SessionRetired => f.write_str("session_retired"),
        }
    }
}
impl ::std::str::FromStr for EventSessionRetiredBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "session_retired" => Ok(Self::SessionRetired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventSessionRetiredBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventSessionRetiredBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventSessionRetiredKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventSessionRetiredKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventSessionRetiredKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventSessionRetiredKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventSessionRetiredKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventSessionRetiredSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventSessionRetiredSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventSessionRetiredSchemaVersion> for i64 {
    fn from(value: EventSessionRetiredSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventSessionRetiredSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventSessionRetiredSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "status variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventStatusDispatchingRunningReconciliationRequiredBody {
    #[doc = "Closed command lifecycle projection."]
    pub state: EventStatusDispatchingRunningReconciliationRequiredBodyState,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventStatusDispatchingRunningReconciliationRequiredBodyType,
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
pub enum EventStatusDispatchingRunningReconciliationRequiredBodyState {
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
impl ::std::fmt::Display for EventStatusDispatchingRunningReconciliationRequiredBodyState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Dispatching => f.write_str("dispatching"),
            Self::Running => f.write_str("running"),
            Self::ReconciliationRequired => f.write_str("reconciliation_required"),
        }
    }
}
impl ::std::str::FromStr for EventStatusDispatchingRunningReconciliationRequiredBodyState {
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
impl ::std::convert::TryFrom<&str>
    for EventStatusDispatchingRunningReconciliationRequiredBodyState
{
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for EventStatusDispatchingRunningReconciliationRequiredBodyState
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed variant discriminator."]
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
pub enum EventStatusDispatchingRunningReconciliationRequiredBodyType {
    #[serde(rename = "status")]
    #[doc = "`Status` alternative; see the parent type's schema contract."]
    Status,
}
impl ::std::fmt::Display for EventStatusDispatchingRunningReconciliationRequiredBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Status => f.write_str("status"),
        }
    }
}
impl ::std::str::FromStr for EventStatusDispatchingRunningReconciliationRequiredBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "status" => Ok(Self::Status),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventStatusDispatchingRunningReconciliationRequiredBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for EventStatusDispatchingRunningReconciliationRequiredBodyType
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventStatusDispatchingRunningReconciliationRequiredKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventStatusDispatchingRunningReconciliationRequiredKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventStatusDispatchingRunningReconciliationRequiredKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventStatusDispatchingRunningReconciliationRequiredKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String>
    for EventStatusDispatchingRunningReconciliationRequiredKind
{
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventStatusDispatchingRunningReconciliationRequiredSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventStatusDispatchingRunningReconciliationRequiredSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventStatusDispatchingRunningReconciliationRequiredSchemaVersion>
    for i64
{
    fn from(value: EventStatusDispatchingRunningReconciliationRequiredSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64>
    for EventStatusDispatchingRunningReconciliationRequiredSchemaVersion
{
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de>
    for EventStatusDispatchingRunningReconciliationRequiredSchemaVersion
{
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Stable product contract field."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventSurfaceBody {
    #[doc = "Full surface recovery state committed with this event."]
    pub surface: SurfaceState,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventSurfaceBodyType,
}
#[doc = "Closed variant discriminator."]
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
pub enum EventSurfaceBodyType {
    #[serde(rename = "surface")]
    #[doc = "`Surface` alternative; see the parent type's schema contract."]
    Surface,
}
impl ::std::fmt::Display for EventSurfaceBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Surface => f.write_str("surface"),
        }
    }
}
impl ::std::str::FromStr for EventSurfaceBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "surface" => Ok(Self::Surface),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventSurfaceBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventSurfaceBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventSurfaceKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventSurfaceKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventSurfaceKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventSurfaceKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventSurfaceKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventSurfaceSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventSurfaceSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventSurfaceSchemaVersion> for i64 {
    fn from(value: EventSurfaceSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventSurfaceSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventSurfaceSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "terminal variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventTerminalBody {
    #[doc = "Definite model-turn result; no implication about business side effects."]
    pub outcome: Outcome,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventTerminalBodyType,
}
#[doc = "Closed variant discriminator."]
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
pub enum EventTerminalBodyType {
    #[serde(rename = "terminal")]
    #[doc = "`Terminal` alternative; see the parent type's schema contract."]
    Terminal,
}
impl ::std::fmt::Display for EventTerminalBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Terminal => f.write_str("terminal"),
        }
    }
}
impl ::std::str::FromStr for EventTerminalBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "terminal" => Ok(Self::Terminal),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventTerminalBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventTerminalBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventTerminalKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventTerminalKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventTerminalKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventTerminalKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventTerminalKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventTerminalSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventTerminalSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventTerminalSchemaVersion> for i64 {
    fn from(value: EventTerminalSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventTerminalSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventTerminalSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "text variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventTextBody {
    #[doc = "Stable product message correlation identifier."]
    #[serde(rename = "messageId")]
    pub message_id: Id,
    #[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
    pub text: EventTextBodyText,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventTextBodyType,
}
#[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct EventTextBodyText(
    #[doc = "`` member; see its generated type and parent schema."] ::std::string::String,
);
impl ::std::ops::Deref for EventTextBodyText {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<EventTextBodyText> for ::std::string::String {
    fn from(value: EventTextBodyText) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for EventTextBodyText {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() > 65536usize {
            return Err("longer than 65536 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for EventTextBodyText {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventTextBodyText {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for EventTextBodyText {
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
#[doc = "Closed variant discriminator."]
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
pub enum EventTextBodyType {
    #[serde(rename = "text")]
    #[doc = "`Text` alternative; see the parent type's schema contract."]
    Text,
}
impl ::std::fmt::Display for EventTextBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Text => f.write_str("text"),
        }
    }
}
impl ::std::str::FromStr for EventTextBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "text" => Ok(Self::Text),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventTextBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventTextBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventTextKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventTextKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventTextKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventTextKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventTextKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventTextSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventTextSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventTextSchemaVersion> for i64 {
    fn from(value: EventTextSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventTextSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventTextSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "tool_proposal variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventToolProposalBody {
    #[doc = "Untrusted tool JSON arguments, including keys, count toward product budgets."]
    pub arguments: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    #[doc = "Provider tool name; not an approved execution action."]
    pub name: Id,
    #[doc = "Untrusted tool proposal correlation identifier."]
    #[serde(rename = "proposalId")]
    pub proposal_id: Id,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventToolProposalBodyType,
}
#[doc = "Closed variant discriminator."]
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
pub enum EventToolProposalBodyType {
    #[serde(rename = "tool_proposal")]
    #[doc = "`ToolProposal` alternative; see the parent type's schema contract."]
    ToolProposal,
}
impl ::std::fmt::Display for EventToolProposalBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ToolProposal => f.write_str("tool_proposal"),
        }
    }
}
impl ::std::str::FromStr for EventToolProposalBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "tool_proposal" => Ok(Self::ToolProposal),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventToolProposalBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventToolProposalBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventToolProposalKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventToolProposalKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventToolProposalKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventToolProposalKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventToolProposalKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventToolProposalSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventToolProposalSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventToolProposalSchemaVersion> for i64 {
    fn from(value: EventToolProposalSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventToolProposalSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventToolProposalSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "tool_result variant; all fields are data, never authentication or execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EventToolResultBody {
    #[doc = "Protocol tool-result disposition, not authoritative business execution status."]
    pub disposition: EventToolResultBodyDisposition,
    #[doc = "Untrusted tool proposal correlation identifier."]
    #[serde(rename = "proposalId")]
    pub proposal_id: Id,
    #[doc = "Untrusted model/user text subject to the whole-envelope budgets."]
    pub text: EventToolResultBodyText,
    #[doc = "Closed variant discriminator."]
    #[serde(rename = "type")]
    pub type_: EventToolResultBodyType,
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
pub enum EventToolResultBodyDisposition {
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
impl ::std::fmt::Display for EventToolResultBodyDisposition {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Returned => f.write_str("returned"),
            Self::Rejected => f.write_str("rejected"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}
impl ::std::str::FromStr for EventToolResultBodyDisposition {
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
impl ::std::convert::TryFrom<&str> for EventToolResultBodyDisposition {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventToolResultBodyDisposition {
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
pub struct EventToolResultBodyText(
    #[doc = "`` member; see its generated type and parent schema."] ::std::string::String,
);
impl ::std::ops::Deref for EventToolResultBodyText {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<EventToolResultBodyText> for ::std::string::String {
    fn from(value: EventToolResultBodyText) -> Self {
        value.0
    }
}
impl ::std::str::FromStr for EventToolResultBodyText {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        if value.chars().count() > 65536usize {
            return Err("longer than 65536 characters".into());
        }
        Ok(Self(value.to_string()))
    }
}
impl ::std::convert::TryFrom<&str> for EventToolResultBodyText {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventToolResultBodyText {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl<'de> ::serde::Deserialize<'de> for EventToolResultBodyText {
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
#[doc = "Closed variant discriminator."]
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
pub enum EventToolResultBodyType {
    #[serde(rename = "tool_result")]
    #[doc = "`ToolResult` alternative; see the parent type's schema contract."]
    ToolResult,
}
impl ::std::fmt::Display for EventToolResultBodyType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ToolResult => f.write_str("tool_result"),
        }
    }
}
impl ::std::str::FromStr for EventToolResultBodyType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "tool_result" => Ok(Self::ToolResult),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventToolResultBodyType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventToolResultBodyType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum EventToolResultKind {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event,
}
impl ::std::fmt::Display for EventToolResultKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Event => f.write_str("event"),
        }
    }
}
impl ::std::str::FromStr for EventToolResultKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "event" => Ok(Self::Event),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EventToolResultKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EventToolResultKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct EventToolResultSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for EventToolResultSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<EventToolResultSchemaVersion> for i64 {
    fn from(value: EventToolResultSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for EventToolResultSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for EventToolResultSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub struct Id(
    #[doc = "`` member; see its generated type and parent schema."] ::std::string::String,
);
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
pub struct InputText(
    #[doc = "`` member; see its generated type and parent schema."] ::std::string::String,
);
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
    #[doc = "A live-generation callback. Restore preserves display history but always makes the previous callback unavailable."]
    #[serde(rename = "callbackLifetime")]
    pub callback_lifetime: CallbackLifetime,
    #[doc = "Ordinary user question only; permission and execution callbacks are forbidden in this lifecycle."]
    pub category: InteractionCategory,
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
    pub kind: InteractionKind,
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
    #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: InteractionSchemaVersion,
    #[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
    pub status: InteractionStatus,
}
#[doc = "Ordinary user question only; permission and execution callbacks are forbidden in this lifecycle."]
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
pub enum InteractionCategory {
    #[serde(rename = "question")]
    #[doc = "`Question` alternative; see the parent type's schema contract."]
    Question,
}
impl ::std::fmt::Display for InteractionCategory {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Question => f.write_str("question"),
        }
    }
}
impl ::std::str::FromStr for InteractionCategory {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "question" => Ok(Self::Question),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for InteractionCategory {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InteractionCategory {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum InteractionKind {
    #[serde(rename = "interaction")]
    #[doc = "`Interaction` alternative; see the parent type's schema contract."]
    Interaction,
}
impl ::std::fmt::Display for InteractionKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Interaction => f.write_str("interaction"),
        }
    }
}
impl ::std::str::FromStr for InteractionKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "interaction" => Ok(Self::Interaction),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for InteractionKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for InteractionKind {
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
pub struct InteractionRequest(
    #[doc = "`` member; see its generated type and parent schema."]
    pub  ::serde_json::Map<::std::string::String, ::serde_json::Value>,
);
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
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct InteractionSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for InteractionSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<InteractionSchemaVersion> for i64 {
    fn from(value: InteractionSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for InteractionSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for InteractionSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
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
#[doc = "`ListRequest`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ListRequest {
    #[doc = "Closed record discriminator."]
    pub kind: ListRequestKind,
    #[doc = "Bounded page query with an opaque caller-bound continuation."]
    pub query: PageQuery,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: ListRequestSchemaVersion,
}
#[doc = "Closed record discriminator."]
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
pub enum ListRequestKind {
    #[serde(rename = "listRequest")]
    #[doc = "`ListRequest` alternative; see the parent type's schema contract."]
    ListRequest,
}
impl ::std::fmt::Display for ListRequestKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ListRequest => f.write_str("listRequest"),
        }
    }
}
impl ::std::str::FromStr for ListRequestKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "listRequest" => Ok(Self::ListRequest),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ListRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ListRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct ListRequestSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for ListRequestSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<ListRequestSchemaVersion> for i64 {
    fn from(value: ListRequestSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for ListRequestSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for ListRequestSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
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
#[doc = "Selected product ACP extensions; capability metadata is never execution authority."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Negotiation {
    #[doc = "Explicitly selected upstream version and product catalog."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub a2ui: ::std::option::Option<A2uiNegotiation>,
    #[doc = "Exact ACP protocol version."]
    pub acp: NegotiationAcp,
    #[doc = "Exact product contract version."]
    #[serde(rename = "contractVersion")]
    pub contract_version: NegotiationContractVersion,
    #[doc = "Host supports stable snapshot-to-event attachment."]
    #[serde(rename = "cursorAttach")]
    pub cursor_attach: bool,
    #[doc = "Host supports transactional receipt semantics; memory doubles simulate this only."]
    #[serde(rename = "durableReceipts")]
    pub durable_receipts: bool,
}
#[doc = "Exact ACP protocol version."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct NegotiationAcp(#[doc = "`` member; see its generated type and parent schema."] i64);
impl ::std::ops::Deref for NegotiationAcp {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<NegotiationAcp> for i64 {
    fn from(value: NegotiationAcp) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for NegotiationAcp {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![1_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for NegotiationAcp {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Exact product contract version."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct NegotiationContractVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for NegotiationContractVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<NegotiationContractVersion> for i64 {
    fn from(value: NegotiationContractVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for NegotiationContractVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for NegotiationContractVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
    #[serde(rename = "cancelled")]
    #[doc = "`Cancelled` alternative; see the parent type's schema contract."]
    Cancelled,
    #[serde(rename = "refused")]
    #[doc = "`Refused` alternative; see the parent type's schema contract."]
    Refused,
    #[serde(rename = "max_tokens")]
    #[doc = "`MaxTokens` alternative; see the parent type's schema contract."]
    MaxTokens,
    #[serde(rename = "max_turn_requests")]
    #[doc = "`MaxTurnRequests` alternative; see the parent type's schema contract."]
    MaxTurnRequests,
    #[serde(rename = "failed")]
    #[doc = "`Failed` alternative; see the parent type's schema contract."]
    Failed,
}
impl ::std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Completed => f.write_str("completed"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Refused => f.write_str("refused"),
            Self::MaxTokens => f.write_str("max_tokens"),
            Self::MaxTurnRequests => f.write_str("max_turn_requests"),
            Self::Failed => f.write_str("failed"),
        }
    }
}
impl ::std::str::FromStr for Outcome {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "refused" => Ok(Self::Refused),
            "max_tokens" => Ok(Self::MaxTokens),
            "max_turn_requests" => Ok(Self::MaxTurnRequests),
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
#[doc = "`PageQuery`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PageQuery {
    #[doc = "Opaque continuation of one immutable read view; expires independently of the session."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub continuation: ::std::option::Option<Id>,
    #[doc = "Maximum records in this page, from 1 to 256."]
    pub limit: ::std::num::NonZeroU64,
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
    pub kind: ReceiptKind,
    #[doc = "Trusted storage isolation scope; not copied from model or action content."]
    pub namespace: Namespace,
    #[doc = "Inclusive stored-receipt deadline; cannot be shorter than the retry deadline."]
    #[serde(rename = "receiptUntilMs")]
    pub receipt_until_ms: Counter,
    #[doc = "Inclusive same-command retry deadline, no later than command expiry."]
    #[serde(rename = "retryUntilMs")]
    pub retry_until_ms: Counter,
    #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: ReceiptSchemaVersion,
}
#[doc = "SHA-256 of JCS(command), including every command field; namespace is a separate storage key."]
#[derive(:: serde :: Serialize, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ReceiptContentHash(
    #[doc = "`` member; see its generated type and parent schema."] ::std::string::String,
);
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
#[doc = "Closed product record discriminator."]
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
pub enum ReceiptKind {
    #[serde(rename = "receipt")]
    #[doc = "`Receipt` alternative; see the parent type's schema contract."]
    Receipt,
}
impl ::std::fmt::Display for ReceiptKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Receipt => f.write_str("receipt"),
        }
    }
}
impl ::std::str::FromStr for ReceiptKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "receipt" => Ok(Self::Receipt),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ReceiptKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ReceiptKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct ReceiptSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for ReceiptSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<ReceiptSchemaVersion> for i64 {
    fn from(value: ReceiptSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for ReceiptSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for ReceiptSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "`ResumeRequest`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ResumeRequest {
    #[doc = "Closed record discriminator."]
    pub kind: ResumeRequestKind,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: ResumeRequestSchemaVersion,
    #[doc = "Product session identity within the authenticated caller namespace."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
}
#[doc = "Closed record discriminator."]
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
pub enum ResumeRequestKind {
    #[serde(rename = "resumeRequest")]
    #[doc = "`ResumeRequest` alternative; see the parent type's schema contract."]
    ResumeRequest,
}
impl ::std::fmt::Display for ResumeRequestKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ResumeRequest => f.write_str("resumeRequest"),
        }
    }
}
impl ::std::str::FromStr for ResumeRequestKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "resumeRequest" => Ok(Self::ResumeRequest),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ResumeRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ResumeRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct ResumeRequestSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for ResumeRequestSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<ResumeRequestSchemaVersion> for i64 {
    fn from(value: ResumeRequestSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for ResumeRequestSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for ResumeRequestSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
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
    pub kind: SessionKind,
    #[doc = "Highest committed stable-event sequence at this session revision."]
    #[serde(rename = "lastSequence")]
    pub last_sequence: Counter,
    #[doc = "Trusted storage isolation scope; not copied from model or action content."]
    pub namespace: Namespace,
    #[doc = "Monotonic CAS revision of this product record."]
    pub revision: Counter,
    #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: SessionSchemaVersion,
    #[doc = "Explicit lifecycle state; missing native evidence cannot be inferred from transport loss."]
    pub status: SessionStatus,
}
#[doc = "Closed product record discriminator."]
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
pub enum SessionKind {
    #[serde(rename = "session")]
    #[doc = "`Session` alternative; see the parent type's schema contract."]
    Session,
}
impl ::std::fmt::Display for SessionKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Session => f.write_str("session"),
        }
    }
}
impl ::std::str::FromStr for SessionKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "session" => Ok(Self::Session),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SessionKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SessionKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SessionPage`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SessionPage {
    #[doc = "Caller-scoped sessions in this immutable page."]
    pub items: ::std::vec::Vec<Session>,
    #[doc = "Closed record discriminator."]
    pub kind: SessionPageKind,
    #[doc = "Opaque continuation; absent at end of the read view."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub next: ::std::option::Option<Id>,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: SessionPageSchemaVersion,
}
#[doc = "Closed record discriminator."]
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
pub enum SessionPageKind {
    #[serde(rename = "sessionPage")]
    #[doc = "`SessionPage` alternative; see the parent type's schema contract."]
    SessionPage,
}
impl ::std::fmt::Display for SessionPageKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SessionPage => f.write_str("sessionPage"),
        }
    }
}
impl ::std::str::FromStr for SessionPageKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "sessionPage" => Ok(Self::SessionPage),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SessionPageKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SessionPageKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct SessionPageSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for SessionPageSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<SessionPageSchemaVersion> for i64 {
    fn from(value: SessionPageSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for SessionPageSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for SessionPageSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct SessionSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for SessionSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<SessionSchemaVersion> for i64 {
    fn from(value: SessionSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for SessionSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for SessionSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
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
pub enum SessionStatus {
    #[serde(rename = "active")]
    #[doc = "`Active` alternative; see the parent type's schema contract."]
    Active,
    #[serde(rename = "recovery_required")]
    #[doc = "`RecoveryRequired` alternative; see the parent type's schema contract."]
    RecoveryRequired,
    #[serde(rename = "retired")]
    #[doc = "`Retired` alternative; see the parent type's schema contract."]
    Retired,
}
impl ::std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Active => f.write_str("active"),
            Self::RecoveryRequired => f.write_str("recovery_required"),
            Self::Retired => f.write_str("retired"),
        }
    }
}
impl ::std::str::FromStr for SessionStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "active" => Ok(Self::Active),
            "recovery_required" => Ok(Self::RecoveryRequired),
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
#[doc = "`SnapshotPage`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SnapshotPage {
    #[doc = "Command projections at the watermark."]
    pub commands: ::std::vec::Vec<CommandRecord>,
    #[doc = "Stable event watermark shared by every page."]
    pub cursor: Counter,
    #[doc = "Stable events at or below the watermark."]
    pub events: ::std::vec::Vec<Event>,
    #[doc = "Interaction display state; does not restore a native callback."]
    pub interactions: ::std::vec::Vec<Interaction>,
    #[doc = "Closed record discriminator."]
    pub kind: SnapshotPageKind,
    #[doc = "Opaque continuation; absent at end of the read view."]
    #[serde(skip_serializing_if = "::std::option::Option::is_none")]
    pub next: ::std::option::Option<Id>,
    #[doc = "Zero-based page order within this snapshot."]
    #[serde(rename = "pageIndex")]
    pub page_index: Counter,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: SnapshotPageSchemaVersion,
    #[doc = "Session at the snapshot watermark."]
    pub session: Session,
    #[doc = "Identity shared by all pages from one immutable read view."]
    #[serde(rename = "snapshotId")]
    pub snapshot_id: Id,
    #[doc = "Bounded original A2UI recovery messages and their associations."]
    pub surfaces: ::std::vec::Vec<SurfaceState>,
}
#[doc = "Closed record discriminator."]
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
pub enum SnapshotPageKind {
    #[serde(rename = "snapshotPage")]
    #[doc = "`SnapshotPage` alternative; see the parent type's schema contract."]
    SnapshotPage,
}
impl ::std::fmt::Display for SnapshotPageKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SnapshotPage => f.write_str("snapshotPage"),
        }
    }
}
impl ::std::str::FromStr for SnapshotPageKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "snapshotPage" => Ok(Self::SnapshotPage),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SnapshotPageKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SnapshotPageKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct SnapshotPageSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for SnapshotPageSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<SnapshotPageSchemaVersion> for i64 {
    fn from(value: SnapshotPageSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for SnapshotPageSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for SnapshotPageSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "`SnapshotRequest`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRequest {
    #[doc = "Closed record discriminator."]
    pub kind: SnapshotRequestKind,
    #[doc = "Bounded page query with an opaque caller-bound continuation."]
    pub query: PageQuery,
    #[doc = "Exact product contract version; no legacy readers."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: SnapshotRequestSchemaVersion,
    #[doc = "Product session identity within the authenticated caller namespace."]
    #[serde(rename = "sessionId")]
    pub session_id: Id,
}
#[doc = "Closed record discriminator."]
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
pub enum SnapshotRequestKind {
    #[serde(rename = "snapshotRequest")]
    #[doc = "`SnapshotRequest` alternative; see the parent type's schema contract."]
    SnapshotRequest,
}
impl ::std::fmt::Display for SnapshotRequestKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SnapshotRequest => f.write_str("snapshotRequest"),
        }
    }
}
impl ::std::str::FromStr for SnapshotRequestKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "snapshotRequest" => Ok(Self::SnapshotRequest),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SnapshotRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SnapshotRequestKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product contract version; no legacy readers."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct SnapshotRequestSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for SnapshotRequestSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<SnapshotRequestSchemaVersion> for i64 {
    fn from(value: SnapshotRequestSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for SnapshotRequestSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for SnapshotRequestSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "`Subscription`"]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Subscription {
    #[serde(rename = "event")]
    #[doc = "`Event` alternative; see the parent type's schema contract."]
    Event {
        #[doc = "Stable Host event."]
        event: ::std::boxed::Box<Event>,
    },
    #[serde(rename = "delta")]
    #[doc = "`Delta` alternative; see the parent type's schema contract."]
    Delta {
        #[doc = "Product command identity."]
        #[serde(rename = "commandId")]
        command_id: Id,
        #[doc = "Exact native provider incarnation."]
        generation: Id,
        #[doc = "Identity of the streamed message within a command."]
        #[serde(rename = "messageId")]
        message_id: Id,
        #[doc = "Untrusted display text."]
        text: ::std::string::String,
    },
    #[serde(rename = "resync_required")]
    #[doc = "`ResyncRequired` alternative; see the parent type's schema contract."]
    ResyncRequired,
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
    pub kind: SurfaceActionKind,
    #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
    #[serde(rename = "nativeRunId")]
    pub native_run_id: Id,
    #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: SurfaceActionSchemaVersion,
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
#[doc = "Closed product record discriminator."]
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
pub enum SurfaceActionKind {
    #[serde(rename = "surfaceAction")]
    #[doc = "`SurfaceAction` alternative; see the parent type's schema contract."]
    SurfaceAction,
}
impl ::std::fmt::Display for SurfaceActionKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SurfaceAction => f.write_str("surfaceAction"),
        }
    }
}
impl ::std::str::FromStr for SurfaceActionKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "surfaceAction" => Ok(Self::SurfaceAction),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SurfaceActionKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SurfaceActionKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct SurfaceActionSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for SurfaceActionSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<SurfaceActionSchemaVersion> for i64 {
    fn from(value: SurfaceActionSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for SurfaceActionSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for SurfaceActionSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
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
#[doc = "Single surface record: association, lifecycle and bounded upstream recovery content."]
#[derive(:: serde :: Deserialize, :: serde :: Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct SurfaceState {
    #[doc = "Exact negotiated upstream A2UI version."]
    #[serde(rename = "a2uiVersion")]
    pub a2ui_version: SurfaceStateA2uiVersion,
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
    pub kind: SurfaceStateKind,
    #[doc = "Bounded unchanged upstream messages needed to rebuild this instance; not a second A2UI schema."]
    pub messages: ::std::vec::Vec<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    #[doc = "Trusted storage isolation scope; not copied from model or action content."]
    pub namespace: Namespace,
    #[doc = "Provider-owned model-turn/run identifier, required when the provider exposes it."]
    #[serde(rename = "nativeRunId")]
    pub native_run_id: Id,
    #[doc = "Monotonic CAS revision of this product record."]
    pub revision: Counter,
    #[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
    #[serde(rename = "schemaVersion")]
    pub schema_version: SurfaceStateSchemaVersion,
    #[doc = "Exact upstream source component allowed to emit this action."]
    #[serde(rename = "sourceComponentId")]
    pub source_component_id: Id,
    #[doc = "Deleted is an upstream deletion; invalidated is a product-side loss of action authority. Neither may reactivate."]
    pub status: SurfaceStateStatus,
    #[doc = "Upstream A2UI surface identifier."]
    #[serde(rename = "surfaceId")]
    pub surface_id: Id,
    #[doc = "Fresh product identity for each surface creation; deletion permanently invalidates old actions."]
    #[serde(rename = "surfaceInstanceId")]
    pub surface_instance_id: Id,
}
#[doc = "Exact negotiated upstream A2UI version."]
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
pub enum SurfaceStateA2uiVersion {
    #[serde(rename = "v0.9.1")]
    #[doc = "`V091` alternative; see the parent type's schema contract."]
    V091,
}
impl ::std::fmt::Display for SurfaceStateA2uiVersion {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::V091 => f.write_str("v0.9.1"),
        }
    }
}
impl ::std::str::FromStr for SurfaceStateA2uiVersion {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "v0.9.1" => Ok(Self::V091),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SurfaceStateA2uiVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SurfaceStateA2uiVersion {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Closed product record discriminator."]
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
pub enum SurfaceStateKind {
    #[serde(rename = "surface")]
    #[doc = "`Surface` alternative; see the parent type's schema contract."]
    Surface,
}
impl ::std::fmt::Display for SurfaceStateKind {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Surface => f.write_str("surface"),
        }
    }
}
impl ::std::str::FromStr for SurfaceStateKind {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "surface" => Ok(Self::Surface),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SurfaceStateKind {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SurfaceStateKind {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "Exact product wire version; V1 and V2 are rejected without migration or fallback."]
#[derive(:: serde :: Serialize, Clone)]
#[serde(transparent)]
pub struct SurfaceStateSchemaVersion(
    #[doc = "`` member; see its generated type and parent schema."] i64,
);
impl ::std::ops::Deref for SurfaceStateSchemaVersion {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<SurfaceStateSchemaVersion> for i64 {
    fn from(value: SurfaceStateSchemaVersion) -> Self {
        value.0
    }
}
impl ::std::convert::TryFrom<i64> for SurfaceStateSchemaVersion {
    type Error = self::error::ConversionError;
    fn try_from(value: i64) -> ::std::result::Result<Self, self::error::ConversionError> {
        if ![3_i64].contains(&value) {
            Err("invalid value".into())
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> ::serde::Deserialize<'de> for SurfaceStateSchemaVersion {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        Self::try_from(<i64>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as ::serde::de::Error>::custom(e.to_string()))
    }
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
pub enum SurfaceStateStatus {
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
impl ::std::fmt::Display for SurfaceStateStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Active => f.write_str("active"),
            Self::Deleted => f.write_str("deleted"),
            Self::Invalidated => f.write_str("invalidated"),
        }
    }
}
impl ::std::str::FromStr for SurfaceStateStatus {
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
impl ::std::convert::TryFrom<&str> for SurfaceStateStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SurfaceStateStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
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
    #[doc = "`SurfaceState` alternative; see the parent type's schema contract."]
    SurfaceState(#[doc = "`` member; see its generated type and parent schema."] SurfaceState),
    #[doc = "`SurfaceAction` alternative; see the parent type's schema contract."]
    SurfaceAction(#[doc = "`` member; see its generated type and parent schema."] SurfaceAction),
    #[doc = "`SnapshotPage` alternative; see the parent type's schema contract."]
    SnapshotPage(#[doc = "`` member; see its generated type and parent schema."] SnapshotPage),
    #[doc = "`SessionPage` alternative; see the parent type's schema contract."]
    SessionPage(#[doc = "`` member; see its generated type and parent schema."] SessionPage),
    #[doc = "`SnapshotRequest` alternative; see the parent type's schema contract."]
    SnapshotRequest(
        #[doc = "`` member; see its generated type and parent schema."] SnapshotRequest,
    ),
    #[doc = "`ListRequest` alternative; see the parent type's schema contract."]
    ListRequest(#[doc = "`` member; see its generated type and parent schema."] ListRequest),
    #[doc = "`AttachRequest` alternative; see the parent type's schema contract."]
    AttachRequest(#[doc = "`` member; see its generated type and parent schema."] AttachRequest),
    #[doc = "`DetachRequest` alternative; see the parent type's schema contract."]
    DetachRequest(#[doc = "`` member; see its generated type and parent schema."] DetachRequest),
    #[doc = "`ResumeRequest` alternative; see the parent type's schema contract."]
    ResumeRequest(#[doc = "`` member; see its generated type and parent schema."] ResumeRequest),
    #[doc = "`ActionRequest` alternative; see the parent type's schema contract."]
    ActionRequest(#[doc = "`` member; see its generated type and parent schema."] ActionRequest),
    #[doc = "`AccessUpdate` alternative; see the parent type's schema contract."]
    AccessUpdate(#[doc = "`` member; see its generated type and parent schema."] AccessUpdate),
    #[doc = "`AttachReceipt` alternative; see the parent type's schema contract."]
    AttachReceipt(#[doc = "`` member; see its generated type and parent schema."] AttachReceipt),
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
impl ::std::convert::From<SurfaceState> for WireRecord {
    fn from(value: SurfaceState) -> Self {
        Self::SurfaceState(value)
    }
}
impl ::std::convert::From<SurfaceAction> for WireRecord {
    fn from(value: SurfaceAction) -> Self {
        Self::SurfaceAction(value)
    }
}
impl ::std::convert::From<SnapshotPage> for WireRecord {
    fn from(value: SnapshotPage) -> Self {
        Self::SnapshotPage(value)
    }
}
impl ::std::convert::From<SessionPage> for WireRecord {
    fn from(value: SessionPage) -> Self {
        Self::SessionPage(value)
    }
}
impl ::std::convert::From<SnapshotRequest> for WireRecord {
    fn from(value: SnapshotRequest) -> Self {
        Self::SnapshotRequest(value)
    }
}
impl ::std::convert::From<ListRequest> for WireRecord {
    fn from(value: ListRequest) -> Self {
        Self::ListRequest(value)
    }
}
impl ::std::convert::From<AttachRequest> for WireRecord {
    fn from(value: AttachRequest) -> Self {
        Self::AttachRequest(value)
    }
}
impl ::std::convert::From<DetachRequest> for WireRecord {
    fn from(value: DetachRequest) -> Self {
        Self::DetachRequest(value)
    }
}
impl ::std::convert::From<ResumeRequest> for WireRecord {
    fn from(value: ResumeRequest) -> Self {
        Self::ResumeRequest(value)
    }
}
impl ::std::convert::From<ActionRequest> for WireRecord {
    fn from(value: ActionRequest) -> Self {
        Self::ActionRequest(value)
    }
}
impl ::std::convert::From<AccessUpdate> for WireRecord {
    fn from(value: AccessUpdate) -> Self {
        Self::AccessUpdate(value)
    }
}
impl ::std::convert::From<AttachReceipt> for WireRecord {
    fn from(value: AttachReceipt) -> Self {
        Self::AttachReceipt(value)
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
impl std::fmt::Debug for A2uiNegotiation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(A2uiNegotiation), "([redacted])"))
    }
}
impl std::fmt::Debug for A2uiNegotiationCatalogId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(A2uiNegotiationCatalogId),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for A2uiNegotiationCatalogVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(A2uiNegotiationCatalogVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for A2uiNegotiationVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(A2uiNegotiationVersion), "([redacted])"))
    }
}
impl std::fmt::Debug for AccessUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(AccessUpdate), "([redacted])"))
    }
}
impl std::fmt::Debug for AccessUpdateKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(AccessUpdateKind), "([redacted])"))
    }
}
impl std::fmt::Debug for AccessUpdateSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(AccessUpdateSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for Acknowledgement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Acknowledgement), "([redacted])"))
    }
}
impl std::fmt::Debug for AcknowledgementConfirmation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(AcknowledgementConfirmation),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for ActionRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ActionRequest), "([redacted])"))
    }
}
impl std::fmt::Debug for ActionRequestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ActionRequestKind), "([redacted])"))
    }
}
impl std::fmt::Debug for ActionRequestSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(ActionRequestSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for AttachReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(AttachReceipt), "([redacted])"))
    }
}
impl std::fmt::Debug for AttachReceiptKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(AttachReceiptKind), "([redacted])"))
    }
}
impl std::fmt::Debug for AttachReceiptSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(AttachReceiptSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for AttachRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(AttachRequest), "([redacted])"))
    }
}
impl std::fmt::Debug for AttachRequestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(AttachRequestKind), "([redacted])"))
    }
}
impl std::fmt::Debug for AttachRequestSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(AttachRequestSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Binding), "([redacted])"))
    }
}
impl std::fmt::Debug for CallbackLifetime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(CallbackLifetime), "([redacted])"))
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
impl std::fmt::Debug for CommandKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(CommandKind), "([redacted])"))
    }
}
impl std::fmt::Debug for CommandRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(CommandRecord), "([redacted])"))
    }
}
impl std::fmt::Debug for CommandRecordVariant0Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant0Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant0SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant0SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant0State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant0State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant1Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant1Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant1SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant1SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant1State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant1State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant2Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant2Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant2SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant2SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant2State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant2State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant3Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant3Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant3SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant3SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant3State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant3State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant4Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant4Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant4SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant4SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant4State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant4State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant5Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant5Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant5SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant5SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant5State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant5State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant6Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant6Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant6SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant6SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant6State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant6State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant7Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant7Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant7SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant7SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant7State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant7State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant8Acknowledgement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant8Acknowledgement),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant8AcknowledgementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant8AcknowledgementType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant8Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant8Kind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant8SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant8SchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandRecordVariant8State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(CommandRecordVariant8State),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for CommandSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(CommandSchemaVersion), "([redacted])"))
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
impl std::fmt::Debug for DeliveryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DeliveryKind), "([redacted])"))
    }
}
impl std::fmt::Debug for DeliveryRetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DeliveryRetry), "([redacted])"))
    }
}
impl std::fmt::Debug for DeliverySchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DeliverySchemaVersion), "([redacted])"))
    }
}
impl std::fmt::Debug for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DeliveryStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for DetachRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DetachRequest), "([redacted])"))
    }
}
impl std::fmt::Debug for DetachRequestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(DetachRequestKind), "([redacted])"))
    }
}
impl std::fmt::Debug for DetachRequestSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(DetachRequestSchemaVersion),
            "([redacted])"
        ))
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
impl std::fmt::Debug for EventAcknowledgedBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventAcknowledgedBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventAcknowledgedBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventAcknowledgedBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventAcknowledgedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventAcknowledgedKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventAcknowledgedQueuedCancelledBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventAcknowledgedQueuedCancelledBody),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventAcknowledgedQueuedCancelledBodyAcknowledgement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventAcknowledgedQueuedCancelledBodyAcknowledgement),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventAcknowledgedQueuedCancelledBodyAcknowledgementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventAcknowledgedQueuedCancelledBodyAcknowledgementType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventAcknowledgedQueuedCancelledBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventAcknowledgedQueuedCancelledBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventAcknowledgedQueuedCancelledKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventAcknowledgedQueuedCancelledKind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventAcknowledgedQueuedCancelledSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventAcknowledgedQueuedCancelledSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventAcknowledgedSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventAcknowledgedSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCancelDispatchedBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCancelDispatchedBody),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCancelDispatchedBodyConfirmation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCancelDispatchedBodyConfirmation),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCancelDispatchedBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCancelDispatchedBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCancelDispatchedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCancelDispatchedKind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCancelDispatchedSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCancelDispatchedSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCancelledBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventCancelledBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventCancelledBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventCancelledBodyType), "([redacted])"))
    }
}
impl std::fmt::Debug for EventCancelledKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventCancelledKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventCancelledSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCancelledSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCommandAcceptedBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCommandAcceptedBody),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCommandAcceptedBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCommandAcceptedBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCommandAcceptedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCommandAcceptedKind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventCommandAcceptedSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventCommandAcceptedSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventDispatchBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventDispatchBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventDispatchBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventDispatchBodyType), "([redacted])"))
    }
}
impl std::fmt::Debug for EventDispatchKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventDispatchKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventDispatchSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventDispatchSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventErrorBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventErrorBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventErrorBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventErrorBodyType), "([redacted])"))
    }
}
impl std::fmt::Debug for EventErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventErrorKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventErrorSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventErrorSchemaVersion), "([redacted])"))
    }
}
impl std::fmt::Debug for EventInteractionAnsweredBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionAnsweredBody),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionAnsweredBodyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionAnsweredBodyStatus),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionAnsweredBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionAnsweredBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionAnsweredKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionAnsweredKind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionAnsweredSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionAnsweredSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionExpiredUnavailableBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionExpiredUnavailableBody),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionExpiredUnavailableBodyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionExpiredUnavailableBodyStatus),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionExpiredUnavailableBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionExpiredUnavailableBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionExpiredUnavailableKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionExpiredUnavailableKind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionExpiredUnavailableSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionExpiredUnavailableSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionPendingBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionPendingBody),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionPendingBodyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionPendingBodyStatus),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionPendingBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionPendingBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionPendingKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionPendingKind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInteractionPendingSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInteractionPendingSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInvalidatedBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventInvalidatedBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventInvalidatedBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInvalidatedBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventInvalidatedKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventInvalidatedKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventInvalidatedSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventInvalidatedSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventReconciledBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventReconciledBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventReconciledBodyResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventReconciledBodyResolution),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventReconciledBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventReconciledBodyType), "([redacted])"))
    }
}
impl std::fmt::Debug for EventReconciledKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventReconciledKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventReconciledSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventReconciledSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSessionReboundBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventSessionReboundBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventSessionReboundBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSessionReboundBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSessionReboundKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventSessionReboundKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventSessionReboundSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSessionReboundSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSessionRecoveryUnavailableBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSessionRecoveryUnavailableBody),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSessionRecoveryUnavailableBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSessionRecoveryUnavailableBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSessionRecoveryUnavailableKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSessionRecoveryUnavailableKind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSessionRecoveryUnavailableSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSessionRecoveryUnavailableSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSessionRetiredBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventSessionRetiredBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventSessionRetiredBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSessionRetiredBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSessionRetiredKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventSessionRetiredKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventSessionRetiredSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSessionRetiredSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventStatusDispatchingRunningReconciliationRequiredBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventStatusDispatchingRunningReconciliationRequiredBody),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventStatusDispatchingRunningReconciliationRequiredBodyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventStatusDispatchingRunningReconciliationRequiredBodyState),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventStatusDispatchingRunningReconciliationRequiredBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventStatusDispatchingRunningReconciliationRequiredBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventStatusDispatchingRunningReconciliationRequiredKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventStatusDispatchingRunningReconciliationRequiredKind),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventStatusDispatchingRunningReconciliationRequiredSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventStatusDispatchingRunningReconciliationRequiredSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventSurfaceBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventSurfaceBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventSurfaceBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventSurfaceBodyType), "([redacted])"))
    }
}
impl std::fmt::Debug for EventSurfaceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventSurfaceKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventSurfaceSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventSurfaceSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventTerminalBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventTerminalBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventTerminalBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventTerminalBodyType), "([redacted])"))
    }
}
impl std::fmt::Debug for EventTerminalKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventTerminalKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventTerminalSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventTerminalSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventTextBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventTextBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventTextBodyText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventTextBodyText), "([redacted])"))
    }
}
impl std::fmt::Debug for EventTextBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventTextBodyType), "([redacted])"))
    }
}
impl std::fmt::Debug for EventTextKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventTextKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventTextSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventTextSchemaVersion), "([redacted])"))
    }
}
impl std::fmt::Debug for EventToolProposalBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventToolProposalBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventToolProposalBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventToolProposalBodyType),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventToolProposalKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventToolProposalKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventToolProposalSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventToolProposalSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventToolResultBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventToolResultBody), "([redacted])"))
    }
}
impl std::fmt::Debug for EventToolResultBodyDisposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventToolResultBodyDisposition),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for EventToolResultBodyText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventToolResultBodyText), "([redacted])"))
    }
}
impl std::fmt::Debug for EventToolResultBodyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventToolResultBodyType), "([redacted])"))
    }
}
impl std::fmt::Debug for EventToolResultKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(EventToolResultKind), "([redacted])"))
    }
}
impl std::fmt::Debug for EventToolResultSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(EventToolResultSchemaVersion),
            "([redacted])"
        ))
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
impl std::fmt::Debug for InteractionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(InteractionCategory), "([redacted])"))
    }
}
impl std::fmt::Debug for InteractionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(InteractionKind), "([redacted])"))
    }
}
impl std::fmt::Debug for InteractionRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(InteractionRequest), "([redacted])"))
    }
}
impl std::fmt::Debug for InteractionSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(InteractionSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for InteractionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(InteractionStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for ListRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ListRequest), "([redacted])"))
    }
}
impl std::fmt::Debug for ListRequestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ListRequestKind), "([redacted])"))
    }
}
impl std::fmt::Debug for ListRequestSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(ListRequestSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for Namespace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Namespace), "([redacted])"))
    }
}
impl std::fmt::Debug for Negotiation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Negotiation), "([redacted])"))
    }
}
impl std::fmt::Debug for NegotiationAcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(NegotiationAcp), "([redacted])"))
    }
}
impl std::fmt::Debug for NegotiationContractVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(NegotiationContractVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Outcome), "([redacted])"))
    }
}
impl std::fmt::Debug for PageQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(PageQuery), "([redacted])"))
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
impl std::fmt::Debug for ReceiptKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ReceiptKind), "([redacted])"))
    }
}
impl std::fmt::Debug for ReceiptSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ReceiptSchemaVersion), "([redacted])"))
    }
}
impl std::fmt::Debug for ResumeRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ResumeRequest), "([redacted])"))
    }
}
impl std::fmt::Debug for ResumeRequestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(ResumeRequestKind), "([redacted])"))
    }
}
impl std::fmt::Debug for ResumeRequestSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(ResumeRequestSchemaVersion),
            "([redacted])"
        ))
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
impl std::fmt::Debug for SessionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SessionKind), "([redacted])"))
    }
}
impl std::fmt::Debug for SessionPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SessionPage), "([redacted])"))
    }
}
impl std::fmt::Debug for SessionPageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SessionPageKind), "([redacted])"))
    }
}
impl std::fmt::Debug for SessionPageSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(SessionPageSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for SessionSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SessionSchemaVersion), "([redacted])"))
    }
}
impl std::fmt::Debug for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SessionStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for SnapshotPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SnapshotPage), "([redacted])"))
    }
}
impl std::fmt::Debug for SnapshotPageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SnapshotPageKind), "([redacted])"))
    }
}
impl std::fmt::Debug for SnapshotPageSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(SnapshotPageSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for SnapshotRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SnapshotRequest), "([redacted])"))
    }
}
impl std::fmt::Debug for SnapshotRequestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SnapshotRequestKind), "([redacted])"))
    }
}
impl std::fmt::Debug for SnapshotRequestSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(SnapshotRequestSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(Subscription), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceAction), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceActionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceActionKind), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceActionSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(SurfaceActionSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for SurfaceReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceReference), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceState), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceStateA2uiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceStateA2uiVersion), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceStateKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceStateKind), "([redacted])"))
    }
}
impl std::fmt::Debug for SurfaceStateSchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(
            stringify!(SurfaceStateSchemaVersion),
            "([redacted])"
        ))
    }
}
impl std::fmt::Debug for SurfaceStateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(SurfaceStateStatus), "([redacted])"))
    }
}
impl std::fmt::Debug for WireRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(concat!(stringify!(WireRecord), "([redacted])"))
    }
}
