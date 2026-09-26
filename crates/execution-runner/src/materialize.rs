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
        plan: &FrozenPlan,
        attempt: &AttemptId,
        reference: &VersionedRef,
        max_bytes: u64,
    ) -> Result<InputBytes, Error>;
}
/// Fixed local materialization coordinates. These paths never grant execution authority.
pub struct Artifacts {
    /// Absolute protected interpreter artifact; its exact bytes must match the plan digest.
    pub interpreter: PathBuf,
    /// Absolute protected content artifact, never an arbitrary command or download URL.
    pub content: PathBuf,
    /// Existing private materialization directory owned by the execution identity.
    pub work_root: PathBuf,
    /// Trusted per-attempt input resolver; absent means controlled stdin is refused.
    pub controlled_input: Option<Arc<dyn InputResolver>>,
    /// Fixed auxiliary software artifacts; absent for ordinary process plans.
    pub software: Option<crate::software::SoftwareArtifacts>,
    // Only the unit-test binary accepts caller-owned fixture sources; no production switch exists.
    #[cfg(test)]
    pub(crate) fixture_owned: bool,
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
    pub(crate) software: Option<crate::software::Lease>,
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
    pub(crate) fn prepare(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
        ownership: Option<SoftwareProvenance>,
        control: &crate::software::PreparationControl,
    ) -> Result<Materialized, Error> {
        let p = plan.spec();
        super::platform::identity(&p.run_as, &p.session_requirement)?;
        if !matches!(p.constraints, IsolationPolicy::OsIdentity {}) {
            return Err(Error::Capability);
        }
        super::platform::profile(&p.launch.interpreter.profile)?;
        let software = match (&p.execution, &self.software) {
            (ExecutionSpec::Process {}, None) => None,
            (ExecutionSpec::Software { .. }, Some(source)) => Some(source.prepare(
                plan,
                attempt,
                ownership.ok_or(Error::Denied)?,
                &self.work_root,
                control,
            )?),
            _ => return Err(Error::Denied),
        };
        let entry = software
            .as_ref()
            .map(|s| s.entry(p.execution.software().expect("software")))
            .transpose()?
            .flatten();
        let content_path = entry.as_deref().unwrap_or(&self.content);
        #[cfg(not(test))]
        let content_immutable = entry.is_none();
        #[cfg(test)]
        let content_immutable = !self.fixture_owned && entry.is_none();
        let mut leases = vec![
            super::platform::PathLease::source(&self.interpreter, true)?,
            super::platform::PathLease::source(content_path, content_immutable)?,
        ];
        let (interpreter, _) = exact(
            &self.interpreter,
            &p.launch.interpreter.artifact.sha256,
            256 * 1024 * 1024,
        )?;
        let (content_file, content) =
            exact(content_path, &p.launch.artifact.sha256, 16 * 1024 * 1024)?;
        super::platform::encoding(p.launch.artifact_encoding)?;
        match p.launch.artifact_encoding {
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
        if let Some(spec) = p.execution.software() {
            if spec.mutation != MutationKind::Uninstall {
                if let Some(expected) = crate::software::install_entry(spec.adapter) {
                    if content != expected {
                        return Err(Error::Denied);
                    }
                }
            }
        }
        let query = p.launch.interpreter.profile.id.as_str() == "native-osquery-info-v1";
        if query
            && (content != b"SELECT version FROM osquery_info;\n"
                || p.launch.output.format != (OutputFormat::Json { max_rows: 1 })
                || !matches!(p.run_as, RunAs::System { .. })
                || !p.launch.env.is_empty()
                || !matches!(p.launch.stdin, StandardInput::Closed {}))
        {
            return Err(Error::Denied);
        }
        super::platform::protected_path(&self.work_root, true)?;
        leases.push(super::platform::PathLease::source(&self.work_root, false)?);
        let cwd = super::platform::WorkingDirectory::open(Path::new(&p.launch.cwd))?;
        let payload = super::platform::payload(
            content_file,
            &content,
            &self.work_root,
            attempt,
            &p.launch.interpreter.profile,
        )?;
        let mut args = p
            .launch
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
        super::platform::arguments(&p.launch.interpreter.profile, &args, &payload.path)?;
        if let Some(software) = &software {
            args.extend(software.args.clone());
        }
        let mut env = BTreeMap::new();
        for (key, value) in &p.launch.env {
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
        let stdin = match &p.launch.stdin {
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
            software,
        })
    }
}
