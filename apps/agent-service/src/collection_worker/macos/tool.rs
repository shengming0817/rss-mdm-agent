//! Fixed tool requests are executed by the query supervisor, never by the confined provider.
use super::*;
impl Context<'_> {
    pub(super) fn run(
        &self,
        material: &SoftwareMaterial,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Result<Vec<u8>> {
        self.check()?;
        if !env.is_empty() {
            return Err(Status::Denied.into());
        }
        let name = self
            .input
            .tools
            .iter()
            .find(|(_, value)| value.path == material.path)
            .map(|(name, _)| name.as_str())
            .ok_or(Status::Denied)?;
        let reply = super::super::macos_supervisor::tool(name, args, self.cap)
            .map_err(|_| Fault::Unclosed)?;
        // Tool raw bytes were already counted by their parent owner, including stderr.
        self.bytes
            .set(self.bytes.get().saturating_add(reply.observed));
        self.input
            .observed
            .fetch_add(reply.observed as u64, std::sync::atomic::Ordering::Relaxed);
        if self.bytes.get() > self.cap {
            return Err(Status::Truncated.into());
        }
        if reply.status != Status::Complete {
            return Err(reply.status.into());
        }
        self.check()?;
        Ok(if name == "sysadminctl" {
            reply.diagnostics
        } else {
            reply.bytes
        })
    }
}

pub(crate) fn path(name: &str) -> Option<&'static str> {
    Some(match name {
        "sandbox-exec" => "/usr/bin/sandbox-exec",
        "system_profiler" => "/usr/sbin/system_profiler",
        "ioreg" => "/usr/sbin/ioreg",
        "diskutil" => "/usr/sbin/diskutil",
        "pkgutil" => "/usr/sbin/pkgutil",
        "spctl" => "/usr/sbin/spctl",
        "csrutil" => "/usr/bin/csrutil",
        "fdesetup" => "/usr/bin/fdesetup",
        "socketfilterfw" => "/usr/libexec/ApplicationFirewall/socketfilterfw",
        "systemextensionsctl" => "/usr/bin/systemextensionsctl",
        "profiles" => "/usr/bin/profiles",
        "sysadminctl" => "/usr/sbin/sysadminctl",
        _ => return None,
    })
}
fn names(definition: &w::NativeQueryDefinition) -> &'static [&'static str] {
    use w::{MacosSecuritySection as S, NativeQueryDefinition as D, NativeSystemQuery as Q};
    match definition {
        D::MacosHardware { .. } => &["ioreg", "system_profiler", "diskutil"],
        D::MacosPkgReceipts {} => &["pkgutil"],
        D::System {
            query: Q::MacosSecurity { section },
        } => match section {
            S::Gatekeeper => &["spctl"],
            S::Filevault => &["fdesetup"],
            S::Firewall => &["socketfilterfw"],
            S::Sip => &["csrutil"],
            S::SystemExtensions => &["systemextensionsctl"],
            S::BootstrapToken => &["profiles"],
            S::SecureToken => &["sysadminctl"],
            S::HardwareEncryption | S::Xprotect => &[],
        },
        _ => &[],
    }
}
pub(crate) fn required(plan: &w::NativeCollectionPlan) -> std::collections::BTreeSet<&'static str> {
    let mut all = std::collections::BTreeSet::from(["sandbox-exec"]);
    for q in plan.spec().queries.values() {
        all.extend(names(&q.definition));
    }
    all
}
pub(crate) fn allowed(
    d: &w::NativeQueryDefinition,
    name: &str,
    args: &[String],
    binding: &w::NativeExecutionBinding,
) -> bool {
    use w::{MacosSecuritySection as S, NativeQueryDefinition as D, NativeSystemQuery as Q};
    if !names(d).contains(&name) {
        return false;
    }
    let a: Vec<_> = args.iter().map(String::as_str).collect();
    match (d, name, a.as_slice()) {
        (
            D::MacosHardware {
                section: w::MacosHardwareSection::Identity | w::MacosHardwareSection::Firmware,
            },
            "system_profiler",
            ["-xml", "SPHardwareDataType", "-detailLevel", "basic"],
        ) => true,
        (
            D::MacosHardware {
                section: w::MacosHardwareSection::Battery,
            },
            "ioreg",
            ["-a", "-r", "-c", "AppleSmartBattery"],
        ) => true,
        (
            D::MacosHardware {
                section:
                    w::MacosHardwareSection::Disks
                    | w::MacosHardwareSection::Volumes
                    | w::MacosHardwareSection::ApfsContainers,
            },
            "diskutil",
            ["list", "-plist", "physical"] | ["list", "-plist"] | ["apfs", "list", "-plist"],
        ) => true,
        (
            D::MacosHardware {
                section: w::MacosHardwareSection::Disks | w::MacosHardwareSection::Volumes,
            },
            "diskutil",
            ["info", "-plist", device],
        ) => {
            *device == "/"
                || (device.starts_with("disk") && device.bytes().all(|b| b.is_ascii_alphanumeric()))
        }
        (D::MacosPkgReceipts {}, "pkgutil", ["--pkgs-plist"]) => true,
        (D::MacosPkgReceipts {}, "pkgutil", ["--pkg-info-plist", id]) => {
            !id.starts_with('-') && !id.is_empty() && id.len() <= 4096 && !id.contains('\0')
        }
        (
            D::System {
                query: Q::MacosSecurity { section },
            },
            _,
            args,
        ) => match (section, args) {
            (S::Gatekeeper, ["--status"])
            | (S::Filevault, ["status"])
            | (S::Sip, ["status"])
            | (S::Firewall, ["--getglobalstate"])
            | (S::SystemExtensions, ["list"])
            | (S::BootstrapToken, ["status", "-type", "bootstraptoken"]) => true,
            (S::SecureToken, ["-secureTokenStatus", user]) => {
                if let w::SoftwareExecutionTarget::User { identity, .. } = &binding.target {
                    super::account_name(identity).is_ok_and(|name| name == *user)
                } else {
                    false
                }
            }
            _ => false,
        },
        _ => false,
    }
}
