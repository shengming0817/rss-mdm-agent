//! Local security evidence. Configuration never proves active protection or escrow.
//! ref: osquery/osquery@5.23.1 osquery/tables/system/darwin/{gatekeeper,xprotect,sip_config}.cpp.
use super::*;
use w::MacosSecuritySection as S;
pub(super) fn collect(c: &Context<'_>, section: S, rows: &mut Rows<'_>) -> Result<()> {
    match section {
        S::Gatekeeper => {
            let data = c.command("spctl", &["--status"])?;
            let configured = switch(
                section,
                std::str::from_utf8(&data).map_err(|_| Status::Invalid)?,
            )?;
            let mut r = row(&[
                ("presence", "present".into()),
                ("configured", configured.into()),
                ("running", "unknown".into()),
            ]);
            // Current system policy layout only; no historical fallback and no policy DB access.
            match c.plist(Path::new(
                "/var/db/SystemPolicyConfiguration/gke.bundle/Contents/version.plist",
            )) {
                Ok(value) => {
                    r.insert(
                        "version".into(),
                        text(object(&value)?, "CFBundleShortVersionString")?.into(),
                    );
                }
                Err(Fault::Unclosed) => return Err(Fault::Unclosed),
                Err(Fault::Query(s)) => {
                    rows.push(r)?;
                    return Err(s.into());
                }
            }
            rows.push(r)?;
        }
        S::Xprotect => {
            let value = c.plist(Path::new(
                "/Library/Apple/System/Library/CoreServices/XProtect.bundle/Contents/Info.plist",
            ))?;
            rows.push(row(&[
                ("presence", "present".into()),
                ("configured", "unknown".into()),
                ("running", "unknown".into()),
                (
                    "version",
                    text(object(&value)?, "CFBundleShortVersionString")?.into(),
                ),
            ]))?;
        }
        S::SystemExtensions => {
            let data = c.command("systemextensionsctl", &["list"])?;
            extensions(
                std::str::from_utf8(&data).map_err(|_| Status::Invalid)?,
                rows,
            )?;
        }
        S::HardwareEncryption => {
            // diskutil's Encryption/FileVault describe the volume, not hardware encryption.
            // No approved public source exposes its current hardware state independently.
            rows.push(row(&[("state", "unknown".into())]))?;
            return Err(Status::Unsupported.into());
        }
        _ => {
            let username;
            let (name, args): (&str, Vec<&str>) = match section {
                S::Filevault => ("fdesetup", vec!["status"]),
                S::Firewall => ("socketfilterfw", vec!["--getglobalstate"]),
                S::Sip => ("csrutil", vec!["status"]),
                S::BootstrapToken => ("profiles", vec!["status", "-type", "bootstraptoken"]),
                S::SecureToken => {
                    let w::SoftwareExecutionTarget::User { identity, .. } =
                        &c.input.read.execution.target
                    else {
                        return Err(Status::Denied.into());
                    };
                    username = account_name(identity)?;
                    ("sysadminctl", vec!["-secureTokenStatus", &username])
                }
                _ => unreachable!(),
            };
            let bytes = c.command(name, &args)?;
            let state = switch(
                section,
                std::str::from_utf8(&bytes).map_err(|_| Status::Invalid)?,
            )?;
            let mut r = row(&[("state", state.into())]);
            if section == S::SecureToken {
                r.insert(
                    "owner".into(),
                    c.input
                        .read
                        .execution
                        .generated_uid
                        .as_deref()
                        .ok_or(Status::Denied)?
                        .into(),
                );
            }
            rows.push(r)?;
            if state == "unknown" {
                return Err(Status::Partial.into());
            }
        }
    }
    Ok(())
}
pub(super) fn switch(section: S, output: &str) -> Result<&'static str> {
    let s = output.trim();
    let result = match section {
        S::Gatekeeper => match s {
            "assessments enabled" => "enabled",
            "assessments disabled" => "disabled",
            _ => return Err(Status::Invalid.into()),
        },
        S::Filevault => match s {
            "FileVault is On." => "enabled",
            "FileVault is Off." => "disabled",
            _ if s.contains("Encryption in progress")
                || s.contains("Decryption in progress")
                || s.contains("FileVault is On but Deferred") =>
            {
                "unknown"
            }
            _ => return Err(Status::Invalid.into()),
        },
        S::Firewall => match s {
            "Firewall is enabled. (State = 1)"
            | "Firewall is set to block all non-essential incoming connections. (State = 2)" => {
                "enabled"
            }
            "Firewall is disabled. (State = 0)" => "disabled",
            _ => return Err(Status::Invalid.into()),
        },
        S::Sip => match s {
            "System Integrity Protection status: enabled." => "enabled",
            "System Integrity Protection status: disabled." => "disabled",
            _ if s.starts_with(
                "System Integrity Protection status: unknown (Custom Configuration).",
            ) =>
            {
                "unknown"
            }
            _ => return Err(Status::Invalid.into()),
        },
        S::SecureToken => {
            let enabled = s.contains("Secure token is ENABLED for user ");
            let disabled = s.contains("Secure token is DISABLED for user ");
            match (enabled, disabled) {
                (true, false) => "enabled",
                (false, true) => "disabled",
                _ => return Err(Status::Invalid.into()),
            }
        }
        S::BootstrapToken => {
            // Escrowed-to-server is a local claim, not server custody evidence. This recipe
            // only records whether this host reports a locally escrowed bootstrap token.
            let line = s
                .lines()
                .find(|line| line.contains("Bootstrap Token escrowed to server:"))
                .ok_or(Status::Invalid)?;
            match line.rsplit(':').next().map(str::trim) {
                Some("YES") => "enabled",
                Some("NO") => "disabled",
                _ => return Err(Status::Invalid.into()),
            }
        }
        _ => return Err(Status::Unsupported.into()),
    };
    Ok(result)
}
pub(super) fn extensions(output: &str, rows: &mut Rows<'_>) -> Result<()> {
    let mut lines = output.lines();
    let count = lines
        .next()
        .and_then(|s| s.strip_suffix(" extension(s)"))
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or(Status::Invalid)?;
    let mut seen = 0;
    for line in lines {
        let s = line.trim();
        if s.is_empty() || s.starts_with("---") || s.starts_with("enabled") {
            continue;
        }
        let columns: Vec<_> = s.split_whitespace().filter(|v| *v != "*").collect();
        if columns.len() < 4 {
            return Err(Status::Invalid.into());
        }
        let (team, bundle, version) = (columns[0], columns[1], columns[2]);
        if team.len() != 10
            || !team
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            || !version.starts_with('(')
            || !version.ends_with(')')
        {
            return Err(Status::Invalid.into());
        }
        let configured = if s.ends_with("[activated enabled]") {
            "enabled"
        } else if s.ends_with("[activated disabled]")
            || s.ends_with("[terminated waiting to uninstall on reboot]")
        {
            "disabled"
        } else {
            "unknown"
        };
        rows.push(row(&[
            ("id", format!("{team}:{bundle}").into()),
            ("team_id", team.into()),
            ("bundle_id", bundle.into()),
            ("version", version[1..version.len() - 1].into()),
            ("configured", configured.into()),
            ("running", "unknown".into()),
        ]))?;
        seen += 1;
    }
    if seen != count {
        return Err(Status::Invalid.into());
    }
    Ok(())
}
