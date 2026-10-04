//! Local admission checks over the generated software API values.
//! ref: rss-mdm agent-wire tasks/software.rs at 149c34d5.
use crate::{protocol::*, Error};
use agent_api::runtime::*;
use std::collections::BTreeSet;
fn invalid() -> Error {
    Error::Protocol
}
fn text(s: &str, n: usize) -> Result<(), Error> {
    if s.is_empty() || s.len() > n || s.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(())
}
fn relative(s: &str) -> Result<(), Error> {
    text(s, 1024)?;
    if s.contains(['\\', ':'])
        || s.split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.ends_with([' ', '.']))
    {
        return Err(invalid());
    }
    Ok(())
}
fn invocation(v: &SoftwareTaskInvocation) -> Result<(), Error> {
    if !(1..=86400).contains(&v.timeout_seconds)
        || !(1..=1_048_576).contains(&v.output_bytes)
        || v.arguments.len() > 128
        || v.environment.len() > 32
    {
        return Err(invalid());
    }
    if v.arguments
        .iter()
        .any(|a| a.len() > 4096 || a.contains('\0'))
        || v.exit_codes.success.is_empty()
        || v.exit_codes.success.len() + v.exit_codes.reboot.len() > 32
        || !v.exit_codes.success.is_disjoint(&v.exit_codes.reboot)
    {
        return Err(invalid());
    }
    if v.environment.iter().any(|(k, v)| {
        !k.starts_with("RSS_PARAM_")
            || k.len() > 128
            || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || v.len() > 4096
            || v.contains('\0')
    }) {
        return Err(invalid());
    }
    Ok(())
}
fn script(v: &SoftwareTaskScript) -> Result<(), Error> {
    relative(&v.entry)?;
    invocation(&v.invocation)
}
fn detection(v: &SoftwareTaskDetection) -> Result<(), Error> {
    match v {
        SoftwareTaskDetection::MsiProduct {
            product_code,
            version,
        } => {
            text(product_code, 38)?;
            text(version, 1024)
        }
        SoftwareTaskDetection::PkgReceipt { receipt, version } => {
            text(receipt, 255)?;
            text(version, 1024)
        }
        SoftwareTaskDetection::Registry {
            key,
            value,
            version,
            ..
        } => {
            text(key, 1024)?;
            text(value, 255)?;
            text(version, 1024)
        }
        SoftwareTaskDetection::File {
            path,
            version,
            sha256,
            ..
        } => {
            relative(path)?;
            text(version, 1024)?;
            if *sha256 == [0; 32] {
                return Err(invalid());
            }
            Ok(())
        }
        SoftwareTaskDetection::Script { command } => script(command),
    }
}
fn removal(v: &Option<SoftwareTaskRemoval>) -> Result<(), Error> {
    if let Some(v) = v {
        text(&v.installer, 128)?;
        invocation(&v.invocation)?;
    }
    Ok(())
}
fn native_scope(
    scope: SoftwareTaskScope,
    install: &SoftwareTaskInvocation,
    upgrade: &SoftwareTaskInvocation,
    uninstall: &Option<SoftwareTaskRemoval>,
    policy: SoftwareTaskUpgrade,
) -> Result<(), Error> {
    let run_as = match scope {
        SoftwareTaskScope::System => ExecutionIdentity::System,
        SoftwareTaskScope::User => ExecutionIdentity::LoggedInUser,
    };
    if install.run_as != run_as
        || upgrade.run_as != run_as
        || uninstall
            .as_ref()
            .is_some_and(|u| u.invocation.run_as != run_as)
        || policy == SoftwareTaskUpgrade::UninstallThenInstall && uninstall.is_none()
    {
        return Err(invalid());
    }
    Ok(())
}
fn identity(v: &SoftwareTaskMsixIdentity) -> Result<(), Error> {
    text(&v.name, 255)?;
    text(&v.publisher, 1024)?;
    if v.resource_id.len() > 255 {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn validate_action(v: &SoftwareTaskAction) -> Result<(), Error> {
    text(&v.package, 1024)?;
    text(&v.version, 1024)?;
    text(v.behavior.installer(), 128)?;
    invocation(v.behavior.invocation())?;
    if v.signatures.len() > 64 {
        return Err(invalid());
    }
    for s in &v.signatures {
        text(&s.artifact, 128)?;
        text(&s.publisher, 1024)?;
    }
    match &v.behavior {
        SoftwareTaskBehavior::Msi(n)
        | SoftwareTaskBehavior::Pkg(n)
        | SoftwareTaskBehavior::Winget(n)
        | SoftwareTaskBehavior::Brew(n) => {
            invocation(&n.upgrade_invocation)?;
            native_scope(
                n.scope,
                &n.install,
                &n.upgrade_invocation,
                &n.uninstall,
                n.upgrade,
            )?;
            detection(&n.detect)?;
            removal(&n.uninstall)
        }
        SoftwareTaskBehavior::Exe(n) => {
            invocation(&n.upgrade_invocation)?;
            native_scope(
                n.scope,
                &n.install,
                &n.upgrade_invocation,
                &n.uninstall,
                n.upgrade,
            )?;
            detection(&n.detect)?;
            removal(&n.uninstall)?;
            if n.layout.is_empty() || n.layout.len() > 64 {
                return Err(invalid());
            }
            for (path, key) in &n.layout {
                relative(path)?;
                text(key, 128)?;
            }
            Ok(())
        }
        SoftwareTaskBehavior::Bundle(b) => {
            if b.manifest.schema != 1
                || b.manifest.entries.is_empty()
                || b.manifest.entries.len() > 4096
            {
                return Err(invalid());
            }
            for path in b.manifest.entries.keys() {
                relative(path)?;
            }
            script(&b.install)?;
            if let Some(u) = &b.uninstall {
                script(u)?;
                if u.invocation.run_as != b.install.invocation.run_as {
                    return Err(invalid());
                }
            }
            detection(&b.detect)
        }
        SoftwareTaskBehavior::Dmg(d) => {
            text(&d.volume, 255)?;
            match &d.payload {
                SoftwareTaskDmgPayload::AppCopy { application, .. } => {
                    relative(&application.path)?;
                    relative(&application.target_name)?;
                    text(&application.bundle_id, 255)?;
                    text(&application.version, 1024)
                }
                SoftwareTaskDmgPayload::ContainedPkg {
                    path,
                    length,
                    sha256,
                    receipt,
                    uninstall,
                } => {
                    relative(path)?;
                    text(receipt, 255)?;
                    if *length == 0 || *sha256 == [0; 32] {
                        return Err(invalid());
                    }
                    if uninstall
                        .as_ref()
                        .is_some_and(|removal| removal.invocation.run_as != d.invocation.run_as)
                    {
                        return Err(invalid());
                    }
                    removal(uninstall)
                }
            }
        }
        SoftwareTaskBehavior::Msix(m) => {
            identity(&m.identity)?;
            if !m.invocation.arguments.is_empty()
                || !m.invocation.environment.is_empty()
                || !m.identity.resource_id.is_empty()
                || m.identity
                    .version
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(".")
                    != v.version
            {
                return Err(invalid());
            }
            match &m.deployment {
                SoftwareTaskMsixDeployment::DeviceProvisioning
                    if m.invocation.run_as != ExecutionIdentity::System || m.allow_unsigned =>
                {
                    return Err(invalid());
                }
                SoftwareTaskMsixDeployment::TargetUserRegistration(_)
                    if m.invocation.run_as != ExecutionIdentity::LoggedInUser =>
                {
                    return Err(invalid());
                }
                _ => (),
            }
            if m.dependencies.len() > 32 {
                return Err(invalid());
            }
            for d in &m.dependencies {
                identity(d)?;
            }
            if let SoftwareTaskMsixContainer::Bundle { members, .. } = &m.container {
                if members.is_empty() || members.len() > 64 {
                    return Err(invalid());
                }
                for x in members {
                    relative(&x.path)?;
                    identity(&x.identity)?;
                    if x.length == 0 || x.sha256 == [0; 32] {
                        return Err(invalid());
                    }
                }
            }
            Ok(())
        }
    }
}

fn collect_detector<'a>(
    detector: &'a SoftwareTaskDetection,
    keys: &mut Vec<&'a str>,
    bundle: bool,
) {
    if !bundle {
        if let SoftwareTaskDetection::Script { command } = detector {
            keys.push(&command.entry);
        }
    }
}
fn collect_removal<'a>(value: &'a Option<SoftwareTaskRemoval>, keys: &mut Vec<&'a str>) {
    if let Some(r) = value {
        keys.push(&r.installer);
    }
}
/// Check every material reference within its own step, plus the real target platform.
pub(crate) fn validate_step(
    step: &SoftwareTaskStep,
    index: usize,
    platform: TaskPlatform,
    arch: TaskArchitecture,
) -> Result<(), Error> {
    let b = &step.action.behavior;
    let prefix = format!("{index}/");
    let available: BTreeSet<&str> = step
        .artifacts
        .iter()
        .map(|a| a.key.strip_prefix(&prefix).ok_or_else(invalid))
        .collect::<Result<_, _>>()?;
    let mut required = vec![b.installer()];
    for signature in &step.action.signatures {
        let supported = match signature.mechanism {
            SoftwareTaskSignatureMechanism::AppleDeveloperId => platform == TaskPlatform::Macos,
            SoftwareTaskSignatureMechanism::Authenticode => platform == TaskPlatform::Windows,
            SoftwareTaskSignatureMechanism::Msix => {
                platform == TaskPlatform::Windows
                    && matches!(b, SoftwareTaskBehavior::Msix(_))
                    && signature.artifact == b.installer()
            }
        };
        if !supported {
            return Err(invalid());
        }
        required.push(&signature.artifact);
    }
    let expected = match b {
        SoftwareTaskBehavior::Msi(n) | SoftwareTaskBehavior::Winget(n) => {
            collect_removal(&n.uninstall, &mut required);
            collect_detector(&n.detect, &mut required, false);
            TaskPlatform::Windows
        }
        SoftwareTaskBehavior::Pkg(n) | SoftwareTaskBehavior::Brew(n) => {
            collect_removal(&n.uninstall, &mut required);
            collect_detector(&n.detect, &mut required, false);
            TaskPlatform::Macos
        }
        SoftwareTaskBehavior::Exe(n) => {
            collect_removal(&n.uninstall, &mut required);
            collect_detector(&n.detect, &mut required, false);
            for value in n.layout.values() {
                required.push(value);
            }
            if n.layout.len() != available.len()
                || n.layout.values().collect::<BTreeSet<_>>().len() != n.layout.len()
            {
                return Err(invalid());
            }
            TaskPlatform::Windows
        }
        SoftwareTaskBehavior::Dmg(d) => {
            if let SoftwareTaskDmgPayload::ContainedPkg { uninstall, .. } = &d.payload {
                collect_removal(uninstall, &mut required);
            }
            TaskPlatform::Macos
        }
        SoftwareTaskBehavior::Msix(m) => {
            if !m.identity.architecture.matches(arch)
                || m.dependencies.iter().any(|d| !d.architecture.matches(arch))
                || matches!(&m.container,SoftwareTaskMsixContainer::Bundle{members,..} if members.iter().any(|member|!member.identity.architecture.matches(arch)))
            {
                return Err(invalid());
            }
            if let SoftwareTaskMsixContainer::Bundle { members, .. } = &m.container {
                let mut application = 0;
                let mut paths = BTreeSet::new();
                for member in members {
                    if !paths.insert(&member.path) {
                        return Err(invalid());
                    }
                    if member.identity.name != m.identity.name
                        || member.identity.publisher != m.identity.publisher
                        || member.identity.version != m.identity.version
                    {
                        return Err(invalid());
                    }
                    if member.identity.resource_id.is_empty() {
                        application += 1;
                        if member.identity != m.identity {
                            return Err(invalid());
                        }
                    }
                }
                if application != 1 {
                    return Err(invalid());
                }
            }
            TaskPlatform::Windows
        }
        SoftwareTaskBehavior::Bundle(bundle) => {
            if bundle.manifest.architecture != arch || available.len() != 1 {
                return Err(invalid());
            }
            if !bundle.manifest.entries.contains_key(&bundle.install.entry) {
                return Err(invalid());
            }
            if bundle
                .uninstall
                .as_ref()
                .is_some_and(|r| !bundle.manifest.entries.contains_key(&r.entry))
            {
                return Err(invalid());
            }
            bundle.manifest.platform
        }
    };
    if expected != platform || required.into_iter().any(|key| !available.contains(key)) {
        return Err(invalid());
    }
    Ok(())
}
