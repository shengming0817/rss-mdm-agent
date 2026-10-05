// @generated from the Agent HTTP API schemas. Do not edit.
#[doc = "`BindingProfile`"]
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
pub struct BindingProfile {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    #[serde(rename = "deviceId")]
    pub device_id: ::std::string::String,
    #[serde(rename = "registrationId")]
    pub registration_id: ::uuid::Uuid,
    pub generation: u64,
    pub source: ReportSource,
    pub epoch: ::uuid::Uuid,
    pub capabilities: ::std::vec::Vec<Capability>,
    pub collections: ::std::vec::Vec<CollectionDefinition>,
}
#[doc = "`Capability`"]
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
pub enum Capability {
    #[serde(rename = "inventory.collect.v1")]
    InventoryCollectionV1,
    #[serde(rename = "task.execute.v1")]
    TaskExecuteV1,
    #[serde(rename = "software.msi.system.v1")]
    SoftwareMsiSystemV1,
    #[serde(rename = "software.msi.user.v1")]
    SoftwareMsiUserV1,
    #[serde(rename = "software.pkg.system.v1")]
    SoftwarePkgSystemV1,
    #[serde(rename = "software.bundle.windows.system.v1")]
    SoftwareBundleWindowsSystemV1,
    #[serde(rename = "software.bundle.windows.user.v1")]
    SoftwareBundleWindowsUserV1,
    #[serde(rename = "software.bundle.macos.system.v1")]
    SoftwareBundleMacosSystemV1,
    #[serde(rename = "software.bundle.macos.user.v1")]
    SoftwareBundleMacosUserV1,
    #[serde(rename = "software.winget.system.v1")]
    SoftwareWingetSystemV1,
    #[serde(rename = "software.winget.user.v1")]
    SoftwareWingetUserV1,
    #[serde(rename = "software.brew.bottle.user.v1")]
    SoftwareBrewBottleUserV1,
    #[serde(rename = "software.exe.system.v1")]
    SoftwareExeSystemV1,
    #[serde(rename = "software.exe.user.v1")]
    SoftwareExeUserV1,
    #[serde(rename = "software.dmg.app.system.v1")]
    SoftwareDmgAppSystemV1,
    #[serde(rename = "software.dmg.app.user.v1")]
    SoftwareDmgAppUserV1,
    #[serde(rename = "software.dmg.pkg.system.v1")]
    SoftwareDmgPkgSystemV1,
    #[serde(rename = "software.msix.registration.user.v1")]
    SoftwareMsixRegistrationUserV1,
    #[serde(rename = "software.msix.provisioning.system.v1")]
    SoftwareMsixProvisioningSystemV1,
    #[serde(rename = "mdm.enrollment.v1")]
    MdmEnrollmentV1,
}
impl ::std::fmt::Display for Capability {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::InventoryCollectionV1 => f.write_str("inventory.collect.v1"),
            Self::TaskExecuteV1 => f.write_str("task.execute.v1"),
            Self::SoftwareMsiSystemV1 => f.write_str("software.msi.system.v1"),
            Self::SoftwareMsiUserV1 => f.write_str("software.msi.user.v1"),
            Self::SoftwarePkgSystemV1 => f.write_str("software.pkg.system.v1"),
            Self::SoftwareBundleWindowsSystemV1 => f.write_str("software.bundle.windows.system.v1"),
            Self::SoftwareBundleWindowsUserV1 => f.write_str("software.bundle.windows.user.v1"),
            Self::SoftwareBundleMacosSystemV1 => f.write_str("software.bundle.macos.system.v1"),
            Self::SoftwareBundleMacosUserV1 => f.write_str("software.bundle.macos.user.v1"),
            Self::SoftwareWingetSystemV1 => f.write_str("software.winget.system.v1"),
            Self::SoftwareWingetUserV1 => f.write_str("software.winget.user.v1"),
            Self::SoftwareBrewBottleUserV1 => f.write_str("software.brew.bottle.user.v1"),
            Self::SoftwareExeSystemV1 => f.write_str("software.exe.system.v1"),
            Self::SoftwareExeUserV1 => f.write_str("software.exe.user.v1"),
            Self::SoftwareDmgAppSystemV1 => f.write_str("software.dmg.app.system.v1"),
            Self::SoftwareDmgAppUserV1 => f.write_str("software.dmg.app.user.v1"),
            Self::SoftwareDmgPkgSystemV1 => f.write_str("software.dmg.pkg.system.v1"),
            Self::SoftwareMsixRegistrationUserV1 => {
                f.write_str("software.msix.registration.user.v1")
            }
            Self::SoftwareMsixProvisioningSystemV1 => {
                f.write_str("software.msix.provisioning.system.v1")
            }
            Self::MdmEnrollmentV1 => f.write_str("mdm.enrollment.v1"),
        }
    }
}
impl ::std::str::FromStr for Capability {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "inventory.collect.v1" => Ok(Self::InventoryCollectionV1),
            "task.execute.v1" => Ok(Self::TaskExecuteV1),
            "software.msi.system.v1" => Ok(Self::SoftwareMsiSystemV1),
            "software.msi.user.v1" => Ok(Self::SoftwareMsiUserV1),
            "software.pkg.system.v1" => Ok(Self::SoftwarePkgSystemV1),
            "software.bundle.windows.system.v1" => Ok(Self::SoftwareBundleWindowsSystemV1),
            "software.bundle.windows.user.v1" => Ok(Self::SoftwareBundleWindowsUserV1),
            "software.bundle.macos.system.v1" => Ok(Self::SoftwareBundleMacosSystemV1),
            "software.bundle.macos.user.v1" => Ok(Self::SoftwareBundleMacosUserV1),
            "software.winget.system.v1" => Ok(Self::SoftwareWingetSystemV1),
            "software.winget.user.v1" => Ok(Self::SoftwareWingetUserV1),
            "software.brew.bottle.user.v1" => Ok(Self::SoftwareBrewBottleUserV1),
            "software.exe.system.v1" => Ok(Self::SoftwareExeSystemV1),
            "software.exe.user.v1" => Ok(Self::SoftwareExeUserV1),
            "software.dmg.app.system.v1" => Ok(Self::SoftwareDmgAppSystemV1),
            "software.dmg.app.user.v1" => Ok(Self::SoftwareDmgAppUserV1),
            "software.dmg.pkg.system.v1" => Ok(Self::SoftwareDmgPkgSystemV1),
            "software.msix.registration.user.v1" => Ok(Self::SoftwareMsixRegistrationUserV1),
            "software.msix.provisioning.system.v1" => Ok(Self::SoftwareMsixProvisioningSystemV1),
            "mdm.enrollment.v1" => Ok(Self::MdmEnrollmentV1),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Capability {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Capability {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ChunkedTaskResult`"]
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
pub struct ChunkedTaskResult {
    #[serde(
        rename = "exitCode",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub exit_code: ::std::option::Option<i32>,
    pub quality: OutputQuality,
    pub diagnostics: TaskDiagnostics,
    pub manifest: OutputManifest,
    #[serde(rename = "collectedAt")]
    pub collected_at: i64,
}
#[doc = "`CollectedValue`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", content = "value")]
pub enum CollectedValue {
    #[serde(rename = "value")]
    Value(Scalar),
    #[serde(rename = "null")]
    Null,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "unsupported")]
    Unsupported,
}
impl ::std::convert::From<Scalar> for CollectedValue {
    fn from(value: Scalar) -> Self {
        Self::Value(value)
    }
}
#[doc = "`CollectionDefinition`"]
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
pub struct CollectionDefinition {
    pub dataset: ::std::string::String,
    pub version: ::std::string::String,
    pub source: Source,
    pub fields: ::std::vec::Vec<FieldDefinition>,
}
#[doc = "`EnrollmentEntry`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum EnrollmentEntry {
    #[serde(rename = "windows")]
    Windows { server: ::std::string::String },
    #[serde(rename = "macos")]
    Macos { url: ::std::string::String },
}
#[doc = "`EnrollmentEntryOutcome`"]
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
pub enum EnrollmentEntryOutcome {
    #[serde(rename = "opened")]
    Opened,
    #[serde(rename = "user_required")]
    UserRequired,
    #[serde(rename = "third_party_conflict")]
    ThirdPartyConflict,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::fmt::Display for EnrollmentEntryOutcome {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Opened => f.write_str("opened"),
            Self::UserRequired => f.write_str("user_required"),
            Self::ThirdPartyConflict => f.write_str("third_party_conflict"),
            Self::Unsupported => f.write_str("unsupported"),
            Self::Failed => f.write_str("failed"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for EnrollmentEntryOutcome {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "opened" => Ok(Self::Opened),
            "user_required" => Ok(Self::UserRequired),
            "third_party_conflict" => Ok(Self::ThirdPartyConflict),
            "unsupported" => Ok(Self::Unsupported),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for EnrollmentEntryOutcome {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for EnrollmentEntryOutcome {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`EnrollmentTaskSpec`"]
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
pub struct EnrollmentTaskSpec {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "tenantId")]
    pub tenant_id: ::uuid::Uuid,
    #[serde(rename = "deviceId")]
    pub device_id: ::std::string::String,
    pub platform: TaskPlatform,
    pub architecture: TaskArchitecture,
    #[serde(rename = "registrationId")]
    pub registration_id: ::uuid::Uuid,
    pub generation: u64,
    #[serde(rename = "taskId")]
    pub task_id: ::uuid::Uuid,
    #[serde(rename = "attemptId")]
    pub attempt_id: ::uuid::Uuid,
    pub permit: TaskPermit,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
    pub organization: ::uuid::Uuid,
    pub entry: EnrollmentEntry,
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
    #[serde(rename = "unsupported_wire")]
    UnsupportedWire,
    #[serde(rename = "unsupported_capability")]
    UnsupportedCapability,
    #[serde(rename = "invalid_identity")]
    InvalidIdentity,
    #[serde(rename = "credential_revoked")]
    CredentialRevoked,
    #[serde(rename = "certificate_expired")]
    CertificateExpired,
    #[serde(rename = "credential_retired")]
    CredentialRetired,
    #[serde(rename = "report_not_found")]
    ReportNotFound,
    #[serde(rename = "permission_denied")]
    PermissionDenied,
    #[serde(rename = "task_not_found")]
    TaskNotFound,
    #[serde(rename = "range_not_satisfiable")]
    RangeNotSatisfiable,
    #[serde(rename = "operation_conflict")]
    OperationConflict,
    #[serde(rename = "operation_unknown")]
    OperationUnknown,
    #[serde(rename = "service_unavailable")]
    ServiceUnavailable,
}
impl ::std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::MalformedRequest => f.write_str("malformed_request"),
            Self::UnsupportedWire => f.write_str("unsupported_wire"),
            Self::UnsupportedCapability => f.write_str("unsupported_capability"),
            Self::InvalidIdentity => f.write_str("invalid_identity"),
            Self::CredentialRevoked => f.write_str("credential_revoked"),
            Self::CertificateExpired => f.write_str("certificate_expired"),
            Self::CredentialRetired => f.write_str("credential_retired"),
            Self::ReportNotFound => f.write_str("report_not_found"),
            Self::PermissionDenied => f.write_str("permission_denied"),
            Self::TaskNotFound => f.write_str("task_not_found"),
            Self::RangeNotSatisfiable => f.write_str("range_not_satisfiable"),
            Self::OperationConflict => f.write_str("operation_conflict"),
            Self::OperationUnknown => f.write_str("operation_unknown"),
            Self::ServiceUnavailable => f.write_str("service_unavailable"),
        }
    }
}
impl ::std::str::FromStr for ErrorCode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "malformed_request" => Ok(Self::MalformedRequest),
            "unsupported_wire" => Ok(Self::UnsupportedWire),
            "unsupported_capability" => Ok(Self::UnsupportedCapability),
            "invalid_identity" => Ok(Self::InvalidIdentity),
            "credential_revoked" => Ok(Self::CredentialRevoked),
            "certificate_expired" => Ok(Self::CertificateExpired),
            "credential_retired" => Ok(Self::CredentialRetired),
            "report_not_found" => Ok(Self::ReportNotFound),
            "permission_denied" => Ok(Self::PermissionDenied),
            "task_not_found" => Ok(Self::TaskNotFound),
            "range_not_satisfiable" => Ok(Self::RangeNotSatisfiable),
            "operation_conflict" => Ok(Self::OperationConflict),
            "operation_unknown" => Ok(Self::OperationUnknown),
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
#[doc = "`ExecutionIdentity`"]
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
pub enum ExecutionIdentity {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "logged_in_user")]
    LoggedInUser,
}
impl ::std::fmt::Display for ExecutionIdentity {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::System => f.write_str("system"),
            Self::LoggedInUser => f.write_str("logged_in_user"),
        }
    }
}
impl ::std::str::FromStr for ExecutionIdentity {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "system" => Ok(Self::System),
            "logged_in_user" => Ok(Self::LoggedInUser),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ExecutionIdentity {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ExecutionIdentity {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ExecutorProfile`"]
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
pub enum ExecutorProfile {
    #[serde(rename = "power_shell7")]
    PowerShell7,
    #[serde(rename = "posix_sh")]
    PosixSh,
    #[serde(rename = "bash")]
    Bash,
    #[serde(rename = "osquery")]
    Osquery,
}
impl ::std::fmt::Display for ExecutorProfile {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::PowerShell7 => f.write_str("power_shell7"),
            Self::PosixSh => f.write_str("posix_sh"),
            Self::Bash => f.write_str("bash"),
            Self::Osquery => f.write_str("osquery"),
        }
    }
}
impl ::std::str::FromStr for ExecutorProfile {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "power_shell7" => Ok(Self::PowerShell7),
            "posix_sh" => Ok(Self::PosixSh),
            "bash" => Ok(Self::Bash),
            "osquery" => Ok(Self::Osquery),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ExecutorProfile {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ExecutorProfile {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`FailureCode`"]
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
pub enum FailureCode {
    #[serde(rename = "permissionDenied")]
    PermissionDenied,
    #[serde(rename = "temporarilyUnavailable")]
    TemporarilyUnavailable,
    #[serde(rename = "collectionFailed")]
    CollectionFailed,
}
impl ::std::fmt::Display for FailureCode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::PermissionDenied => f.write_str("permissionDenied"),
            Self::TemporarilyUnavailable => f.write_str("temporarilyUnavailable"),
            Self::CollectionFailed => f.write_str("collectionFailed"),
        }
    }
}
impl ::std::str::FromStr for FailureCode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "permissionDenied" => Ok(Self::PermissionDenied),
            "temporarilyUnavailable" => Ok(Self::TemporarilyUnavailable),
            "collectionFailed" => Ok(Self::CollectionFailed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for FailureCode {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for FailureCode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`FieldDefinition`"]
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
pub struct FieldDefinition {
    pub key: FieldKey,
    pub version: u64,
    #[serde(rename = "valueType")]
    pub value_type: ValueType,
    pub nullable: bool,
    pub manual: bool,
    pub sources: ::std::collections::BTreeMap<::std::string::String, u16>,
    pub platforms: ::std::collections::BTreeSet<Platform>,
    pub sensitivity: Sensitivity,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub unit: ::std::option::Option<::std::string::String>,
    pub searchable: bool,
    #[serde(
        rename = "itemKey",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub item_key: ::std::option::Option<::std::string::String>,
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
#[doc = "`FieldValue`"]
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
pub struct FieldValue {
    pub field: FieldKey,
    pub value: CollectedValue,
}
#[doc = "`IntakeStatus`"]
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
pub enum IntakeStatus {
    #[serde(rename = "durable")]
    Durable,
}
impl ::std::fmt::Display for IntakeStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Durable => f.write_str("durable"),
        }
    }
}
impl ::std::str::FromStr for IntakeStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "durable" => Ok(Self::Durable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for IntakeStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for IntakeStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ObservationStatus`"]
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
pub enum ObservationStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "snapshot")]
    Snapshot,
    #[serde(rename = "needSnapshotPartial")]
    NeedSnapshotPartial,
    #[serde(rename = "needSnapshotCollectionFailed")]
    NeedSnapshotCollectionFailed,
    #[serde(rename = "stale")]
    Stale,
}
impl ::std::fmt::Display for ObservationStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("pending"),
            Self::Snapshot => f.write_str("snapshot"),
            Self::NeedSnapshotPartial => f.write_str("needSnapshotPartial"),
            Self::NeedSnapshotCollectionFailed => f.write_str("needSnapshotCollectionFailed"),
            Self::Stale => f.write_str("stale"),
        }
    }
}
impl ::std::str::FromStr for ObservationStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pending" => Ok(Self::Pending),
            "snapshot" => Ok(Self::Snapshot),
            "needSnapshotPartial" => Ok(Self::NeedSnapshotPartial),
            "needSnapshotCollectionFailed" => Ok(Self::NeedSnapshotCollectionFailed),
            "stale" => Ok(Self::Stale),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ObservationStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ObservationStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`OutputChunk`"]
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
pub struct OutputChunk {
    pub manifest: OutputManifest,
    pub index: u16,
    pub data: ::std::string::String,
}
#[doc = "`OutputManifest`"]
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
pub struct OutputManifest {
    pub bytes: u32,
    pub sha256: [u8; 32usize],
}
#[doc = "`OutputQuality`"]
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
pub enum OutputQuality {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "partial")]
    Partial,
    #[serde(rename = "truncated")]
    Truncated,
    #[serde(rename = "failed")]
    Failed,
}
impl ::std::fmt::Display for OutputQuality {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Complete => f.write_str("complete"),
            Self::Partial => f.write_str("partial"),
            Self::Truncated => f.write_str("truncated"),
            Self::Failed => f.write_str("failed"),
        }
    }
}
impl ::std::str::FromStr for OutputQuality {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "complete" => Ok(Self::Complete),
            "partial" => Ok(Self::Partial),
            "truncated" => Ok(Self::Truncated),
            "failed" => Ok(Self::Failed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for OutputQuality {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for OutputQuality {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
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
#[doc = "`ProjectionStatus`"]
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
pub enum ProjectionStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "applied")]
    Applied,
    #[serde(rename = "notApplicable")]
    NotApplicable,
}
impl ::std::fmt::Display for ProjectionStatus {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Pending => f.write_str("pending"),
            Self::Applied => f.write_str("applied"),
            Self::NotApplicable => f.write_str("notApplicable"),
        }
    }
}
impl ::std::str::FromStr for ProjectionStatus {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "pending" => Ok(Self::Pending),
            "applied" => Ok(Self::Applied),
            "notApplicable" => Ok(Self::NotApplicable),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ProjectionStatus {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ProjectionStatus {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`ReportAck`"]
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
pub struct ReportAck {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "reportId")]
    pub report_id: ::uuid::Uuid,
    #[serde(rename = "receivedAt")]
    pub received_at: i64,
    pub intake: IntakeStatus,
}
#[doc = "`ReportBody`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ReportBody {
    #[serde(rename = "snapshot")]
    Snapshot { values: ::std::vec::Vec<FieldValue> },
    #[serde(rename = "partial")]
    Partial { values: ::std::vec::Vec<FieldValue> },
    #[serde(rename = "failed")]
    Failed { code: FailureCode },
}
#[doc = "`ReportRequest`"]
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
pub struct ReportRequest {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "reportId")]
    pub report_id: ::uuid::Uuid,
    pub sequence: u64,
    #[serde(rename = "observedAt")]
    pub observed_at: i64,
    pub collection: CollectionDefinition,
    pub body: ReportBody,
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
#[doc = "`ReportStatus`"]
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
pub struct ReportStatus {
    pub ack: ReportAck,
    pub observation: ObservationStatus,
    pub projection: ProjectionStatus,
}
#[doc = "`RunCancellation`"]
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
pub enum RunCancellation {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "requested")]
    Requested,
    #[serde(rename = "confirmed")]
    Confirmed,
}
impl ::std::fmt::Display for RunCancellation {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::None => f.write_str("none"),
            Self::Requested => f.write_str("requested"),
            Self::Confirmed => f.write_str("confirmed"),
        }
    }
}
impl ::std::str::FromStr for RunCancellation {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "none" => Ok(Self::None),
            "requested" => Ok(Self::Requested),
            "confirmed" => Ok(Self::Confirmed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for RunCancellation {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RunCancellation {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`Scalar`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", content = "value")]
pub enum Scalar {
    #[serde(rename = "string")]
    String(::std::string::String),
    #[serde(rename = "integer")]
    Integer(i64),
    #[serde(rename = "number")]
    Number(crate::Number),
    #[serde(rename = "array")]
    Array(::std::vec::Vec<Scalar>),
    #[serde(rename = "object")]
    Object(::std::collections::BTreeMap<::std::string::String, Scalar>),
    #[serde(rename = "boolean")]
    Boolean(bool),
    #[serde(rename = "time")]
    Time(i64),
}
impl ::std::convert::From<crate::Number> for Scalar {
    fn from(value: crate::Number) -> Self {
        Self::Number(value)
    }
}
impl ::std::convert::From<::std::vec::Vec<Scalar>> for Scalar {
    fn from(value: ::std::vec::Vec<Scalar>) -> Self {
        Self::Array(value)
    }
}
impl ::std::convert::From<::std::collections::BTreeMap<::std::string::String, Scalar>> for Scalar {
    fn from(value: ::std::collections::BTreeMap<::std::string::String, Scalar>) -> Self {
        Self::Object(value)
    }
}
impl ::std::convert::From<bool> for Scalar {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
#[doc = "`ScriptOutputReference`"]
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
pub struct ScriptOutputReference {
    pub bytes: u32,
    pub sha256: ::std::string::String,
}
#[doc = "`ScriptParameterSource`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", content = "value")]
pub enum ScriptParameterSource {
    #[serde(rename = "fixed")]
    Fixed(::serde_json::Value),
    #[serde(rename = "input")]
    Input,
}
impl ::std::convert::From<::serde_json::Value> for ScriptParameterSource {
    fn from(value: ::serde_json::Value) -> Self {
        Self::Fixed(value)
    }
}
#[doc = "`ScriptRunResult`"]
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
pub struct ScriptRunResult {
    #[serde(rename = "collectedAt")]
    pub collected_at: i64,
    #[serde(rename = "receivedAt")]
    pub received_at: i64,
    #[serde(
        rename = "exitCode",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub exit_code: ::std::option::Option<i32>,
    pub quality: OutputQuality,
    #[serde(rename = "schemaValid")]
    pub schema_valid: bool,
    #[serde(rename = "budgetValid")]
    pub budget_valid: bool,
    pub output: ::serde_json::Value,
    #[serde(
        rename = "outputReference",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub output_reference: ::std::option::Option<ScriptOutputReference>,
    pub diagnostics: TaskDiagnostics,
    pub trusted: bool,
}
#[doc = "`SelfServiceAccepted`"]
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
pub struct SelfServiceAccepted {
    #[serde(rename = "requestId")]
    pub request_id: ::uuid::Uuid,
    #[serde(rename = "runId")]
    pub run_id: ::uuid::Uuid,
    #[serde(rename = "policyId")]
    pub policy_id: ::uuid::Uuid,
    #[serde(rename = "policyRevision")]
    pub policy_revision: u64,
    #[serde(rename = "policyVersion")]
    pub policy_version: ::uuid::Uuid,
}
#[doc = "`SelfServiceCaller`"]
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
pub enum SelfServiceCaller {
    #[serde(rename = "human")]
    Human,
    #[serde(rename = "ai")]
    Ai,
}
impl ::std::fmt::Display for SelfServiceCaller {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Human => f.write_str("human"),
            Self::Ai => f.write_str("ai"),
        }
    }
}
impl ::std::str::FromStr for SelfServiceCaller {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "human" => Ok(Self::Human),
            "ai" => Ok(Self::Ai),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SelfServiceCaller {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SelfServiceCaller {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SelfServiceDirectory`"]
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
pub struct SelfServiceDirectory {
    pub source: SelfServiceCaller,
    #[serde()]
    pub after: ::std::option::Option<::uuid::Uuid>,
}
#[doc = "`SelfServiceDirectoryItem`"]
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
pub struct SelfServiceDirectoryItem {
    #[serde(rename = "policyId")]
    pub policy_id: ::uuid::Uuid,
    #[serde(rename = "policyRevision")]
    pub policy_revision: u64,
    #[serde(rename = "policyVersion")]
    pub policy_version: ::uuid::Uuid,
    #[serde(rename = "selfService")]
    pub self_service: SelfServiceMetadata,
    pub task: SelfServiceTask,
}
#[doc = "`SelfServiceDirectoryResponse`"]
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
pub struct SelfServiceDirectoryResponse {
    pub items: ::std::vec::Vec<SelfServiceDirectoryItem>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: ::std::option::Option<::uuid::Uuid>,
    #[serde(rename = "asOf")]
    pub as_of: i64,
}
#[doc = "`SelfServiceInvoke`"]
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
pub struct SelfServiceInvoke {
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    #[serde(rename = "policyId")]
    pub policy_id: ::uuid::Uuid,
    #[serde(rename = "policyRevision")]
    pub policy_revision: u64,
    #[serde(rename = "policyVersion")]
    pub policy_version: ::uuid::Uuid,
    pub source: SelfServiceCaller,
    pub confirmed: bool,
    pub parameters: ::std::collections::BTreeMap<::std::string::String, ::serde_json::Value>,
}
#[doc = "`SelfServiceMetadata`"]
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
pub struct SelfServiceMetadata {
    pub published: bool,
    #[serde(rename = "displayName")]
    pub display_name: ::std::string::String,
    pub description: ::std::string::String,
    pub prerequisites: ::std::string::String,
    #[serde(rename = "sideEffects")]
    pub side_effects: ::std::string::String,
    pub category: ::std::string::String,
    pub keywords: ::std::vec::Vec<::std::string::String>,
    #[serde(rename = "allowAi")]
    pub allow_ai: bool,
    #[serde(rename = "riskLevel")]
    pub risk_level: u8,
}
#[doc = "`SelfServiceResult`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SelfServiceResult {
    #[serde(rename = "script")]
    Script { evidence: ScriptRunResult },
    #[serde(rename = "software")]
    Software { effect: SelfServiceSoftwareEffect },
}
#[doc = "`SelfServiceSoftwareEffect`"]
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
pub enum SelfServiceSoftwareEffect {
    #[serde(rename = "verified")]
    Verified,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "waiting_reboot")]
    WaitingReboot,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::fmt::Display for SelfServiceSoftwareEffect {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Verified => f.write_str("verified"),
            Self::Failed => f.write_str("failed"),
            Self::WaitingReboot => f.write_str("waiting_reboot"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for SelfServiceSoftwareEffect {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "verified" => Ok(Self::Verified),
            "failed" => Ok(Self::Failed),
            "waiting_reboot" => Ok(Self::WaitingReboot),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SelfServiceSoftwareEffect {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SelfServiceSoftwareEffect {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SelfServiceStatus`"]
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
pub struct SelfServiceStatus {
    #[serde(rename = "requestId")]
    pub request_id: ::uuid::Uuid,
    #[serde(rename = "runId")]
    pub run_id: ::uuid::Uuid,
    #[serde(rename = "attemptId")]
    pub attempt_id: ::std::option::Option<::uuid::Uuid>,
    pub state: TaskRunState,
    #[serde(rename = "availableAt")]
    pub available_at: i64,
    pub deadline: i64,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub result: ::std::option::Option<SelfServiceResult>,
}
#[doc = "`SelfServiceSubject`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", content = "user")]
pub enum SelfServiceSubject {
    #[serde(rename = "device")]
    Device,
    #[serde(rename = "user")]
    User(SelfServiceUser),
}
impl ::std::convert::From<SelfServiceUser> for SelfServiceSubject {
    fn from(value: SelfServiceUser) -> Self {
        Self::User(value)
    }
}
#[doc = "`SelfServiceTask`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SelfServiceTask {
    #[serde(rename = "script")]
    Script {
        resource: TaskResource,
        #[serde(rename = "runAs")]
        run_as: ExecutionIdentity,
        #[serde(rename = "parameterSources")]
        parameter_sources:
            ::std::collections::BTreeMap<::std::string::String, ScriptParameterSource>,
        #[serde(rename = "parameterSchema")]
        parameter_schema: ::serde_json::Value,
    },
    #[serde(rename = "software")]
    Software {
        resource: TaskResource,
        #[serde(rename = "runAs")]
        run_as: ExecutionIdentity,
        target: SoftwareExecutionTarget,
    },
}
#[doc = "`SelfServiceUser`"]
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
pub struct SelfServiceUser {
    #[serde(rename = "tenantId")]
    pub tenant_id: ::uuid::Uuid,
    #[serde(rename = "instanceId")]
    pub instance_id: ::uuid::Uuid,
    #[serde(rename = "principalId")]
    pub principal_id: ::uuid::Uuid,
}
#[doc = "`Sensitivity`"]
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
pub enum Sensitivity {
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "sensitive")]
    Sensitive,
}
impl ::std::fmt::Display for Sensitivity {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Standard => f.write_str("standard"),
            Self::Sensitive => f.write_str("sensitive"),
        }
    }
}
impl ::std::str::FromStr for Sensitivity {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "standard" => Ok(Self::Standard),
            "sensitive" => Ok(Self::Sensitive),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Sensitivity {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Sensitivity {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SignedTask`"]
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
pub struct SignedTask {
    pub payload: TaskPayload,
    #[serde(rename = "keyId")]
    pub key_id: ::std::string::String,
    pub signature: ::std::string::String,
}
#[doc = "`SoftwareDetectionObservation`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "state", deny_unknown_fields)]
pub enum SoftwareDetectionObservation {
    #[serde(rename = "present")]
    Present {
        version: ::std::string::String,
        #[serde(rename = "evidenceSha256")]
        evidence_sha256: [u8; 32usize],
        #[serde(rename = "observedAt")]
        observed_at: i64,
    },
    #[serde(rename = "absent")]
    Absent {
        #[serde(rename = "evidenceSha256")]
        evidence_sha256: [u8; 32usize],
        #[serde(rename = "observedAt")]
        observed_at: i64,
    },
    #[serde(rename = "unknown")]
    Unknown { diagnostic: ::std::string::String },
}
#[doc = "`SoftwareExecutionContext`"]
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
pub struct SoftwareExecutionContext {
    pub revision: u64,
    #[serde(rename = "msixSideload")]
    pub msix_sideload: bool,
    #[serde(rename = "msixUnsigned")]
    pub msix_unsigned: bool,
    #[serde(rename = "osVersion")]
    pub os_version: [u16; 4usize],
    #[serde(rename = "systemBroker")]
    pub system_broker: bool,
    #[serde(rename = "userHelper")]
    pub user_helper: bool,
    #[serde(
        rename = "interactiveUser",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub interactive_user: ::std::option::Option<SoftwareInteractiveUser>,
    #[serde(rename = "sourceCredentials")]
    pub source_credentials: ::std::vec::Vec<SoftwareReadCredential>,
}
#[doc = "`SoftwareExecutionTarget`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SoftwareExecutionTarget {
    #[serde(rename = "device")]
    Device,
    #[serde(rename = "user")]
    User {
        identity: ::std::string::String,
        #[serde(rename = "sessionId")]
        session_id: ::uuid::Uuid,
    },
}
#[doc = "`SoftwareExportArtifact`"]
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
pub struct SoftwareExportArtifact {
    pub url: ::std::string::String,
    pub length: u64,
    pub sha256: [u8; 32usize],
}
#[doc = "`SoftwareExportBinding`"]
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
pub struct SoftwareExportBinding {
    pub source: ::std::string::String,
    #[serde(rename = "tenantId")]
    pub tenant_id: ::uuid::Uuid,
    pub ring: SoftwareExportRing,
    pub publication: [u8; 32usize],
    #[serde(rename = "sourceDigest")]
    pub source_digest: [u8; 32usize],
    pub resource: ::std::string::String,
    #[serde(rename = "resourceVersion")]
    pub resource_version: ::std::string::String,
    #[serde(rename = "resourceDigest")]
    pub resource_digest: [u8; 32usize],
    #[serde(rename = "definitionDigest")]
    pub definition_digest: [u8; 32usize],
    #[serde(rename = "documentSha256")]
    pub document_sha256: [u8; 32usize],
    pub dependencies: ::std::vec::Vec<SoftwareExportDependency>,
    pub artifacts: ::std::vec::Vec<SoftwareExportArtifact>,
}
#[doc = "`SoftwareExportDependency`"]
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
pub struct SoftwareExportDependency {
    pub resource: ::std::string::String,
    pub version: ::std::string::String,
    pub sha256: [u8; 32usize],
}
#[doc = "`SoftwareExportRing`"]
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
pub enum SoftwareExportRing {
    #[serde(rename = "test")]
    Test,
    #[serde(rename = "pilot")]
    Pilot,
    #[serde(rename = "production")]
    Production,
}
impl ::std::fmt::Display for SoftwareExportRing {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Test => f.write_str("test"),
            Self::Pilot => f.write_str("pilot"),
            Self::Production => f.write_str("production"),
        }
    }
}
impl ::std::str::FromStr for SoftwareExportRing {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "test" => Ok(Self::Test),
            "pilot" => Ok(Self::Pilot),
            "production" => Ok(Self::Production),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareExportRing {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareExportRing {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareInteractiveUser`"]
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
pub struct SoftwareInteractiveUser {
    pub identity: ::std::string::String,
    #[serde(rename = "sessionId")]
    pub session_id: ::uuid::Uuid,
    pub administrator: bool,
}
#[doc = "`SoftwareObservedIdentity`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SoftwareObservedIdentity {
    #[serde(rename = "msi_product")]
    MsiProduct {
        #[serde(rename = "productCode")]
        product_code: ::std::string::String,
    },
    #[serde(rename = "pkg_receipt")]
    PkgReceipt { receipt: ::std::string::String },
    #[serde(rename = "mac_application")]
    MacApplication {
        #[serde(rename = "bundleId")]
        bundle_id: ::std::string::String,
        #[serde(rename = "targetName")]
        target_name: ::std::string::String,
    },
    #[serde(rename = "registry")]
    Registry {
        scope: SoftwareTaskScope,
        key: ::std::string::String,
        value: ::std::string::String,
    },
    #[serde(rename = "file")]
    File {
        scope: SoftwareTaskScope,
        path: ::std::string::String,
    },
    #[serde(rename = "controlled_detector")]
    ControlledDetector { entry: ::std::string::String },
    #[serde(rename = "msix_registration")]
    MsixRegistration { identity: SoftwareTaskMsixIdentity },
    #[serde(rename = "msix_provisioning")]
    MsixProvisioning { identity: SoftwareTaskMsixIdentity },
}
#[doc = "`SoftwareProcessObservation`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SoftwareProcessObservation {
    #[serde(rename = "not_run")]
    NotRun,
    #[serde(rename = "exited")]
    Exited { code: i32 },
    #[serde(rename = "failed")]
    Failed { failure: TaskFailure },
}
#[doc = "`SoftwareReadCredential`"]
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
pub struct SoftwareReadCredential {
    pub source: ::std::string::String,
    pub reference: ::std::string::String,
}
#[doc = "`SoftwareStartMode`"]
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
pub enum SoftwareStartMode {
    #[serde(rename = "automatic")]
    Automatic,
    #[serde(rename = "user_initiated")]
    UserInitiated,
}
impl ::std::fmt::Display for SoftwareStartMode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Automatic => f.write_str("automatic"),
            Self::UserInitiated => f.write_str("user_initiated"),
        }
    }
}
impl ::std::str::FromStr for SoftwareStartMode {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "automatic" => Ok(Self::Automatic),
            "user_initiated" => Ok(Self::UserInitiated),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareStartMode {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareStartMode {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareStepResult`"]
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
pub struct SoftwareStepResult {
    pub index: u8,
    #[serde(rename = "stepDigest")]
    pub step_digest: [u8; 32usize],
    pub target: SoftwareExecutionTarget,
    pub package: ::std::string::String,
    pub identity: SoftwareObservedIdentity,
    pub before: SoftwareDetectionObservation,
    pub after: SoftwareDetectionObservation,
    pub process: SoftwareProcessObservation,
    #[serde(rename = "rebootRequired")]
    pub reboot_required: bool,
    pub diagnostics: TaskDiagnostics,
}
#[doc = "`SoftwareTaskAction`"]
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
pub struct SoftwareTaskAction {
    pub package: ::std::string::String,
    pub version: ::std::string::String,
    pub behavior: SoftwareTaskBehavior,
    pub signatures: ::std::vec::Vec<SoftwareTaskSignature>,
    pub reboot: SoftwareTaskReboot,
    pub downgrade: SoftwareTaskDowngrade,
}
#[doc = "`SoftwareTaskArtifact`"]
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
pub struct SoftwareTaskArtifact {
    pub key: ::std::string::String,
    pub length: u64,
    pub sha256: [u8; 32usize],
}
#[doc = "`SoftwareTaskBehavior`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SoftwareTaskBehavior {
    #[serde(rename = "msi")]
    Msi(SoftwareTaskNative),
    #[serde(rename = "pkg")]
    Pkg(SoftwareTaskNative),
    #[serde(rename = "bundle")]
    Bundle(SoftwareTaskBundleBehavior),
    #[serde(rename = "winget")]
    Winget(SoftwareTaskNative),
    #[serde(rename = "brew")]
    Brew(SoftwareTaskNative),
    #[serde(rename = "exe")]
    Exe(SoftwareTaskExe),
    #[serde(rename = "dmg")]
    Dmg(SoftwareTaskDmg),
    #[serde(rename = "msix")]
    Msix(SoftwareTaskMsix),
}
#[doc = "`SoftwareTaskBundle`"]
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
pub struct SoftwareTaskBundle {
    pub schema: u32,
    pub platform: TaskPlatform,
    pub architecture: TaskArchitecture,
    pub entries: ::std::collections::BTreeMap<::std::string::String, SoftwareTaskBundleEntry>,
}
#[doc = "`SoftwareTaskBundleBehavior`"]
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
pub struct SoftwareTaskBundleBehavior {
    pub archive: ::std::string::String,
    pub manifest: SoftwareTaskBundle,
    pub install: SoftwareTaskScript,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub uninstall: ::std::option::Option<SoftwareTaskScript>,
    pub detect: SoftwareTaskDetection,
}
#[doc = "`SoftwareTaskBundleEntry`"]
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
pub struct SoftwareTaskBundleEntry {
    pub length: u64,
    pub sha256: [u8; 32usize],
}
#[doc = "`SoftwareTaskDetection`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SoftwareTaskDetection {
    #[serde(rename = "msi_product")]
    MsiProduct {
        #[serde(rename = "productCode")]
        product_code: ::std::string::String,
        version: ::std::string::String,
    },
    #[serde(rename = "pkg_receipt")]
    PkgReceipt {
        receipt: ::std::string::String,
        version: ::std::string::String,
    },
    #[serde(rename = "registry")]
    Registry {
        scope: SoftwareTaskScope,
        key: ::std::string::String,
        value: ::std::string::String,
        version: ::std::string::String,
    },
    #[serde(rename = "file")]
    File {
        scope: SoftwareTaskScope,
        path: ::std::string::String,
        version: ::std::string::String,
        sha256: [u8; 32usize],
    },
    #[serde(rename = "script")]
    Script { command: SoftwareTaskScript },
}
#[doc = "`SoftwareTaskDmg`"]
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
pub struct SoftwareTaskDmg {
    pub image: ::std::string::String,
    pub volume: ::std::string::String,
    pub scope: SoftwareTaskScope,
    pub invocation: SoftwareTaskInvocation,
    pub upgrade: SoftwareTaskUpgrade,
    pub payload: SoftwareTaskDmgPayload,
}
#[doc = "`SoftwareTaskDmgPayload`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SoftwareTaskDmgPayload {
    #[serde(rename = "app_copy")]
    AppCopy {
        application: SoftwareTaskMacApplication,
        uninstall: bool,
    },
    #[serde(rename = "contained_pkg")]
    ContainedPkg {
        path: ::std::string::String,
        length: u64,
        sha256: [u8; 32usize],
        receipt: ::std::string::String,
        #[serde(deserialize_with = "::std::option::Option::deserialize")]
        uninstall: ::std::option::Option<SoftwareTaskRemoval>,
    },
}
#[doc = "`SoftwareTaskDowngrade`"]
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
pub enum SoftwareTaskDowngrade {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "allow")]
    Allow,
}
impl ::std::fmt::Display for SoftwareTaskDowngrade {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Deny => f.write_str("deny"),
            Self::Allow => f.write_str("allow"),
        }
    }
}
impl ::std::str::FromStr for SoftwareTaskDowngrade {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "deny" => Ok(Self::Deny),
            "allow" => Ok(Self::Allow),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareTaskDowngrade {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareTaskDowngrade {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareTaskExe`"]
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
pub struct SoftwareTaskExe {
    pub installer: ::std::string::String,
    pub scope: SoftwareTaskScope,
    pub install: SoftwareTaskInvocation,
    #[serde(rename = "upgradeInvocation")]
    pub upgrade_invocation: SoftwareTaskInvocation,
    pub upgrade: SoftwareTaskUpgrade,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub uninstall: ::std::option::Option<SoftwareTaskRemoval>,
    pub layout: ::std::collections::BTreeMap<::std::string::String, ::std::string::String>,
    pub detect: SoftwareTaskDetection,
}
#[doc = "`SoftwareTaskExitCodes`"]
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
pub struct SoftwareTaskExitCodes {
    pub success: ::std::collections::BTreeSet<i32>,
    pub reboot: ::std::collections::BTreeSet<i32>,
}
#[doc = "`SoftwareTaskExport`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SoftwareTaskExport {
    #[serde(rename = "direct")]
    Direct,
    #[serde(rename = "winget")]
    Winget {
        binding: SoftwareExportBinding,
        uri: ::std::string::String,
        identifier: ::std::string::String,
    },
    #[serde(rename = "brew")]
    Brew {
        binding: SoftwareExportBinding,
        uri: ::std::string::String,
        commit: ::std::string::String,
        tap: ::std::string::String,
        #[serde(rename = "credentialReference")]
        credential_reference: ::std::string::String,
    },
}
#[doc = "`SoftwareTaskIntent`"]
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
pub enum SoftwareTaskIntent {
    #[serde(rename = "install")]
    Install,
    #[serde(rename = "detect")]
    Detect,
    #[serde(rename = "uninstall")]
    Uninstall,
}
impl ::std::fmt::Display for SoftwareTaskIntent {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Install => f.write_str("install"),
            Self::Detect => f.write_str("detect"),
            Self::Uninstall => f.write_str("uninstall"),
        }
    }
}
impl ::std::str::FromStr for SoftwareTaskIntent {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "install" => Ok(Self::Install),
            "detect" => Ok(Self::Detect),
            "uninstall" => Ok(Self::Uninstall),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareTaskIntent {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareTaskIntent {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareTaskInterpreter`"]
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
pub enum SoftwareTaskInterpreter {
    #[serde(rename = "power_shell7")]
    PowerShell7,
    #[serde(rename = "posix_sh")]
    PosixSh,
    #[serde(rename = "bash")]
    Bash,
}
impl ::std::fmt::Display for SoftwareTaskInterpreter {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::PowerShell7 => f.write_str("power_shell7"),
            Self::PosixSh => f.write_str("posix_sh"),
            Self::Bash => f.write_str("bash"),
        }
    }
}
impl ::std::str::FromStr for SoftwareTaskInterpreter {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "power_shell7" => Ok(Self::PowerShell7),
            "posix_sh" => Ok(Self::PosixSh),
            "bash" => Ok(Self::Bash),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareTaskInterpreter {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareTaskInterpreter {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareTaskInvocation`"]
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
pub struct SoftwareTaskInvocation {
    #[serde(rename = "runAs")]
    pub run_as: ExecutionIdentity,
    pub arguments: ::std::vec::Vec<::std::string::String>,
    pub environment: ::std::collections::BTreeMap<::std::string::String, ::std::string::String>,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: u32,
    #[serde(rename = "outputBytes")]
    pub output_bytes: u32,
    #[serde(rename = "exitCodes")]
    pub exit_codes: SoftwareTaskExitCodes,
}
#[doc = "`SoftwareTaskMacApplication`"]
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
pub struct SoftwareTaskMacApplication {
    pub path: ::std::string::String,
    #[serde(rename = "bundleId")]
    pub bundle_id: ::std::string::String,
    pub version: ::std::string::String,
    #[serde(rename = "targetName")]
    pub target_name: ::std::string::String,
}
#[doc = "`SoftwareTaskMsix`"]
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
pub struct SoftwareTaskMsix {
    pub container: SoftwareTaskMsixContainer,
    pub identity: SoftwareTaskMsixIdentity,
    pub dependencies: ::std::vec::Vec<SoftwareTaskMsixIdentity>,
    pub deployment: SoftwareTaskMsixDeployment,
    #[serde(rename = "minimumOs")]
    pub minimum_os: [u16; 4usize],
    #[serde(rename = "requireSideload")]
    pub require_sideload: bool,
    #[serde(rename = "allowUnsigned")]
    pub allow_unsigned: bool,
    pub uninstall: bool,
    pub invocation: SoftwareTaskInvocation,
    pub upgrade: SoftwareTaskUpgrade,
}
#[doc = "`SoftwareTaskMsixArchitecture`"]
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
pub enum SoftwareTaskMsixArchitecture {
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "aarch64")]
    Aarch64,
    #[serde(rename = "neutral")]
    Neutral,
}
impl ::std::fmt::Display for SoftwareTaskMsixArchitecture {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::X86_64 => f.write_str("x86_64"),
            Self::Aarch64 => f.write_str("aarch64"),
            Self::Neutral => f.write_str("neutral"),
        }
    }
}
impl ::std::str::FromStr for SoftwareTaskMsixArchitecture {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "x86_64" => Ok(Self::X86_64),
            "aarch64" => Ok(Self::Aarch64),
            "neutral" => Ok(Self::Neutral),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareTaskMsixArchitecture {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareTaskMsixArchitecture {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareTaskMsixContainer`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SoftwareTaskMsixContainer {
    #[serde(rename = "package")]
    Package { installer: ::std::string::String },
    #[serde(rename = "bundle")]
    Bundle {
        installer: ::std::string::String,
        members: ::std::vec::Vec<SoftwareTaskMsixMember>,
    },
}
#[doc = "`SoftwareTaskMsixDeployment`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", content = "target")]
pub enum SoftwareTaskMsixDeployment {
    #[serde(rename = "target_user_registration")]
    TargetUserRegistration(SoftwareTaskUser),
    #[serde(rename = "device_provisioning")]
    DeviceProvisioning,
}
impl ::std::convert::From<SoftwareTaskUser> for SoftwareTaskMsixDeployment {
    fn from(value: SoftwareTaskUser) -> Self {
        Self::TargetUserRegistration(value)
    }
}
#[doc = "`SoftwareTaskMsixIdentity`"]
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
pub struct SoftwareTaskMsixIdentity {
    pub name: ::std::string::String,
    pub publisher: ::std::string::String,
    pub version: [u16; 4usize],
    pub architecture: SoftwareTaskMsixArchitecture,
    #[serde(rename = "resourceId")]
    pub resource_id: ::std::string::String,
}
#[doc = "`SoftwareTaskMsixMember`"]
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
pub struct SoftwareTaskMsixMember {
    pub path: ::std::string::String,
    pub identity: SoftwareTaskMsixIdentity,
    pub length: u64,
    pub sha256: [u8; 32usize],
}
#[doc = "`SoftwareTaskNative`"]
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
pub struct SoftwareTaskNative {
    pub installer: ::std::string::String,
    pub scope: SoftwareTaskScope,
    pub install: SoftwareTaskInvocation,
    #[serde(rename = "upgradeInvocation")]
    pub upgrade_invocation: SoftwareTaskInvocation,
    pub upgrade: SoftwareTaskUpgrade,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub uninstall: ::std::option::Option<SoftwareTaskRemoval>,
    pub detect: SoftwareTaskDetection,
}
#[doc = "`SoftwareTaskReboot`"]
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
pub enum SoftwareTaskReboot {
    #[serde(rename = "forbid")]
    Forbid,
    #[serde(rename = "report")]
    Report,
}
impl ::std::fmt::Display for SoftwareTaskReboot {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Forbid => f.write_str("forbid"),
            Self::Report => f.write_str("report"),
        }
    }
}
impl ::std::str::FromStr for SoftwareTaskReboot {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "forbid" => Ok(Self::Forbid),
            "report" => Ok(Self::Report),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareTaskReboot {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareTaskReboot {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareTaskRemoval`"]
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
pub struct SoftwareTaskRemoval {
    pub installer: ::std::string::String,
    pub invocation: SoftwareTaskInvocation,
}
#[doc = "`SoftwareTaskResult`"]
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
pub struct SoftwareTaskResult {
    pub intent: SoftwareTaskIntent,
    #[serde(rename = "definitionDigest")]
    pub definition_digest: [u8; 32usize],
    pub steps: ::std::vec::Vec<SoftwareStepResult>,
}
#[doc = "`SoftwareTaskScope`"]
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
pub enum SoftwareTaskScope {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
}
impl ::std::fmt::Display for SoftwareTaskScope {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::System => f.write_str("system"),
            Self::User => f.write_str("user"),
        }
    }
}
impl ::std::str::FromStr for SoftwareTaskScope {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareTaskScope {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareTaskScope {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareTaskScript`"]
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
pub struct SoftwareTaskScript {
    pub interpreter: SoftwareTaskInterpreter,
    pub entry: ::std::string::String,
    pub invocation: SoftwareTaskInvocation,
}
#[doc = "`SoftwareTaskSignature`"]
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
pub struct SoftwareTaskSignature {
    pub artifact: ::std::string::String,
    pub mechanism: SoftwareTaskSignatureMechanism,
    pub publisher: ::std::string::String,
}
#[doc = "`SoftwareTaskSignatureMechanism`"]
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
pub enum SoftwareTaskSignatureMechanism {
    #[serde(rename = "authenticode")]
    Authenticode,
    #[serde(rename = "apple_developer_id")]
    AppleDeveloperId,
    #[serde(rename = "msix")]
    Msix,
}
impl ::std::fmt::Display for SoftwareTaskSignatureMechanism {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Authenticode => f.write_str("authenticode"),
            Self::AppleDeveloperId => f.write_str("apple_developer_id"),
            Self::Msix => f.write_str("msix"),
        }
    }
}
impl ::std::str::FromStr for SoftwareTaskSignatureMechanism {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "authenticode" => Ok(Self::Authenticode),
            "apple_developer_id" => Ok(Self::AppleDeveloperId),
            "msix" => Ok(Self::Msix),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareTaskSignatureMechanism {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareTaskSignatureMechanism {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareTaskSpec`"]
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
pub struct SoftwareTaskSpec {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "tenantId")]
    pub tenant_id: ::uuid::Uuid,
    #[serde(rename = "deviceId")]
    pub device_id: ::std::string::String,
    pub platform: TaskPlatform,
    pub architecture: TaskArchitecture,
    #[serde(rename = "registrationId")]
    pub registration_id: ::uuid::Uuid,
    pub generation: u64,
    #[serde(rename = "taskId")]
    pub task_id: ::uuid::Uuid,
    #[serde(rename = "attemptId")]
    pub attempt_id: ::uuid::Uuid,
    pub permit: TaskPermit,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
    #[serde(rename = "executionContext")]
    pub execution_context: SoftwareExecutionContext,
    pub authorization: TaskAuthorization,
    pub steps: ::std::vec::Vec<SoftwareTaskStep>,
    #[serde(rename = "definitionDigest")]
    pub definition_digest: [u8; 32usize],
    pub intent: SoftwareTaskIntent,
    #[serde(rename = "startMode")]
    pub start_mode: SoftwareStartMode,
}
#[doc = "`SoftwareTaskStep`"]
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
pub struct SoftwareTaskStep {
    pub action: SoftwareTaskAction,
    pub artifacts: ::std::vec::Vec<SoftwareTaskArtifact>,
    pub export: SoftwareTaskExport,
    pub target: SoftwareExecutionTarget,
}
#[doc = "`SoftwareTaskUpgrade`"]
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
pub enum SoftwareTaskUpgrade {
    #[serde(rename = "in_place")]
    InPlace,
    #[serde(rename = "uninstall_then_install")]
    UninstallThenInstall,
    #[serde(rename = "deny")]
    Deny,
}
impl ::std::fmt::Display for SoftwareTaskUpgrade {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::InPlace => f.write_str("in_place"),
            Self::UninstallThenInstall => f.write_str("uninstall_then_install"),
            Self::Deny => f.write_str("deny"),
        }
    }
}
impl ::std::str::FromStr for SoftwareTaskUpgrade {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "in_place" => Ok(Self::InPlace),
            "uninstall_then_install" => Ok(Self::UninstallThenInstall),
            "deny" => Ok(Self::Deny),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for SoftwareTaskUpgrade {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for SoftwareTaskUpgrade {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`SoftwareTaskUser`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", content = "identity")]
pub enum SoftwareTaskUser {
    #[serde(rename = "active_interactive")]
    ActiveInteractive,
    #[serde(rename = "exact")]
    Exact(::std::string::String),
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
#[doc = "`TaskArchitecture`"]
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
pub enum TaskArchitecture {
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "aarch64")]
    Aarch64,
}
impl ::std::fmt::Display for TaskArchitecture {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::X86_64 => f.write_str("x86_64"),
            Self::Aarch64 => f.write_str("aarch64"),
        }
    }
}
impl ::std::str::FromStr for TaskArchitecture {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "x86_64" => Ok(Self::X86_64),
            "aarch64" => Ok(Self::Aarch64),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TaskArchitecture {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TaskArchitecture {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TaskAuthorization`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum TaskAuthorization {
    #[serde(rename = "policy")]
    Policy {
        #[serde(rename = "policyId")]
        policy_id: ::uuid::Uuid,
        #[serde(rename = "policyVersion")]
        policy_version: ::uuid::Uuid,
    },
    #[serde(rename = "remote_operation")]
    RemoteOperation {
        #[serde(rename = "operationId")]
        operation_id: ::uuid::Uuid,
    },
    #[serde(rename = "self_service")]
    SelfService {
        #[serde(rename = "requestId")]
        request_id: ::uuid::Uuid,
        subject: SelfServiceSubject,
        source: SelfServiceCaller,
        #[serde(rename = "policyId")]
        policy_id: ::uuid::Uuid,
        #[serde(rename = "policyRevision")]
        policy_revision: u64,
        #[serde(rename = "policyVersion")]
        policy_version: ::uuid::Uuid,
        #[serde(rename = "allowAi")]
        allow_ai: bool,
        #[serde(rename = "riskLevel")]
        risk_level: u8,
        confirmed: bool,
    },
}
#[doc = "`TaskCancellation`"]
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
pub struct TaskCancellation {
    #[serde(rename = "taskId")]
    pub task_id: ::uuid::Uuid,
    #[serde(rename = "attemptId")]
    pub attempt_id: ::uuid::Uuid,
}
#[doc = "`TaskClaimRequest`"]
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
pub struct TaskClaimRequest {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    pub profiles: ::std::vec::Vec<ExecutorProfile>,
    #[serde(rename = "executionContext")]
    pub execution_context: SoftwareExecutionContext,
}
#[doc = "`TaskClaimResponse`"]
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
pub struct TaskClaimResponse {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub task: ::std::option::Option<SignedTask>,
    pub cancellations: ::std::vec::Vec<TaskCancellation>,
}
#[doc = "`TaskContent`"]
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
pub struct TaskContent {
    pub length: u64,
    pub sha256: [u8; 32usize],
}
#[doc = "`TaskDelivery`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum TaskDelivery {
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "claimed")]
    Claimed {
        attempt: ::uuid::Uuid,
        #[serde(rename = "leaseUntil")]
        lease_until: i64,
    },
    #[serde(rename = "received")]
    Received {
        attempt: ::uuid::Uuid,
        #[serde(rename = "leaseUntil")]
        lease_until: i64,
    },
}
#[doc = "`TaskDiagnostics`"]
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
pub struct TaskDiagnostics {
    pub stdout: ::std::string::String,
    pub stderr: ::std::string::String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    #[serde(rename = "executedAt")]
    pub executed_at: i64,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub failure: ::std::option::Option<TaskFailure>,
}
#[doc = "`TaskEvent`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum TaskEvent {
    #[serde(rename = "received")]
    Received,
    #[serde(rename = "start")]
    Start,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "result")]
    Result(TaskResult),
    #[serde(rename = "output_chunk")]
    OutputChunk(OutputChunk),
    #[serde(rename = "chunked_result")]
    ChunkedResult(ChunkedTaskResult),
    #[serde(rename = "software_result")]
    SoftwareResult(SoftwareTaskResult),
    #[serde(rename = "enrollment_result")]
    EnrollmentResult { outcome: EnrollmentEntryOutcome },
}
#[doc = "`TaskEventAck`"]
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
pub struct TaskEventAck {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    pub accepted: bool,
    #[serde(deserialize_with = "::std::option::Option::deserialize")]
    pub permit: ::std::option::Option<SignedTask>,
    #[serde(rename = "cancelRequested")]
    pub cancel_requested: bool,
}
#[doc = "`TaskEventRequest`"]
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
pub struct TaskEventRequest {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "operationId")]
    pub operation_id: ::uuid::Uuid,
    #[serde(rename = "attemptId")]
    pub attempt_id: ::uuid::Uuid,
    pub event: TaskEvent,
    #[serde(rename = "executionContext")]
    pub execution_context: SoftwareExecutionContext,
}
#[doc = "`TaskExecution`"]
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
pub enum TaskExecution {
    #[serde(rename = "not_started")]
    NotStarted,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "waiting_reboot")]
    WaitingReboot,
    #[serde(rename = "unknown")]
    Unknown,
}
impl ::std::fmt::Display for TaskExecution {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::NotStarted => f.write_str("not_started"),
            Self::Running => f.write_str("running"),
            Self::Succeeded => f.write_str("succeeded"),
            Self::Failed => f.write_str("failed"),
            Self::WaitingReboot => f.write_str("waiting_reboot"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
impl ::std::str::FromStr for TaskExecution {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "not_started" => Ok(Self::NotStarted),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "waiting_reboot" => Ok(Self::WaitingReboot),
            "unknown" => Ok(Self::Unknown),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TaskExecution {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TaskExecution {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TaskFailure`"]
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
pub enum TaskFailure {
    #[serde(rename = "launch_failed")]
    LaunchFailed,
    #[serde(rename = "timed_out")]
    TimedOut,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "non_zero_exit")]
    NonZeroExit,
    #[serde(rename = "output_limit")]
    OutputLimit,
    #[serde(rename = "capture_failed")]
    CaptureFailed,
}
impl ::std::fmt::Display for TaskFailure {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::LaunchFailed => f.write_str("launch_failed"),
            Self::TimedOut => f.write_str("timed_out"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::NonZeroExit => f.write_str("non_zero_exit"),
            Self::OutputLimit => f.write_str("output_limit"),
            Self::CaptureFailed => f.write_str("capture_failed"),
        }
    }
}
impl ::std::str::FromStr for TaskFailure {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "launch_failed" => Ok(Self::LaunchFailed),
            "timed_out" => Ok(Self::TimedOut),
            "cancelled" => Ok(Self::Cancelled),
            "non_zero_exit" => Ok(Self::NonZeroExit),
            "output_limit" => Ok(Self::OutputLimit),
            "capture_failed" => Ok(Self::CaptureFailed),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TaskFailure {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TaskFailure {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TaskPayload`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(untagged)]
pub enum TaskPayload {
    TaskSpec(::std::boxed::Box<TaskSpec>),
    SoftwareTaskSpec(::std::boxed::Box<SoftwareTaskSpec>),
    EnrollmentTaskSpec(EnrollmentTaskSpec),
}
impl ::std::convert::From<TaskSpec> for TaskPayload {
    fn from(value: TaskSpec) -> Self {
        Self::TaskSpec(::std::boxed::Box::new(value))
    }
}
impl ::std::convert::From<SoftwareTaskSpec> for TaskPayload {
    fn from(value: SoftwareTaskSpec) -> Self {
        Self::SoftwareTaskSpec(::std::boxed::Box::new(value))
    }
}
impl ::std::convert::From<EnrollmentTaskSpec> for TaskPayload {
    fn from(value: EnrollmentTaskSpec) -> Self {
        Self::EnrollmentTaskSpec(value)
    }
}
#[doc = "`TaskPermit`"]
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
pub enum TaskPermit {
    #[serde(rename = "offer")]
    Offer,
    #[serde(rename = "start")]
    Start,
}
impl ::std::fmt::Display for TaskPermit {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Offer => f.write_str("offer"),
            Self::Start => f.write_str("start"),
        }
    }
}
impl ::std::str::FromStr for TaskPermit {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "offer" => Ok(Self::Offer),
            "start" => Ok(Self::Start),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TaskPermit {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TaskPermit {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TaskPlatform`"]
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
pub enum TaskPlatform {
    #[serde(rename = "windows")]
    Windows,
    #[serde(rename = "macos")]
    Macos,
}
impl ::std::fmt::Display for TaskPlatform {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Windows => f.write_str("windows"),
            Self::Macos => f.write_str("macos"),
        }
    }
}
impl ::std::str::FromStr for TaskPlatform {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "windows" => Ok(Self::Windows),
            "macos" => Ok(Self::Macos),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TaskPlatform {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TaskPlatform {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
#[doc = "`TaskResource`"]
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
pub struct TaskResource {
    pub id: ::std::string::String,
    pub version: ::std::string::String,
    pub variant: ::std::string::String,
}
#[doc = "`TaskResult`"]
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
pub struct TaskResult {
    #[serde(
        rename = "exitCode",
        deserialize_with = "::std::option::Option::deserialize"
    )]
    pub exit_code: ::std::option::Option<i32>,
    pub quality: OutputQuality,
    pub output: ::serde_json::Value,
    pub diagnostics: TaskDiagnostics,
    #[serde(rename = "collectedAt")]
    pub collected_at: i64,
}
#[doc = "`TaskRunState`"]
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
pub struct TaskRunState {
    pub delivery: TaskDelivery,
    pub execution: TaskExecution,
    pub cancellation: RunCancellation,
    pub deadline: i64,
    #[serde(rename = "startedAt")]
    pub started_at: ::std::option::Option<i64>,
}
#[doc = "`TaskSpec`"]
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
pub struct TaskSpec {
    #[serde(rename = "wireVersion")]
    pub wire_version: u8,
    #[serde(rename = "tenantId")]
    pub tenant_id: ::uuid::Uuid,
    #[serde(rename = "deviceId")]
    pub device_id: ::std::string::String,
    pub platform: TaskPlatform,
    pub architecture: TaskArchitecture,
    #[serde(rename = "registrationId")]
    pub registration_id: ::uuid::Uuid,
    pub generation: u64,
    #[serde(rename = "taskId")]
    pub task_id: ::uuid::Uuid,
    #[serde(rename = "attemptId")]
    pub attempt_id: ::uuid::Uuid,
    pub permit: TaskPermit,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
    #[serde(rename = "resourceDigest")]
    pub resource_digest: [u8; 32usize],
    pub resource: TaskResource,
    pub authorization: TaskAuthorization,
    pub parameters: ::std::collections::BTreeMap<::std::string::String, ::serde_json::Value>,
    pub deadline: i64,
    pub content: TaskContent,
    pub profile: ExecutorProfile,
    #[serde(rename = "sqlParameters")]
    pub sql_parameters: ::std::option::Option<
        ::std::collections::BTreeMap<::std::string::String, ::serde_json::Value>,
    >,
    #[serde(rename = "runAs")]
    pub run_as: ExecutionIdentity,
    #[serde(rename = "executionTarget")]
    pub execution_target: SoftwareExecutionTarget,
    pub arguments: ::std::vec::Vec<::std::string::String>,
    pub environment: ::std::collections::BTreeMap<::std::string::String, ::std::string::String>,
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: u32,
    #[serde(rename = "outputBytes")]
    pub output_bytes: u32,
    #[serde(rename = "maxRows")]
    pub max_rows: u16,
}
#[doc = "`ValueType`"]
#[derive(
    :: serde :: Deserialize,
    :: serde :: Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    schemars :: JsonSchema,
)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ValueType {
    #[serde(rename = "string")]
    String {
        #[serde(rename = "maxLength")]
        max_length: u32,
        #[serde(rename = "allowEmpty")]
        allow_empty: bool,
    },
    #[serde(rename = "enum")]
    Enum {
        values: ::std::vec::Vec<::std::string::String>,
    },
    #[serde(rename = "integer")]
    Integer,
    #[serde(rename = "number")]
    Number,
    #[serde(rename = "boolean")]
    Boolean,
    #[serde(rename = "time")]
    Time,
    #[serde(rename = "array")]
    Array {
        items: ::std::boxed::Box<ValueType>,
        #[serde(rename = "maxItems")]
        max_items: u32,
    },
    #[serde(rename = "object")]
    Object {
        properties: ::std::collections::BTreeMap<::std::string::String, ValueType>,
    },
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
pub const WIRE_VERSION: u8 = 1;
pub const SCHEMA_FINGERPRINT: &str =
    "a3493c8e185d8f8a53c4b899a4bc82ae64dcc72e08be9ba23b3ff1021336f811";
pub const SCHEMA_MANIFEST: &str = include_str!("../schema/runtime/agent-v1.schema-manifest.json");
