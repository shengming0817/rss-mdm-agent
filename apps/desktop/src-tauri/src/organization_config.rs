//! Build-time defaults and user connections share one origin/tenant/name validator.
//! ref: url src/lib.rs@v2.5.7 (Url parsing and origin normalization).
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OrganizationConfiguration {
    pub origin: String,
    pub tenant: String,
    pub label: String,
    /// Optional deployment CA public certificate, embedded only in the native binary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_pem: Option<String>,
}

#[derive(Debug)]
pub struct ConfigurationError(pub &'static str);
impl std::fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Configure {} in root .env and rebuild the desktop",
            self.0
        )
    }
}
impl std::error::Error for ConfigurationError {}

impl OrganizationConfiguration {
    pub fn normalize(mut self) -> Result<Self, ConfigurationError> {
        let invalid_origin = || ConfigurationError("RSS_MDM_ORIGIN (a real HTTPS origin)");
        let mut url = url::Url::parse(&self.origin).map_err(|_| invalid_origin())?;
        if self.origin.len() > 2048
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(invalid_origin());
        }
        let host = url
            .host_str()
            .ok_or_else(invalid_origin)?
            .trim_end_matches('.')
            .to_owned();
        if host.is_empty() || host == "example" || host.ends_with(".example") {
            return Err(invalid_origin());
        }
        // Normalize a DNS root dot as well as case/default port before deriving identity.
        if !host.starts_with('[') {
            url.set_host(Some(&host)).map_err(|_| invalid_origin())?;
        }
        self.origin = url.origin().ascii_serialization();
        let tenant = uuid::Uuid::parse_str(&self.tenant)
            .map_err(|_| ConfigurationError("RSS_MDM_TENANT_ID (a non-zero UUID)"))?;
        if tenant.is_nil() {
            return Err(ConfigurationError("RSS_MDM_TENANT_ID (a non-zero UUID)"));
        }
        self.tenant = tenant.to_string();
        self.label = self.label.trim().to_owned();
        if self.label.is_empty()
            || self.label.chars().count() > 64
            || self.label.chars().any(char::is_control)
        {
            return Err(ConfigurationError(
                "RSS_MDM_ORGANIZATION_LABEL (1–64 characters without controls)",
            ));
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> OrganizationConfiguration {
        OrganizationConfiguration {
            origin: "https://MDM.fixture.test:443/".into(),
            tenant: "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into(),
            label: " Organization ".into(),
            ca_pem: None,
        }
    }
    #[test]
    fn normalizes_identity_and_rejects_invalid_deployment_fields() {
        let normalized = config().normalize().unwrap();
        assert_eq!(normalized.origin, "https://mdm.fixture.test");
        assert_eq!(normalized.tenant, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
        assert_eq!(normalized.label, "Organization");
        for origin in [
            "https://mdm.example",
            "https://MDM.EXAMPLE.:443/",
            "https://nested.mdm.example../",
            "https://example/",
            "http://mdm.fixture.test",
            "https://user:pass@mdm.fixture.test",
            "https://mdm.fixture.test/api",
            "https://mdm.fixture.test/?secret=x",
            "https://mdm.fixture.test/#fragment",
        ] {
            let mut value = config();
            value.origin = origin.into();
            assert!(value.normalize().is_err(), "{origin}");
        }
        for tenant in ["", "invalid", "00000000-0000-0000-0000-000000000000"] {
            let mut value = config();
            value.tenant = tenant.into();
            assert!(value.normalize().is_err());
        }
        for label in [" ".to_owned(), "x".repeat(65), "name\ncontrol".into()] {
            let mut value = config();
            value.label = label;
            assert!(value.normalize().is_err());
        }
    }
}
