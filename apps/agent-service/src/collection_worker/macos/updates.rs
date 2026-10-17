//! Existing local evidence only. No softwareupdate command, scan, download or install.
use super::*;
const CACHE: &str = "/Library/Preferences/com.apple.SoftwareUpdate.plist";
const HISTORY: &str = "/Library/Receipts/InstallHistory.plist";
pub(super) fn collect(
    c: &Context<'_>,
    section: w::UpdateSection,
    mode: w::UpdateScanMode,
    rows: &mut Rows<'_>,
) -> Result<()> {
    if mode != w::UpdateScanMode::Local {
        return Err(Status::Unsupported.into());
    }
    match section {
        w::UpdateSection::Scan | w::UpdateSection::Available => {
            let value = c.plist(Path::new(CACHE)).map_err(source_error)?;
            cached(object(&value)?, section, rows)
        }
        w::UpdateSection::History => {
            let value = c.plist(Path::new(HISTORY)).map_err(source_error)?;
            let items = value.as_array().ok_or(Status::Invalid)?;
            rows.result.scan.as_mut().ok_or(Status::Invalid)?.source =
                Some("apple_install_history".into());
            for item in items {
                c.check()?;
                let d = object(item)?;
                let date = timestamp(d.get("date").ok_or(Status::Invalid)?)?;
                let title = text(d, "displayName")?;
                let version = text(d, "displayVersion")?;
                // History is an event, never the current installed state; repeated dates
                // and names are not silently merged into inventory instances.
                let id =
                    hex(&serde_json::to_vec(&(date, title, version))
                        .map_err(|_| Status::Invalid)?);
                rows.push(row(&[
                    ("id", id.into()),
                    ("title", title.into()),
                    ("version", version.into()),
                    ("origin", "apple_install_history".into()),
                    ("state", "installed".into()),
                ]))?;
            }
            // Historical events do not provide an update scan interval or complete scope.
            Err(Status::Partial.into())
        }
        w::UpdateSection::Installed => {
            let value = c.plist(Path::new(
                "/System/Library/CoreServices/SystemVersion.plist",
            ))?;
            let d = object(&value)?;
            rows.result.scan.as_mut().ok_or(Status::Invalid)?.source =
                Some("apple_system_version".into());
            rows.push(row(&[
                ("id", "macos".into()),
                ("title", text(d, "ProductName")?.into()),
                (
                    "version",
                    format!(
                        "{} ({})",
                        text(d, "ProductVersion")?,
                        text(d, "ProductBuildVersion")?
                    )
                    .into(),
                ),
                ("origin", "apple_system_version".into()),
                ("state", "installed".into()),
            ]))?;
            // Verifies the running OS; does not claim all historical patches remain installed.
            Err(Status::Partial.into())
        }
        w::UpdateSection::Reboot => {
            // There is no supported global pending-reboot read in the approved local sources.
            // RecommendedUpdates / InstallHistory cannot answer this question.
            rows.push(row(&[("state", "unknown".into())]))?;
            Err(Status::Unsupported.into())
        }
    }
}
pub(super) fn timestamp(value: &plist::Value) -> Result<i64> {
    let time: std::time::SystemTime = value.as_date().ok_or(Status::Invalid)?.into();
    i64::try_from(
        time.duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Status::Invalid)?
            .as_secs(),
    )
    .map_err(|_| Status::Invalid.into())
}
pub(super) fn cached(
    d: &plist::Dictionary,
    section: w::UpdateSection,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let result = d
        .get("LastResultCode")
        .and_then(plist::Value::as_signed_integer)
        .ok_or(Status::Invalid)?;
    let items = d
        .get("RecommendedUpdates")
        .and_then(plist::Value::as_array)
        .ok_or(Status::Invalid)?;
    let end = d.get("LastSuccessfulDate").map(timestamp).transpose()?;
    let scan = rows.result.scan.as_mut().ok_or(Status::Invalid)?;
    scan.source = Some("apple_softwareupdate_cache".into());
    // LastSuccessfulDate is a completion timestamp, not a start timestamp. The wire
    // cannot encode an end without a start. Row/evidence must agree; without
    // the original interval, leave both absent and report Partial.
    // Do not manufacture started_at=end or substitute this local read's clock.
    if end.is_some_and(|t| t > rows.result.collected_at || t < 1) {
        return Err(Status::Invalid.into());
    }
    if section == w::UpdateSection::Scan {
        rows.push(row(&[
            (
                "availability",
                if result != 0 || end.is_none() {
                    "unknown"
                } else if items.is_empty() {
                    "unavailable"
                } else {
                    "available"
                }
                .into(),
            ),
            ("mode", "local".into()),
            ("source", "apple_softwareupdate_cache".into()),
            (
                "coverage",
                if end.is_none() {
                    "not_scanned"
                } else {
                    "partial"
                }
                .into(),
            ),
        ]))?;
    } else {
        for item in items {
            let d = object(item)?;
            rows.push(row(&[
                ("id", text(d, "Product Key")?.into()),
                ("title", text(d, "Display Name")?.into()),
                ("version", text(d, "Display Version")?.into()),
                ("origin", "apple_softwareupdate_cache".into()),
                ("state", "available".into()),
            ]))?;
        }
    }
    Err(if result != 0 {
        Status::Failed
    } else {
        Status::Partial
    }
    .into())
}

fn source_error(error: Fault) -> Fault {
    match error {
        Fault::Query(Status::Missing) => Fault::Query(Status::SourceUnavailable),
        other => other,
    }
}
