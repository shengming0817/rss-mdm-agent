//! Client-generated representations of the existing Agent HTTP API.
//! The backend owns the schema. These values do not grant execution authority.
#![forbid(unsafe_code)]
pub mod enroll;
pub mod runtime;
/// Finite JSON number with deterministic equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Number(ordered_float::NotNan<f64>);
impl TryFrom<f64> for Number {
    type Error = DecodeError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            return Err(DecodeError);
        }
        ordered_float::NotNan::new(value)
            .map(Self)
            .map_err(|_| DecodeError)
    }
}
impl schemars::JsonSchema for Number {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Number".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        f64::json_schema(generator)
    }
}
mod codec;
pub use codec::{decode, validate, DecodeError};
mod secret;
pub use secret::Secret;
