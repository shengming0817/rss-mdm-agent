//! ref: Microsoft msi.h MsiGetProductInfoExW/MsiConfigureProductExW;
//! microsoft/windows-rs Windows/{ApplicationModel,Management/Deployment}@0.61.3.
use super::*;
use windows::{
    core::HSTRING,
    ApplicationModel::{Package, PackageSignatureKind},
    Management::Deployment::PackageManager,
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
};
use windows_sys::Win32::System::Registry::*;
#[link(name = "msi")]
unsafe extern "system" {
    fn MsiGetProductInfoExW(
        product: *const u16,
        sid: *const u16,
        context: u32,
        property: *const u16,
        value: *mut u16,
        length: *mut u32,
    ) -> u32;
    fn MsiConfigureProductExW(
        product: *const u16,
        level: i32,
        state: i32,
        properties: *const u16,
    ) -> u32;
    fn MsiSetInternalUI(level: u32, owner: *mut isize) -> u32;
}
fn wide(value: &str) -> Result<Vec<u16>, Error> {
    if value.contains('\0') || value.len() > 32768 {
        return Err(Error::Untrusted);
    }
    Ok(value.encode_utf16().chain(Some(0)).collect())
}
fn property(
    product: &str,
    subject: Option<&str>,
    context: u32,
    name: &str,
) -> Result<Option<String>, Error> {
    let product = wide(product)?;
    let name = wide(name)?;
    let sid = subject.map(wide).transpose()?;
    let mut bytes = vec![0u16; 32769];
    let mut length = 32768;
    // SAFETY: all terminated inputs and bounded output remain live for this synchronous call.
    let rc = unsafe {
        MsiGetProductInfoExW(
            product.as_ptr(),
            sid.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            context,
            name.as_ptr(),
            bytes.as_mut_ptr(),
            &mut length,
        )
    };
    match rc {
        0 => Ok(Some(
            String::from_utf16(&bytes[..length as usize]).map_err(|_| Error::Untrusted)?,
        )),
        1605 => Ok(None),
        _ => Err(Error::Unavailable),
    }
}
fn flag(product: &str, subject: Option<&str>, view: u32, name: &str) -> Result<u32, Error> {
    let path = wide(&format!(
        "{}SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        subject.map(|s| format!("{s}\\")).unwrap_or_default(),
        product
    ))?;
    let name = wide(name)?;
    let mut result = 0u32;
    let mut size = 4;
    // RegGetValue's WOW64 selector applies to this exact subkey; no UninstallString is read.
    let rc = unsafe {
        RegGetValueW(
            if subject.is_some() {
                HKEY_USERS
            } else {
                HKEY_LOCAL_MACHINE
            },
            path.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD
                | if view == 32 {
                    RRF_SUBKEY_WOW6432KEY
                } else {
                    RRF_SUBKEY_WOW6464KEY
                },
            std::ptr::null_mut(),
            (&mut result as *mut u32).cast(),
            &mut size,
        )
    };
    match rc {
        0 => Ok(result),
        2 => Ok(0),
        _ => Err(Error::Untrusted),
    }
}
fn msi(product: &str, subject: Option<&str>, view: u32) -> Result<Target, Error> {
    let guid = uuid::Uuid::parse_str(product).map_err(|_| Error::Unsupported)?;
    if product.to_ascii_uppercase() != format!("{{{}}}", guid.hyphenated()).to_ascii_uppercase() {
        return Err(Error::Unsupported);
    }
    if flag(product, subject, view, "WindowsInstaller")? != 1
        || flag(product, subject, view, "SystemComponent")? != 0
        || flag(product, subject, view, "NoRemove")? != 0
    {
        return Err(Error::Denied);
    }
    let context = if subject.is_some() { 2 } else { 4 };
    let version = property(product, subject, context, "VersionString")?.ok_or(Error::Conflict)?;
    let publisher = property(product, subject, context, "Publisher")?.ok_or(Error::Untrusted)?;
    // System servicing components need an explicit organization recipe.
    if publisher.is_empty() || publisher.starts_with("Microsoft") {
        return Err(Error::Denied);
    }
    let cached_path = PathBuf::from(
        property(product, subject, context, "LocalPackage")?.ok_or(Error::Untrusted)?,
    );
    let cached = crate::backend::plan::pin(&cached_path)?;
    Ok(Target::Msi {
        product: product.into(),
        version,
        publisher,
        scope: if subject.is_some() {
            LocalScope::User
        } else {
            LocalScope::Device
        },
        subject: subject.map(str::to_owned),
        cached_path,
        cached_sha256: cached.artifact.sha256.as_str().into(),
        view,
    })
}
struct Apartment;
impl Apartment {
    fn new() -> Result<Self, Error> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|_| Error::Unavailable)?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() }
    }
}
fn find(manager: &PackageManager, subject: &str, full: &str) -> Result<Option<Package>, Error> {
    let packages = manager
        .FindPackagesByUserSecurityId(&HSTRING::from(subject))
        .map_err(|_| Error::Unavailable)?;
    let mut found = None;
    for (i, p) in packages.into_iter().enumerate() {
        if i >= 65535 {
            return Err(Error::Capacity);
        }
        if p.Id()
            .and_then(|id| id.FullName())
            .map_err(|_| Error::Unavailable)?
            .to_string()
            == full
        {
            if found.is_some() {
                return Err(Error::Untrusted);
            }
            found = Some(p);
        }
    }
    Ok(found)
}
fn msix(p: &Package) -> Result<Target, Error> {
    if p.IsFramework().map_err(|_| Error::Unavailable)?
        || p.IsResourcePackage().map_err(|_| Error::Unavailable)?
        || matches!(
            p.SignatureKind().map_err(|_| Error::Unavailable)?,
            PackageSignatureKind::None | PackageSignatureKind::System
        )
    {
        return Err(Error::Denied);
    }
    let id = p.Id().map_err(|_| Error::Unavailable)?;
    let v = id.Version().map_err(|_| Error::Unavailable)?;
    Ok(Target::Msix {
        full_name: id.FullName().map_err(|_| Error::Unavailable)?.to_string(),
        version: format!("{}.{}.{}.{}", v.Major, v.Minor, v.Build, v.Revision),
        publisher: id.Publisher().map_err(|_| Error::Unavailable)?.to_string(),
    })
}
pub(super) fn select(item: &InstalledSoftware, _row: &serde_json::Value) -> Result<Target, Error> {
    let target = match item.kind {
        InstalledSoftwareKind::WindowsProgram => {
            let subject = match &item.scope {
                DataScope::User { subject } => Some(subject.as_str()),
                DataScope::Device => None,
            };
            let view = match item.origin.as_str() {
                "win32.uninstall.32" => 32,
                "win32.uninstall.64" => 64,
                _ => return Err(Error::Unsupported),
            };
            msi(&item.native_id, subject, view)?
        }
        InstalledSoftwareKind::Msix => {
            let DataScope::User { subject } = &item.scope else {
                return Err(Error::Unsupported);
            };
            if item.origin != "msix.registered" {
                return Err(Error::Unsupported);
            }
            let _apartment = Apartment::new()?;
            let manager = PackageManager::new().map_err(|_| Error::Unavailable)?;
            msix(&find(&manager, subject.as_str(), &item.native_id)?.ok_or(Error::Conflict)?)?
        }
        _ => return Err(Error::Unsupported),
    };
    let (version, publisher) = match &target {
        Target::Msi {
            version, publisher, ..
        }
        | Target::Msix {
            version, publisher, ..
        } => (version, publisher),
        _ => return Err(Error::Unsupported),
    };
    if item.version.as_ref() != Some(version) || item.publisher.as_ref() != Some(publisher) {
        return Err(Error::Conflict);
    }
    Ok(target)
}
pub(super) fn execute(input: &Input) -> Result<Option<SoftwareState>, Error> {
    match &input.target {
        Target::Msi {
            product,
            version,
            subject,
            view,
            ..
        } => {
            if msi(product, subject.as_deref(), *view)? != input.target {
                return Err(Error::Conflict);
            }
            if input.action == LocalSoftwareAction::Detect {
                return present(version);
            }
            if input.action != LocalSoftwareAction::Uninstall {
                return Err(Error::Unsupported);
            }
            // A user-context API must not fall through to a machine product with the same code.
            if subject.is_some() && property(product, None, 4, "VersionString")?.is_some() {
                return Err(Error::Denied);
            }
            let product = wide(product)?;
            let properties = wide("REBOOT=ReallySuppress")?;
            // UILEVEL_NONE: no interactive prompts or elevation credentials.
            unsafe {
                MsiSetInternalUI(2, std::ptr::null_mut());
            }
            let rc = unsafe { MsiConfigureProductExW(product.as_ptr(), 0, 2, properties.as_ptr()) };
            if !matches!(rc, 0 | 3010) {
                return Err(Error::Unavailable);
            }
            let Target::Msi { product, .. } = &input.target else {
                unreachable!()
            };
            if property(
                product,
                subject.as_deref(),
                if subject.is_some() { 2 } else { 4 },
                "VersionString",
            )?
            .is_some()
            {
                return Err(Error::Conflict);
            }
            Ok(Some(SoftwareState::Absent {}))
        }
        Target::Msix {
            full_name, version, ..
        } => {
            if !matches!(input.run_as, RunAs::User { .. }) {
                return Err(Error::Denied);
            }
            let _apartment = Apartment::new()?;
            let manager = PackageManager::new().map_err(|_| Error::Unavailable)?;
            let p = find(&manager, "", full_name)?.ok_or(Error::Conflict)?;
            if msix(&p)? != input.target {
                return Err(Error::Conflict);
            }
            match input.action {
                LocalSoftwareAction::Detect => present(version),
                LocalSoftwareAction::Launch => {
                    let apps = p
                        .GetAppListEntriesAsync()
                        .map_err(|_| Error::Unavailable)?
                        .get()
                        .map_err(|_| Error::Unavailable)?;
                    // Multiple entry points need explicit selection; never guess by display name.
                    if apps.Size().map_err(|_| Error::Unavailable)? != 1 {
                        return Err(Error::Unsupported);
                    }
                    if !apps
                        .GetAt(0)
                        .map_err(|_| Error::Unavailable)?
                        .LaunchAsync()
                        .map_err(|_| Error::Unavailable)?
                        .get()
                        .map_err(|_| Error::Unavailable)?
                    {
                        return Err(Error::Unavailable);
                    }
                    Ok(None)
                }
                LocalSoftwareAction::Uninstall => {
                    let completion = manager
                        .RemovePackageAsync(&HSTRING::from(full_name))
                        .map_err(|_| Error::Unavailable)?
                        .get()
                        .map_err(|_| Error::Unavailable)?;
                    if completion
                        .ExtendedErrorCode()
                        .map_err(|_| Error::Unavailable)?
                        .is_err()
                    {
                        return Err(Error::Unavailable);
                    }
                    if find(&manager, "", full_name)?.is_some() {
                        return Err(Error::Conflict);
                    }
                    Ok(Some(SoftwareState::Absent {}))
                }
                _ => Err(Error::Unsupported),
            }
        }
        _ => Err(Error::Unsupported),
    }
}
fn present(version: &str) -> Result<Option<SoftwareState>, Error> {
    Ok(Some(SoftwareState::Present {
        version: PackageValue::new(version).map_err(|_| Error::Untrusted)?,
    }))
}
