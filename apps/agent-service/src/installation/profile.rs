//! Installed module selection and channel-specific public trust. Only an administrator can
//! introduce a backend. A package default is resolved once, at first installation.
use super::*;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assembly {
    pub mdm: bool,
    pub da: bool,
}
impl Assembly {
    pub const CURRENT: Self = Self {
        mdm: true,
        da: true,
    };

    fn accepts(self, selection: &Modules) -> Result<(), Error> {
        if !selection.mdm.enabled() && !selection.da.enabled() {
            return Err(Error::Configuration);
        }
        if selection.mdm.enabled() && !self.mdm || selection.da.enabled() && !self.da {
            return Err(Error::Unsupported);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MdmBackend {
    pub instance: Uuid,
    pub tenant: Uuid,
    pub connections: rss_mdm_agent_wire::connections::SignedConfiguration,
    pub signing_keys: BTreeMap<String, String>,
    pub ca_pem: String,
}
impl MdmBackend {
    fn validate(&self) -> Result<(), Error> {
        use base64::Engine;
        if self.instance.is_nil()
            || self.tenant.is_nil()
            || self.signing_keys.is_empty()
            || self.signing_keys.len() > 32
        {
            return Err(Error::Configuration);
        }
        let keys = self
            .signing_keys
            .iter()
            .map(|(id, value)| {
                let key = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(value)
                    .map_err(|_| Error::Configuration)?;
                if id.is_empty()
                    || id.len() > 128
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    || key.len() != 32
                {
                    return Err(Error::Configuration);
                }
                Ok((id.clone(), key))
            })
            .collect::<Result<_, Error>>()?;
        agent_client::validate_public_ca(self.ca_pem.as_bytes())?;
        agent_client::verify_connections(&self.connections, self.instance, self.tenant, &keys)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Module<T> {
    Disabled,
    Enabled { backend: T },
}
impl<T> Module<T> {
    pub fn enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }
    pub fn backend(&self) -> Result<&T, Error> {
        match self {
            Self::Enabled { backend } => Ok(backend),
            Self::Disabled => Err(Error::Unsupported),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modules {
    pub revision: u64,
    pub mdm: Module<MdmBackend>,
    pub da: Module<agent_client::da::Backend>,
}
impl Modules {
    pub(crate) fn validate_for(&self, assembly: Assembly) -> Result<(), Error> {
        assembly.accepts(self)?;
        if self.revision == 0 {
            return Err(Error::Configuration);
        }
        if let Module::Enabled { backend } = &self.mdm {
            backend.validate()?;
        }
        if let Module::Enabled { backend } = &self.da {
            backend.validate()?;
        }
        Ok(())
    }
    pub(super) fn authorize_update(&self, old: &Deployment) -> Result<(), Error> {
        if self.da != old.modules.da || self.mdm.enabled() && self.mdm != old.modules.mdm {
            return Err(Error::Denied);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Profile {
    pub modules: Modules,
    pub subjects: Vec<String>,
    pub helper_work_roots: BTreeMap<String, PathBuf>,
}
impl Profile {
    pub(super) fn resolve(self, old: Option<&Deployment>) -> Result<(LocalAccess, Modules), Error> {
        if let Some(old) = old {
            if self.modules.revision < old.modules.revision
                || self.modules.revision == old.modules.revision
                    && (self.modules != old.modules
                        || self.subjects != old.clients.subjects
                        || self.helper_work_roots != old.helper_work_roots)
            {
                return Err(Error::Conflict);
            }
        }
        Ok((
            LocalAccess {
                subjects: self.subjects,
                helper_work_roots: self.helper_work_roots,
            },
            self.modules,
        ))
    }
}
impl LocalAccess {
    pub(super) fn from_deployment(d: &Deployment) -> Self {
        Self {
            subjects: d.clients.subjects.clone(),
            helper_work_roots: d.helper_work_roots.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_defaults_select_real_channels_and_remote_update_cannot_join_da() {
        let root = crate::protocol_test_support::Root::new();
        let mut package = super::super::tests::candidate(&root);
        let mdm = package.defaults.mdm.clone();
        let da = Module::Enabled {
            backend: agent_client::da::Backend {
                instance: Uuid::new_v4(),
                tenant: Uuid::new_v4(),
                origin: "https://da.example.test".into(),
                ca_pem: String::from_utf8(crate::protocol_test_support::pki::ca()).unwrap(),
            },
        };
        for (m, d, valid) in [
            (Module::Disabled, Module::Disabled, false),
            (mdm.clone(), Module::Disabled, true),
            (Module::Disabled, da.clone(), true),
            (mdm, da, true),
        ] {
            package.defaults.mdm = m;
            package.defaults.da = d;
            assert_eq!(package.validate_trust().is_ok(), valid);
        }
        let old = crate::deployment::tests::deployment();
        assert_eq!(package.defaults.authorize_update(&old), Err(Error::Denied));
        for field in ["assembly", "defaults"] {
            let mut value = serde_json::to_value(&package).unwrap();
            value.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<Candidate>(value).is_err());
        }
    }
}
