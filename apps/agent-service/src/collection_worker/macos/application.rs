//! Only approved application identities. No execution, arbitrary path or TCC access.
use super::*;
use std::collections::BTreeSet;
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_listallpids(buffer: *mut libc::c_void, size: i32) -> i32;
}
pub(super) fn collect(
    c: &Context<'_>,
    target: &w::MacosApplicationTarget,
    permissions: &BTreeSet<w::ApplicationPermission>,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let (identity, signer, presence, signature, running) = match target {
        w::MacosApplicationTarget::Agent => {
            let image = std::env::current_exe()?;
            let evidence = signature::inspect(&image, None)?;
            c.check()?;
            let image = installation_security::Artifact {
                path: image,
                sha256: c.input.image_sha256.as_str().to_owned(),
                cdhash: evidence.cdhash,
            };
            let running = if image.cdhash.is_some()
                && signature::pinned_process_matches(unsafe { libc::getppid() }, &image)?
            {
                "enabled"
            } else {
                "unknown"
            };
            (
                "rss-mdm-agent".to_owned(),
                evidence.team.unwrap_or_default(),
                "present",
                state(evidence.status),
                running,
            )
        }
        w::MacosApplicationTarget::Signed { bundle_id, team_id } => {
            let mut matched = 0;
            let mut invalid = false;
            native::app_paths(c, bundle_id, |bytes| {
                let path = Path::new(std::str::from_utf8(bytes).map_err(|_| Status::Invalid)?);
                let evidence = signature::inspect(path, Some((bundle_id, team_id)))?;
                c.check()?;
                if evidence.identifier.as_ref() == Some(bundle_id)
                    && evidence.team.as_ref() == Some(team_id)
                    && evidence.status == "signed"
                {
                    matched += 1;
                } else {
                    invalid = true;
                }
                Ok(())
            })?;
            let running = running(c, bundle_id, team_id)?;
            // Launch Services is not an exhaustive disk inventory. An unregistered copy
            // or an inaccessible signature cannot be presented as proven absence.
            (
                bundle_id.clone(),
                team_id.clone(),
                if matched > 0 || running {
                    "present"
                } else {
                    "unknown"
                },
                if matched > 0 || running {
                    "valid"
                } else if invalid {
                    "invalid"
                } else {
                    "unknown"
                },
                if running { "enabled" } else { "unknown" },
            )
        }
    };
    let permissions: Vec<_> = permissions
        .iter()
        .map(|p| {
            format!(
                "{}:unknown",
                match p {
                    w::ApplicationPermission::FullDiskAccess => "full_disk_access",
                    w::ApplicationPermission::Accessibility => "accessibility",
                    w::ApplicationPermission::ScreenCapture => "screen_capture",
                }
            )
        })
        .collect();
    // Public preflight APIs report the caller, not this requested target. Do not use
    // the worker's identity or read personal files as a proxy for the target's rights.
    rows.push(row(&[
        ("identity", identity.into()),
        ("signer", signer.into()),
        ("presence", presence.into()),
        ("signature", signature.into()),
        ("running", running.into()),
        ("health", "unknown".into()),
        ("permissions", serde_json::json!(permissions)),
    ]))?;
    Ok(())
}
fn state(status: &str) -> &'static str {
    match status {
        "signed" => "valid",
        "invalid" | "unsigned" => "invalid",
        _ => "unknown",
    }
}
fn running(c: &Context<'_>, bundle: &str, team: &str) -> Result<bool> {
    c.check()?;
    let remaining = c.cap.saturating_sub(c.bytes.get());
    let capacity = (remaining / std::mem::size_of::<i32>()).min(16384);
    if capacity == 0 {
        return Err(Status::Truncated.into());
    }
    let mut pids = vec![0i32; capacity];
    super::super::macos_supervisor::begin_read().map_err(|_| Fault::Unclosed)?;
    let count = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), (pids.len() * 4) as i32) };
    let accounted = if count > 0 {
        c.consume((count as usize).min(pids.len()) * 4)
    } else {
        Err(Status::Failed.into())
    };
    let ended = super::super::macos_supervisor::end_read().map_err(|_| Fault::Unclosed);
    accounted?;
    ended?;
    if count as usize >= pids.len() {
        return Err(Status::Truncated.into());
    }
    for pid in &pids[..count as usize] {
        c.check()?;
        if *pid > 0 && signature::process_matches(*pid, bundle, team)? {
            return Ok(true);
        }
    }
    Ok(false)
}
