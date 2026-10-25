//! Fixed PassportForWork adapter. This module never owns PINs or Hello private keys.
//! ref: MicrosoftDocs/win32 desktop-src/DMWmiBridgeProv/mdm-passportforwork-policies02.md;
//! microsoft/windows-rs 0.61.3 Win32/System/Wmi (IWbemContext, GetObject, PutInstance).
use super::super::*;
use device_access::hello::{HelloPolicy, HelloRecord};
use execution_ipc::device_access::{WindowsHelloAvailability as Availability, WindowsHelloStatus};
use w::{AccessFailure as Failure, AccessObservation as O};
use windows_sys::Win32::System::Registry::*;

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
fn dword(root: HKEY, path: &str, name: &str) -> Result<Option<u32>, Failure> {
    let mut key = std::ptr::null_mut();
    let result = unsafe {
        RegOpenKeyExW(
            root,
            wide(path).as_ptr(),
            0,
            KEY_READ | KEY_WOW64_64KEY,
            &mut key,
        )
    };
    if result == 2 || result == 3 {
        return Ok(None);
    }
    if result != 0 {
        return Err(Failure::NativeFailure);
    }
    let key = Key(key);
    let mut value = 0u32;
    let mut kind = 0;
    let mut size = 4;
    let result = unsafe {
        RegQueryValueExW(
            key.0,
            wide(name).as_ptr(),
            std::ptr::null(),
            &mut kind,
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    if result == 2 {
        return Ok(None);
    }
    if result != 0 || kind != REG_DWORD || size != 4 {
        return Err(Failure::NativeFailure);
    }
    Ok(Some(value))
}
#[repr(C)]
struct Version {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform: u32,
    service_pack: [u16; 128],
}
#[repr(C)]
struct TpmInfo {
    version: u32,
    tpm: u32,
    interface: u32,
    revision: u32,
}
#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(version: *mut Version) -> i32;
}
#[link(name = "tbs")]
unsafe extern "system" {
    fn Tbsi_GetDeviceInfo(size: u32, info: *mut std::ffi::c_void) -> u32;
}
fn supported() -> bool {
    let mut version = Version {
        size: std::mem::size_of::<Version>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        service_pack: [0; 128],
    };
    let mut tpm = TpmInfo {
        version: 1,
        tpm: 0,
        interface: 0,
        revision: 0,
    };
    cfg!(target_arch = "x86_64")
        && unsafe { RtlGetVersion(&mut version) } == 0
        && version.major == 10
        && version.build >= 22000
        && unsafe {
            Tbsi_GetDeviceInfo(
                std::mem::size_of::<TpmInfo>() as u32,
                (&mut tpm as *mut TpmInfo).cast(),
            )
        } == 0
        && tpm.tpm == 2
}
fn tenant(record: &HelloRecord) -> Result<uuid::Uuid, Failure> {
    match record.authority.source {
        w::AccessWindowsAuthority::Entra { tenant_id, .. } => Ok(tenant_id),
        _ => Err(Failure::Unsupported),
    }
}
fn user_path(record: &HelloRecord) -> Result<String, Failure> {
    Ok(format!(
        "SOFTWARE\\Microsoft\\Policies\\PassportForWork\\{}\\{}\\Policies",
        tenant(record)?,
        record.authority.sid
    ))
}
fn conflicts(record: &HelloRecord) -> Result<bool, Failure> {
    let sid = &record.authority.sid;
    for (root, path) in [
        (
            HKEY_LOCAL_MACHINE,
            "SOFTWARE\\Policies\\Microsoft\\PassportForWork".to_owned(),
        ),
        (
            HKEY_USERS,
            format!("{sid}\\SOFTWARE\\Policies\\Microsoft\\PassportForWork"),
        ),
    ] {
        for name in ["Enabled", "RequireSecurityDevice"] {
            if dword(root, &path, name)?.is_some() {
                return Ok(true);
            }
        }
    }
    let path = format!(
        "SOFTWARE\\Microsoft\\Policies\\PassportForWork\\{}\\Device\\Policies",
        tenant(record)?
    );
    if dword(HKEY_LOCAL_MACHINE, &path, "UsePassportForWork")?
        .is_some_and(|v| v != u32::from(record.enabled))
        || dword(HKEY_LOCAL_MACHINE, &path, "RequireSecurityDevice")?.is_some_and(|v| v != 1)
    {
        return Ok(true);
    }
    Ok(!record.policy.accepts(policy_value(record)?))
}
fn policy_value(record: &HelloRecord) -> Result<Option<bool>, Failure> {
    let path = user_path(record)?;
    match (
        dword(HKEY_LOCAL_MACHINE, &path, "UsePassportForWork")?,
        dword(HKEY_LOCAL_MACHINE, &path, "RequireSecurityDevice")?,
    ) {
        (None, None) => Ok(None),
        (Some(0), Some(1)) => Ok(Some(false)),
        (Some(1), Some(1)) => Ok(Some(true)),
        _ => Err(Failure::Unsupported),
    }
}
fn policy(record: &HelloRecord) -> Result<O, Failure> {
    if matches!(record.policy, HelloPolicy::Unmanaged) || conflicts(record)? {
        return Ok(O::Unknown);
    }
    Ok(match policy_value(record)? {
        Some(true) => O::Present,
        Some(false) => O::Absent,
        None => O::Unknown,
    })
}
/// Read-only eligibility and exact before-state, persisted by the worker before PutInstance.
pub(super) fn prepare(
    native: &super::NativeWindows,
    record: &HelloRecord,
) -> Result<Option<bool>, Failure> {
    verify(native, record)?;
    if conflicts(record)? {
        return Err(Failure::Unsupported);
    }
    policy_value(record)
}
fn user(subject: &str) -> Result<execution_ipc::device_access::WindowsHelloUser, Failure> {
    let deployment = crate::deployment::Deployment::metadata(
        &crate::deployment::Deployment::default_path().map_err(|_| Failure::NativeFailure)?,
    )
    .map_err(|_| Failure::NativeFailure)?;
    let connection = deployment
        .helpers()
        .connect(None)
        .map_err(|_| Failure::NeedsUser)?;
    let context = connection.context();
    let session = context.session;
    if context.subject != subject {
        return Err(Failure::NeedsUser);
    }
    execution_ipc::windows_identity::verify_unlocked_console(subject, session, &context.binding)
        .map_err(|_| Failure::NeedsUser)?;
    let reply = connection
        .exchange(execution_ipc::helper::Command::WindowsHello)
        .map_err(|_| Failure::NativeFailure)?;
    execution_ipc::windows_identity::verify_unlocked_console(subject, session, &context.binding)
        .map_err(|_| Failure::NeedsUser)?;
    match reply {
        execution_ipc::helper::Reply::WindowsHello(value) => Ok(value),
        _ => Err(Failure::NativeFailure),
    }
}
fn verify(native: &super::NativeWindows, record: &HelloRecord) -> Result<(), Failure> {
    let deployment = crate::deployment::Deployment::metadata(
        &crate::deployment::Deployment::default_path().map_err(|_| Failure::NativeFailure)?,
    )
    .map_err(|_| Failure::NativeFailure)?;
    if deployment.service_id != native.installation
        || super::super::root(&deployment.execution) != native.root
    {
        return Err(Failure::AccountChanged);
    }
    device_access::windows::require_system().map_err(|_| Failure::NativeFailure)?;
    device_access::windows::account::verify_bound(&record.authority)?;
    if !matches!(
        record.authority.source,
        w::AccessWindowsAuthority::Entra { .. }
    ) {
        return Err(Failure::Unsupported);
    }
    if record.enabled && !supported() {
        return Err(Failure::Unsupported);
    }
    Ok(())
}
pub(super) fn observe(
    native: &super::NativeWindows,
    record: &HelloRecord,
    mutate: bool,
) -> Result<(w::AccessWhfbFacts, bool), Failure> {
    verify(native, record)?;
    let w::AccessWindowsAuthority::Entra {
        tenant_id,
        object_id,
    } = record.authority.source
    else {
        return Err(Failure::Unsupported);
    };
    if mutate
        && (conflicts(record)?
            || !matches!(record.policy, HelloPolicy::WritePending { enabled, .. } if enabled == record.enabled))
    {
        return Err(Failure::Unsupported);
    }
    let mut observed = record.clone();
    if mutate {
        set_policy(record)?;
        observed.policy = HelloPolicy::Owned {
            enabled: record.enabled,
        };
    }
    let policy = policy(&observed)?;
    let current = user(&record.authority.sid).ok();
    let now = (crate::wall().map_err(|_| Failure::NativeFailure)? / 1000) as i64;
    let facts = w::AccessWhfbFacts {
        factor_generation: record.authority.factor_generation,
        tenant_id,
        object_id,
        policy,
        registration: current.as_ref().map_or(O::Unknown, |v| v.registration),
        hardware_key: current.as_ref().map_or(O::Unknown, |v| v.hardware_key),
        key_id: current.as_ref().and_then(|v| v.key_id),
        observed_at: current.as_ref().map_or(now, |v| v.observed_at),
    };
    facts
        .validate(&w::AccessAccount::Windows {
            sid: record.authority.sid.clone(),
            authority: record.authority.source.clone(),
        })
        .map_err(|_| Failure::NativeFailure)?;
    device_access::windows::account::verify_bound(&record.authority)?;
    let confirmed = !matches!(observed.policy, HelloPolicy::Unmanaged)
        && policy
            == if record.enabled {
                O::Present
            } else {
                O::Absent
            };
    Ok((facts, confirmed))
}
pub(crate) fn status(root: &Path, installation: uuid::Uuid, subject: &str) -> WindowsHelloStatus {
    let mut status = WindowsHelloStatus {
        availability: Availability::Unavailable,
        desired_enabled: None,
        retired: false,
        replacement_required: false,
        facts: None,
    };
    let native = match device_access::windows::account::inspect(subject) {
        Ok(v) => v,
        Err(_) => return status,
    };
    if !matches!(native.source, w::AccessWindowsAuthority::Entra { .. }) || !supported() {
        status.availability = Availability::Unsupported;
        return status;
    }
    let store = match device_access::Store::open(root, installation) {
        Ok(v) => v,
        Err(_) => return status,
    };
    let record = match store.hello(subject) {
        Ok(Some(v)) => v,
        Ok(None) => {
            status.availability = Availability::Eligible;
            return status;
        }
        Err(_) => return status,
    };
    status.desired_enabled = Some(record.enabled);
    status.retired = record.retired;
    if conflicts(&record).unwrap_or(true) {
        status.availability = Availability::Conflict;
        return status;
    }
    match observe(
        &super::NativeWindows {
            root: root.into(),
            installation,
        },
        &record,
        false,
    ) {
        Ok((mut facts, _)) => {
            status.availability = Availability::Eligible;
            status.replacement_required = record.rejects_key(facts.key_id);
            if status.replacement_required && !record.retired {
                facts.registration = O::Unknown;
                facts.hardware_key = O::Unknown;
            }
            status.facts = Some(facts);
        }
        Err(_) => (),
    }
    status
}

// A narrow WMI adapter: namespace, class, properties and context keys are all fixed.
use ::windows::{
    core::{BSTR, PCWSTR},
    Win32::{
        Foundation::VARIANT_BOOL,
        System::{
            Com::*,
            Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE},
            Variant::*,
            Wmi::*,
        },
    },
};
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}
struct Variant(VARIANT);
impl Drop for Variant {
    fn drop(&mut self) {
        unsafe {
            let _ = VariantClear(&mut self.0);
        }
    }
}
fn string(value: &str) -> Variant {
    let mut result = Variant(VARIANT::default());
    unsafe {
        let inner = &mut *result.0.Anonymous.Anonymous;
        inner.vt = VT_BSTR;
        inner.Anonymous.bstrVal = std::mem::ManuallyDrop::new(BSTR::from(value));
    }
    result
}
fn boolean(value: bool) -> Variant {
    let mut result = Variant(VARIANT::default());
    unsafe {
        let inner = &mut *result.0.Anonymous.Anonymous;
        inner.vt = VT_BOOL;
        inner.Anonymous.boolVal = VARIANT_BOOL(if value { -1 } else { 0 });
    }
    result
}
fn set_policy(record: &HelloRecord) -> Result<(), Failure> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|_| Failure::NativeFailure)?;
        let _apartment = Apartment;
        if let Err(e) = CoInitializeSecurity(
            None,
            -1,
            None,
            None,
            RPC_C_AUTHN_LEVEL_DEFAULT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
            None,
        ) {
            if e.code().0 as u32 != 0x80010119 {
                return Err(Failure::NativeFailure);
            }
        }
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| Failure::NativeFailure)?;
        let context: IWbemContext = CoCreateInstance(&WbemContext, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| Failure::NativeFailure)?;
        for (name, value) in [
            (
                "PolicyPlatformContext_PrincipalContext_Type",
                "PolicyPlatform_UserContext",
            ),
            (
                "PolicyPlatformContext_PrincipalContext_Id",
                record.authority.sid.as_str(),
            ),
        ] {
            context
                .SetValue(PCWSTR(wide(name).as_ptr()), 0, &string(value).0)
                .map_err(|_| Failure::NativeFailure)?;
        }
        let services = locator
            .ConnectServer(
                &BSTR::from("ROOT\\CIMV2\\MDM\\DMMAP"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                WBEM_FLAG_CONNECT_USE_MAX_WAIT.0,
                &BSTR::new(),
                Some(&context),
            )
            .map_err(|_| Failure::NativeFailure)?;
        CoSetProxyBlanket(
            &services,
            RPC_C_AUTHN_WINNT,
            RPC_C_AUTHZ_NONE,
            None,
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )
        .map_err(|_| Failure::NativeFailure)?;
        let parent = format!("./Vendor/MSFT/PassportForWork/{}", tenant(record)?);
        let path =
            format!("MDM_PassportForWork_Policies02.ParentID=\"{parent}\",InstanceID=\"Policies\"");
        let mut object = None;
        if let Err(e) = services.GetObject(
            &BSTR::from(path.as_str()),
            WBEM_GENERIC_FLAG_TYPE(0),
            Some(&context),
            Some(&mut object),
            None,
        ) {
            if e.code().0 as u32 != 0x80041002 {
                return Err(Failure::NativeFailure);
            }
            let mut class = None;
            services
                .GetObject(
                    &BSTR::from("MDM_PassportForWork_Policies02"),
                    WBEM_GENERIC_FLAG_TYPE(0),
                    Some(&context),
                    Some(&mut class),
                    None,
                )
                .map_err(|_| Failure::NativeFailure)?;
            object = Some(
                class
                    .ok_or(Failure::NativeFailure)?
                    .SpawnInstance(0)
                    .map_err(|_| Failure::NativeFailure)?,
            );
        }
        let object = object.ok_or(Failure::NativeFailure)?;
        for (name, value) in [
            ("ParentID", string(&parent)),
            ("InstanceID", string("Policies")),
            ("UsePassportForWork", boolean(record.enabled)),
            ("RequireSecurityDevice", boolean(true)),
        ] {
            object
                .Put(PCWSTR(wide(name).as_ptr()), 0, &value.0, 0)
                .map_err(|_| Failure::NativeFailure)?;
        }
        services
            .PutInstance(&object, WBEM_GENERIC_FLAG_TYPE(0), Some(&context), None)
            .map_err(|_| Failure::NativeFailure)?;
        let mut confirmed = record.clone();
        confirmed.policy = HelloPolicy::Owned {
            enabled: record.enabled,
        };
        if policy(&confirmed)?
            != if record.enabled {
                O::Present
            } else {
                O::Absent
            }
        {
            return Err(Failure::NativeFailure);
        }
        Ok(())
    }
}
