//! Fixed system worker. The existing runner owns admission, cancellation and process lifetime.
//! ref: microsoft/windows-rs@0.61.0 Win32/System/Wmi/mod.rs.
use agent_client::Error;
use execution_contract::{Digest, RunAs, SessionRequirement};
use rss_mdm_agent_wire::runtime as w;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
#[cfg(any(windows, test))]
use zeroize::Zeroizing;
#[cfg(any(windows, test))]
mod engine;
mod envelope;
mod state;
#[cfg(test)]
mod tests;
#[cfg(windows)]
mod windows;
pub(crate) const PROFILE: &str = "native-credential-worker";
pub(crate) const OUTPUT_BYTES: u64 = 262144;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub task: w::CredentialTaskSpec,
    pub root: PathBuf,
    pub image_sha256: Digest,
    pub run_as: RunAs,
    pub session: SessionRequirement,
}
#[cfg(any(windows, test))]
pub(crate) struct Material {
    pub subject: w::CredentialSubject,
    pub secret: Zeroizing<String>,
}
#[cfg(any(windows, test))]
pub(crate) struct Inventory {
    pub materials: Vec<Material>,
    pub failure: Option<w::CredentialFailure>,
}
#[cfg(any(windows, test))]
pub(crate) trait Native {
    fn collect(&mut self) -> Result<Inventory, w::CredentialFailure>;
    fn check_account(&mut self, sid: &str) -> Result<(), w::CredentialFailure>;
    fn apply(
        &mut self,
        operation: &w::CredentialOperation,
        secret: &str,
    ) -> Result<(), w::CredentialFailure>;
    fn locate(
        &mut self,
        operation: &w::CredentialOperation,
        secret: &str,
    ) -> Result<Option<w::CredentialSubject>, w::CredentialFailure>;
    fn remove(&mut self, volume: &str, protector: uuid::Uuid) -> Result<(), w::CredentialFailure>;
    fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>, Error>;
    fn unprotect(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error>;
}
pub(crate) fn failed(
    task: &w::CredentialTaskSpec,
    failure: w::CredentialFailure,
) -> w::CredentialResult {
    w::CredentialResult {
        definition_digest: task.definition_digest,
        recovery: None,
        failure: Some(failure),
        complete: false,
        items: vec![],
    }
}
/// Entry accepts only the runner's retained, protected input and OS identity.
pub fn run(path: &Path) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 262144)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    input.task.validate()?;
    if input.task.permit != w::TaskPermit::Start
        || !matches!(
            input.run_as,
            RunAs::System {
                platform: execution_contract::Platform::Windows
            }
        )
        || !matches!(input.session, SessionRequirement::NotRequired {})
    {
        return Err(Error::Denied);
    }
    let _image =
        execution_runner::staging::verify_retained(&std::env::current_exe()?, &input.image_sha256)
            .map_err(crate::error::app_error)?;
    execution_runner::staging::verify_worker_identity(&input.run_as, &input.session)
        .map_err(crate::error::app_error)?;
    #[cfg(windows)]
    let result = {
        let mut native = windows::NativeWindows::open().map_err(|_| Error::Unavailable)?;
        engine::execute(&input, &mut native)
            .unwrap_or_else(|_| failed(&input.task, w::CredentialFailure::RecoveryState))
    };
    #[cfg(not(windows))]
    let result = failed(&input.task, w::CredentialFailure::Unsupported);
    std::io::stdout().write_all(&serde_json::to_vec(&result).map_err(|_| Error::Protocol)?)?;
    Ok(0)
}
pub(crate) fn root(config: &crate::ExecutionConfig) -> PathBuf {
    config.material_root.join("credentials")
}
pub(crate) fn prepare_root(config: &crate::ExecutionConfig) -> Result<PathBuf, Error> {
    let root = root(config);
    platform_private_storage::PrivateDirectory::create(&root)?;
    Ok(root)
}
/// Read ciphertext only. Worker output and execution history never carry escrow payloads.
pub(crate) fn pending(
    root: &Path,
    task: &w::CredentialTaskSpec,
) -> Result<Option<w::CredentialEscrow>, Error> {
    state::read_optional(root, &state::name(task, "escrow"))
}
pub(crate) fn acknowledge(
    root: &Path,
    task: &w::CredentialTaskSpec,
    escrow: &w::CredentialEscrow,
) -> Result<(), Error> {
    state::write(root, &state::name(task, "ack"), &envelope::digest(escrow)?)
}
