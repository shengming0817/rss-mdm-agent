// @generated from the Agent HTTP API schemas. Do not edit.
use super::runtime::{
    BindingProfile, Capability, SoftwareExecutionContext, TaskArchitecture, TaskPlatform,
};
#[doc = "`ActivationReceipt`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ActivationReceipt {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "issuanceId")]
    pub issuance_id: ::uuid::Uuid,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    pub runtime: BindingProfile,
}
#[doc = "`ActivationRequest`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ActivationRequest {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "issuanceId")]
    pub issuance_id: ::uuid::Uuid,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
}
#[doc = "`AuthorizationRequest`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationRequest {
    pub grant: GrantReceipt,
    #[serde(rename = "publicKeyFingerprint")]
    pub public_key_fingerprint: ::std::string::String,
}
#[doc = "`Candidate`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "issuanceId")]
    pub issuance_id: ::uuid::Uuid,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    #[serde(rename = "certificateChain")]
    pub certificate_chain: ::std::vec::Vec<::std::string::String>,
    #[serde(rename = "activateBefore")]
    pub activate_before: i64,
}
#[doc = "`CertificateRequest`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CertificateRequest {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    #[serde(rename = "grantId")]
    pub grant_id: ::uuid::Uuid,
    pub secret: crate::Secret,
    pub csr: ::std::string::String,
    pub runtime: RuntimeProfile,
}
#[doc = "`ErrorBody`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ErrorBody {
    pub code: ErrorCode,
    #[serde(
        rename = "issuanceId",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub issuance_id: ::std::option::Option<::uuid::Uuid>,
}
#[doc = "`ErrorCode`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    schemars :: JsonSchema,
)]
pub enum ErrorCode {
    #[serde(rename = "malformed_request")]
    MalformedRequest,
    #[serde(rename = "invalid_identity")]
    InvalidIdentity,
    #[serde(rename = "permission_denied")]
    PermissionDenied,
    #[serde(rename = "grant_revoked")]
    GrantRevoked,
    #[serde(rename = "grant_expired")]
    GrantExpired,
    #[serde(rename = "registration_limit")]
    RegistrationLimit,
    #[serde(rename = "operation_conflict")]
    OperationConflict,
    #[serde(rename = "issuance_unknown")]
    IssuanceUnknown,
    #[serde(rename = "activation_unknown")]
    ActivationUnknown,
    #[serde(rename = "credential_revoked")]
    CredentialRevoked,
    #[serde(rename = "certificate_expired")]
    CertificateExpired,
    #[serde(rename = "credential_retired")]
    CredentialRetired,
    #[serde(rename = "service_unavailable")]
    ServiceUnavailable,
}
impl ::std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::MalformedRequest => f.write_str("malformed_request"),
            Self::InvalidIdentity => f.write_str("invalid_identity"),
            Self::PermissionDenied => f.write_str("permission_denied"),
            Self::GrantRevoked => f.write_str("grant_revoked"),
            Self::GrantExpired => f.write_str("grant_expired"),
            Self::RegistrationLimit => f.write_str("registration_limit"),
            Self::OperationConflict => f.write_str("operation_conflict"),
            Self::IssuanceUnknown => f.write_str("issuance_unknown"),
            Self::ActivationUnknown => f.write_str("activation_unknown"),
            Self::CredentialRevoked => f.write_str("credential_revoked"),
            Self::CertificateExpired => f.write_str("certificate_expired"),
            Self::CredentialRetired => f.write_str("credential_retired"),
            Self::ServiceUnavailable => f.write_str("service_unavailable"),
        }
    }
}
impl ::std::str::FromStr for ErrorCode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "malformed_request" => Ok(Self::MalformedRequest),
            "invalid_identity" => Ok(Self::InvalidIdentity),
            "permission_denied" => Ok(Self::PermissionDenied),
            "grant_revoked" => Ok(Self::GrantRevoked),
            "grant_expired" => Ok(Self::GrantExpired),
            "registration_limit" => Ok(Self::RegistrationLimit),
            "operation_conflict" => Ok(Self::OperationConflict),
            "issuance_unknown" => Ok(Self::IssuanceUnknown),
            "activation_unknown" => Ok(Self::ActivationUnknown),
            "credential_revoked" => Ok(Self::CredentialRevoked),
            "certificate_expired" => Ok(Self::CertificateExpired),
            "credential_retired" => Ok(Self::CredentialRetired),
            "service_unavailable" => Ok(Self::ServiceUnavailable),
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
#[doc = "`FieldKey`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    schemars :: JsonSchema,
)]
#[serde(transparent)]
pub struct FieldKey(pub ::std::string::String);
impl ::std::ops::Deref for FieldKey {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<FieldKey> for ::std::string::String {
    fn from(value: FieldKey) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for FieldKey {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for FieldKey {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for FieldKey {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`GrantReceipt`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct GrantReceipt {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "grantId")]
    pub grant_id: ::uuid::Uuid,
    pub state: GrantState,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
    pub platform: TaskPlatform,
}
#[doc = "`GrantRequest`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct GrantRequest {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    pub secret: crate::Secret,
    pub platform: TaskPlatform,
    #[serde(rename = "lifetimeSeconds")]
    pub lifetime_seconds: ::std::option::Option<u32>,
    #[serde()]
    pub csr: ::std::option::Option<::std::string::String>,
}
#[doc = "`GrantState`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    schemars :: JsonSchema,
)]
pub enum GrantState {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "available")]
    Available,
    #[serde(rename = "consumed")]
    Consumed,
    #[serde(rename = "revoked")]
    Revoked,
    #[serde(rename = "expired")]
    Expired,
}
impl ::std::fmt::Display for GrantState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("pending"),
            Self::Available => f.write_str("available"),
            Self::Consumed => f.write_str("consumed"),
            Self::Revoked => f.write_str("revoked"),
            Self::Expired => f.write_str("expired"),
        }
    }
}
impl ::std::str::FromStr for GrantState {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pending" => Ok(Self::Pending),
            "available" => Ok(Self::Available),
            "consumed" => Ok(Self::Consumed),
            "revoked" => Ok(Self::Revoked),
            "expired" => Ok(Self::Expired),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for GrantState {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for GrantState {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`NativeGrantRequest`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct NativeGrantRequest {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    #[serde(rename = "installationOperation")]
    pub installation_operation: ::uuid::Uuid,
    pub secret: crate::Secret,
    pub csr: ::std::string::String,
    pub runtime: RuntimeProfile,
}
#[doc = "`Platform`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    schemars :: JsonSchema,
)]
pub enum Platform {
    #[serde(rename = "windows")]
    Windows,
    #[serde(rename = "macos")]
    Macos,
}
impl ::std::fmt::Display for Platform {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Windows => f.write_str("windows"),
            Self::Macos => f.write_str("macos"),
        }
    }
}
impl ::std::str::FromStr for Platform {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "windows" => Ok(Self::Windows),
            "macos" => Ok(Self::Macos),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Platform {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Platform {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Progress`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub grant: GrantReceipt,
    #[serde(
        rename = "issuanceId",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub issuance_id: ::std::option::Option<::uuid::Uuid>,
    #[serde(rename = "issuanceUnknown")]
    pub issuance_unknown: bool,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub activation: ::std::option::Option<ActivationReceipt>,
}
#[doc = "`RecoverIssuance`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RecoverIssuance {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    #[serde(rename = "certificateChain")]
    pub certificate_chain: ::std::vec::Vec<::std::string::String>,
}
#[doc = "`RenewalStatusRequest`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RenewalStatusRequest {
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    pub secret: crate::Secret,
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
}
#[doc = "`ReportSource`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    schemars :: JsonSchema,
)]
pub enum ReportSource {
    #[serde(rename = "agent.builtin")]
    AgentBuiltin,
}
impl ::std::fmt::Display for ReportSource {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::AgentBuiltin => f.write_str("agent.builtin"),
        }
    }
}
impl ::std::str::FromStr for ReportSource {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "agent.builtin" => Ok(Self::AgentBuiltin),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ReportSource {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ReportSource {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`RuntimeProfile`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProfile {
    pub capabilities: ::std::vec::Vec<Capability>,
    pub platform: TaskPlatform,
    pub architecture: TaskArchitecture,
    #[serde(rename = "executionContext")]
    pub execution_context: SoftwareExecutionContext,
}
#[doc = "`Secret`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    schemars :: JsonSchema,
)]
#[serde(transparent)]
pub struct Secret(pub ::std::string::String);
impl ::std::ops::Deref for Secret {
    type Target = ::std::string::String;
    fn deref(&self) -> &::std::string::String {
        &self.0
    }
}
impl ::std::convert::From<Secret> for ::std::string::String {
    fn from(value: Secret) -> Self {
        value.0
    }
}
impl ::std::convert::From<::std::string::String> for Secret {
    fn from(value: ::std::string::String) -> Self {
        Self(value)
    }
}
impl ::std::fmt::Display for Secret {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
impl ::std::str::FromStr for Secret {
    type Err = ::std::convert::Infallible;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.to_string()))
    }
}
#[doc = "`Source`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    schemars :: JsonSchema,
)]
pub enum Source {
    #[serde(rename = "agent.script")]
    AgentScript,
    #[serde(rename = "agent.osquery")]
    AgentOsquery,
    #[serde(rename = "mdm.apple")]
    MdmApple,
    #[serde(rename = "agent.builtin")]
    AgentBuiltin,
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "mdm.windows")]
    MdmWindows,
}
impl ::std::fmt::Display for Source {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::AgentScript => f.write_str("agent.script"),
            Self::AgentOsquery => f.write_str("agent.osquery"),
            Self::MdmApple => f.write_str("mdm.apple"),
            Self::AgentBuiltin => f.write_str("agent.builtin"),
            Self::Manual => f.write_str("manual"),
            Self::MdmWindows => f.write_str("mdm.windows"),
        }
    }
}
impl ::std::str::FromStr for Source {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "agent.script" => Ok(Self::AgentScript),
            "agent.osquery" => Ok(Self::AgentOsquery),
            "mdm.apple" => Ok(Self::MdmApple),
            "agent.builtin" => Ok(Self::AgentBuiltin),
            "manual" => Ok(Self::Manual),
            "mdm.windows" => Ok(Self::MdmWindows),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Source {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Source {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`StatusRequest`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct StatusRequest {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "grantId")]
    pub grant_id: ::uuid::Uuid,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    pub secret: crate::Secret,
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
pub const API_VERSION: u8 = 1;
pub const SCHEMA_FINGERPRINT: &str =
    "89d7c13e7689a5cbe385e8bf1e1345a140b789ea5f6e2e3943f8042e29992d3b";
pub const SCHEMA_MANIFEST: &str = include_str!("../schema/enroll/manifest.json");
