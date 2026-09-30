use execution_app::Error;
use execution_contract::*;
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek},
    path::{Path, PathBuf},
    sync::Arc,
};
use zeroize::Zeroize;

/// Bounded material coordinates shared with the service owner. No process or permission is stored.
#[derive(Clone)]
pub struct MaterialRegistry {
    pub(crate) entries: Arc<std::sync::Mutex<BTreeMap<String, Arc<Artifacts>>>>,
    capacity: usize,
}
impl MaterialRegistry {
    /// Create an empty registry with an explicit finite capacity.
    pub fn new(capacity: usize) -> Result<Self, Error> {
        Self::from_materials(BTreeMap::new(), capacity)
    }
    pub(crate) fn from_materials(
        values: BTreeMap<String, Artifacts>,
        capacity: usize,
    ) -> Result<Self, Error> {
        if capacity == 0 || capacity > 4096 || values.len() > capacity {
            return Err(Error::Configuration);
        }
        Ok(Self {
            entries: Arc::new(std::sync::Mutex::new(
                values.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
            )),
            capacity,
        })
    }
    /// Register exact material coordinates once. Re-registration cannot replace live or idle content.
    pub fn register(&self, plan: &FrozenExecution, value: Artifacts) -> Result<(), Error> {
        value.inspect(plan)?;
        let mut entries = self.entries.lock().map_err(|_| Error::Unavailable)?;
        if entries.contains_key(plan.digest().as_str()) {
            return Err(Error::Conflict);
        }
        if entries.len() >= self.capacity {
            return Err(Error::Capacity);
        }
        entries.insert(plan.digest().as_str().into(), Arc::new(value));
        Ok(())
    }
    pub(crate) fn get(&self, key: &str) -> Result<Option<Arc<Artifacts>>, Error> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(key)
            .cloned())
    }
    /// Recheck actual OS prerequisites, without executing or reserving an attempt.
    pub fn inspect(&self, plan: &FrozenExecution) -> Result<(), Error> {
        self.get(plan.digest().as_str())?
            .ok_or(Error::Unbound)?
            .inspect(plan)
    }
    /// Whether this exact immutable recipe is already registered; never replaces it.
    pub fn registered(&self, plan: &FrozenExecution) -> Result<bool, Error> {
        Ok(self
            .entries
            .lock()
            .map_err(|_| Error::Unavailable)?
            .contains_key(plan.digest().as_str()))
    }
    /// Retire settled material coordinates; in-use leases remain owned by the running attempt.
    /// The service calls this only after the journal and delivery owners have settled the task.
    pub fn retire(&self, plan: &FrozenExecution) -> Result<(), Error> {
        let mut entries = self.entries.lock().map_err(|_| Error::Unavailable)?;
        if entries
            .get(plan.digest().as_str())
            .is_some_and(|v| Arc::strong_count(v) > 1)
        {
            return Err(Error::Conflict);
        }
        entries.remove(plan.digest().as_str());
        Ok(())
    }
}

