//! Protected local software rules and fixed recipes. This is not an enterprise policy issuer.
//! ref: rustls rustls/src/verify.rs@0.23.45 (private verification markers).
use agent_client::Error;
use execution_contract::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
pub(crate) mod compile;
pub mod native;

#[cfg(test)]
pub(crate) type FixturePolicyReader = fn(&LocalSoftwareConfig) -> Result<Policy, Error>;

/// Installer-owned configuration. A missing policy is unavailable, never an implicit allow.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSoftwareConfig {
    /// Administrator-protected, bounded rule document.
    pub policy_file: PathBuf,
}
/// Explicit installation scope; never inferred from process identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalScope {
    User,
    Device,
}
/// One immutable recipe uses the existing local execution model, not a remote wire replica.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recipe {
    /// Exact installed native identity.
    pub native_id: String,
    /// Expected installation family.
    pub kind: InstalledSoftwareKind,
    /// Exact effect scope.
    pub scope: LocalScope,
    /// Only update or uninstall recipes are admitted.
    pub action: LocalSoftwareAction,
    /// Exact native publisher, when supplied by the installation facts.
    pub publisher: Option<String>,
    /// Exact source versions for this fixed update candidate; never a generic version comparison.
    pub from_versions: Vec<String>,
    /// Fixed target version for updates.
    pub target_version: Option<String>,
    /// Existing local execution template; caller identity/target are rebound by the service.
    pub input: ExecutionInput,
    /// Exact locally staged material paths, keyed by their declared SHA256.
    pub files: Vec<SoftwareMaterial>,
}
/// Small product rule document, writable only by the installer/administrator.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rules {
    /// Current local rule format.
    pub version: u8,
    /// Allow native removal of qualified non-protected software, including device scope.
    pub allow_native_uninstall: bool,
    /// Allow trusted GUI opening in the caller's own login.
    pub allow_launch: bool,
    /// Allow fixed current-user update recipes.
    pub allow_user_update: bool,
    /// Complete exact native identities protected by this local/organization policy.
    pub protected: Vec<String>,
    /// Explicit fixed recipes; a missing recipe never causes command inference.
    pub recipes: Vec<Recipe>,
}
impl Rules {
    /// Product installation default selected by the user's accepted software-management policy.
    pub fn installation_default() -> Self {
        Self {
            version: 1,
            allow_native_uninstall: true,
            allow_launch: true,
            allow_user_update: true,
            protected: vec![
                "com.rss.mdmagent".into(),
                "com.rss-mdm.agent".into(),
                "RSS MDM Agent".into(),
            ],
            recipes: vec![],
        }
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.version != 1
            || self.protected.len() > 256
            || self.recipes.len() > 128
            || self
                .protected
                .iter()
                .any(|v| v.is_empty() || v.len() > 1024 || v.chars().any(char::is_control))
        {
            return Err(Error::Configuration);
        }
        for r in &self.recipes {
            if r.native_id.is_empty()
                || r.native_id.len() > 1024
                || r.files.len() > 256
                || r.from_versions.len() > 128
                || !matches!(
                    r.action,
                    LocalSoftwareAction::Update | LocalSoftwareAction::Uninstall
                )
                || r.input.budget.max_attempts != 1
                || r.input.budget.total_timeout_ms > 3_600_000
                || r.input.budget.total_output_bytes > 1_048_576
                || r.files
                    .iter()
                    .any(|m| !std::path::Path::new(&m.path).is_absolute())
            {
                return Err(Error::Configuration);
            }
            if r.action == LocalSoftwareAction::Update
                && (r.scope != LocalScope::User
                    || r.from_versions.is_empty()
                    || r.target_version
                        .as_ref()
                        .is_none_or(|v| v.is_empty() || r.from_versions.contains(v)))
            {
                return Err(Error::Configuration);
            }
            let ExecutionSpec::SoftwareProgram { program } = &r.input.execution else {
                return Err(Error::Configuration);
            };
            let expected = if r.action == LocalSoftwareAction::Uninstall {
                SoftwareOperation::Uninstall
            } else {
                SoftwareOperation::Install
            };
            if program.intent != expected
                || (r.action == LocalSoftwareAction::Update
                    && program
                        .steps
                        .last()
                        .is_none_or(|s| Some(s.version.as_str()) != r.target_version.as_deref()))
            {
                return Err(Error::Configuration);
            }
            FrozenExecution::freeze(
                r.input.clone(),
                &crate::backend::plan::storage_limits().input,
            )
            .map_err(|_| Error::Configuration)?;
        }
        Ok(())
    }
    pub(crate) fn protects(&self, item: &InstalledSoftware) -> bool {
        let native = item.native_id.to_ascii_lowercase();
        native.starts_with("com.apple.")
            || native == "com.rss.mdmagent"
            || native == "com.rss-mdm.agent"
            || native.starts_with("rss.mdm.agent_")
            || self
                .protected
                .iter()
                .any(|id| id.eq_ignore_ascii_case(&item.native_id))
    }
    pub(crate) fn recipe(
        &self,
        item: &InstalledSoftware,
        action: LocalSoftwareAction,
    ) -> Option<&Recipe> {
        let scope = match item.scope {
            DataScope::Device => LocalScope::Device,
            DataScope::User { .. } => LocalScope::User,
        };
        let mut found = self.recipes.iter().filter(|r| {
            r.native_id == item.native_id
                && r.kind == item.kind
                && r.scope == scope
                && (r.action == action
                    || (action == LocalSoftwareAction::Detect
                        && r.action == LocalSoftwareAction::Uninstall))
                && (r.action != LocalSoftwareAction::Uninstall
                    || match &r.input.execution {
                        ExecutionSpec::SoftwareProgram { program } => {
                            program.steps.last().is_some_and(|step| {
                                item.version.as_deref() == Some(step.version.as_str())
                            })
                        }
                        _ => false,
                    })
                && (action != LocalSoftwareAction::Update
                    || item
                        .version
                        .as_ref()
                        .is_some_and(|v| r.from_versions.contains(v)))
                && (action != LocalSoftwareAction::Update
                    || match &r.input.execution {
                        ExecutionSpec::SoftwareProgram { program } => {
                            item.version.as_deref().is_some_and(|installed| {
                                program.steps.last().is_some_and(|step| {
                                    execution_runner::software::forward_update(step, installed)
                                })
                            })
                        }
                        _ => false,
                    })
                && r.publisher
                    .as_ref()
                    .is_none_or(|publisher| item.publisher.as_ref() == Some(publisher))
        });
        let recipe = found.next()?;
        // Ambiguous source selection is never resolved by file order.
        found.next().is_none().then_some(recipe)
    }
}
/// Validated rule contents and their exact revision; only a protected read constructs this value.
pub(crate) struct Policy {
    pub(crate) rules: Rules,
    pub(crate) revision: Digest,
    pub(crate) path: PathBuf,
    #[cfg(test)]
    fixture: bool,
}
impl Policy {
    pub(crate) fn open(config: &LocalSoftwareConfig) -> Result<Self, Error> {
        let bytes = installation_security::read_protected(&config.policy_file)
            .map_err(|_| Error::Denied)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Error::Capacity);
        }
        let rules: Rules = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        rules.validate()?;
        Ok(Self {
            rules,
            revision: crate::collected_data::digest(&bytes)?,
            path: config.policy_file.clone(),
            #[cfg(test)]
            fixture: false,
        })
    }
    #[cfg(test)]
    pub(crate) fn fixture(config: &LocalSoftwareConfig) -> Result<Self, Error> {
        let bytes = std::fs::read(&config.policy_file)?;
        if bytes.len() > 65536 {
            return Err(Error::Capacity);
        }
        let rules: Rules = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        rules.validate()?;
        Ok(Self {
            rules,
            revision: crate::collected_data::digest(&bytes)?,
            path: config.policy_file.clone(),
            fixture: true,
        })
    }
    pub(crate) fn current(&self) -> Result<(), Error> {
        let config = LocalSoftwareConfig {
            policy_file: self.path.clone(),
        };
        #[cfg(test)]
        let current = if self.fixture {
            Self::fixture(&config)
        } else {
            Self::open(&config)
        }?;
        #[cfg(not(test))]
        let current = Self::open(&config)?;
        if current.revision != self.revision {
            return Err(Error::Conflict);
        }
        Ok(())
    }
}
/// Private per-submission evidence. Serialized intents cannot construct a permit.
pub(crate) struct Permit {
    plan: FrozenExecution,
    policy: Policy,
}
impl Permit {
    pub(crate) fn new(plan: FrozenExecution, policy: Policy) -> Result<Self, Error> {
        policy.current()?;
        if !plan
            .spec()
            .policy
            .id
            .as_str()
            .starts_with("local-software:")
            || matches!(
                plan.spec().request.initiator,
                Initiator::Backend { .. } | Initiator::Policy { .. }
            )
        {
            return Err(Error::Denied);
        }
        Ok(Self { plan, policy })
    }
    pub(crate) fn verify(&self, plan: &FrozenExecution, now: u64) -> Result<(), Error> {
        if plan.digest() != self.plan.digest() || now >= plan.spec().validity.expires_at_unix_ms {
            return Err(Error::Denied);
        }
        self.policy.current()
    }
}
/// Establish the rule file only during explicit protected installation, preserving existing rules.
pub(crate) fn initialize(config: &LocalSoftwareConfig) -> Result<(), Error> {
    if config.policy_file.try_exists()? {
        Policy::open(config)?;
        return Ok(());
    }
    installation_security::protected(config.policy_file.parent().ok_or(Error::Configuration)?)
        .map_err(|_| Error::Denied)?;
    let bytes =
        serde_json::to_vec(&Rules::installation_default()).map_err(|_| Error::Configuration)?;
    crate::installation::publish_local_rules(&config.policy_file, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str) -> InstalledSoftware {
        InstalledSoftware {
            reference: SoftwareRef {
                query: QueryRef {
                    id: Id::new("query").unwrap(),
                    revision: Digest::new("11".repeat(32)).unwrap(),
                },
                cursor: DataCursor {
                    snapshot: Digest::new("22".repeat(32)).unwrap(),
                    offset: 0,
                },
                installation: Id::new("installation").unwrap(),
            },
            kind: InstalledSoftwareKind::Application,
            name: "App".into(),
            version: Some("1.0".into()),
            native_id: id.into(),
            origin: "application".into(),
            scope: DataScope::Device,
            publisher: Some("TEAM".into()),
            actions: vec![],
        }
    }
    #[test]
    fn installation_default_allows_native_removal_but_never_invents_a_recipe_or_update() {
        let r = Rules::installation_default();
        r.validate().unwrap();
        assert!(r.allow_native_uninstall);
        assert!(!r.protects(&item("org.example.app")));
        assert!(r.protects(&item("com.apple.finder")));
        assert!(r.protects(&item("com.rss.mdmagent")));
        assert!(r
            .recipe(&item("org.example.app"), LocalSoftwareAction::Update)
            .is_none());
        assert!(!native::allowed(&r, LocalSoftwareAction::Update));
    }
    #[test]
    fn missing_rules_never_activate_product_defaults_at_runtime() {
        let path = std::env::temp_dir().join(format!("rss-missing-rules-{}", uuid::Uuid::new_v4()));
        assert!(Policy::open(&LocalSoftwareConfig {
            policy_file: path.clone()
        })
        .is_err());
        assert!(!path.exists());
    }
}
