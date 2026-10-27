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
    generation: Arc<std::sync::atomic::AtomicU64>,
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
            generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }
    /// Register exact material coordinates once. Re-registration cannot replace live or idle content.
    pub fn register(&self, plan: &FrozenExecution, value: Artifacts) -> Result<(), Error> {
        let mut entries = self.entries.lock().map_err(|_| Error::Unavailable)?;
        if entries.contains_key(plan.digest().as_str()) {
            return Err(Error::Conflict);
        }
        if entries.len() >= self.capacity {
            return Err(Error::Capacity);
        }
        self.advance_generation()?;
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
    /// Current recipe membership generation; snapshots never count as active execution leases.
    pub fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::Acquire)
    }
    fn advance_generation(&self) -> Result<(), Error> {
        self.generation
            .fetch_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |n| n.checked_add(1),
            )
            .map_err(|_| Error::Capacity)?;
        Ok(())
    }
    /// Start an incremental reference walk. Weak handles do not block legitimate retirement.
    pub fn retained_materials(&self) -> Result<RetainedMaterials, Error> {
        let mut walk = RetainedMaterials {
            roots: Vec::new(),
            stack: Vec::new(),
            seen: Default::default(),
        };
        self.refresh_retained(&mut walk)?;
        Ok(walk)
    }
    /// Fold new immutable recipes into an in-flight walk, retaining its earlier progress.
    pub fn refresh_retained(&self, walk: &mut RetainedMaterials) -> Result<(), Error> {
        let entries = self.entries.lock().map_err(|_| Error::Unavailable)?;
        for value in entries.values() {
            // Weak handles keep allocations unique for this walk; no pointer is dereferenced.
            if walk.seen.contains(&(Arc::as_ptr(value) as usize)) {
                continue;
            }
            if walk.roots.len() >= 32768 {
                return Err(Error::Capacity);
            }
            walk.seen.insert(Arc::as_ptr(value) as usize);
            let root = walk.roots.len();
            walk.roots.push(Arc::downgrade(value));
            walk.stack.push(ReferenceCursor {
                root,
                route: Vec::new(),
                field: 0,
                step: 0,
                file: 0,
                child: 0,
            });
        }
        Ok(())
    }
    /// Retire settled material coordinates; in-use leases remain owned by the running attempt.
    /// The service calls this only after the journal and delivery owners have settled the task.
    pub fn retire(&self, digest: &Digest) -> Result<(), Error> {
        let mut entries = self.entries.lock().map_err(|_| Error::Unavailable)?;
        if entries
            .get(digest.as_str())
            .is_some_and(|v| Arc::strong_count(v) > 1)
        {
            return Err(Error::Conflict);
        }
        if entries.contains_key(digest.as_str()) {
            self.advance_generation()?;
            entries.remove(digest.as_str());
        }
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
#[derive(Clone)]
pub struct Artifacts {
    /// Ordered physical recipes for a software program; empty for a single process.
    pub program: Vec<SoftwareStepArtifacts>,
    /// Optional exact user-session mechanism; the system runner retains journal ownership.
    pub delegate: Option<Arc<execution_ipc::helper::Connection>>,
    /// Absolute protected interpreter artifact; its exact bytes must match the plan digest.
    pub interpreter: PathBuf,
    /// Absolute protected content artifact, never an arbitrary command or download URL.
    pub content: PathBuf,
    /// Existing private materialization directory owned by the execution identity.
    pub work_root: PathBuf,
    /// Trusted per-attempt input resolver; absent means controlled stdin is refused.
    pub controlled_input: Option<Arc<dyn InputResolver>>,
    // Only the unit-test binary accepts caller-owned pinned fixture artifacts; no production switch exists.
    #[cfg(test)]
    pub(crate) fixture_owned: bool,
}
/// Immutable material coordinates for one backend-defined step.
#[derive(Clone)]
pub struct SoftwareStepArtifacts {
    /// Exact physical recipes in the original attempt, keyed by durable phase.
    pub mutations: BTreeMap<SoftwarePhase, Box<Artifacts>>,
    /// Exact detector recipe, shared by before/after observations.
    pub detection: Option<Box<Artifacts>>,
    /// Additional exact manager/package objects retained during the invocation.
    pub files: Vec<(PathBuf, ExactArtifactRef)>,
}
pub(crate) struct Payload {
    #[cfg(target_os = "macos")]
    pub input: Option<(std::io::PipeWriter, Vec<u8>)>,
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
    #[cfg(target_os = "macos")]
    pub(crate) fn take_payload_input(&mut self) -> Option<(std::io::PipeWriter, Vec<u8>)> {
        self.payload.input.take()
    }
    pub(crate) fn configure(&self, command: &mut std::process::Command) -> Result<(), Error> {
        self.cwd.configure(
            command,
            self.payload.file.as_ref().ok_or(Error::Unavailable)?,
        )
    }
}
fn exact(
    path: &Path,
    digest: &Digest,
    limit: u64,
    immutable: bool,
) -> Result<(File, Vec<u8>, super::platform::PathLease), Error> {
    let (mut file, lease) = super::platform::PathLease::source(path, immutable)?.into_file()?;
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
    Ok((file, bytes, lease))
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
                for (phase, material) in &sources.mutations {
                    if phase.is_observation() {
                        return Err(Error::Unbound);
                    }
                    let invocation = program.invocation(index, *phase).ok_or(Error::Unbound)?;
                    material.inspect_recipe(Recipe::invocation(invocation))?;
                }
                if program.intent != SoftwareOperation::Detect
                    && !sources.mutations.contains_key(&SoftwarePhase::Mutation)
                {
                    return Err(Error::Unbound);
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
            let context = delegate.context();
            if !matches!(&plan.spec().run_as, RunAs::User { account } if account.subject.as_str() == context.subject)
                || !matches!(&plan.spec().session_requirement, SessionRequirement::ActiveUser { session, .. } if session == &context.binding)
                || self.work_root != context.work_root
            {
                return Err(Error::Denied);
            }
            return match delegate.exchange(execution_ipc::helper::Command::Inspect {
                input: Box::new(plan.spec().clone()),
                interpreter: self.interpreter.clone(),
                content: self.content.clone(),
            })? {
                execution_ipc::helper::Reply::Prepared => Ok(()),
                _ => Err(Error::InvalidInput),
            };
        }
        let p = plan.spec();
        super::platform::identity(&p.run_as, &p.session_requirement)?;
        super::platform::profile(&p.launch.interpreter.profile)?;
        if !matches!(p.constraints, IsolationPolicy::OsIdentity {}) {
            return Err(Error::Capability);
        }
        super::platform::protected_path(&self.work_root, true)?;
        exact(
            &self.interpreter,
            &p.launch.interpreter.artifact.sha256,
            256 * 1024 * 1024,
            true,
        )?;
        if p.execution.software_program().is_none() {
            exact(
                &self.content,
                &p.launch.artifact.sha256,
                16 * 1024 * 1024,
                true,
            )?;
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
            true,
        )?;
        exact(
            &self.content,
            &recipe.launch.artifact.sha256,
            16 * 1024 * 1024,
            true,
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
        let work_lease = super::platform::PathLease::directory(&self.work_root, false)?;
        let content_path = &self.content;
        #[cfg(not(test))]
        let source_immutable = true;
        #[cfg(test)]
        let source_immutable = !self.fixture_owned;
        let (interpreter, _, interpreter_lease) = exact(
            &self.interpreter,
            &launch.interpreter.artifact.sha256,
            256 * 1024 * 1024,
            source_immutable,
        )?;
        let (content_file, content, content_lease) = exact(
            content_path,
            &launch.artifact.sha256,
            16 * 1024 * 1024,
            source_immutable,
        )?;
        let leases = vec![work_lease, interpreter_lease, content_lease];
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
        let query = launch.interpreter.profile.id.as_str() == "native-osquery-template";
        if query {
            let OutputFormat::Json { max_rows } = launch.output.format else {
                return Err(Error::Denied);
            };
            if !matches!(run_as, RunAs::System { .. })
                || !launch.env.is_empty()
                || !matches!(launch.stdin, StandardInput::Closed {})
            {
                return Err(Error::Denied);
            }
            let expected = crate::osquery::request_arguments(
                &content,
                &p.request.parameters,
                max_rows,
                p.request.target.platform,
            )?;
            if launch.argv.len() != expected.len()
                || !launch
                    .argv
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| matches!(a,LaunchArg::Literal{value} if value==b))
            {
                return Err(Error::Denied);
            }
        }
        let cwd = super::platform::WorkingDirectory::open(Path::new(&launch.cwd))?;
        let bundled = plan
            .spec()
            .execution
            .software_program()
            .is_some_and(|program| {
                program.steps.iter().any(|step| {
                    step.format
                        .bundle()
                        .map(|(manifest, _)| manifest)
                        .is_some_and(|manifest| {
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
                #[cfg(target_os = "macos")]
                input: None,
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
    verify_material_kind(path, expected, true)
}
pub(crate) fn verify_material_kind(
    path: &Path,
    expected: &Digest,
    immutable: bool,
) -> Result<(File, super::platform::PathLease), Error> {
    let (mut file, lease) = super::platform::PathLease::source(path, immutable)?.into_file()?;
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

struct ReferenceCursor {
    root: usize,
    route: Vec<(usize, usize)>,
    field: usize,
    step: usize,
    file: usize,
    child: usize,
}
/// Incremental snapshot of registered native/helper recipes, independent of task history.
pub struct RetainedMaterials {
    roots: Vec<std::sync::Weak<Artifacts>>,
    seen: std::collections::BTreeSet<usize>,
    stack: Vec<ReferenceCursor>,
}
impl RetainedMaterials {
    /// Whether all recipe references have been inspected.
    pub fn complete(&self) -> bool {
        self.stack.is_empty()
    }
    /// Inspect at most 64 recipe/path coordinates; additions are folded through refresh_retained.
    pub fn step(&mut self) -> Result<Vec<PathBuf>, Error> {
        let mut paths = Vec::new();
        for _ in 0..64 {
            let Some(cursor) = self.stack.last_mut() else {
                break;
            };
            let Some(root) = self.roots[cursor.root].upgrade() else {
                // Retirement is allowed only without active Arc leases.
                self.stack.pop();
                continue;
            };
            let mut node = root.as_ref();
            for &(step, child) in &cursor.route {
                let step = node.program.get(step).ok_or(Error::Conflict)?;
                node = step
                    .mutations
                    .values()
                    .map(Box::as_ref)
                    .chain(step.detection.as_deref())
                    .nth(child)
                    .ok_or(Error::Conflict)?;
            }
            if cursor.field < 3 {
                let path = [&node.interpreter, &node.content, &node.work_root][cursor.field];
                cursor.field += 1;
                if !path.as_os_str().is_empty() {
                    paths.push(path.clone());
                }
            } else if let Some(step) = node.program.get(cursor.step) {
                if let Some((path, _)) = step.files.get(cursor.file) {
                    cursor.file += 1;
                    paths.push(path.clone());
                } else if step
                    .mutations
                    .values()
                    .map(Box::as_ref)
                    .chain(step.detection.as_deref())
                    .nth(cursor.child)
                    .is_some()
                {
                    let mut route = cursor.route.clone();
                    route.push((cursor.step, cursor.child));
                    cursor.child += 1;
                    let root = cursor.root;
                    if route.len() > 48 || self.stack.len() >= 32768 {
                        return Err(Error::Capacity);
                    }
                    self.stack.push(ReferenceCursor {
                        root,
                        route,
                        field: 0,
                        step: 0,
                        file: 0,
                        child: 0,
                    });
                } else {
                    cursor.step += 1;
                    cursor.file = 0;
                    cursor.child = 0;
                }
            } else {
                self.stack.pop();
            }
        }
        Ok(paths)
    }
}
