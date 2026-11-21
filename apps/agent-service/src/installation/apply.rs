//! A single installation-lock holder drives one durable operation. The next step is
//! published before invoking launchd/SCM. Each native action verifies its exact owner.
use super::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Source {
    Package,
    Administrator,
    Msi,
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
    CancelStop,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<String>,
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
    fn compensating(&self) -> bool {
        matches!(
            self.phase,
            Phase::CancelStop
                | Phase::StopCandidate
                | Phase::RemoveCandidate
                | Phase::Restore
                | Phase::Restart
        )
    }
    fn phase_name(&self) -> &'static str {
        use Phase::*;
        match self.phase {
            Received => "profile-preflight",
            Stop => "stop-current",
            Remove => "remove-current",
            Publish => "publish-candidate",
            Register => "register-candidate",
            Desktop => "desktop-entry",
            StopCandidate => "stop-candidate",
            RemoveCandidate => "remove-candidate",
            Restore => "restore-current",
            Restart => "register-current",
            CancelStop => "confirm-old-stopped",
            Applied => "profile-applied",
            Rejected => "profile-rejected",
        }
    }
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
    fn report(&self) -> Result<(), Error> {
        let state = match self.phase {
            Phase::Received => "received",
            Phase::Applied => "applied",
            Phase::Rejected => "rejected",
            _ => "pending",
        };
        outcome::details(serde_json::json!({"state":state,"operation":self.operation,
            "phase":self.phase,"revision":self.revision,"failure":self.failure}))
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
                .join("install-operations")
                .join("profiles"),
        })
    }
    fn active(&self) -> PathBuf {
        self.root.join("profile-active.json")
    }
    fn file(&self, id: Uuid, kind: &str) -> PathBuf {
        self.root.join(format!("{id}.profile-{kind}.json"))
    }
    fn create(&self) -> Result<(), Error> {
        platform::public_directory(self.root.parent().ok_or(Error::Storage)?)?;
        platform_private_storage::directory(&self.root)?;
        Ok(())
    }
    fn read(&self, path: &Path) -> Result<Option<Record>, Error> {
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
            Ok(_) => {
                let bytes =
                    platform_private_storage::read(path, installation_security::MAX_FRAME * 4)?;
                Ok(Some(
                    serde_json::from_slice(&bytes).map_err(|_| Error::Storage)?,
                ))
            }
        }
    }
    fn save(&self, record: &Record) -> Result<(), Error> {
        let staged = self.root.join(format!("{}.tmp", Uuid::new_v4()));
        platform_private_storage::write_new(&staged, &json(record)?)?;
        let result = platform_private_storage::replace(&staged, &self.active());
        if result.is_err() {
            let _ = std::fs::remove_file(staged);
        }
        result?;
        if record.compensating() {
            outcome::compensation("running");
        }
        outcome::phase(record.phase_name())
    }
    fn archive(&self, record: &Record) -> Result<(), Error> {
        // Atomically move the terminal checkpoint to immutable per-operation history.
        platform_private_storage::replace(&self.active(), &self.file(record.operation, "result"))?;
        for kind in ["before", "after", "stop", "stop-candidate"] {
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

#[cfg(windows)]
pub(super) fn check_msi_rollback(
    config: &Path,
    operation: Uuid,
    finished: bool,
) -> Result<(), Error> {
    let j = Journal::new(config)?;
    if let Some(mut r) = j.read(&j.active())? {
        if r.operation != operation || r.config != config || r.source != Source::Msi {
            return Err(Error::Conflict);
        }
        if finished {
            r.phase = Phase::Rejected;
            r.failure = Some(Error::Unavailable);
            r.input = None;
            j.save(&r)?;
            j.archive(&r)?;
        }
    }
    Ok(())
}
pub(super) fn status(config: &Path, id: Uuid) -> Result<bool, Error> {
    let j = Journal::new(config)?;
    let record = match j.read(&j.file(id, "result"))? {
        Some(record) => Some(record),
        None => j.read(&j.active())?.filter(|r| r.operation == id),
    };
    if let Some(r) = record {
        if r.config != config || r.operation != id {
            return Err(Error::Identity);
        }
        r.report()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn binding(d: &Deployment) -> Result<Option<(Uuid, Uuid)>, Error> {
    if !d.state_root.join("communication").try_exists()? {
        return Ok(None);
    }
    let network = d.network()?;
    let binding =
        agent_client::inspect_optional_binding(&d.state_root.join("communication"), &network)?;
    Ok(binding.map(|b| (b.device_id.into_uuid(), b.epoch)))
}

pub(super) fn module_status(config: &Path) -> Result<(), Error> {
    if optional(config)?.is_none() {
        let j = Journal::new(config)?;
        if let Some(record) = j.read(&j.active())? {
            record.report()?;
        }
        return Ok(());
    }
    let deployment = Deployment::metadata(config)?;
    let candidate = Candidate::retained(config, &deployment)?;
    let observed = binding(&deployment);
    let registered = observed.as_ref().ok().map(|value| value.is_some());
    let j = Journal::new(config)?;
    let pending = j.read(&j.active())?.map(|r| r.operation);
    outcome::details(serde_json::json!({
        "available": candidate.assembly,
        "enabled": { "mdm":deployment.modules.as_ref().is_none_or(|m| m.mdm.enabled()), "da":deployment.modules.as_ref().is_some_and(|m| m.da.enabled()) },
        "mdmRegistered":registered,"pendingOperation":pending,
        "revision":deployment.modules.as_ref().map(|m|m.revision),
    }))?;
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

fn stop(deployment: &Deployment, config: &Path, evidence: &Path) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        platform::stop_for_apply(deployment, config, evidence)
    }
    #[cfg(windows)]
    {
        let _ = evidence;
        platform::quiescent(deployment, config)?;
        platform::stop(true, &deployment.service.path, config)
    }
}

fn rollback(r: &mut Record, config: &Path, failure: Error) -> Result<(), Error> {
    if r.compensating() || r.terminal() {
        return Ok(());
    }
    r.failure = Some(failure);
    if r.phase == Phase::Received {
        r.phase = Phase::Rejected;
        return Ok(());
    }
    let active = optional(config)?.as_deref().map(hash);
    if r.before.is_none() {
        return Err(failure);
    }
    r.phase = if active == r.after
        && matches!(r.phase, Phase::Publish | Phase::Register | Phase::Desktop)
    {
        Phase::StopCandidate
    } else if active == r.before {
        Phase::CancelStop
    } else {
        return Err(Error::Identity);
    };
    Ok(())
}

fn drive(
    config: &Path,
    candidate: Option<&Candidate>,
    j: &Journal,
    r: &mut Record,
) -> Result<(), Error> {
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
    let operation = r.operation;
    drive_steps(
        r,
        old.is_some(),
        |r| j.save(r),
        |step| {
            let pointer = optional(config)?;
            let correct = match step {
                Stop | CancelStop | Remove | Restart => pointer == previous,
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
                Stop | CancelStop | Remove | Restart => {
                    let old = old.as_ref().ok_or(Error::Storage)?;
                    match step {
                        Stop | CancelStop => stop(old, config, &j.file(operation, "stop")),
                        Remove => platform::remove(true, &old.service.path, config),
                        _ => platform::restore_registration(&old.service.path, config),
                    }
                }
                Publish => {
                    if let Source::Backend { device, epoch, .. } = source {
                        let d = old.as_ref().ok_or(Error::Denied)?;
                        let secrets = crate::DeviceSecrets::open(
                            &d.state_root.join("secrets"),
                            &d.namespace()?,
                        )?;
                        secrets.verify_storage(&d.state_root)?;
                        let current = agent_client::validate_identity(
                            &d.state_root.join("communication"),
                            &d.network()?,
                            &secrets,
                            &SystemClock::new()?,
                        )?;
                        if (current.device_id.into_uuid(), current.epoch) != (device, epoch) {
                            return Err(Error::Denied);
                        }
                    }
                    crate::local_permissions::initialize(&next.execution.local_permissions)?;
                    publish(config, &next_bytes)
                }
                Register => platform::register(true, &next.service.path, config),
                Desktop => platform::desktop_entry(candidate.ok_or(Error::Identity)?),
                StopCandidate => stop(&next, config, &j.file(operation, "stop-candidate")),
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
                CancelStop => Restart,
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
    j.create()?;
    let digest = profile.map(hash);
    let resume =
        source == Source::Administrator && profile.is_none() && supplied.is_none() && id.is_some();
    let manifests = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-manifests");
    let supplied_bytes = supplied
        .map(|p| optional(p)?.ok_or(Error::NotFound))
        .transpose()?;
    let supplied_digest = supplied_bytes.as_deref().map(hash);
    let matches = |r: &Record| {
        r.config == config
            && (resume
                || (r.source == source
                    && r.profile_digest == digest
                    && supplied_digest.as_ref().is_none_or(|d| d == &r.artifact)))
    };
    let mut pending = j.read(&j.active())?;
    if let Some(r) = pending.as_ref().filter(|r| r.terminal()) {
        j.archive(r)?;
        pending = None;
    }
    if let Some(id) = id {
        if let Some(r) = j.read(&j.file(id, "result"))? {
            if !matches(&r) {
                return Err(Error::Conflict);
            }
            r.report()?;
            return r.result();
        }
    }
    let mut record = if let Some(mut r) = pending {
        if id.is_some_and(|id| id != r.operation) || !matches(&r) {
            r.report()?;
            return Err(Error::Conflict);
        }
        if resume && matches!(r.source, Source::Backend { .. }) {
            rollback(&mut r, config, Error::Denied)?;
            j.save(&r)?;
        }
        r
    } else {
        if resume {
            return Err(Error::NotFound);
        }
        let old = optional(config)?
            .map(|_| Deployment::metadata(config))
            .transpose()?;
        let artifact = match (supplied_bytes, supplied_digest) {
            (Some(bytes), Some(digest)) => {
                let candidate: Candidate =
                    serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
                candidate.validate_trust()?;
                platform::public_directory(&manifests)?;
                retain_public(&manifests.join(format!("{digest}.json")), &bytes)?;
                digest
            }
            _ => {
                let old = old.as_ref().ok_or(Error::Configuration)?;
                Candidate::retained(config, old)?;
                old.installation_artifact.clone()
            }
        };
        let bytes = snapshot(&manifests.join(format!("{artifact}.json")), &artifact)?;
        let candidate: Candidate =
            serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
        // Retain only strictly decoded public profile fields, never an invalid payload.
        let input = profile.map(|raw| {
            let text = std::str::from_utf8(raw).map_err(|_| Error::Configuration)?;
            serde_json::from_str::<profile::Profile>(text).map_err(|_| Error::Configuration)?;
            Ok::<String, Error>(text.to_owned())
        });
        let malformed = input.as_ref().is_some_and(Result::is_err);
        let r = Record {
            operation: id.unwrap_or_else(Uuid::new_v4),
            config: config.to_owned(),
            source,
            artifact,
            profile_digest: digest,
            input: input.and_then(Result::ok),
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
            phase: if malformed {
                Phase::Rejected
            } else {
                Phase::Received
            },
            failure: malformed.then_some(Error::Configuration),
        };
        j.save(&r)?;
        r
    };
    outcome::application(record.operation)?;
    record.report()?;
    let candidate = if record.compensating() || record.terminal() {
        None
    } else {
        let result = Candidate::load(&manifests.join(format!("{}.json", record.artifact)))
            .and_then(|(c, digest)| {
                if digest != record.artifact {
                    return Err(Error::Identity);
                }
                if c.format()? == "windows_msix" {
                    return Err(Error::Unsupported);
                }
                Ok(c)
            });
        match result {
            Ok(c) => Some(c),
            Err(e) => {
                rollback(&mut record, config, e)?;
                j.save(&record)?;
                None
            }
        }
    };
    if record.phase == Phase::Received {
        let input = record.input.take();
        if input.as_deref().map(|s| hash(s.as_bytes())) != record.profile_digest {
            return Err(Error::Storage);
        }
        if let Err(e) = prepare(
            config,
            candidate.as_ref().ok_or(Error::Storage)?,
            input.as_deref().map(str::as_bytes),
            &j,
            &mut record,
        ) {
            record.failure = Some(e);
            record.phase = Phase::Rejected;
        }
        j.save(&record)?;
    }
    if !record.terminal() {
        drive(config, candidate.as_ref(), &j, &mut record)?;
    }
    record.input = None;
    j.save(&record)?;
    j.archive(&record)?;
    record.report()?;
    record.result()
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
            input: None,
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

    #[test]
    fn private_checkpoint_reopens_frozen_input_and_retains_original_result() {
        let temp = crate::protocol_test_support::Root::new();
        let journal = Journal {
            root: temp.path.join("profiles"),
        };
        platform_private_storage::directory(&journal.root).unwrap();
        let mut r = record(Phase::Received);
        r.input = Some("original profile".into());
        r.profile_digest = Some(hash(r.input.as_ref().unwrap().as_bytes()));
        journal.save(&r).unwrap();
        let recovered = journal.read(&journal.active()).unwrap().unwrap();
        assert_eq!(recovered.input, r.input);
        assert_eq!(recovered.profile_digest, r.profile_digest);
        r.input = None;
        r.phase = Phase::Rejected;
        r.failure = Some(Error::Unsupported);
        journal.save(&r).unwrap();
        journal.archive(&r).unwrap();
        assert!(journal.read(&journal.active()).unwrap().is_none());
        let archived = journal
            .read(&journal.file(r.operation, "result"))
            .unwrap()
            .unwrap();
        assert_eq!(archived.result(), Err(Error::Unsupported));
        assert!(archived.input.is_none());
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
