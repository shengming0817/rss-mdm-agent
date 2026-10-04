//! Protected credential value used by client adapters and generated request bindings.
use base64::Engine;
use zeroize::Zeroizing;
/// A canonical 256-bit URL-safe credential; debug output never exposes it.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Zeroizing<String>);
impl Secret {
    /// Parse one canonical unpadded base64url credential.
    pub fn parse(value: &str) -> Result<Self, crate::DecodeError> {
        let bytes = Zeroizing::new(
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(value)
                .map_err(|_| crate::DecodeError)?,
        );
        if bytes.len() != 32
            || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes.as_slice()) != value
        {
            return Err(crate::DecodeError);
        }
        Ok(Self(Zeroizing::new(value.to_owned())))
    }
    /// Expose only at the protected provider or request encoder boundary.
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
impl serde::Serialize for Secret {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.expose())
    }
}
impl<'de> serde::Deserialize<'de> for Secret {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Zeroizing::new(<String as serde::Deserialize>::deserialize(d)?);
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}
impl schemars::JsonSchema for Secret {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Secret".into()
    }
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(g)
    }
}
