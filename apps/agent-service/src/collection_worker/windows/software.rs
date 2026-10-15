//! Complete device software evidence. Package deployment functions are never called.
//! ref: microsoft/windows-rs@0.61.0 Windows/Management/Deployment/mod.rs.
use super::*;
use ::windows::{
    core::HSTRING,
    ApplicationModel::Package,
    Management::Deployment::{PackageInstallState, PackageManager, PackageTypes},
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
};
use std::collections::{BTreeMap, BTreeSet};
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}
fn text(value: HSTRING) -> Result<String, Status> {
    if value.len() > 65536 {
        return Err(Status::Truncated);
    }
    String::from_utf16(value.as_wide()).map_err(|_| Status::Invalid)
}
fn profiles(budget: &Budget) -> Result<BTreeMap<String, bool>, Status> {
    let raw = standard::query(
        "root\\cimv2",
        "Win32_UserProfile",
        &[("SID", w::CimType::String), ("Loaded", w::CimType::Boolean)],
        budget,
    )?;
    if raw.len() > 4096 {
        return Err(Status::Truncated);
    }
    let mut result = BTreeMap::new();
    for row in raw {
        let sid = standard::text(&row, "SID")?;
        let loaded = row
            .get("Loaded")
            .and_then(|v| v.as_bool())
            .ok_or(Status::Invalid)?;
        if result.insert(sid.to_owned(), loaded).is_some() {
            return Err(Status::Invalid);
        }
    }
    let ids = result.keys().cloned().collect::<BTreeSet<_>>();
    let coverage = w::NativeUserCoverage {
        enumeration_complete: true,
        expected: ids.clone(),
        observed: ids.clone(),
        completed: ids,
    };
    if !coverage.complete() {
        return Err(Status::Invalid);
    }
    Ok(result)
}
fn item(
    origin: &str,
    owner: &str,
    view: &str,
    native: &str,
    name: String,
    version: String,
    publisher: String,
    location: String,
) -> Result<Row, Status> {
    if native.is_empty() || native.contains('\0') {
        return Err(Status::Invalid);
    }
    let id = standard::item_id(&[origin, &owner.to_lowercase(), view, &native.to_lowercase()]);
    Ok(Row::from([
        ("id".into(), id.into()),
        ("native_id".into(), native.into()),
        ("name".into(), name.into()),
        ("version".into(), version.into()),
        ("publisher".into(), publisher.into()),
        ("location".into(), location.into()),
        ("origin".into(), origin.into()),
        ("owner".into(), owner.into()),
        (
            "scope".into(),
            if owner.is_empty() { "device" } else { "user" }.into(),
        ),
    ]))
}
fn uninstall(
    rows: &mut Rows<'_>,
    owner: &str,
    view: w::RegistryView,
    budget: &Budget,
) -> Result<(), Status> {
    const PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
    let open = |path: &str| {
        if owner.is_empty() {
            registry::machine(path, view)
        } else {
            registry::user(owner, path, view)
        }
    };
    if !owner.is_empty() {
        let _hive = registry::user(owner, "", view)?;
    }
    let root = match open(PATH) {
        Ok(k) => k,
        Err(Status::Missing) => return Ok(()),
        Err(e) => return Err(e),
    };
    let before = registry::names(&root, budget, 65535)?;
    for native in &before {
        budget.check()?;
        let key = open(&format!("{PATH}\\{native}"))?;
        let optional = |name: &str| -> Result<String, Status> {
            match registry::value(&key, name, w::RegistryType::Sz) {
                Ok(v) => v.as_str().map(str::to_owned).ok_or(Status::Invalid),
                Err(Status::Missing) => Ok(String::new()),
                Err(e) => Err(e),
            }
        };
        let view = match view {
            w::RegistryView::Registry32 => "32",
            w::RegistryView::Registry64 => "64",
        };
        rows.push(item(
            &format!("win32.uninstall.{view}"),
            owner,
            view,
            native,
            optional("DisplayName")?,
            optional("DisplayVersion")?,
            optional("Publisher")?,
            optional("InstallLocation")?,
        )?)?;
    }
    let after = registry::names(&root, budget, 65535)?;
    if before.into_iter().collect::<BTreeSet<_>>() != after.into_iter().collect() {
        return Err(Status::Invalid);
    }
    Ok(())
}
fn package(p: &Package, owner: &str, origin: &str) -> Result<Row, Status> {
    let identity = p.Id().map_err(error)?;
    let full = text(identity.FullName().map_err(error)?)?;
    let name = text(identity.Name().map_err(error)?)?;
    let publisher = text(identity.Publisher().map_err(error)?)?;
    let version = identity.Version().map_err(error)?;
    let version = format!(
        "{}.{}.{}.{}",
        version.Major, version.Minor, version.Build, version.Revision
    );
    let location = text(
        p.InstalledLocation()
            .map_err(error)?
            .Path()
            .map_err(error)?,
    )?;
    item(origin, owner, "", &full, name, version, publisher, location)
}
fn packages(
    rows: &mut Rows<'_>,
    expected: &BTreeSet<String>,
    budget: &Budget,
) -> Result<(), Status> {
    // SAFETY: query process/thread owns and balances its WinRT initialization.
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(error)?;
    let _apartment = Apartment;
    let manager = PackageManager::new().map_err(error)?;
    let packages = manager
        .FindPackagesWithPackageTypes(PackageTypes::All)
        .map_err(error)?;
    let iterator = packages.First().map_err(error)?;
    let mut count = 0usize;
    while iterator.HasCurrent().map_err(error)? {
        budget.check()?;
        count += 1;
        if count > 65535 {
            return Err(Status::Truncated);
        }
        let p = iterator.Current().map_err(error)?;
        let full = p.Id().map_err(error)?.FullName().map_err(error)?;
        let users = manager
            .FindUsers(&full)
            .map_err(error)?
            .First()
            .map_err(error)?;
        let mut owners = BTreeSet::new();
        while users.HasCurrent().map_err(error)? {
            budget.check()?;
            let user = users.Current().map_err(error)?;
            let sid = text(user.UserSecurityId().map_err(error)?)?;
            if !expected.contains(&sid)
                || !owners.insert(sid.clone())
                || user.InstallState().map_err(error)? != PackageInstallState::Installed
            {
                return Err(Status::Invalid);
            }
            rows.push(package(&p, &sid, "msix.registered")?)?;
            users.MoveNext().map_err(error)?;
        }
        if owners.is_empty() {
            return Err(Status::Invalid);
        }
        iterator.MoveNext().map_err(error)?;
    }
    let provisioned = manager.FindProvisionedPackages().map_err(error)?;
    let count = provisioned.Size().map_err(error)?;
    if count > 65535 {
        return Err(Status::Truncated);
    }
    for i in 0..count {
        budget.check()?;
        rows.push(package(
            &provisioned.GetAt(i).map_err(error)?,
            "",
            "msix.provisioned",
        )?)?;
    }
    if provisioned.Size().map_err(error)? != count {
        return Err(Status::Invalid);
    }
    Ok(())
}
pub(super) fn read(rows: &mut Rows<'_>, budget: &Budget) -> Result<(), Status> {
    let before = profiles(budget)?;
    let expected = before.keys().cloned().collect::<BTreeSet<_>>();
    rows.result.users = Some(w::NativeUserCoverage {
        enumeration_complete: false,
        expected: expected.clone(),
        observed: Default::default(),
        completed: Default::default(),
    });
    let mut failure = None;
    for owner in std::iter::once("").chain(before.keys().map(String::as_str)) {
        if before.get(owner) == Some(&false) {
            failure.get_or_insert(Status::Unsupported);
            continue;
        }
        for view in [w::RegistryView::Registry32, w::RegistryView::Registry64] {
            if let Err(e) = uninstall(rows, owner, view, budget) {
                failure.get_or_insert(e);
            }
        }
    }
    if let Err(e) = packages(rows, &expected, budget) {
        failure.get_or_insert(e);
    }
    let after = profiles(budget)?;
    let coverage = rows.result.users.as_mut().ok_or(Status::Invalid)?;
    coverage.observed = after.keys().cloned().collect();
    coverage.enumeration_complete = true;
    if before != after {
        failure.get_or_insert(Status::Invalid);
    }
    if failure.is_none() {
        coverage.completed = expected;
    }
    failure.map_or(Ok(()), Err)
}
