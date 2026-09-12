use crate::{ContractError, ErrorKind, Field, Rule};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

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
            Err(D::Error::custom(
                ContractError::new(ErrorKind::UnsupportedVersion, Field::Version, Rule::Version)
                    .for_serde(),
            ))
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
    ($name:ident, $field:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(
            #[schemars(length(min = 1, max = 128), pattern(r"^[A-Za-z0-9][A-Za-z0-9._:/-]*$"))]
            String,
        );
        impl $name {
            /// Accept 1–128 ASCII bytes matching `[A-Za-z0-9][A-Za-z0-9._:/-]*`; no normalization or authentication.
            /// Returns InvalidValue with this identifier field and Identifier rule on failure.
            pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
                let value = value.into();
                if !valid_id(&value) {
                    return Err(ContractError::new(
                        ErrorKind::InvalidValue,
                        Field::$field,
                        Rule::Identifier,
                    ));
                }
                Ok(Self(value))
            }
            /// Borrow the validated representation without normalization, resolution or authority checks.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Self::new(String::deserialize(d)?)
                    .map_err(|error| D::Error::custom(error.for_serde()))
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
    Identifier,
    "Opaque local reference identifier; syntax validity is not authenticity."
);
identifier!(
    ActorId,
    Actor,
    "Product actor reference, not an authenticated principal."
);
identifier!(
    DeviceId,
    Device,
    "Device reference, not verified registration evidence."
);
identifier!(RequestId, Request, "Local execution request identity.");
identifier!(PlanId, Plan, "Immutable local plan identity.");
identifier!(
    AttemptId,
    Attempt,
    "Execution attempt correlation identity."
);
identifier!(EventId, Event, "Audit event correlation identity.");

/// Lowercase SHA-256 bytes expressed as hex; a digest alone grants no trust.
#[derive(Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Digest(#[schemars(length(min = 64, max = 64), pattern("^[0-9a-f]{64}$"))] String);
impl Digest {
    /// Accept exactly 64 lowercase ASCII hex digits; returns an InvalidValue/Digest diagnostic otherwise.
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ContractError::new(
                ErrorKind::InvalidValue,
                Field::Digest,
                Rule::Digest,
            ));
        }
        Ok(Self(value))
    }
    /// Borrow the validated representation without normalization, resolution or authority checks.
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
        Self::new(String::deserialize(d)?).map_err(|error| D::Error::custom(error.for_serde()))
    }
}
