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
pub(super) enum Phase {
    Received,
    Stop,
    Remove,
    Publish,
    Register,
    Desktop,
    AwaitCommit,
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
    instance: Uuid,
    tenant: Uuid,
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
            AwaitCommit => "await-msi-commit",
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
    fn observed(self) -> Result<Self, Error> {
        #[cfg(windows)]
        if self.phase == Phase::AwaitCommit {
            if lifecycle::msi_applied(&self.config, self.operation)? {
                return Ok(Self {
                    phase: Phase::Applied,
                    ..self
                });
            }
            let j = Journal::new(&self.config)?;
            if let Some(terminal) = j.read(&j.file(self.operation, "result"))? {
                return Ok(terminal);
            }
        }
        Ok(self)
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

pub(super) fn ensure_idle(config: &Path) -> Result<(), Error> {
    #[cfg(windows)]
    lifecycle::ensure_outside_msi(config)?;
    let j = Journal::new(config)?;
    if j.read(&j.active())?.is_some_and(|r| !r.terminal()) {
        Err(Error::Conflict)
    } else {
        Ok(())
    }
}

#[cfg(any(windows, test))]
pub(super) fn msi_awaiting(config: &Path, operation: Uuid) -> Result<(), Error> {
    let j = Journal::new(config)?;
    let r = j.read(&j.active())?.ok_or(Error::NotFound)?;
    if r.source == Source::Msi
        && r.operation == operation
        && r.config == config
        && r.phase == Phase::AwaitCommit
    {
        Ok(())
    } else {
        Err(Error::Conflict)
    }
}
#[cfg(any(windows, test))]
pub(super) fn finish_msi(config: &Path, operation: Uuid, commit: bool) -> Result<(), Error> {
    let j = Journal::new(config)?;
    let Some(mut r) = j.read(&j.active())? else {
        return match j.read(&j.file(operation, "result"))? {
            Some(r)
                if r.source == Source::Msi
                    && r.config == config
                    && (r.phase == Phase::Applied) == commit =>
            {
                Ok(())
            }
            None if !commit => Ok(()),
            _ => Err(Error::Conflict),
        };
    };
    if r.operation != operation || r.config != config || r.source != Source::Msi {
        return Err(Error::Conflict);
    }
    if commit {
        if !matches!(r.phase, Phase::AwaitCommit | Phase::Applied) {
            return Err(Error::Conflict);
        }
        r.phase = Phase::Applied;
    } else {
        if r.phase == Phase::Applied {
            return Err(Error::Conflict);
        }
        rollback(&mut r, config, Error::Unavailable)?;
        j.save(&r)?;
        if !r.terminal() {
            drive(config, None, &j, &mut r)?;
        }
    }
    j.save(&r)?;
    j.archive(&r)
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
        r.observed()?.report()?;
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
    let j = Journal::new(config)?;
    let pending = j
        .read(&j.active())?
        .map(Record::observed)
        .transpose()?
        .filter(|r| !r.terminal())
        .map(|r| r.operation);
    outcome::details(serde_json::json!({
        "available": available,
        "enabled": { "mdm":deployment.modules.as_ref().is_none_or(|m| m.mdm.enabled()), "da":deployment.modules.as_ref().is_some_and(|m| m.da.enabled()) },
        "mdmRegistered":registered,"pendingOperation":pending,
        "revision":deployment.modules.as_ref().map(|m|m.revision),
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
        if (*instance, *tenant) != (d.service_id, d.tenant) {
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
    let recovery = j.recovery_identity();
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
            None => (j.initial_access(r, lifecycle::initial_profile)?, None),
        },
    };
    if let Some(m) = &modules {
        m.validate(candidate)?;
        r.revision = Some(m.revision);
    }
    platform::preflight_desktop_entry(candidate)?;
    let staged = j.file(r.operation, "after");
    let target = j.plan(r.operation, || {
        let mut next = candidate.deployment(r.artifact.clone(), access, config)?;
        next.installation_id = r.installation;
        next.modules = modules.clone();
        if let Some(old) = &old {
            lifecycle::inherit(old, &mut next)?;
        }
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
    } else if next.state_root.try_exists()? {
        return Err(Error::Identity);
    }
    retain_public(&j.file(r.operation, "desktop"), &json(candidate)?)?;
    if let Some(before) = original {
        retain_public(&j.file(r.operation, "before"), &before)?;
        r.before = Some(hash(&before));
    }
    r.after = Some(hash(&target));
    if let Some(old) = old.as_ref().filter(|_| active) {
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
        if old.is_some() {
            lifecycle::preflight_candidate_state(&next, &staged)?;
        }
    }
    // A byte-equivalent effective deployment needs only native registration readback/repair.
    r.phase = if r.before == r.after {
        Phase::Register
    } else if active {
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
    platform::stop_for_apply(deployment, config, evidence)
}

fn rollback(r: &mut Record, config: &Path, failure: Error) -> Result<(), Error> {
    if r.compensating() || r.terminal() {
        return Ok(());
    }
    r.fail(failure);
    if r.phase == Phase::Received {
        r.phase = Phase::Rejected;
        return Ok(());
    }
    let active = optional(config)?.as_deref().map(hash);
    r.phase = if active == r.after
        && matches!(
            r.phase,
            Phase::Publish | Phase::Register | Phase::Desktop | Phase::AwaitCommit
        ) {
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
                    crate::local_permissions::initialize(&next.execution.local_permissions)?;
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
    while !r.terminal() && r.phase != AwaitCommit {
        authorize(r)?;
        if r.terminal() {
            break;
        }
        let step = r.phase;
        r.phase = match effect(step) {
            Ok(()) => match step {
                Stop => Remove,
                CancelStop => Restart,
                Remove => Publish,
                Publish => Register,
                Register => Desktop,
                Desktop if r.source == Source::Msi => AwaitCommit,
                Desktop => Applied,
                StopCandidate => RemoveCandidate,
                RemoveCandidate => Restore,
                Restore if upgrading => Restart,
                Restore | Restart if matches!(r.failure_phase, Some(Desktop | AwaitCommit)) => {
                    RestoreDesktop
                }
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
    if source != Source::Msi {
        lifecycle::ensure_outside_msi(config)?;
    }
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
            initial_access: None,
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
            record.fail(e);
            record.phase = Phase::Rejected;
        }
        record.initial_access = None;
        j.save(&record)?;
    }
    if !record.terminal() && record.phase != Phase::AwaitCommit {
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
    if record.phase == Phase::AwaitCommit {
        return record.report();
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
            initial_access: None,
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

    #[test]
    fn msi_receipt_stays_pending_until_outer_commit_and_commit_is_idempotent() {
        let temp = crate::protocol_test_support::Root::new();
        let config = temp.path.join("deployment.json");
        let j = Journal::new(&config).unwrap();
        std::fs::create_dir(j.root.parent().unwrap()).unwrap();
        platform_private_storage::directory(&j.root).unwrap();
        let mut r = record(Phase::Desktop);
        r.config = config.clone();
        r.source = Source::Msi;
        drive_steps(&mut r, true, |_| Ok(()), |r| j.save(r), |_| Ok(())).unwrap();
        assert_eq!(r.phase, Phase::AwaitCommit);
        assert!(j.read(&j.file(r.operation, "result")).unwrap().is_none());
        assert!(status(&config, r.operation).unwrap());
        msi_awaiting(&config, r.operation).unwrap();
        assert!(msi_awaiting(&config, Uuid::new_v4()).is_err());
        finish_msi(&config, r.operation, true).unwrap();
        finish_msi(&config, r.operation, true).unwrap();
        assert!(status(&config, r.operation).unwrap());
        assert_eq!(
            j.read(&j.file(r.operation, "result"))
                .unwrap()
                .unwrap()
                .phase,
            Phase::Applied
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires root and a root-owned TMPDIR to validate the real installation filesystem"]
    fn msi_commit_keeps_rollback_evidence_when_archiving_fails_and_save_is_recoverable() {
        use super::super::msi_evidence::tests::{observed, ticket};
        use lifecycle::Action::*;
        let msi_transaction = |action, config: &Path, id| {
            let engine = matches!(action, MsiSave).then(ticket);
            lifecycle::msi_transaction(action, config, id, engine)
        };
        let tmp = crate::protocol_test_support::Root::new();
        let config = tmp.path.join("deployment.json");
        let pending = config.with_extension("msi-rollback");
        let finalizing = config.with_extension("msi-finalizing");
        let confirmed = config.with_extension("msi-confirmed");
        let first = Uuid::new_v4();
        msi_transaction(MsiPrepare, &config, first).unwrap();
        msi_transaction(MsiSave, &config, first).unwrap();
        msi_transaction(MsiRollback, &config, first).unwrap();
        assert!(!pending.exists());

        let mut r = record(Phase::AwaitCommit);
        r.config = config.clone();
        r.source = Source::Msi;
        msi_transaction(MsiSave, &config, r.operation).unwrap();
        let j = Journal::new(&config).unwrap();
        j.create().unwrap();
        j.save(&r).unwrap();
        let result = j.file(r.operation, "result");
        std::fs::create_dir(&result).unwrap();
        msi_transaction(MsiCommit, &config, r.operation).unwrap();
        // Simulate engine exit before successful InstallFinalize. Neither a later prepare,
        // an unrelated confirmation, nor the absence of any running installer can settle it.
        let next = Uuid::new_v4();
        assert!(msi_transaction(MsiPrepare, &config, next).is_err());
        assert!(lifecycle::reconcile_msi(&config, |t, r| observed(t, r, false)).is_err());
        assert!(msi_transaction(MsiRollback, &config, next).is_err());
        assert!(!lifecycle::msi_applied(&config, r.operation).unwrap());
        assert!(finalizing.exists());
        assert_eq!(
            j.read(&j.active()).unwrap().unwrap().phase,
            Phase::AwaitCommit
        );

        assert!(lifecycle::reconcile_msi(&config, |t, r| observed(t, r, true)).is_err());
        assert!(lifecycle::msi_applied(&config, r.operation).unwrap());
        // Failed history archival occurs only after confirmed success and retains the proof.
        assert!(msi_transaction(MsiPrepare, &config, next).is_err());
        assert!(confirmed.exists());
        std::fs::remove_dir(result).unwrap();
        msi_transaction(MsiPrepare, &config, next).unwrap();
        assert!(!confirmed.exists());
        assert_eq!(
            j.read(&j.file(r.operation, "result"))
                .unwrap()
                .unwrap()
                .phase,
            Phase::Applied
        );

        // If the engine instead runs rollback, its matching transaction can reject the
        // operation and release the marker even after the commit hook was invoked.
        let rolled_back = Uuid::new_v4();
        msi_transaction(MsiSave, &config, rolled_back).unwrap();
        r.operation = rolled_back;
        r.phase = Phase::AwaitCommit;
        r.after = Some(hash(b"candidate"));
        r.before = None;
        j.save(&r).unwrap();
        msi_transaction(MsiCommit, &config, rolled_back).unwrap();
        msi_transaction(MsiRollback, &config, rolled_back).unwrap();
        assert!(!finalizing.exists());
        assert_eq!(
            j.read(&j.file(rolled_back, "result"))
                .unwrap()
                .unwrap()
                .phase,
            Phase::Rejected
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
