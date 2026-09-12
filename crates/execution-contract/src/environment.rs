use crate::{ContractError, ErrorKind, Field, Platform, Rule};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

/// Portable ASCII environment name. The representation preserves case until platform freezing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct EnvironmentKey(
    #[schemars(length(min = 1, max = 128), pattern("^[A-Za-z_][A-Za-z0-9_]*$"))] String,
);
impl EnvironmentKey {
    /// Validate a portable name, excluding empty names, equals signs and non-ASCII characters.
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || value.as_bytes()[0].is_ascii_digit()
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(ContractError::new(
                ErrorKind::InvalidValue,
                Field::Environment,
                Rule::EnvironmentName,
            ));
        }
        Ok(Self(value))
    }
    /// Borrow the name. Frozen Windows plans expose uppercase names; other platforms preserve case.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn canonical_for(&self, platform: Platform) -> Self {
        if platform == Platform::Windows {
            Self(self.0.to_ascii_uppercase())
        } else {
            self.clone()
        }
    }
}
impl<'de> Deserialize<'de> for EnvironmentKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(d)?)
            .map_err(|error| serde::de::Error::custom(error.for_serde()))
    }
}
