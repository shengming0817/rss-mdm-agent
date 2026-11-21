//! Fixed local operations; the journal, admission and supervisor remain the existing owners.
use super::*;
use std::path::Path;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub(crate) const PROFILE: &str = "native-local-software-worker";
/// Native identities are selected only from an authorized retained observation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Target {
    App {
        path: PathBuf,
        device: u64,
        inode: u64,
        bundle: String,
        version: String,
        team: String,
    },
    Msi {
        product: String,
        version: String,
        publisher: String,
        scope: LocalScope,
        subject: Option<String>,
        cached_path: PathBuf,
        cached_sha256: String,
        view: u32,
    },
    Msix {
        full_name: String,
        version: String,
        publisher: String,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Input {
    pub target: Target,
    pub action: LocalSoftwareAction,
    pub request: RequestId,
    pub run_as: RunAs,
    pub session: SessionRequirement,
}
pub(crate) fn select(
    item: &InstalledSoftware,
    row: &serde_json::Value,
    action: LocalSoftwareAction,
) -> Result<Target, Error> {
    #[cfg(target_os = "macos")]
    {
        macos::select(item, row, action)
    }
    #[cfg(windows)]
    {
        windows::select(item, row, action)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (item, row, action);
        Err(Error::Unsupported)
    }
}
pub(crate) fn supports(target: &Target, action: LocalSoftwareAction) -> bool {
    match target {
        Target::App { .. } => matches!(
            action,
            LocalSoftwareAction::Launch
                | LocalSoftwareAction::Uninstall
                | LocalSoftwareAction::Detect
        ),
        Target::Msi { .. } => matches!(
            action,
            LocalSoftwareAction::Uninstall | LocalSoftwareAction::Detect
        ),
        Target::Msix { .. } => matches!(
            action,
            LocalSoftwareAction::Launch
                | LocalSoftwareAction::Uninstall
                | LocalSoftwareAction::Detect
        ),
    }
}
pub(crate) fn allowed(rules: &Restrictions, action: LocalSoftwareAction) -> bool {
    match action {
        LocalSoftwareAction::Launch => rules.allow_launch,
        LocalSoftwareAction::Uninstall => rules.allow_native_uninstall,
        LocalSoftwareAction::Detect => true,
        LocalSoftwareAction::Update => false,
    }
}
/// Physical adapter entry; a CLI invocation carries no service admission authority.
pub fn run(path: &Path) -> Result<i32, Error> {
    let bytes = execution_runner::staging::read_worker_input(path, 65536)
        .map_err(crate::error::app_error)?;
    let input: Input = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
    execution_runner::staging::verify_worker_identity(&input.run_as, &input.session)
        .map_err(crate::error::app_error)?;
    if !supports(&input.target, input.action) {
        return Err(Error::Unsupported);
    }
    #[cfg(target_os = "macos")]
    let result = macos::execute(&input);
    #[cfg(windows)]
    let result = windows::execute(&input);
    #[cfg(not(any(target_os = "macos", windows)))]
    let result: Result<Option<SoftwareState>, Error> = Err(Error::Unsupported);
    let (code, detected, closed, diagnostics) = match result {
        Ok(state) => (0, state, true, String::new()),
        // Native cancellation/failure does not establish rollback or absence.
        Err(error) => (
            1,
            None,
            input.action == LocalSoftwareAction::Detect,
            error.to_string(),
        ),
    };
    println!(
        "{}",
        serde_json::to_string(&SoftwareWorkerResult {
            native_output_bytes: 0,
            detected,
            closed,
            diagnostics,
        })
        .map_err(|_| Error::Protocol)?
    );
    Ok(code)
}
