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
pub(super) enum Phase {
    Received,
    Stop,
    Remove,
    Publish,
    Register,
    Desktop,
    PublishRevision,
    RestoreDesktop,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_access: Option<LocalAccess>,
    installation: Uuid,
    device: Option<Uuid>,
    epoch: Option<Uuid>,
    revision: Option<u64>,
    before: Option<String>,
    after: Option<String>,
    phase: Phase,
    failure: Option<Error>,
    failure_phase: Option<Phase>,
    compensation_error: Option<Error>,
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
                | Phase::RestoreDesktop
        )
    }
    fn fail(&mut self, error: Error) {
        if self.failure.is_none() {
            self.failure = Some(error);
            self.failure_phase = Some(self.phase);
        }
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
            PublishRevision => "publish-profile-revision",
            RestoreDesktop => "restore-desktop",
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
            "phase":self.phase,"revision":self.revision,"failure":self.failure, "failurePhase":self.failure_phase, "compensationError":self.compensation_error}))
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
    fn recovery_identity(&self) -> PathBuf {
        self.root.join("inactive-deployment.json")
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
        match platform_private_storage::read(path, installation_security::MAX_FRAME * 4) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).map_err(|_| Error::Storage)?,
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    // Once published, this operation's complete deployment plan is authoritative, including
    // probed interpreters and package managers. Revalidate its artifacts on every preflight.
    fn plan(
        &self,
        id: Uuid,
        build: impl FnOnce() -> Result<Vec<u8>, Error>,
    ) -> Result<Vec<u8>, Error> {
        let path = self.file(id, "after");
        match platform_private_storage::read(&path, installation_security::MAX_FRAME) {
            Ok(bytes) => return Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
        let bytes = build()?;
        if bytes.len() > installation_security::MAX_FRAME {
            return Err(Error::Capacity);
        }
        let tmp = self.root.join(format!("{}.tmp", Uuid::new_v4()));
        platform_private_storage::write_new(&tmp, &bytes)?;
        let result = platform_private_storage::publish_new(&tmp, &path);
        if result.is_err() {
            let _ = std::fs::remove_file(tmp);
        }
        result?;
        Ok(bytes)
    }
    fn initial_access(
        &self,
        record: &mut Record,
        discover: impl FnOnce() -> Result<LocalAccess, Error>,
    ) -> Result<LocalAccess, Error> {
        if let Some(access) = &record.initial_access {
            return Ok(access.clone());
        }
        let access = discover()?;
        record.initial_access = Some(access.clone());
        self.save(record)?;
        Ok(access)
    }
    fn save(&self, record: &Record) -> Result<(), Error> {
        let staged = self.root.join(format!("{}.tmp", Uuid::new_v4()));
        platform_private_storage::write_new(&staged, &json(record)?)?;
        let result = platform_private_storage::replace(&staged, &self.active());
        if result.is_err() {
            let _ = std::fs::remove_file(staged);
        }
        result?;
        if let (Some(step), Some(error)) = (record.failure_phase, record.failure) {
            outcome::profile_failure(step, error);
        }
        if record.phase == Phase::Rejected
            && record.failure_phase != Some(Phase::Received)
            && record.after.is_some()
        {
            outcome::compensation("completed");
        } else if record.compensating() {
            outcome::compensation("running");
        }
        outcome::phase(record.phase_name())
    }
    fn archive(&self, record: &Record) -> Result<(), Error> {
        // Atomically move the terminal checkpoint to immutable per-operation history.
        platform_private_storage::replace(&self.active(), &self.file(record.operation, "result"))?;
        if record.phase == Phase::Applied && self.recovery_identity().try_exists()? {
            std::fs::remove_file(self.recovery_identity())?;
        }
        for kind in ["before", "after", "desktop", "stop", "stop-candidate"] {
            match std::fs::remove_file(self.file(record.operation, kind)) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

pub(super) fn inactive_deployment(config: &Path) -> Result<PathBuf, Error> {
    Ok(Journal::new(config)?.recovery_identity())
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
    let record = match j.read(&j.active())?.filter(|r| r.operation == id) {
        Some(record) => Some(record),
        None => j.read(&j.file(id, "result"))?,
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
    if !d.modules.mdm.enabled() || !d.state_root.join("communication").try_exists()? {
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
    let manifest = config
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-manifests")
        .join(format!("{}.json", deployment.installation_artifact));
    let available = if config != Deployment::default_path()? && !manifest.try_exists()? {
        None
    } else {
        Some(Candidate::retained(config, &deployment)?.assembly)
    };
    let observed = binding(&deployment);
    let registered = observed.as_ref().ok().map(|value| value.is_some());
    let da = crate::da_service::status(&deployment);
    let j = Journal::new(config)?;
    let pending = j
        .read(&j.active())?
        .filter(|r| !r.terminal())
        .map(|r| r.operation);
    outcome::details(serde_json::json!({
        "available": available,
        "enabled": { "mdm":deployment.modules.mdm.enabled(), "da":deployment.modules.da.enabled() },
        "mdmRegistered":registered,"da":da.as_ref().ok().and_then(|state|state.as_ref()),"daError":da.as_ref().err(),"pendingOperation":pending,
        "revision":deployment.modules.revision,
    }))?;
    Ok(())
}
fn verify_backend(source: &Source, old: Option<&Deployment>) -> Result<(), Error> {
    if let Source::Backend {
        instance,
        tenant,
        device,
        epoch,
    } = source
    {
        let d = old.ok_or(Error::Denied)?;
        if (*instance, *tenant) != (d.mdm()?.instance, d.mdm()?.tenant) {
            return Err(Error::Denied);
        }
        let secrets = crate::DeviceSecrets::open(&d.state_root.join("secrets"), &d.namespace()?)?;
        secrets.verify_storage(&d.state_root)?;
        let current = agent_client::validate_identity(
            &d.state_root.join("communication"),
            &d.network()?,
            &secrets,
            &SystemClock::new()?,
        )?;
        if (current.device_id.into_uuid(), current.epoch) != (*device, *epoch) {
            return Err(Error::Denied);
        }
    }
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
    let active = original.is_some();
    let recovery = inactive_deployment(config)?;
    let old = if active {
        Some(Deployment::metadata(config)?)
    } else if recovery.try_exists()? {
        Some(Deployment::metadata(&recovery)?)
    } else {
        None
    };
    if let Some(old) = &old {
        if old.installation_id != r.installation {
            return Err(Error::Identity);
        }
        (r.device, r.epoch) = binding(old)?.unzip();
    }
    verify_backend(&r.source, old.as_ref())?;
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
                    != (d.mdm()?.instance, d.mdm()?.tenant, r.device, r.epoch)
                    || p.subjects != d.clients.subjects
                    || p.helper_work_roots != d.helper_work_roots
                {
                    return Err(Error::Denied);
                }
                p.modules.authorize_update(d)?;
            }
            let (a, m) = p.resolve(old.as_ref())?;
            (a, m)
        }
        None => match &old {
            Some(d) => (LocalAccess::from_deployment(d), d.modules.clone()),
            None => (
                j.initial_access(r, || {
                    lifecycle::initial_profile(candidate.defaults.mdm.enabled())
                })?,
                candidate.defaults.clone(),
            ),
        },
    };
    modules.validate_for(candidate.assembly)?;
    r.revision = Some(modules.revision);
    let package = r.source == Source::Package;
    if package {
        platform::preflight_desktop_entry(candidate)?;
    }
    let staged = j.file(r.operation, "after");
    let target = j.plan(r.operation, || {
        let mut next = if package {
            let mut next =
                candidate.deployment(r.artifact.clone(), access, config, old.as_ref())?;
            next.installation_id = r.installation;
            if let Some(old) = &old {
                lifecycle::inherit(old, &mut next)?;
            }
            next
        } else {
            let mut next = old
                .as_ref()
                .filter(|_| active)
                .ok_or(Error::NotFound)?
                .clone();
            access.validate()?;
            next.registration_clients.subjects = access.subjects.clone();
            next.clients.subjects = access.subjects;
            next.helper_work_roots = access.helper_work_roots;
            next
        };
        next.modules = modules.clone();
        match (&old, &original) {
            (Some(old), Some(bytes)) if json(old)? == json(&next)? => Ok(bytes.clone()),
            _ => json(&next),
        }
    })?;
    let next = Deployment::metadata(&staged)?;
    if next.installation_id != r.installation
        || next.installation_artifact != r.artifact
        || next.modules != modules
    {
        return Err(Error::Identity);
    }
    if let Some(old) = &old {
        lifecycle::preserved(old, &next)?;
    } else if next.state_root.try_exists()? {
        return Err(Error::Identity);
    }
    if package {
        retain_public(&j.file(r.operation, "desktop"), &json(candidate)?)?;
    }
    if let Some(before) = original {
        retain_public(&j.file(r.operation, "before"), &before)?;
        r.before = Some(hash(&before));
    }
    r.after = Some(hash(&target));
    if let Some(old) = old.as_ref().filter(|_| active) {
        if !package && same_runtime(old, &next) {
            r.phase = Phase::PublishRevision;
            return Ok(());
        }
        lifecycle::preflight_refresh(old, config, &next, &staged)?;
        r.phase = if r.before == r.after {
            Phase::Register
        } else {
            Phase::Stop
        };
    } else {
        Deployment::load(&staged)?;
        if old.is_some() {
            lifecycle::preflight_candidate_state(&next, &staged)?;
        }
        r.phase = Phase::Publish;
    }
    Ok(())
}

fn same_runtime(old: &Deployment, next: &Deployment) -> bool {
    old.modules.mdm == next.modules.mdm
        && old.modules.da == next.modules.da
        && old.clients.subjects == next.clients.subjects
        && old.registration_clients.subjects == next.registration_clients.subjects
        && old.helper_work_roots == next.helper_work_roots
}

fn snapshot(path: &Path, digest: &str) -> Result<Vec<u8>, Error> {
    optional(path)?
        .filter(|b| hash(b) == digest)
        .ok_or(Error::Identity)
}

fn stop(deployment: &Deployment, config: &Path, evidence: &Path) -> Result<(), Error> {
    platform::stop_for_apply(deployment, config, evidence)
}

fn rollback(r: &mut Record, config: &Path, failure: Error) -> Result<(), Error> {
    if r.compensating() || r.terminal() {
        return Ok(());
    }
    if r.phase == Phase::PublishRevision {
        let pointer = optional(config)?.as_deref().map(hash);
        if pointer == r.after && r.before != r.after {
            r.phase = Phase::Applied;
        } else if pointer == r.before {
            r.fail(failure);
            r.phase = Phase::Rejected;
        } else {
            return Err(Error::Conflict);
        }
        return Ok(());
    }
    r.fail(failure);
    if r.phase == Phase::Received {
        r.phase = Phase::Rejected;
        return Ok(());
    }
    let active = optional(config)?.as_deref().map(hash);
    r.phase = if active == r.after
        && matches!(r.phase, Phase::Publish | Phase::Register | Phase::Desktop)
    {
        Phase::StopCandidate
    } else if active == r.before {
        if r.before.is_some() {
            Phase::CancelStop
        } else {
            Phase::Rejected
        }
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

    let operation = r.operation;
    if r.phase == PublishRevision {
        let pointer = optional(config)?;
        if pointer == previous {
            if let Err(error) = verify_backend(&r.source, old.as_ref()) {
                r.fail(error);
                r.phase = Rejected;
                return j.save(r);
            }
            if pointer.as_ref() != Some(&next_bytes) {
                publish(config, &next_bytes)?;
            }
        } else if pointer.as_ref() != Some(&next_bytes) {
            return Err(Error::Conflict);
        }
        r.phase = Applied;
        return j.save(r);
    }
    let package = r.source == Source::Package;
    drive_steps(
        r,
        old.is_some(),
        |r| {
            if !r.compensating() {
                if let Err(error) = verify_backend(&r.source, old.as_ref()) {
                    if (r.phase == Stop && !j.file(operation, "stop").try_exists()?)
                        || (r.phase == Register && r.before == r.after)
                    {
                        r.fail(error);
                        r.phase = Rejected;
                    } else {
                        rollback(r, config, error)?;
                    }
                    j.save(r)?;
                }
            }
            Ok(())
        },
        |r| j.save(r),
        |step| {
            let pointer = optional(config)?;
            let correct = match step {
                Stop | CancelStop | Remove | Restart | RestoreDesktop => pointer == previous,
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
                    if package {
                        crate::local_permissions::initialize(&next.execution.local_permissions)?;
                    }
                    publish(config, &next_bytes)
                }
                Register => platform::register(true, &next.service.path, config),
                Desktop => platform::desktop_entry(candidate.ok_or(Error::Identity)?),
                StopCandidate => stop(&next, config, &j.file(operation, "stop-candidate")),
                RemoveCandidate => platform::remove(true, &next.service.path, config),
                RestoreDesktop => {
                    let bytes = optional(&j.file(operation, "desktop"))?.ok_or(Error::Storage)?;
                    let new: Candidate =
                        serde_json::from_slice(&bytes).map_err(|_| Error::Identity)?;
                    let old = old
                        .as_ref()
                        .map(|d| Candidate::retained(config, d))
                        .transpose()?;
                    platform::restore_desktop_entry(&new, old.as_ref())
                }
                Restore => match &previous {
                    Some(bytes) => publish(config, bytes),
                    None if pointer.is_some() => platform::replace(config, &j.recovery_identity()),
                    None => {
                        snapshot(&j.recovery_identity(), &hash(&next_bytes))?;
                        Ok(())
                    }
                },
                _ => Err(Error::Storage),
            }
        },
    )
}

fn drive_steps(
    r: &mut Record,
    upgrading: bool,
    mut authorize: impl FnMut(&mut Record) -> Result<(), Error>,
    mut checkpoint: impl FnMut(&Record) -> Result<(), Error>,
    mut effect: impl FnMut(Phase) -> Result<(), Error>,
) -> Result<(), Error> {
    use Phase::*;
    let package = r.source == Source::Package;
    while !r.terminal() {
        authorize(r)?;
        if r.terminal() {
            break;
        }
        let step = r.phase;
        r.phase = match effect(step) {
            Ok(()) => match step {
                Stop if package => Remove,
                Stop => Publish,
                CancelStop => Restart,
                Remove => Publish,
                Publish => Register,
                Register if package => Desktop,
                Register | Desktop => Applied,
                StopCandidate if package => RemoveCandidate,
                StopCandidate => Restore,
                RemoveCandidate => Restore,
                Restore if upgrading => Restart,
                Restore | Restart if matches!(r.failure_phase, Some(Desktop)) => RestoreDesktop,
                Restore | Restart | RestoreDesktop => Rejected,
                _ => return Err(Error::Storage),
            },
            Err(e) => {
                if r.compensating() {
                    r.compensation_error = Some(e);
                    checkpoint(r)?;
                    return Err(e);
                }
                r.fail(e);
                match (step, upgrading) {
                    (Stop, true) => CancelStop,
                    (Publish, false) => {
                        // Publication can return an error after making the new pointer visible.
                        checkpoint(r)?;
                        return Err(e);
                    }
                    (Remove, true) => Restart,
                    (Publish, true) => Restore,
                    (Register | Desktop, _) => StopCandidate,
                    _ => {
                        checkpoint(r)?;
                        return Err(e);
                    }
                }
            }
        };
        r.compensation_error = None;
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
    #[cfg(windows)]
    lifecycle::allow_configuration_write(config)?;
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
        let old = if optional(config)?.is_some() {
            Some(Deployment::metadata(config)?)
        } else if j.recovery_identity().try_exists()? {
            Some(Deployment::metadata(&j.recovery_identity())?)
        } else {
            None
        };
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
            initial_access: None,
            installation: old
                .as_ref()
                .map_or_else(Uuid::new_v4, |d| d.installation_id),
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
            failure_phase: malformed.then_some(Phase::Received),
            compensation_error: None,
        };
        j.save(&r)?;
        r
    };
    outcome::application(record.operation)?;
    record.report()?;
    let candidate = if record.compensating() || record.terminal() {
        None
    } else {
        let result = (|| {
            let path = manifests.join(format!("{}.json", record.artifact));
            let bytes = snapshot(&path, &record.artifact)?;
            let candidate: Candidate =
                serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
            if record.source == Source::Package {
                candidate.validate()?;
                if candidate.format()? == "windows_msi" {
                    return Err(Error::Unsupported);
                }
            } else {
                candidate.validate_trust()?;
            }
            if candidate.format()? == "windows_msix" {
                return Err(Error::Unsupported);
            }
            Ok(candidate)
        })();
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
            record.fail(e);
            record.phase = Phase::Rejected;
        }
        record.initial_access = None;
        j.save(&record)?;
    }
    if !record.terminal() {
        if let Err(error) = drive(config, candidate.as_ref(), &j, &mut record) {
            // Only publication has ambiguous success. Resolve from its durable pointer.
            if record.phase != Phase::Publish {
                return Err(error);
            }
            rollback(&mut record, config, error)?;
            j.save(&record)?;
            if !record.terminal() {
                drive(config, None, &j, &mut record)?;
            }
        }
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
            source: Source::Package,
            config: "/unused".into(),
            artifact: "a".repeat(64),
            profile_digest: None,
            input: None,
            initial_access: None,
            installation: Uuid::from_u128(12),
            device: None,
            epoch: None,
            revision: Some(1),
            before: None,
            after: None,
            phase,
            failure: None,
            failure_phase: None,
            compensation_error: None,
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

    #[test]
    fn installed_runtime_change_only_stops_publishes_and_starts() {
        use Phase::*;
        let mut r = record(Stop);
        r.source = Source::Administrator;
        let mut steps = Vec::new();
        drive_steps(
            &mut r,
            true,
            |_| Ok(()),
            |_| Ok(()),
            |step| {
                steps.push(step);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(r.phase, Applied);
        assert_eq!(steps, [Stop, Publish, Register]);

        r.phase = Stop;
        steps.clear();
        drive_steps(
            &mut r,
            true,
            |_| Ok(()),
            |_| Ok(()),
            |step| {
                steps.push(step);
                if step == Register {
                    Err(Error::Unavailable)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        assert_eq!(r.phase, Rejected);
        assert_eq!(
            steps,
            [Stop, Publish, Register, StopCandidate, Restore, Restart]
        );
    }

    #[cfg(target_os = "macos")]
    fn installed_fixture(
        root: &crate::protocol_test_support::Root,
        candidate: &Candidate,
    ) -> (PathBuf, Deployment) {
        let config = root.path.join("deployment.json");
        let mut old = crate::deployment::tests::deployment();
        old.state_root = root.path.join("state");
        old.execution.material_root = root.path.join("materials");
        old.modules = candidate.defaults.clone();
        old.modules.revision = 7;
        old.service = candidate.service.clone();
        old.cli = Some(execution_ipc::entry::CliPolicy {
            image: candidate.cli.clone(),
            machine_actions: vec![],
        });
        old.clients.images = vec![candidate.desktop.clone(), candidate.cli.clone()];
        old.registration_clients.images = vec![candidate.desktop.clone()];
        let manifest = json(candidate).unwrap();
        old.installation_artifact = hash(&manifest);
        let manifests = root.path.join("install-manifests");
        platform::public_directory(&manifests).unwrap();
        publish(
            &manifests.join(format!("{}.json", old.installation_artifact)),
            &manifest,
        )
        .unwrap();
        publish(&config, &json(&old).unwrap()).unwrap();
        (config, old)
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires root and a root-owned TMPDIR for the real profile CLI"]
    fn revision_only_cli_preserves_installed_deployment_and_replays_its_result() {
        let root = crate::protocol_test_support::Root::new();
        let candidate = super::super::tests::candidate(&root);
        let (config, old) = installed_fixture(&root, &candidate);
        let mut expected = old.clone();
        expected.modules.revision = 8;
        let path = root.path.join("profile");
        let profile = serde_json::json!({"modules":expected.modules,"subjects":old.clients.subjects,"helper_work_roots":old.helper_work_roots});
        publish(&path, &json(&profile).unwrap()).unwrap();
        let id = Uuid::new_v4();
        let args = |id: Uuid| {
            vec![
                "apply".into(),
                "--config".into(),
                config.clone().into(),
                "--profile".into(),
                path.clone().into(),
                "--operation".into(),
                id.to_string().into(),
            ]
        };
        // No images, service registration or desktop resources exist in this isolated deployment.
        // The real CLI must persist revision 8 and return the original result on replay.
        lifecycle::run(&args(id)).unwrap();
        lifecycle::run(&args(id)).unwrap();
        let j = Journal::new(&config).unwrap();
        assert_eq!(
            j.read(&j.file(id, "result")).unwrap().unwrap().phase,
            Phase::Applied
        );
        assert_eq!(
            optional(&config).unwrap().unwrap(),
            json(&expected).unwrap()
        );
        let mut invalid = profile;
        invalid["modules"]["revision"] = 9.into();
        invalid["modules"]["da"] = invalid["modules"]["mdm"].clone();
        publish(&path, &json(&invalid).unwrap()).unwrap();
        let bad = Uuid::new_v4();
        assert_eq!(lifecycle::run(&args(bad)), Err(Error::Unsupported));
        assert_eq!(
            j.read(&j.file(bad, "result")).unwrap().unwrap().phase,
            Phase::Rejected
        );
        assert_eq!(
            optional(&config).unwrap().unwrap(),
            json(&expected).unwrap()
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "requires root, a root-owned TMPDIR and RSS_TEST_INSTALLATION_SERVICE pointing to the native service"]
    async fn damaged_execution_state_rejects_runtime_profile_before_stop() {
        use crate::protocol_test_support::{Root, Secrets};
        let root = Root::new();
        let mut candidate = super::super::tests::candidate(&root);
        let source = PathBuf::from(
            std::env::var_os("RSS_TEST_INSTALLATION_SERVICE").expect("native service binary"),
        );
        let signature = Command::new("/usr/bin/codesign")
            .args(["-d", "--verbose=4"])
            .arg(&source)
            .output()
            .unwrap();
        assert!(signature.status.success());
        let signature = String::from_utf8(signature.stderr).unwrap();
        let cdhash = signature
            .lines()
            .find_map(|s| s.strip_prefix("CDHash="))
            .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let executable = std::fs::read(&source).unwrap();
        let digest = hash(&executable);
        for image in [
            &mut candidate.service,
            &mut candidate.desktop,
            &mut candidate.cli,
        ] {
            std::fs::write(&image.path, &executable).unwrap();
            std::fs::set_permissions(&image.path, std::fs::Permissions::from_mode(0o755)).unwrap();
            image.sha256 = digest.clone();
            image.cdhash = Some(cdhash.to_owned());
        }
        let (config, old) = installed_fixture(&root, &candidate);
        Deployment::load(&config).unwrap();
        platform_private_storage::directory(&old.state_root).unwrap();
        let namespace = old.namespace().unwrap();
        platform_private_storage::write_new(
            &old.state_root.join("identity-binding"),
            namespace.as_bytes(),
        )
        .unwrap();
        let comm = old.state_root.join("communication");
        platform_private_storage::directory(&comm).unwrap();
        drop(
            agent_client::Client::open(
                &comm,
                old.network().unwrap(),
                agent_client::OpenMode::Create,
                Secrets::new(),
                SystemClock::new().unwrap(),
            )
            .unwrap(),
        );
        old.preflight().unwrap();
        platform_private_storage::write_new(
            &old.state_root.join("execution-initialized"),
            namespace.as_bytes(),
        )
        .unwrap();
        assert_eq!(old.preflight(), Err(Error::Storage));
        let mut modules = old.modules.clone();
        modules.revision += 1;
        let profile =
            serde_json::json!({"modules": modules, "subjects": ["501"], "helper_work_roots": {}});
        let op = Uuid::new_v4();
        let _lock = lifecycle::installation_lock(&config, true).unwrap();
        assert!(run(
            &config,
            None,
            Some(&json(&profile).unwrap()),
            Some(op),
            Source::Administrator
        )
        .is_err());
        let j = Journal::new(&config).unwrap();
        let result = j.read(&j.file(op, "result")).unwrap().unwrap();
        assert_eq!(
            (result.phase, result.failure_phase),
            (Phase::Rejected, Some(Phase::Received))
        );
        assert_eq!(optional(&config).unwrap().unwrap(), json(&old).unwrap());
        assert!(!old.state_root.join("execution.sqlite").exists());
    }

    #[test]
    fn first_install_reopens_the_original_login_selection_before_preflight_replays() {
        let temp = crate::protocol_test_support::Root::new();
        let journal = Journal {
            root: temp.path.join("profiles"),
        };
        platform_private_storage::directory(&journal.root).unwrap();
        let mut r = record(Phase::Received);
        let access = LocalAccess {
            subjects: vec!["501".into()],
            helper_work_roots: BTreeMap::new(),
        };
        journal
            .initial_access(&mut r, || Ok(access.clone()))
            .unwrap();
        let mut reopened = journal.read(&journal.active()).unwrap().unwrap();
        assert_eq!(
            journal
                .initial_access(&mut reopened, || panic!("unexpected login discovery"))
                .unwrap()
                .subjects,
            access.subjects
        );
    }

    #[test]
    fn first_install_reverses_partial_desktop_and_keeps_original_error() {
        let mut r = record(Phase::Desktop);
        let mut seen = Vec::new();
        let result = drive_steps(
            &mut r,
            false,
            |_| Ok(()),
            |_| Ok(()),
            |phase| {
                seen.push(phase);
                match phase {
                    Phase::Desktop => Err(Error::Identity),
                    Phase::RestoreDesktop => Err(Error::Unavailable),
                    _ => Ok(()),
                }
            },
        );
        assert_eq!(
            seen,
            [
                Phase::Desktop,
                Phase::StopCandidate,
                Phase::RemoveCandidate,
                Phase::Restore,
                Phase::RestoreDesktop
            ]
        );
        assert_eq!(result, Err(Error::Unavailable));
        assert_eq!(r.failure, Some(Error::Identity));
        assert_eq!(r.failure_phase, Some(Phase::Desktop));
        assert_eq!(r.compensation_error, Some(Error::Unavailable));
        drive_steps(&mut r, false, |_| Ok(()), |_| Ok(()), |_| Ok(())).unwrap();
        assert_eq!(r.phase, Phase::Rejected);
        assert_eq!(r.compensation_error, None);
        assert_eq!(r.result(), Err(Error::Identity));
    }

    #[test]
    fn preflight_reopens_the_entire_first_plan_without_probing_resources_again() {
        let tmp = crate::protocol_test_support::Root::new();
        let j = Journal {
            root: tmp.path.join("profiles"),
        };
        platform_private_storage::directory(&j.root).unwrap();
        let operation = Uuid::new_v4();
        let initial = j
            .plan(operation, || Ok(b"original interpreter image".to_vec()))
            .unwrap();
        let reopened = Journal {
            root: j.root.clone(),
        };
        assert_eq!(
            reopened
                .plan(operation, || panic!(
                    "must not probe a replacement interpreter"
                ))
                .unwrap(),
            initial
        );
    }

    #[test]
    fn broken_desktop_restoration_leaves_the_original_service_running() {
        use Phase::*;
        let mut r = record(Desktop);
        let mut service_running = false;
        let mut steps = Vec::new();
        for _ in 0..2 {
            let result = drive_steps(
                &mut r,
                true,
                |_| Ok(()),
                |_| Ok(()),
                |p| {
                    steps.push(p);
                    match p {
                        Desktop | RestoreDesktop => Err(Error::Unavailable),
                        Restart => {
                            service_running = true;
                            Ok(())
                        }
                        _ => Ok(()),
                    }
                },
            );
            assert_eq!(result, Err(Error::Unavailable));
            assert!(service_running);
            assert_eq!(r.phase, RestoreDesktop);
        }
        assert_eq!(
            steps,
            [
                Desktop,
                StopCandidate,
                RemoveCandidate,
                Restore,
                Restart,
                RestoreDesktop,
                RestoreDesktop
            ]
        );
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
            |_| Ok(()),
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
