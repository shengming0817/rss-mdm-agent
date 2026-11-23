//! Administrator selection is evaluated against the channels in the service assembly.
//! Native login images are components of that assembly, not independent product channels.
use super::*;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assembly {
    pub mdm: bool,
    pub da: bool,
}
impl Assembly {
    // The production service currently links only the MDM registration/maintenance channel.
    pub const CURRENT: Self = Self {
        mdm: true,
        da: false,
    };
    pub fn baseline_mdm() -> Self {
        Self {
            mdm: true,
            da: false,
        }
    }
    fn accepts(self, modules: &Modules) -> Result<(), Error> {
        match (modules.mdm.enabled(), modules.da.enabled()) {
            (false, false) => Err(Error::Configuration),
            (true, _) if !self.mdm => Err(Error::Unsupported),
            (_, true) if !self.da => Err(Error::Unsupported),
            _ => Ok(()),
        }
    }
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
    pub(crate) fn require_mdm_assembly(&self, deployment: &Deployment) -> Result<(), Error> {
        if self.revision == 0 {
            return Err(Error::Configuration);
        }
        Assembly::CURRENT.accepts(self)?;
        let Module::Enabled { backend } = &self.mdm else {
            return Err(Error::Unsupported);
        };
        if backend.instance != deployment.service_id
            || backend.tenant != deployment.tenant
            || backend.signing_keys != deployment.signing_keys
            || deployment.connections.configuration.preferred_origin != backend.origin
            || !digest_valid(&backend.ca_sha256)
        {
            return Err(Error::Identity);
        }
        Ok(())
    }

    pub(crate) fn verify_ca(&self, ca: Option<&[u8]>) -> Result<(), Error> {
        if let Module::Enabled { backend } = &self.mdm {
            if ca.is_none_or(|pem| hash(pem) != backend.ca_sha256) {
                return Err(Error::Identity);
            }
        }
        Ok(())
    }
    pub(super) fn authorize_update(&self, old: &Deployment) -> Result<(), Error> {
        // Remote changes can select only backends already enrolled by the administrator.
        // The signed connections seed belongs to MDM and cannot authorize a DA backend.
        if self.da.enabled() && !old.modules.as_ref().is_some_and(|m| m.da == self.da) {
            return Err(Error::Denied);
        }
        if let Module::Enabled { backend } = &self.mdm {
            if backend.instance != old.service_id
                || backend.tenant != old.tenant
                || backend.signing_keys != old.signing_keys
                || old.connections.configuration.preferred_origin != backend.origin
                || old
                    .network()?
                    .ca_pem
                    .as_ref()
                    .is_none_or(|ca| hash(ca) != backend.ca_sha256)
            {
                return Err(Error::Denied);
            }
        }
        Ok(())
    }

    pub(super) fn validate(&self, candidate: &Candidate) -> Result<(), Error> {
        if self.revision == 0 {
            return Err(Error::Configuration);
        }
        candidate.assembly.accepts(self)?;
        let Module::Enabled { backend } = &self.mdm else {
            return Err(Error::Unsupported);
        };
        if candidate.service_id != backend.instance
            || candidate.tenant != backend.tenant
            || backend.signing_keys != candidate.signing_keys
            || candidate.connections.configuration.preferred_origin != backend.origin
            || candidate
                .ca_pem
                .as_ref()
                .is_none_or(|ca| hash(ca.as_bytes()) != backend.ca_sha256)
        {
            return Err(Error::Identity);
        }
        Ok(())
    }
}

impl Profile {
    pub(super) fn resolve(self, old: Option<&Deployment>) -> Result<(LocalAccess, Modules), Error> {
        if let Some((old, previous)) = old.and_then(|d| d.modules.as_ref().map(|m| (d, m))) {
            if self.modules.revision < previous.revision
                || self.modules.revision == previous.revision
                    && (self.modules != *previous
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
    fn backend() -> Backend {
        Backend {
            instance: Uuid::from_u128(1),
            tenant: Uuid::from_u128(2),
            origin: "https://agent.example.org".into(),
            ca_sha256: "b".repeat(64),
            signing_keys: BTreeMap::new(),
        }
    }
    #[test]
    fn explicit_profile_requires_both_choices_and_the_linked_channel() {
        assert!(serde_json::from_value::<Profile>(
            serde_json::json!({"subjects":[],"helper_work_roots":{}})
        )
        .is_err());
        let enabled = Module::Enabled { backend: backend() };
        let dual = Assembly {
            mdm: true,
            da: true,
        };
        for (mdm, da, result) in [
            (false, false, Err(Error::Configuration)),
            (true, false, Ok(())),
            (false, true, Err(Error::Unsupported)),
            (true, true, Err(Error::Unsupported)),
        ] {
            let modules = Modules {
                revision: 1,
                mdm: if mdm {
                    enabled.clone()
                } else {
                    Module::Disabled
                },
                da: if da {
                    enabled.clone()
                } else {
                    Module::Disabled
                },
            };
            assert_eq!(Assembly::CURRENT.accepts(&modules), result);
            assert_eq!(dual.accepts(&modules).is_ok(), mdm || da);
            let mut value = serde_json::to_value(&modules).unwrap();
            value.as_object_mut().unwrap().remove("da");
            assert!(serde_json::from_value::<Modules>(value).is_err());
        }
    }
    #[test]
    fn backend_cannot_join_an_independent_product_or_change_authority() {
        let d = crate::deployment::tests::deployment();
        let mut modules = Modules {
            revision: 1,
            mdm: Module::Disabled,
            da: Module::Enabled { backend: backend() },
        };
        assert_eq!(modules.authorize_update(&d), Err(Error::Denied));
        assert_eq!(modules.require_mdm_assembly(&d), Err(Error::Unsupported));
        std::mem::swap(&mut modules.mdm, &mut modules.da);
        assert_eq!(modules.authorize_update(&d), Err(Error::Denied));
        assert_eq!(modules.require_mdm_assembly(&d), Err(Error::Identity));
    }
}
