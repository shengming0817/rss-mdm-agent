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
    "8992983a33e04828771dcea61d508ef8590de145317a3e3f2ef5d3708eed27d5";
pub const SCHEMA_MANIFEST: &str = include_str!("../schema/enroll/manifest.json");
