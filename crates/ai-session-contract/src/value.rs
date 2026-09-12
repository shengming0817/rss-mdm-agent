use crate::{ContractError, ErrorKind, Field, Rule};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

/// Only V1 is accepted; there are no legacy aliases or version fallbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct V1;
impl Serialize for V1 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(1)
    }
}
impl<'de> Deserialize<'de> for V1 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if serde_json::Number::deserialize(d)?.as_u64() != Some(1) {
            return Err(D::Error::custom(
                ContractError::new(ErrorKind::UnsupportedVersion, Field::Version, Rule::Version)
                    .for_serde(),
            ));
        }
        Ok(Self)
    }
}
impl JsonSchema for V1 {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AiSessionV1".into()
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
            #[schemars(
                length(min = 1, max = 128),
                pattern(r"^[A-Za-z0-9][A-Za-z0-9._:/+-]*$")
            )]
            String,
        );
        impl $name {
            /// Accept 1–128 ASCII bytes matching `[A-Za-z0-9][A-Za-z0-9._:/+-]*`; no normalization or authentication.
            /// Returns InvalidValue with this identifier field and Identifier rule on failure.
            pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
                let value = value.into();
                if value.is_empty()
                    || value.len() > 128
                    || !value.as_bytes()[0].is_ascii_alphanumeric()
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._:/+-".contains(&b))
                {
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
identifier!(
    ConversationId,
    Conversation,
    "Conversation identity independent of any repository."
);
identifier!(TurnId, Turn, "One model turn identity.");
identifier!(
    MessageId,
    Message,
    "Stable message identity for streamed updates."
);
identifier!(
    ToolCallId,
    ToolCall,
    "Model proposal identity; not an execution task or approval."
);
identifier!(
    Name,
    Reference,
    "Opaque provider/tool/config reference, never a principal."
);
