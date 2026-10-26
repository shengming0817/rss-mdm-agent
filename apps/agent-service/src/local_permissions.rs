//! Protected restrictions on capabilities compiled into the installed service.
//! ref: rustls rustls/src/verify.rs@0.23.45 (private verification markers).
use agent_client::Error;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[cfg(test)]
pub(crate) type FixturePolicyReader = fn(&Config) -> Result<Policy, Error>;
/// Administrator-protected restrictions; the document cannot define executable content.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// Protected restriction document established by the installer.
    pub policy_file: PathBuf,
}
/// Small product rule document, writable only by the installer/administrator.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Restrictions {
    /// Current local rule format.
    pub version: u8,
    /// Allow native removal of qualified non-protected software, including device scope.
    pub allow_native_uninstall: bool,
    /// Allow trusted GUI opening in the caller's own login.
    pub allow_launch: bool,
    /// Permit code-owned display configuration in the requesting login.
    pub allow_display: bool,
    /// Delegate code-owned proxy changes; never arbitrary system commands.
    pub allow_proxy: bool,
    /// Delegate code-owned power preferences.
    pub allow_power: bool,
    /// Complete exact native identities protected by this local/organization policy.
    pub protected: Vec<String>,
}
impl Restrictions {
    /// Product installation default selected by the user's accepted software-management policy.
    pub fn installation_default() -> Self {
        Self {
            version: 2,
            allow_native_uninstall: true,
            allow_launch: true,
            allow_display: true,
            allow_proxy: false,
            allow_power: false,
            protected: vec![
                "com.rss.mdmagent".into(),
                "com.rss-mdm.agent".into(),
                "RSS MDM Agent".into(),
            ],
        }
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.version != 2
            || self.protected.len() > 256
            || self
                .protected
                .iter()
                .any(|v| v.is_empty() || v.len() > 1024 || v.chars().any(char::is_control))
        {
            return Err(Error::Configuration);
        }
        Ok(())
    }
    pub(crate) fn protects(&self, native_id: &str) -> bool {
        let native = native_id.to_ascii_lowercase();
        native == "com.rss.mdmagent"
            || native == "com.rss-mdm.agent"
            || native.starts_with("rss.mdm.agent_")
            || self
                .protected
                .iter()
                .any(|id| id.eq_ignore_ascii_case(native_id))
    }
}
/// Validated restrictions constructed only by a protected read.
pub(crate) struct Policy {
    pub(crate) rules: Restrictions,
    pub(crate) path: PathBuf,
    #[cfg(test)]
    fixture: bool,
}
impl Policy {
    pub(crate) fn open(config: &Config) -> Result<Self, Error> {
        let bytes = installation_security::read_protected(&config.policy_file)
            .map_err(|_| Error::Denied)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Error::Capacity);
        }
        let rules: Restrictions =
            serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        rules.validate()?;
        Ok(Self {
            rules,
            path: config.policy_file.clone(),
            #[cfg(test)]
            fixture: false,
        })
    }
    #[cfg(test)]
    pub(crate) fn fixture(config: &Config) -> Result<Self, Error> {
        let bytes = std::fs::read(&config.policy_file)?;
        if bytes.len() > 65536 {
            return Err(Error::Capacity);
        }
        let rules: Restrictions =
            serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        rules.validate()?;
        Ok(Self {
            rules,
            path: config.policy_file.clone(),
            fixture: true,
        })
    }
    pub(crate) fn current(&self) -> Result<Self, Error> {
        let config = Config {
            policy_file: self.path.clone(),
        };
        #[cfg(test)]
        if self.fixture {
            return Self::fixture(&config);
        }
        Self::open(&config)
    }
}
/// Establish the rule file only during explicit protected installation, preserving existing rules.
pub(crate) fn initialize(config: &Config) -> Result<(), Error> {
    if config.policy_file.try_exists()? {
        Policy::open(config)?;
        return Ok(());
    }
    installation_security::protected(config.policy_file.parent().ok_or(Error::Configuration)?)
        .map_err(|_| Error::Denied)?;
    let bytes = serde_json::to_vec(&Restrictions::installation_default())
        .map_err(|_| Error::Configuration)?;
    crate::installation::publish_local_rules(&config.policy_file, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_contract::LocalSoftwareAction;

    #[test]
    fn installation_default_allows_native_removal_but_never_invents_a_recipe_or_update() {
        let r = Restrictions::installation_default();
        r.validate().unwrap();
        assert!(r.allow_native_uninstall);
        assert!(!r.protects("org.example.app"));
        assert!(!r.protects("com.apple.finder"));
        assert!(r.protects("com.rss.mdmagent"));
        assert!(!crate::local_software::native::allowed(
            &r,
            LocalSoftwareAction::Update
        ));
    }
    #[test]
    fn missing_rules_never_activate_product_defaults_at_runtime() {
        let path = std::env::temp_dir().join(format!("rss-missing-rules-{}", uuid::Uuid::new_v4()));
        assert!(Policy::open(&Config {
            policy_file: path.clone()
        })
        .is_err());
        assert!(!path.exists());
    }
}
