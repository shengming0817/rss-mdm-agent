use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A syntax error contains no input values or provider diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ContractError {
    #[error("invalid contract encoding")]
    Encoding,
    #[error("unsupported contract version")]
    Version,
    #[error("invalid contract value")]
    Value,
    #[error("contract limit exceeded")]
    Limit,
    #[error("invalid execution budget or validity window")]
    Budget,
    #[error("inconsistent execution context")]
    Context,
}

/// The only accepted local contract version. This is not the Agent wire version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct V1;
impl Serialize for V1 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(1)
    }
}
impl<'de> Deserialize<'de> for V1 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let version = serde_json::Number::deserialize(d)?;
        if version.as_u64() == Some(1) {
            Ok(Self)
        } else {
            Err(D::Error::custom(ContractError::Version))
        }
    }
}
impl JsonSchema for V1 {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "LocalContractV1".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({"type":"integer", "const":1})
    }
}

macro_rules! identifier {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(
            #[schemars(length(min = 1, max = 128), pattern(r"^[A-Za-z0-9][A-Za-z0-9._:/-]*$"))]
            String,
        );
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
                let value = value.into();
                if !valid_id(&value) {
                    return Err(ContractError::Value);
                }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Self::new(String::deserialize(d)?).map_err(D::Error::custom)
            }
        }
    };
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/-".contains(&b))
}
identifier!(
    Id,
    "Opaque local reference identifier; syntax validity is not authenticity."
);
identifier!(
    ActorId,
    "Product actor reference, not an authenticated principal."
);
identifier!(
    DeviceId,
    "Device reference, not verified registration evidence."
);
identifier!(RequestId, "Local execution request identity.");
identifier!(PlanId, "Immutable local plan identity.");
identifier!(AttemptId, "Execution attempt correlation identity.");
identifier!(EventId, "Audit event correlation identity.");

/// Lowercase SHA-256 bytes expressed as hex; a digest alone grants no trust.
#[derive(Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Digest(#[schemars(length(min = 64, max = 64), pattern("^[0-9a-f]{64}$"))] String);
impl Digest {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ContractError::Value);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
}
impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Digest([redacted])")
    }
}
impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?).map_err(D::Error::custom)
    }
}
