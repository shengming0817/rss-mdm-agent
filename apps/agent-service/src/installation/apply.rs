//! A single installation-lock holder drives one durable operation. The next step is
//! published before invoking launchd/SCM. Each native action verifies its exact owner.
use super::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Source {
    Package,
    Administrator,
    Backend {
        instance: Uuid,
        tenant: Uuid,
        device: Uuid,
        epoch: Uuid,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Received,
    Stop,
    Remove,
    Publish,
    Register,
    Desktop,
    StopCandidate,
    RemoveCandidate,
    Restore,
    Restart,
    Applied,
    Rejected,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    operation: Uuid,
    source: Source,
    config: PathBuf,
    artifact: String,
    profile_digest: Option<String>,
    installation: Uuid,
    instance: Uuid,
    tenant: Uuid,
    device: Option<Uuid>,
    epoch: Option<Uuid>,
    revision: Option<u64>,
    before: Option<String>,
    after: Option<String>,
    phase: Phase,
    failure: Option<Error>,
}

impl Record {
    fn terminal(&self) -> bool {
        matches!(self.phase, Phase::Applied | Phase::Rejected)
    }
    fn result(&self) -> Result<(), Error> {
        if self.phase == Phase::Applied {
            Ok(())
        } else {
            Err(self.failure.unwrap_or(Error::Unavailable))
        }
    }
    fn report(&self) {
        let state = match self.phase {
            Phase::Received => "received",
            Phase::Applied => "applied",
            Phase::Rejected => "rejected",
            _ => "pending",
        };
        println!("{}", serde_json::json!({"state":state,"application":self}));
    }
}

fn optional(path: &Path) -> Result<Option<Vec<u8>>, Error> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
        Ok(_) => installation_security::read_protected(path)
            .map(Some)
            .map_err(|_| Error::Identity),
    }
}

