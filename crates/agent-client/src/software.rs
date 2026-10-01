//! Borrowed executable inputs from the producer's current closed software model.
use crate::{wire, Error};
/// Borrowed commands; no alternate persisted software schema or compatibility decoder.
pub struct SoftwareCommands<'a> {
    /// Native or bundle install budget and literal inputs.
    pub install: &'a wire::SoftwareTaskInvocation,
    /// Explicit removal budget and literal inputs.
    pub uninstall: Option<&'a wire::SoftwareTaskInvocation>,
    /// Independent detector.
    pub detection: &'a wire::SoftwareTaskDetection,
    /// Bundle install entry, when applicable.
    pub install_script: Option<&'a wire::SoftwareTaskScript>,
    /// Bundle removal entry, when applicable.
    pub uninstall_script: Option<&'a wire::SoftwareTaskScript>,
    /// Exact removal artifact; it may differ from the install artifact.
    pub removal_artifact: Option<&'a str>,
}
/// Read the existing Agent-supported formats without guessing a native implementation.
pub fn software_commands(action: &wire::SoftwareTaskAction) -> Result<SoftwareCommands<'_>, Error> {
    use wire::SoftwareTaskBehavior as B;
    Ok(match &action.behavior {
        B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => SoftwareCommands {
            install: &n.install,
            uninstall: n.uninstall.as_ref().map(|v| &v.invocation),
            detection: &n.detect,
            install_script: None,
            uninstall_script: None,
            removal_artifact: n.uninstall.as_ref().map(|v| v.installer.as_str()),
        },
        B::Bundle(b) => SoftwareCommands {
            install: &b.install.invocation,
            uninstall: b.uninstall.as_ref().map(|v| &v.invocation),
            detection: &b.detect,
            install_script: Some(&b.install),
            uninstall_script: b.uninstall.as_ref(),
            removal_artifact: None,
        },
        _ => return Err(Error::Unsupported),
    })
}
