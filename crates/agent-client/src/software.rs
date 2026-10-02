//! Borrowed inputs from the producer's current closed software model.
use crate::{wire, Error};
/// Borrowed commands; no alternate persisted software schema or compatibility decoder.
pub struct SoftwareCommands<'a> {
    /// Install identity and shared mutation budget.
    pub install: &'a wire::SoftwareTaskInvocation,
    /// Distinct approved update command, if any.
    pub upgrade: Option<&'a wire::SoftwareTaskInvocation>,
    /// Frozen update strategy.
    pub upgrade_policy: wire::SoftwareTaskUpgrade,
    /// Explicit removal budget and literal inputs.
    pub uninstall: Option<&'a wire::SoftwareTaskInvocation>,
    /// Independent detector, or format-native identity observation.
    pub detection: Option<&'a wire::SoftwareTaskDetection>,
    /// Bundle install entry.
    pub install_script: Option<&'a wire::SoftwareTaskScript>,
    /// Bundle removal entry.
    pub uninstall_script: Option<&'a wire::SoftwareTaskScript>,
    /// Exact removal artifact; it may differ from the install artifact.
    pub removal_artifact: Option<&'a str>,
}
/// Select source-authorized physical inputs without inventing arguments or native selectors.
pub fn software_commands(action: &wire::SoftwareTaskAction) -> Result<SoftwareCommands<'_>, Error> {
    use wire::SoftwareTaskBehavior as B;
    Ok(match &action.behavior {
        B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => SoftwareCommands {
            install: &n.install,
            upgrade: Some(&n.upgrade_invocation),
            upgrade_policy: n.upgrade,
            uninstall: n.uninstall.as_ref().map(|v| &v.invocation),
            detection: Some(&n.detect),
            install_script: None,
            uninstall_script: None,
            removal_artifact: n.uninstall.as_ref().map(|v| v.installer.as_str()),
        },
        B::Exe(n) => SoftwareCommands {
            install: &n.install,
            upgrade: Some(&n.upgrade_invocation),
            upgrade_policy: n.upgrade,
            uninstall: n.uninstall.as_ref().map(|v| &v.invocation),
            detection: Some(&n.detect),
            install_script: None,
            uninstall_script: None,
            removal_artifact: n.uninstall.as_ref().map(|v| v.installer.as_str()),
        },
        B::Bundle(b) => SoftwareCommands {
            install: &b.install.invocation,
            upgrade: Some(&b.install.invocation),
            upgrade_policy: wire::SoftwareTaskUpgrade::InPlace,
            uninstall: b.uninstall.as_ref().map(|v| &v.invocation),
            detection: Some(&b.detect),
            install_script: Some(&b.install),
            uninstall_script: b.uninstall.as_ref(),
            removal_artifact: None,
        },
        B::Dmg(d) => {
            let (uninstall, removal_artifact) = match &d.payload {
                wire::SoftwareTaskDmgPayload::AppCopy { uninstall, .. } => {
                    (uninstall.then_some(&d.invocation), None)
                }
                wire::SoftwareTaskDmgPayload::ContainedPkg { uninstall, .. } => (
                    uninstall.as_ref().map(|v| &v.invocation),
                    uninstall.as_ref().map(|v| v.installer.as_str()),
                ),
            };
            SoftwareCommands {
                install: &d.invocation,
                upgrade: Some(&d.invocation),
                upgrade_policy: d.upgrade,
                uninstall,
                detection: None,
                install_script: None,
                uninstall_script: None,
                removal_artifact,
            }
        }
        B::Msix(m) => SoftwareCommands {
            install: &m.invocation,
            upgrade: Some(&m.invocation),
            upgrade_policy: m.upgrade,
            uninstall: m.uninstall.then_some(&m.invocation),
            detection: None,
            install_script: None,
            uninstall_script: None,
            removal_artifact: None,
        },
    })
}
/// One source-authorized attempt budget, including the longest possible approved mutation path.
/// Before/after detectors share a single observation budget; DMG phases share its invocation budget.
pub fn software_budget(spec: &wire::SoftwareTaskSpec) -> Result<(u64, u64), Error> {
    let mut timeout = 0u64;
    let mut output = 0u64;
    for step in &spec.steps {
        let c = software_commands(&step.action)?;
        let mut mutation = Vec::new();
        match spec.intent {
            wire::SoftwareTaskIntent::Install => {
                mutation.push(c.install);
                if c.upgrade_policy == wire::SoftwareTaskUpgrade::UninstallThenInstall {
                    mutation.push(c.uninstall.ok_or(Error::Unsupported)?);
                } else if let Some(upgrade) = c.upgrade {
                    timeout = timeout.saturating_add(
                        u64::from(c.install.timeout_seconds.max(upgrade.timeout_seconds)) * 1000,
                    );
                    output = output.saturating_add(u64::from(
                        c.install.output_bytes.max(upgrade.output_bytes),
                    ));
                    mutation.clear();
                }
            }
            wire::SoftwareTaskIntent::Uninstall => {
                mutation.push(c.uninstall.ok_or(Error::Unsupported)?)
            }
            wire::SoftwareTaskIntent::Detect => (),
        }
        for invocation in mutation {
            timeout = timeout.saturating_add(u64::from(invocation.timeout_seconds) * 1000);
            output = output.saturating_add(u64::from(invocation.output_bytes));
        }
        match c.detection {
            Some(wire::SoftwareTaskDetection::Script { command }) => {
                timeout =
                    timeout.saturating_add(u64::from(command.invocation.timeout_seconds) * 1000);
                output = output.saturating_add(u64::from(command.invocation.output_bytes));
            }
            // Fixed native observations use this same invocation budget, not a new grant.
            _ if spec.intent == wire::SoftwareTaskIntent::Detect => {
                timeout = timeout.saturating_add(u64::from(c.install.timeout_seconds) * 1000);
                output = output.saturating_add(u64::from(c.install.output_bytes));
            }
            _ => (),
        }
    }
    Ok((
        timeout.clamp(1000, 86_400_000),
        output.clamp(1024, 1_048_576),
    ))
}
