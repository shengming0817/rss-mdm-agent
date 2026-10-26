//! Borrowed inputs from the producer's current closed software model.
use rss_mdm_agent_wire::runtime as wire;
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
    /// Exact removal artifact; it may differ from the install artifact.
    pub removal_artifact: Option<&'a str>,
}
/// Select source-authorized physical inputs without inventing arguments or native selectors.
pub fn software_commands(action: &wire::SoftwareTaskAction) -> SoftwareCommands<'_> {
    use wire::SoftwareTaskBehavior as B;
    match &action.behavior {
        B::Msi(n) | B::Pkg(n) | B::Winget(n) | B::Brew(n) => SoftwareCommands {
            install: &n.install,
            upgrade: Some(&n.upgrade_invocation),
            upgrade_policy: n.upgrade,
            uninstall: n.uninstall.as_ref().map(|v| &v.invocation),
            detection: Some(&n.detect),
            install_script: None,
            removal_artifact: n.uninstall.as_ref().map(|v| v.installer.as_str()),
        },
        B::Exe(n) => SoftwareCommands {
            install: &n.install,
            upgrade: Some(&n.upgrade_invocation),
            upgrade_policy: n.upgrade,
            uninstall: n.uninstall.as_ref().map(|v| &v.invocation),
            detection: Some(&n.detect),
            install_script: None,
            removal_artifact: n.uninstall.as_ref().map(|v| v.installer.as_str()),
        },
        B::Bundle(b) => SoftwareCommands {
            install: &b.install.invocation,
            upgrade: Some(&b.install.invocation),
            upgrade_policy: wire::SoftwareTaskUpgrade::InPlace,
            uninstall: b.uninstall.as_ref().map(|v| &v.invocation),
            detection: Some(&b.detect),
            install_script: Some(&b.install),
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
            removal_artifact: None,
        },
    }
}

/// Original task-local artifact keys needed by this intent and all its legal before-state branches.
/// This borrows signed declarations; it creates no second plan or persisted identity.
pub fn software_artifact_keys(
    step: &wire::SoftwareTaskStep,
    intent: wire::SoftwareTaskIntent,
) -> std::collections::BTreeSet<&str> {
    use wire::{SoftwareTaskBehavior as B, SoftwareTaskIntent as I};
    let commands = software_commands(&step.action);
    let mut keys = std::collections::BTreeSet::new();
    if let Some(wire::SoftwareTaskDetection::Script { command }) = commands.detection {
        keys.insert(if step.action.behavior.bundle().is_some() {
            step.action.behavior.installer()
        } else {
            &command.entry
        });
    }
    if intent == I::Install {
        keys.insert(step.action.behavior.installer());
    }
    if intent == I::Uninstall
        || intent == I::Install
            && commands.upgrade_policy == wire::SoftwareTaskUpgrade::UninstallThenInstall
    {
        if let Some(key) = commands.removal_artifact {
            keys.insert(key);
        }
        if matches!(step.action.behavior, B::Bundle(_)) {
            keys.insert(step.action.behavior.installer());
        }
    }
    if intent != I::Detect {
        // The existing EXE staging operation consumes its entire declared layout.
        if let B::Exe(exe) = &step.action.behavior {
            keys.extend(exe.layout.values().map(String::as_str));
        }
    }
    keys
}
