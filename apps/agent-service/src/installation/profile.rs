//! Administrator selection is evaluated against the channels in the service assembly.
//! Native login images are components of that assembly, not independent product channels.
use super::*;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assembly {
    #[default]
    Mdm,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backend {
    pub instance: Uuid,
    pub tenant: Uuid,
    pub origin: String,
    pub ca_sha256: String,
    pub signing_keys: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Module {
    Disabled,
    Enabled { backend: Backend },
}

impl Module {
    pub fn enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modules {
    pub revision: u64,
    pub mdm: Module,
    pub da: Module,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Profile {
    pub modules: Modules,
    pub subjects: Vec<String>,
    pub helper_work_roots: BTreeMap<String, PathBuf>,
}

impl Modules {
    pub(super) fn authorize_update(&self, old: &Deployment) -> Result<(), Error> {
        // Remote changes can select only backends already enrolled by the administrator.
        // The signed connections seed belongs to MDM and cannot authorize a DA backend.
        if self.da.enabled() && !old.modules.as_ref().is_some_and(|m| m.da == self.da) {
            return Err(Error::Denied);
        }
        if let Module::Enabled { backend } = &self.mdm {
            if backend.instance != old.service_id || backend.tenant != old.tenant
                || backend.signing_keys != old.signing_keys
                || !old.connections.configuration.origins.contains(&backend.origin)
                || old.network()?.ca_pem.as_ref().is_none_or(|ca| hash(ca) != backend.ca_sha256)
            { return Err(Error::Denied); }
        }
        Ok(())
    }

    pub(super) fn validate(&self, candidate: &Candidate) -> Result<(), Error> {
        if self.revision == 0 || !(self.mdm.enabled() || self.da.enabled()) {
            return Err(Error::Configuration);
        }
        if self.da.enabled() {
            return Err(Error::Unsupported);
        }
        let Module::Enabled { backend } = &self.mdm else {
            return Err(Error::Unsupported);
        };
        if candidate.service_id != backend.instance
            || candidate.tenant != backend.tenant
            || backend.signing_keys != candidate.signing_keys
            || !candidate.connections.configuration.origins.contains(&backend.origin)
            || candidate.ca_pem.as_ref().is_none_or(|ca| hash(ca.as_bytes()) != backend.ca_sha256)
        {
            return Err(Error::Identity);
        }
        Ok(())
    }
}

impl Profile {
    pub(super) fn resolve(self, candidate: &Candidate, old: Option<&Deployment>)
        -> Result<(LocalAccess, Modules), Error>
    {
        self.modules.validate(candidate)?;
        if let Some(previous) = old.and_then(|d| d.modules.as_ref()) {
            let old = old.ok_or(Error::Identity)?;
            if self.modules.revision < previous.revision
                || self.modules.revision == previous.revision
                    && (self.modules != *previous || self.subjects != old.clients.subjects
                        || self.helper_work_roots != old.helper_work_roots)
            {
                return Err(Error::Conflict);
            }
        }
        Ok((LocalAccess { subjects: self.subjects, helper_work_roots: self.helper_work_roots }, self.modules))
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