struct Journal {
    root: PathBuf,
}
impl Journal {
    fn new(config: &Path) -> Result<Self, Error> {
        Ok(Self {
            root: config
                .parent()
                .ok_or(Error::Configuration)?
                .join("install-operations"),
        })
    }
    fn active(&self) -> PathBuf {
        self.root.join("profile-active.json")
    }
    fn file(&self, id: Uuid, kind: &str) -> PathBuf {
        self.root.join(format!("{id}.profile-{kind}.json"))
    }
    fn read(&self, path: &Path) -> Result<Option<Record>, Error> {
        optional(path)?
            .map(|b| serde_json::from_slice(&b).map_err(|_| Error::Storage))
            .transpose()
    }
    fn save(&self, record: &Record) -> Result<(), Error> {
        publish(&self.active(), &json(record)?)
    }
    fn archive(&self, record: &Record) -> Result<(), Error> {
        retain_public(&self.file(record.operation, "result"), &json(record)?)?;
        std::fs::remove_file(self.active())?;
        for kind in ["before", "after"] {
            match std::fs::remove_file(self.file(record.operation, kind)) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

pub(super) fn ensure_idle(config: &Path) -> Result<(), Error> {
    let j = Journal::new(config)?;
    if j.read(&j.active())?.is_some_and(|r| !r.terminal()) {
        Err(Error::Conflict)
    } else {
        Ok(())
    }
}

pub(super) fn status(config: &Path, id: Uuid) -> Result<bool, Error> {
    let j = Journal::new(config)?;
    let record = match j.read(&j.file(id, "result"))? {
        Some(record) => Some(record),
        None => j.read(&j.active())?.filter(|r| r.operation == id),
    };
    if let Some(r) = record {
        r.report();
        Ok(true)
    } else {
        Ok(false)
    }
}

fn binding(d: &Deployment) -> Result<Option<(Uuid, Uuid)>, Error> {
    if !d.state_root.join("communication").exists() {
        return Ok(None);
    }
    let network = d.network()?;
    let binding =
        agent_client::inspect_optional_binding(&d.state_root.join("communication"), &network)?;
    Ok(binding.map(|b| (b.device_id.into_uuid(), b.epoch)))
}

pub(super) fn module_status(config: &Path) -> Result<(), Error> {
    if optional(config)?.is_none() {
        return Ok(());
    }
    let deployment = Deployment::metadata(config)?;
    let candidate = Candidate::retained(config, &deployment)?;
    let observed = binding(&deployment);
    let registered = observed.as_ref().ok().map(|value| value.is_some());
    println!(
        "{}",
        serde_json::json!({
            "available": candidate.assembly,
            "enabled": { "mdm":deployment.modules.as_ref().is_none_or(|m| m.mdm.enabled()),
                         "da":deployment.modules.as_ref().is_some_and(|m| m.da.enabled()) },
            "mdmRegistered": registered,
            "revision":deployment.modules.as_ref().map(|m| m.revision),
        })
    );
    Ok(())
}
fn prepare(
    config: &Path,
    candidate: &Candidate,
    bytes: Option<&[u8]>,
    j: &Journal,
    r: &mut Record,
) -> Result<(), Error> {
    let original = optional(config)?;
    let old = original
        .as_ref()
        .map(|_| Deployment::metadata(config))
        .transpose()?;
    if let Some(old) = &old {
        if old.installation_id != r.installation {
            return Err(Error::Identity);
        }
        (r.device, r.epoch) = binding(old)?.unzip();
    }
    let (access, modules) = match bytes {
        Some(bytes) => {
            let p: profile::Profile =
                serde_json::from_slice(bytes).map_err(|_| Error::Configuration)?;
            r.revision = Some(p.modules.revision);
            if let Source::Backend {
                instance,
                tenant,
                device,
                epoch,
            } = &r.source
            {
                let d = old.as_ref().ok_or(Error::Denied)?;
                if (*instance, *tenant, Some(*device), Some(*epoch))
                    != (d.service_id, d.tenant, r.device, r.epoch)
                    || p.subjects != d.clients.subjects
                    || p.helper_work_roots != d.helper_work_roots
                {
                    return Err(Error::Denied);
                }
                p.modules.authorize_update(d)?;
            }
            let (a, m) = p.resolve(candidate, old.as_ref())?;
            (a, Some(m))
        }
        None => match &old {
            Some(d) => (LocalAccess::from_deployment(d), d.modules.clone()),
            None => (lifecycle::initial_profile()?, None),
        },
    };
    if let Some(m) = &modules {
        m.validate(candidate)?;
        r.revision = Some(m.revision);
    }
    platform::preflight_desktop_entry(candidate)?;
    let mut next = candidate.deployment(r.artifact.clone(), access, config)?;
    next.installation_id = r.installation;
    next.modules = modules;
    if let Some(old) = &old {
        lifecycle::inherit(old, &mut next)?;
        if let Some(ca) = &old.ca_file {
            if installation_security::read_protected(ca).map_err(|_| Error::Identity)?
                != candidate
                    .ca_pem
                    .as_deref()
                    .ok_or(Error::Identity)?
                    .as_bytes()
            {
                return Err(Error::Identity);
            }
        }
    } else if std::fs::symlink_metadata(&next.state_root).is_ok() {
        return Err(Error::Identity);
    }
    let target = match (&old, &original) {
        (Some(old), Some(bytes)) if json(old)? == json(&next)? => bytes.clone(),
        _ => json(&next)?,
    };
    let staged = j.file(r.operation, "after");
    retain_public(&staged, &target)?;
    if let Some(before) = original {
        retain_public(&j.file(r.operation, "before"), &before)?;
        r.before = Some(hash(&before));
    }
    r.after = Some(hash(&target));
    if let Some(old) = &old {
        lifecycle::preflight_refresh(old, config, &next, &staged)?;
    } else {
        if let Some(ca) = &next.ca_file {
            retain_public(
                ca,
                candidate
                    .ca_pem
                    .as_deref()
                    .ok_or(Error::Identity)?
                    .as_bytes(),
            )?;
        }
        Deployment::load(&staged)?;
    }
    // A byte-equivalent effective deployment needs only native registration readback/repair.
    r.phase = if r.before == r.after {
        Phase::Register
    } else if old.is_some() {
        Phase::Stop
    } else {
        Phase::Publish
    };
    Ok(())
}

fn snapshot(path: &Path, digest: &str) -> Result<Vec<u8>, Error> {
    optional(path)?
        .filter(|b| hash(b) == digest)
        .ok_or(Error::Identity)
}

fn drive(config: &Path, candidate: &Candidate, j: &Journal, r: &mut Record) -> Result<(), Error> {
    use Phase::*;
    let next_bytes = snapshot(
        &j.file(r.operation, "after"),
        r.after.as_deref().ok_or(Error::Storage)?,
    )?;
    let next: Deployment = serde_json::from_slice(&next_bytes).map_err(|_| Error::Storage)?;
    let previous = r
        .before
        .as_ref()
        .map(|digest| snapshot(&j.file(r.operation, "before"), digest))
        .transpose()?;
    let old: Option<Deployment> = previous
        .as_ref()
        .map(|b| serde_json::from_slice(b).map_err(|_| Error::Storage))
        .transpose()?;
    if r.config != config
        || next.installation_id != r.installation
        || next.installation_artifact != r.artifact
    {
        return Err(Error::Identity);
    }

    let source = r.source.clone();
    drive_steps(
        r,
        old.is_some(),
        |r| j.save(r),
        |step| {
            let pointer = optional(config)?;
            let correct = match step {
                Stop | Remove | Restart => pointer == previous,
                Register | Desktop | StopCandidate | RemoveCandidate => {
                    pointer.as_ref() == Some(&next_bytes)
                }
                Publish | Restore => pointer == previous || pointer.as_ref() == Some(&next_bytes),
                _ => false,
            };
            if !correct {
                return Err(Error::Identity);
            }
            match step {
                Stop | Remove | Restart => {
                    let old = old.as_ref().ok_or(Error::Storage)?;
                    match step {
                        Stop => {
                            platform::quiescent(old, config)?;
                            platform::stop(true, &old.service.path, config)
                        }
                        Remove => platform::remove(true, &old.service.path, config),
                        _ => platform::restore_registration(&old.service.path, config),
                    }
                }
                Publish => {
                    if let Source::Backend { device, epoch, .. } = source {
                        if binding(old.as_ref().ok_or(Error::Denied)?)? != Some((device, epoch)) {
                            return Err(Error::Denied);
                        }
                    }
                    crate::local_permissions::initialize(&next.execution.local_permissions)?;
                    publish(config, &next_bytes)
                }
                Register => platform::register(true, &next.service.path, config),
                Desktop => platform::desktop_entry(candidate),
                StopCandidate => {
                    platform::quiescent(&next, config)?;
                    platform::stop(true, &next.service.path, config)
                }
                RemoveCandidate => platform::remove(true, &next.service.path, config),
                Restore => publish(config, previous.as_ref().ok_or(Error::Storage)?),
                _ => Err(Error::Storage),
            }
        },
    )
}

fn drive_steps(
    r: &mut Record,
    upgrading: bool,
    mut checkpoint: impl FnMut(&Record) -> Result<(), Error>,
    mut effect: impl FnMut(Phase) -> Result<(), Error>,
) -> Result<(), Error> {
    use Phase::*;
    while !r.terminal() {
        let step = r.phase;
        r.phase = match effect(step) {
            Ok(()) => match step {
                Stop => Remove,
                Remove => Publish,
                Publish => Register,
                Register => Desktop,
                Desktop => Applied,
                StopCandidate => RemoveCandidate,
                RemoveCandidate => Restore,
                Restore => Restart,
                Restart => Rejected,
                _ => return Err(Error::Storage),
            },
            Err(e) => {
                r.failure = Some(e);
                match (step, upgrading) {
                    (Remove, true) => Restart,
                    (Publish, true) => Restore,
                    (Register, true) => StopCandidate,
                    _ => {
                        checkpoint(r)?;
                        return Err(e);
                    }
                }
            }
        };
        if r.phase == Applied {
            r.failure = None;
        }
        checkpoint(r)?;
    }
    Ok(())
}

pub(super) fn run(
    config: &Path,
    supplied: Option<&Path>,
    profile: Option<&[u8]>,
    id: Option<Uuid>,
    source: Source,
) -> Result<(), Error> {
    let j = Journal::new(config)?;
    platform::public_directory(&j.root)?;
    let mut pending = j.read(&j.active())?;
    if let Some(r) = pending.as_ref().filter(|r| r.terminal()) {
        j.archive(r)?;
        pending = None;
    }
    let prior = id
        .map(|id| j.read(&j.file(id, "result")))
        .transpose()?
        .flatten();
    let resume =
        supplied.is_none() && profile.is_none() && id.is_some() && source == Source::Administrator;
    if resume {
        let id = id.ok_or(Error::Configuration)?;
        if let Some(r) = prior.as_ref() {
            if r.config != config {
                return Err(Error::Identity);
            }
            r.report();
            return r.result();
        }
        let r = pending
            .as_ref()
            .filter(|r| r.operation == id && r.config == config)
            .ok_or(Error::NotFound)?;
        if matches!(r.source, Source::Backend { .. })
            || (r.phase == Phase::Received && r.profile_digest.is_some())
        {
            return Err(Error::Denied);
        }
    }
    let digest = profile.map(hash);
    if let Some(prior) = prior.as_ref() {
        if prior.config != config
            || prior.source != source
            || prior.profile_digest != digest
            || supplied.is_some_and(|p| {
                optional(p)
                    .ok()
                    .flatten()
                    .is_none_or(|b| hash(&b) != prior.artifact)
            })
        {
            return Err(Error::Conflict);
        }
        prior.report();
        return prior.result();
    }
    let old = optional(config)?
        .map(|_| Deployment::metadata(config))
        .transpose()?;
    let manifests = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-manifests");
    let candidate_path = match supplied {
        Some(path) => path.to_owned(),
        None => {
            if let Some(r) = pending.as_ref().filter(|r| id == Some(r.operation)) {
                manifests.join(format!("{}.json", r.artifact))
            } else {
                let d = old.as_ref().ok_or(Error::Configuration)?;
                Candidate::retained(config, d)?;
                manifests.join(format!("{}.json", d.installation_artifact))
            }
        }
    };
    let (candidate, artifact) = Candidate::load(&candidate_path)?;
    let matches_request = |r: &Record| {
        r.config == config
            && (resume
                || (r.profile_digest == digest && r.artifact == artifact && r.source == source))
    };
    platform::public_directory(&manifests)?;
    retain_public(
        &manifests.join(format!("{artifact}.json")),
        &snapshot(&candidate_path, &artifact)?,
    )?;
    let mut r = if let Some(r) = pending {
        if id.is_some_and(|id| id != r.operation) || !matches_request(&r) {
            return Err(Error::Conflict);
        }
        r
    } else {
        let r = Record {
            operation: id.unwrap_or_else(Uuid::new_v4),
            source,
            config: config.to_owned(),
            artifact,
            profile_digest: digest,
            installation: old
                .as_ref()
                .map_or_else(Uuid::new_v4, |d| d.installation_id),
            instance: candidate.service_id,
            tenant: candidate.tenant,
            device: None,
            epoch: None,
            revision: None,
            before: None,
            after: None,
            phase: Phase::Received,
            failure: None,
        };
        j.save(&r)?;
        r
    };
    outcome::application(r.operation)?;
    r.report();
    let mut rejected = None;
    if r.phase == Phase::Received {
        if let Err(e) = prepare(config, &candidate, profile, &j, &mut r) {
            r.failure = Some(e);
            r.phase = Phase::Rejected;
            rejected = Some(e);
        }
        j.save(&r)?;
    }
    if !r.terminal() {
        drive(config, &candidate, &j, &mut r)?;
    }
    j.archive(&r)?;
    r.report();
    if let Some(e) = rejected {
        return Err(e);
    }
    r.result()
}

/// Result of the trusted channel's purpose-specific authorization at the installed
/// device. Only the verified profile bytes returned here enter the installation owner.
/// This callback runs inside the privileged maintenance worker, without a transport.
pub struct AuthorizedProfile {
    pub operation: Uuid,
    pub instance: Uuid,
    pub tenant: Uuid,
    pub device: Uuid,
    pub epoch: Uuid,
    pub profile: Vec<u8>,
}

pub fn apply_authorized_profile(
    authorize: impl FnOnce(&Deployment) -> Result<AuthorizedProfile, Error>,
) -> Result<(), Error> {
    let config = Deployment::default_path()?;
    let deployment = Deployment::load(&config)?;
    deployment.require_service()?;
    let _lock = lifecycle::installation_lock(&config, true)?;
    let deployment = Deployment::load(&config)?;
    deployment.require_service()?;
    let update = authorize(&deployment)?;
    if update.operation.is_nil() || update.profile.len() > installation_security::MAX_FRAME {
        return Err(Error::Configuration);
    }
    let source = Source::Backend {
        instance: update.instance,
        tenant: update.tenant,
        device: update.device,
        epoch: update.epoch,
    };
    run(
        &config,
        None,
        Some(&update.profile),
        Some(update.operation),
        source,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(phase: Phase) -> Record {
        Record {
            operation: Uuid::from_u128(11),
            source: Source::Administrator,
            config: "/unused".into(),
            artifact: "a".repeat(64),
            profile_digest: None,
            installation: Uuid::from_u128(12),
            instance: Uuid::from_u128(13),
            tenant: Uuid::from_u128(14),
            device: None,
            epoch: None,
            revision: Some(1),
            before: None,
            after: None,
            phase,
            failure: None,
        }
    }

    // Exercise the production phase driver across an actual process exit between the effect
    // and its checkpoint. The file represents OS readback; no machine service is replaced.
    #[test]
    fn interrupted_apply_child() {
        let Some(root) = std::env::var_os("RSS_TEST_APPLY_CRASH_ROOT").map(PathBuf::from) else {
            return;
        };
        let state = root.join("state");
        let mut r: Record = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
        let crash: Phase =
            serde_json::from_slice(&std::fs::read(root.join("crash")).unwrap()).unwrap();
        drive_steps(
            &mut r,
            true,
            |r| {
                std::fs::write(&state, json(r)?)?;
                Ok(())
            },
            |step| {
                let marker = root.join(format!("{step:?}"));
                if !marker.exists() {
                    std::fs::write(marker, b"observed")?;
                    if step == crash {
                        std::process::exit(86);
                    }
                }
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn process_exit_resumes_the_original_effect_and_operation() {
        for (start, crash) in [
            (Phase::Publish, Phase::Publish),
            (Phase::Stop, Phase::Stop),
            (Phase::Stop, Phase::Publish),
            (Phase::Stop, Phase::Register),
        ] {
            let root = crate::protocol_test_support::Root::new();
            let state = root.path.join("state");
            let original = record(start);
            std::fs::write(&state, json(&original).unwrap()).unwrap();
            std::fs::write(root.path.join("crash"), json(&crash).unwrap()).unwrap();
            let mut child = Command::new(std::env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "installation::apply::tests::interrupted_apply_child",
                ])
                .env("RSS_TEST_APPLY_CRASH_ROOT", &root.path);
            assert_eq!(child.status().unwrap().code(), Some(86));
            let pending: Record = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
            assert_eq!(pending.phase, crash);
            assert!(child.status().unwrap().success());
            let completed: Record = serde_json::from_slice(&std::fs::read(state).unwrap()).unwrap();
            assert_eq!(
                (completed.operation, completed.phase),
                (original.operation, Phase::Applied)
            );
        }
    }

    #[test]
    fn uncertain_candidate_stop_preserves_pointer_until_compensation_can_finish() {
        let mut r = record(Phase::Register);
        let mut seen = Vec::new();
        assert_eq!(
            drive_steps(
                &mut r,
                true,
                |_| Ok(()),
                |step| {
                    seen.push(step);
                    Err(Error::Unavailable)
                }
            ),
            Err(Error::Unavailable)
        );
        assert_eq!(seen, [Phase::Register, Phase::StopCandidate]);
        assert_eq!(r.phase, Phase::StopCandidate);
        drive_steps(
            &mut r,
            true,
            |_| Ok(()),
            |step| {
                seen.push(step);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(r.phase, Phase::Rejected);
        assert_eq!(r.result(), Err(Error::Unavailable));
    }
}
