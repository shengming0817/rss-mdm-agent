use crate::{wire, Error};
/// Redacts the communication owner's actual credentials while preserving task output.
/// The secret list is private and never serialized or printed.
pub struct CredentialRedactor(pub(crate) Vec<wire::Secret>);
impl CredentialRedactor {
    /// Remove the owner's actual credentials from an outbound string.
    pub fn redact(&self, text: &str) -> Result<String, Error> {
        let mut value = text.to_owned();
        for secret in &self.0 {
            value = value.replace(secret.expose(), "[redacted]");
        }
        Ok(value)
    }
}
