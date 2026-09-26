use execution_app::Error;
use execution_contract::*;
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Host-provided exact local artifacts. Registration/download/trust remain their own owners.
/// Paths do not confer execution permission; dispatch still consumes AuthorizedDispatch.
pub struct Artifacts {
    pub interpreter: PathBuf,
    pub content: PathBuf,
    pub work_root: PathBuf,
    pub controlled_input: Option<Vec<u8>>,
}
pub(crate) struct Materialized {
    pub interpreter: PathBuf,
    pub script: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub stdin: Option<Vec<u8>>,
    pub cwd: PathBuf,
    // Hold immutable open objects while launching, and own only our uniquely created directory.
    _interpreter: File,
    _script: File,
    directory: PathBuf,
}
impl Drop for Materialized {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.script);
        let _ = std::fs::remove_dir(&self.directory);
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
    Ok((file, bytes))
}
impl Artifacts {
    pub(crate) fn prepare(
        &self,
        plan: &FrozenPlan,
        attempt: &AttemptId,
    ) -> Result<Materialized, Error> {
        let p = plan.spec();
        super::platform::identity(&p.run_as, &p.session_requirement)?;
        if !matches!(p.constraints, IsolationPolicy::OsIdentity {}) {
            return Err(Error::Capability);
        }
        super::platform::profile(&p.launch.interpreter.profile)?;
        let (interpreter, _) = exact(
            &self.interpreter,
            &p.launch.interpreter.artifact.sha256,
            256 * 1024 * 1024,
        )?;
        let (_, content) = exact(&self.content, &p.launch.artifact.sha256, 16 * 1024 * 1024)?;
        match p.launch.artifact_encoding {
            ArtifactEncoding::Utf8
                if !content.starts_with(&[0xef, 0xbb, 0xbf])
                    && std::str::from_utf8(&content).is_ok() => {}
            ArtifactEncoding::Utf8Bom
                if content.starts_with(&[0xef, 0xbb, 0xbf])
                    && std::str::from_utf8(&content[3..]).is_ok() => {}
            ArtifactEncoding::Utf16LeBom
                if content.starts_with(&[0xff, 0xfe]) && content.len().is_multiple_of(2) => {}
            _ => return Err(Error::InvalidInput),
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
        super::platform::protected_path(Path::new(&p.launch.cwd), true)?;
        let key = format!("{:x}", Sha256::digest(attempt.as_str().as_bytes()));
        let directory = self.work_root.join(key);
        super::platform::create_private_directory(&directory)?;
        let script = directory.join("payload");
        let staged = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&script)
                .map_err(|_| Error::Unavailable)?;
            super::platform::restrict_file(&file)?;
            file.write_all(&content)
                .and_then(|_| file.sync_all())
                .map_err(|_| Error::Unavailable)?;
            let mut args = Vec::new();
            for arg in &p.launch.argv {
                args.push(match arg {
                    LaunchArg::Literal { value } => value.clone(),
                    LaunchArg::ArtifactPath {} => {
                        script.to_str().ok_or(Error::InvalidInput)?.into()
                    }
                });
            }
            // Recheck the exact convention even for plans not produced by script-plan.
            super::platform::arguments(&p.launch.interpreter.profile, &args, &script)?;
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
                    reference: _,
                } => {
                    let bytes = self.controlled_input.as_ref().ok_or(Error::Unbound)?;
                    if bytes.len() as u64 > *max_bytes {
                        return Err(Error::Capacity);
                    }
                    if crate::output::decode(bytes, *encoding).is_none() {
                        return Err(Error::InvalidInput);
                    }
                    Some(bytes.clone())
                }
            };
            Ok(Materialized {
                interpreter: self.interpreter.clone(),
                script: script.clone(),
                args,
                env,
                stdin,
                cwd: PathBuf::from(&p.launch.cwd),
                _interpreter: interpreter,
                _script: file,
                directory: directory.clone(),
            })
        })();
        if staged.is_err() {
            let _ = std::fs::remove_file(script);
            let _ = std::fs::remove_dir(directory);
        }
        staged
    }
}
