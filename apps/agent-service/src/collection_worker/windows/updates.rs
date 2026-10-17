//! WUA discovery only. No update service registration, source writes or deployment APIs.
//! ref: microsoft/windows-rs 0.61.3 Win32/System/UpdateAgent.
use super::*;
use ::windows::{
    core::{implement, IUnknown, Interface, Ref, BSTR},
    Win32::{
        Foundation::VARIANT_BOOL,
        System::{Com::*, UpdateAgent::*, Variant::VARIANT},
    },
};
#[implement(ISearchCompletedCallback)]
struct Completed;
impl ISearchCompletedCallback_Impl for Completed_Impl {
    fn Invoke(
        &self,
        _: Ref<'_, ISearchJob>,
        _: Ref<'_, ISearchCompletedCallbackArgs>,
    ) -> ::windows::core::Result<()> {
        Ok(())
    }
}
struct Search(ISearchJob);
impl Drop for Search {
    fn drop(&mut self) {
        unsafe {
            if !self.0.IsCompleted().is_ok_and(|v| v.as_bool()) {
                let _ = self.0.RequestAbort();
            }
            // Provider cleanup can block; the existing query-process deadline remains the final bound.
            let _ = self.0.CleanUp();
        }
    }
}
fn failure(error_value: ::windows::core::Error) -> Status {
    match error_value.code().0 as u32 {
        0x8024001e | 0x8024401c | 0x8024402c | 0x80072ee2 | 0x80072ee7 | 0x80072efd => {
            Status::SourceUnavailable
        }
        0x80240024 => Status::NotScanned,
        0x80070005 => Status::Denied,
        0x80040154 => Status::Unsupported,
        _ => Status::Failed,
    }
}
fn source_id(value: BSTR) -> Result<String, Status> {
    uuid::Uuid::parse_str(value.to_string().trim_matches(['{', '}']))
        .map(|id| id.to_string())
        .map_err(|_| Status::Partial)
}
unsafe fn source() -> Result<String, Status> {
    unsafe {
        let manager: IUpdateServiceManager =
            CoCreateInstance(&UpdateServiceManager, None, CLSCTX_INPROC_SERVER).map_err(failure)?;
        let services = manager.Services().map_err(failure)?;
        let count = services.Count().map_err(failure)?;
        if !(0..=256).contains(&count) {
            return Err(Status::Invalid);
        }
        let mut found = None;
        for i in 0..count {
            let service: IUpdateService2 = services
                .get_Item(i)
                .map_err(failure)?
                .cast()
                .map_err(failure)?;
            if service.IsDefaultAUService().map_err(failure)?.as_bool() {
                if found.is_some() {
                    return Err(Status::Partial);
                }
                found = Some(source_id(service.ServiceID().map_err(failure)?)?);
            }
        }
        found.ok_or(Status::SourceUnavailable)
    }
}
unsafe fn search(
    searcher: &IUpdateSearcher,
    criteria: &str,
    budget: &Budget,
) -> Result<ISearchResult, Status> {
    unsafe {
        let callback: ISearchCompletedCallback = Completed.into();
        let callback: IUnknown = callback.cast().map_err(failure)?;
        let job = Search(
            searcher
                .BeginSearch(&BSTR::from(criteria), &callback, &VARIANT::default())
                .map_err(failure)?,
        );
        while !job.0.IsCompleted().map_err(failure)?.as_bool() {
            if budget.start.elapsed() >= budget.timeout.saturating_sub(Duration::from_millis(500)) {
                let _ = job.0.RequestAbort();
                return Err(Status::Timeout);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        searcher.EndSearch(&job.0).map_err(failure)
    }
}
fn item(
    id: &IUpdateIdentity,
    title: BSTR,
    origin: &str,
    state: &str,
    suffix: &str,
) -> Result<Row, Status> {
    unsafe {
        let guid = source_id(id.UpdateID().map_err(failure)?)?;
        let revision = id.RevisionNumber().map_err(failure)?;
        let title = title.to_string();
        if revision < 0 || title.len() > 65536 {
            return Err(Status::Invalid);
        }
        Ok(Row::from([
            ("id".into(), format!("{guid}.{revision}{suffix}").into()),
            ("title".into(), title.into()),
            ("version".into(), revision.to_string().into()),
            ("origin".into(), origin.into()),
            ("state".into(), state.into()),
        ]))
    }
}
pub(super) fn read(
    query: &w::NativeSystemQuery,
    rows: &mut Rows<'_>,
    budget: &Budget,
) -> Result<(), Status> {
    let w::NativeSystemQuery::WindowsUpdates { section, mode } = query else {
        return Err(Status::Invalid);
    };
    let _apartment = wmi::Apartment::open()?;
    unsafe {
        if *section == w::UpdateSection::Reboot {
            let info: ISystemInformation =
                CoCreateInstance(&SystemInformation, None, CLSCTX_INPROC_SERVER)
                    .map_err(failure)?;
            let required = info.RebootRequired().map_err(failure)?.as_bool();
            return rows.push(Row::from([(
                "state".into(),
                if required { "required" } else { "not_required" }.into(),
            )]));
        }
        let session: IUpdateSession =
            CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER).map_err(failure)?;
        let searcher = session.CreateUpdateSearcher().map_err(failure)?;
        searcher
            .SetCanAutomaticallyUpgradeService(VARIANT_BOOL(0))
            .map_err(failure)?;
        searcher
            .SetOnline(VARIANT_BOOL(if *mode == w::UpdateScanMode::Online {
                -1
            } else {
                0
            }))
            .map_err(failure)?;
        let origin = source()?;
        rows.result.scan.as_mut().ok_or(Status::Invalid)?.source = Some(origin.clone());
        if *section == w::UpdateSection::History {
            let count = searcher.GetTotalHistoryCount().map_err(failure)?;
            if count < 0 {
                return Err(Status::Invalid);
            }
            let mut offset = 0;
            while offset < count {
                budget.check()?;
                let entries = searcher
                    .QueryHistory(offset, (count - offset).min(32))
                    .map_err(failure)?;
                let n = entries.Count().map_err(failure)?;
                if n <= 0 || n > 32 {
                    return Err(Status::Partial);
                }
                for i in 0..n {
                    budget.check()?;
                    let entry = entries.get_Item(i).map_err(failure)?;
                    // A historical source is evidence in its own right, never the default rewritten.
                    if source_id(entry.ServiceID().map_err(failure)?)? != origin {
                        return Err(Status::Partial);
                    }
                    let state = match (
                        entry.Operation().map_err(failure)?,
                        entry.ResultCode().map_err(failure)?,
                    ) {
                        (uoInstallation, orcSucceeded) => "installed",
                        (_, orcFailed | orcAborted | orcSucceededWithErrors) => "failed",
                        _ => "unknown",
                    };
                    rows.push(item(
                        &entry.UpdateIdentity().map_err(failure)?,
                        entry.Title().map_err(failure)?,
                        &origin,
                        state,
                        &format!(".history.{}", offset + i),
                    )?)?;
                }
                offset += n;
            }
            return Err(if count == 0 {
                Status::NotScanned
            } else {
                Status::Partial
            });
        }
        if *mode == w::UpdateScanMode::Online {
            rows.result.scan.as_mut().ok_or(Status::Invalid)?.started_at =
                Some(now().map_err(|_| Status::Failed)?);
        }
        let result = search(
            &searcher,
            if *section == w::UpdateSection::Installed {
                "IsInstalled=1"
            } else {
                "IsInstalled=0"
            },
            budget,
        )?;
        let finished = now().map_err(|_| Status::Failed)?;
        rows.result.collected_at = finished;
        if *mode == w::UpdateScanMode::Online {
            rows.result
                .scan
                .as_mut()
                .ok_or(Status::Invalid)?
                .finished_at = Some(finished);
        }
        // Reject source drift during the operation; no cached or mixed-source completeness claim.
        if source()? != origin {
            return Err(Status::Partial);
        }
        let complete = result.ResultCode().map_err(failure)? == orcSucceeded;
        let updates = result.Updates().map_err(failure)?;
        let count = updates.Count().map_err(failure)?;
        if count < 0 {
            return Err(Status::Invalid);
        }
        if *section == w::UpdateSection::Scan {
            let scan = rows.result.scan.as_ref().ok_or(Status::Invalid)?;
            let mut row = Row::from([
                ("availability".into(), "available".into()),
                (
                    "mode".into(),
                    if *mode == w::UpdateScanMode::Online {
                        "online"
                    } else {
                        "local"
                    }
                    .into(),
                ),
                ("source".into(), origin.clone().into()),
                (
                    "coverage".into(),
                    if complete && *mode == w::UpdateScanMode::Online {
                        "complete"
                    } else {
                        "partial"
                    }
                    .into(),
                ),
            ]);
            if let Some(start) = scan.started_at {
                row.insert("scan_started_at".into(), start.into());
            }
            if let Some(end) = scan.finished_at {
                row.insert("scan_finished_at".into(), end.into());
            }
            rows.push(row)?;
        } else {
            for i in 0..count {
                budget.check()?;
                let update = updates.get_Item(i).map_err(failure)?;
                rows.push(item(
                    &update.Identity().map_err(failure)?,
                    update.Title().map_err(failure)?,
                    &origin,
                    if *section == w::UpdateSection::Installed {
                        "installed"
                    } else {
                        "available"
                    },
                    "",
                )?)?;
            }
        }
        if !complete || *mode == w::UpdateScanMode::Local {
            Err(if count == 0 && *mode == w::UpdateScanMode::Local {
                Status::NotScanned
            } else {
                Status::Partial
            })
        } else {
            Ok(())
        }
    }
}