/// A single-use input buffer. It is never cloned, serialized, or included in Debug output.
pub struct InputBytes(Vec<u8>);
impl InputBytes {
    /// Take ownership of bytes and zeroize them on drop; the resolver enforces their bound.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }
}
impl Drop for InputBytes {
    fn drop(&mut self) {
        self.0.zeroize()
    }
}
/// Trusted product input resolution, invoked only after consuming dispatch authority.
/// Implementations must authorize the exact plan, reference and attempt, and honor the byte bound.
pub trait InputResolver: Send + Sync {
    /// Resolve once for this admitted attempt. Return an error for absent, unauthorized or oversized input.
    fn resolve(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        reference: &VersionedRef,
        max_bytes: u64,
    ) -> Result<InputBytes, Error>;
}
/// Fixed local materialization coordinates. These paths never grant execution authority.
pub struct Artifacts {
    /// Ordered physical recipes for a software program; empty for a single process.
    pub program: Vec<SoftwareStepArtifacts>,
    /// Optional exact user-session mechanism; the system runner retains journal ownership.
    pub delegate: Option<Arc<crate::helper::Connection>>,
    /// Absolute protected interpreter artifact; its exact bytes must match the plan digest.
    pub interpreter: PathBuf,
    /// Absolute protected content artifact, never an arbitrary command or download URL.
    pub content: PathBuf,
    /// Existing private materialization directory owned by the execution identity.
    pub work_root: PathBuf,
    /// Trusted per-attempt input resolver; absent means controlled stdin is refused.
    pub controlled_input: Option<Arc<dyn InputResolver>>,
    // Only the unit-test binary accepts caller-owned fixture sources; no production switch exists.
    #[cfg(test)]
    pub(crate) fixture_owned: bool,
}
/// Immutable material coordinates for one backend-defined step.
pub struct SoftwareStepArtifacts {
    /// Exact mutation recipe selected by the program intent.
    pub mutation: Option<Box<Artifacts>>,
    /// Exact detector recipe, shared by before/after observations.
    pub detection: Option<Box<Artifacts>>,
    /// Additional exact manager/package objects retained during the invocation.
    pub files: Vec<(PathBuf, ExactArtifactRef)>,
}
pub(crate) struct Payload {
    pub path: PathBuf,
    pub file: Option<File>,
    pub directory: Option<PathBuf>,
}
impl Drop for Payload {
    fn drop(&mut self) {
        self.file.take();
        if let Some(root) = &self.directory {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_dir(root);
        }
    }
}
pub(crate) struct Materialized {
    /// Absolute protected interpreter artifact; its exact bytes must match the plan digest.
    pub interpreter: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub stdin: Option<InputBytes>,
    payload: Payload,
    cwd: super::platform::WorkingDirectory,
    _interpreter: File,
    _leases: Vec<super::platform::PathLease>,
}
/// A borrowed process recipe from the original frozen intent. Selecting a step never
/// manufactures a second FrozenExecution, request identity, authorization or journal.
#[derive(Clone, Copy)]
pub(crate) struct Recipe<'a> {
    pub launch: &'a LaunchSpec,
    pub run_as: &'a RunAs,
    pub session: &'a SessionRequirement,
}
impl<'a> Recipe<'a> {
    pub(crate) fn root(plan: &'a FrozenExecution) -> Self {
        Self {
            launch: &plan.spec().launch,
            run_as: &plan.spec().run_as,
            session: &plan.spec().session_requirement,
        }
    }
    pub(crate) fn invocation(invocation: &'a SoftwareInvocation) -> Self {
        Self {
            launch: &invocation.launch,
            run_as: &invocation.run_as,
            session: &invocation.session_requirement,
        }
    }
}
impl Materialized {
    pub(crate) fn configure(&self, command: &mut std::process::Command) -> Result<(), Error> {
        self.cwd.configure(
            command,
            self.payload.file.as_ref().ok_or(Error::Unavailable)?,
        )
    }
}
fn exact(path: &Path, digest: &Digest, limit: u64) -> Result<(File, Vec<u8>), Error> {
    super::platform::protected_path(path, false)?;
    let mut file = super::platform::open_file(path)?;
    if file.metadata().map_err(|_| Error::Unavailable)?.len() > limit {
        return Err(Error::Capacity);
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable)?;
    if bytes.len() as u64 > limit || format!("{:x}", Sha256::digest(&bytes)) != digest.as_str() {
        return Err(Error::Denied);
    }
    file.rewind().map_err(|_| Error::Unavailable)?;
    Ok((file, bytes))
}
impl Artifacts {
    /// Inspect actual OS identity and protected interpreter/content before network Start.
    /// No input resolver, installer, subprocess or approval is invoked here.
    pub fn inspect(&self, plan: &FrozenExecution) -> Result<(), Error> {
        if let Some(program) = plan.spec().execution.software_program() {
            if self.program.len() != program.steps.len() {
                return Err(Error::Unbound);
            }
            for (index, sources) in self.program.iter().enumerate() {
                if sources.files.len() > 4096 {
                    return Err(Error::Capacity);
                }
                for (path, artifact) in &sources.files {
                    verify_material(path, &artifact.sha256)?;
                }
                if let Some(invocation) = program.invocation(index, SoftwarePhase::Mutation) {
                    sources
                        .mutation
                        .as_ref()
                        .ok_or(Error::Unbound)?
                        .inspect_recipe(Recipe::invocation(invocation))?;
                }
                if let Some(invocation) = program.invocation(index, SoftwarePhase::Before) {
                    sources
                        .detection
                        .as_deref()
                        .ok_or(Error::Unbound)?
                        .inspect_recipe(Recipe::invocation(invocation))?;
                }
            }
            return Ok(());
        }
        if let Some(delegate) = &self.delegate {
            return delegate.inspect(plan, self);
        }
        let p = plan.spec();
        super::platform::identity(&p.run_as, &p.session_requirement)?;
        super::platform::profile(&p.launch.interpreter.profile)?;
        if !matches!(p.constraints, IsolationPolicy::OsIdentity {}) {
            return Err(Error::Capability);
        }
        super::platform::protected_path(&self.work_root, true)?;
        let _interpreter = super::platform::PathLease::source(&self.interpreter, true)?;
        exact(
            &self.interpreter,
            &p.launch.interpreter.artifact.sha256,
            256 * 1024 * 1024,
        )?;
        if p.execution.software_program().is_none() {
            let _content = super::platform::PathLease::source(&self.content, true)?;
            exact(&self.content, &p.launch.artifact.sha256, 16 * 1024 * 1024)?;
        }
        Ok(())
    }
    pub(crate) fn inspect_recipe(&self, recipe: Recipe<'_>) -> Result<(), Error> {
        if let Some(delegate) = &self.delegate {
            if !matches!(recipe.run_as, RunAs::User { account } if account.subject.as_str() == delegate.context().subject)
                || !matches!(recipe.session, SessionRequirement::ActiveUser { session, .. } if session == &delegate.context().binding)
            {
                return Err(Error::Unbound);
            }
        } else {
            super::platform::identity(recipe.run_as, recipe.session)?;
        }
        super::platform::profile(&recipe.launch.interpreter.profile)?;
        exact(
            &self.interpreter,
            &recipe.launch.interpreter.artifact.sha256,
            256 * 1024 * 1024,
        )?;
        exact(
            &self.content,
            &recipe.launch.artifact.sha256,
            16 * 1024 * 1024,
        )?;
        Ok(())
    }
    pub(crate) fn prepare(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        control: &crate::software::PreparationControl,
    ) -> Result<Materialized, Error> {
        self.prepare_recipe(plan, attempt, Recipe::root(plan), control)
    }
    pub(crate) fn prepare_recipe(
        &self,
        plan: &FrozenExecution,
        attempt: &AttemptId,
        recipe: Recipe<'_>,
        control: &crate::software::PreparationControl,
    ) -> Result<Materialized, Error> {
        control.check()?;
        let p = plan.spec();
        let Recipe {
            launch,
            run_as,
            session,
        } = recipe;
        super::platform::identity(run_as, session)?;
        if !matches!(p.constraints, IsolationPolicy::OsIdentity {}) {
            return Err(Error::Capability);
        }
        super::platform::profile(&launch.interpreter.profile)?;
        super::platform::protected_path(&self.work_root, true)?;
        let work_lease = super::platform::PathLease::source(&self.work_root, false)?;
        let content_path = &self.content;
        #[cfg(not(test))]
        let content_immutable = true;
        #[cfg(test)]
        let content_immutable = !self.fixture_owned;
        let mut leases = vec![
            work_lease,
            super::platform::PathLease::source(&self.interpreter, true)?,
            super::platform::PathLease::source(content_path, content_immutable)?,
        ];
        let (interpreter, _) = exact(
            &self.interpreter,
            &launch.interpreter.artifact.sha256,
            256 * 1024 * 1024,
        )?;
        let (content_file, content) =
            exact(content_path, &launch.artifact.sha256, 16 * 1024 * 1024)?;
        super::platform::encoding(launch.artifact_encoding)?;
        match launch.artifact_encoding {
            ArtifactEncoding::Utf8
                if !content.starts_with(&[0xef, 0xbb, 0xbf])
                    && std::str::from_utf8(&content).is_ok() => {}
            ArtifactEncoding::Utf8Bom
                if content.starts_with(&[0xef, 0xbb, 0xbf])
                    && std::str::from_utf8(&content[3..]).is_ok() => {}
            ArtifactEncoding::Utf16LeBom
                if content.starts_with(&[0xff, 0xfe])
                    && crate::output::decode(&content[2..], TextEncoding::Utf16Le).is_some() => {}
            _ => return Err(Error::InvalidInput),
        }
        let query = launch.interpreter.profile.id.as_str() == "native-osquery-info-v1";
        if query
            && (content != b"SELECT version FROM osquery_info;\n"
                || launch.output.format != (OutputFormat::Json { max_rows: 1 })
                || !matches!(run_as, RunAs::System { .. })
                || !launch.env.is_empty()
                || !matches!(launch.stdin, StandardInput::Closed {}))
        {
            return Err(Error::Denied);
        }
        super::platform::protected_path(&self.work_root, true)?;
        leases.push(super::platform::PathLease::source(&self.work_root, false)?);
        let cwd = super::platform::WorkingDirectory::open(Path::new(&launch.cwd))?;
        let bundled = plan
            .spec()
            .execution
            .software_program()
            .is_some_and(|program| {
                program.steps.iter().any(|step| {
                    step.bundle.as_ref().is_some_and(|manifest| {
                        manifest.entries.iter().any(|(name, entry)| {
                            Path::new(&launch.cwd).join(name) == *content_path
                                && entry
                                    .sha256
                                    .iter()
                                    .map(|b| format!("{b:02x}"))
                                    .collect::<String>()
                                    == launch.artifact.sha256.as_str()
                        })
                    })
                })
            });
        let payload = if bundled {
            Payload {
                path: content_path.clone(),
                file: Some(content_file),
                directory: None,
            }
        } else {
            super::platform::payload(
                content_file,
                &content,
                &self.work_root,
                attempt,
                &launch.interpreter.profile,
            )?
        };
        let args = launch
            .argv
            .iter()
            .map(|arg| match arg {
                LaunchArg::Literal { value } => Ok(value.clone()),
                LaunchArg::ArtifactPath {} => payload
                    .path
                    .to_str()
                    .map(str::to_owned)
                    .ok_or(Error::InvalidInput),
            })
            .collect::<Result<Vec<_>, _>>()?;
        super::platform::arguments(&launch.interpreter.profile, &args, &payload.path)?;
        let mut env = BTreeMap::new();
        for (key, value) in &launch.env {
            let upper = key.as_str().to_ascii_uppercase();
            if ["LD_", "DYLD_", "DOTNET_", "COMPLUS_", "CORECLR_", "COR_"]
                .iter()
                .any(|p| upper.starts_with(p))
                || [
                    "ENV",
                    "BASH_ENV",
                    "BASHOPTS",
                    "SHELLOPTS",
                    "PSMODULEPATH",
                    "GCONV_PATH",
                    "GLIBC_TUNABLES",
                ]
                .contains(&upper.as_str())
            {
                return Err(Error::Denied);
            }
            let InputValue::Literal {
                value: serde_json::Value::String(value),
            } = value
            else {
                return Err(Error::Unsupported);
            };
            env.insert(key.as_str().into(), value.clone());
        }
        let stdin = match &launch.stdin {
            StandardInput::Closed {} => None,
            StandardInput::Controlled {
                max_bytes,
                encoding,
                reference,
            } => {
                let bytes = self
                    .controlled_input
                    .as_ref()
                    .ok_or(Error::Unbound)?
                    .resolve(plan, attempt, reference, *max_bytes)?;
                if bytes.bytes().len() as u64 > *max_bytes {
                    return Err(Error::Capacity);
                }
                if !crate::output::valid_encoding(bytes.bytes(), *encoding) {
                    return Err(Error::InvalidInput);
                }
                Some(bytes)
            }
        };
        Ok(Materialized {
            interpreter: self.interpreter.clone(),
            args,
            env,
            stdin,
            payload,
            cwd,
            _interpreter: interpreter,
            _leases: leases,
        })
    }
}
pub(crate) fn verify_material(
    path: &Path,
    expected: &Digest,
) -> Result<(File, super::platform::PathLease), Error> {
    let lease = super::platform::PathLease::source(path, true)?;
    let mut file = super::platform::open_file(path)?;
    if file.metadata().map_err(|_| Error::Unavailable)?.len() > 8 * 1024 * 1024 * 1024 {
        return Err(Error::Capacity);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(|_| Error::Unavailable)?;
        if n == 0 {
            break;
        }
        total = total.checked_add(n as u64).ok_or(Error::Capacity)?;
        if total > 8 * 1024 * 1024 * 1024 {
            return Err(Error::Capacity);
        }
        digest.update(&buffer[..n]);
    }
    if format!("{:x}", digest.finalize()) != expected.as_str() {
        return Err(Error::Denied);
    }
    file.rewind().map_err(|_| Error::Unavailable)?;
    Ok((file, lease))
}
