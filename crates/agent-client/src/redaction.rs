use crate::Error;
/// Redacts the communication owner's actual credentials while preserving task output.
/// The secret list is private and never serialized or printed.
pub struct CredentialRedactor {
    pub(crate) device: Vec<crate::Secret>,
    pub(crate) software: Vec<zeroize::Zeroizing<String>>,
    pub(crate) employee: Vec<std::sync::Weak<zeroize::Zeroizing<String>>>,
}
impl CredentialRedactor {
    /// Retain native employee credentials solely for output protection, including delayed results.
    pub fn protect_user_session(
        &mut self,
        value: &std::sync::Arc<zeroize::Zeroizing<String>>,
    ) -> Result<(), Error> {
        if value.is_empty() || value.len() > 4096 {
            return Err(Error::Identity);
        }
        self.employee.retain(|lease| lease.strong_count() > 0);
        if !self
            .employee
            .iter()
            .any(|lease| lease.ptr_eq(&std::sync::Arc::downgrade(value)))
        {
            self.employee.push(std::sync::Arc::downgrade(value));
        }
        Ok(())
    }
    /// Remove the owner's actual credentials from an outbound string.
    pub fn redact(&self, text: &str) -> Result<String, Error> {
        let mut value = text.to_owned();
        for secret in &self.device {
            value = value.replace(secret.expose(), "[redacted]");
        }
        for secret in &self.software {
            value = value.replace(secret.as_str(), "[redacted]");
        }
        for secret in &self.employee {
            if let Some(secret) = secret.upgrade() {
                value = value.replace(secret.as_str(), "[redacted]");
            }
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn retired_sessions_drain_without_dropping_an_inflight_output_lease() {
        let mut redactor = CredentialRedactor {
            device: vec![],
            software: vec![],
            employee: vec![],
        };
        let pending = Arc::new(zeroize::Zeroizing::new("pending-credential".into()));
        redactor.protect_user_session(&pending).unwrap();
        for i in 0..300 {
            let current = Arc::new(zeroize::Zeroizing::new(format!("session-{i:04}")));
            redactor.protect_user_session(&current).unwrap();
            assert_eq!(
                redactor
                    .redact(&format!("{} {}", pending.as_str(), current.as_str()))
                    .unwrap(),
                "[redacted] [redacted]"
            );
            assert!(redactor.employee.len() <= 2);
        }
        drop(pending);
        let current = Arc::new(zeroize::Zeroizing::new("current".into()));
        redactor.protect_user_session(&current).unwrap();
        assert_eq!(redactor.employee.len(), 1);
    }
}
