use crate::{wire, Error};
/// Redacts the communication owner's actual credentials while preserving task output.
/// The secret list is private and never serialized or printed.
pub struct CredentialRedactor {
    pub(crate) device: Vec<wire::Secret>,
    pub(crate) employee: Vec<zeroize::Zeroizing<String>>,
}
impl CredentialRedactor {
    /// Retain native employee credentials solely for output protection, including delayed results.
    pub fn protect_employee(&mut self, value: &str) -> Result<(), Error> {
        if value.is_empty() || value.len() > 4096 {
            return Err(Error::Identity);
        }
        if !self.employee.iter().any(|v| v.as_str() == value) {
            if self.employee.len() >= 128 {
                return Err(Error::Capacity);
            }
            self.employee
                .push(zeroize::Zeroizing::new(value.to_owned()));
        }
        Ok(())
    }
    /// Remove the owner's actual credentials from an outbound string.
    pub fn redact(&self, text: &str) -> Result<String, Error> {
        let mut value = text.to_owned();
        for secret in &self.device {
            value = value.replace(secret.expose(), "[redacted]");
        }
        for secret in &self.employee {
            value = value.replace(secret.as_str(), "[redacted]");
        }
        Ok(value)
    }
}
