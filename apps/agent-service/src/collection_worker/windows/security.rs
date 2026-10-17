//! Fixed local security reads; never configuration changes or application execution.
//! ref: microsoft/windows-rs 0.61.3 WindowsFirewall, Services, WinTrust.
use super::super::system_values::enabled;
use super::*;
use ::windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        NetworkManagement::WindowsFirewall::*,
        Security::{Cryptography::*, WinTrust::*},
        System::{Com::*, Services::*},
    },
};
struct Service(SC_HANDLE);
impl Drop for Service {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}
fn service(name: &str) -> Result<(bool, String), Status> {
    unsafe {
        let manager = Service(
            OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT).map_err(error)?,
        );
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let handle = Service(
            OpenServiceW(
                manager.0,
                PCWSTR(name.as_ptr()),
                SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG,
            )
            .map_err(|e| {
                if e.code().0 as u32 == 0x80070424 {
                    Status::Missing
                } else {
                    error(e)
                }
            })?,
        );
        let mut status = SERVICE_STATUS_PROCESS::default();
        let mut size = 0;
        let bytes = std::slice::from_raw_parts_mut(
            (&mut status as *mut SERVICE_STATUS_PROCESS).cast::<u8>(),
            std::mem::size_of::<SERVICE_STATUS_PROCESS>(),
        );
        QueryServiceStatusEx(handle.0, SC_STATUS_PROCESS_INFO, Some(bytes), &mut size)
            .map_err(error)?;
        let mut required = 0;
        let _ = QueryServiceConfigW(handle.0, None, 0, &mut required);
        if required == 0 || required > 65536 {
            return Err(Status::Invalid);
        }
        let mut storage = vec![0usize; (required as usize).div_ceil(std::mem::size_of::<usize>())];
        let config = storage.as_mut_ptr().cast::<QUERY_SERVICE_CONFIGW>();
        QueryServiceConfigW(handle.0, Some(config), required, &mut required).map_err(error)?;
        Ok((
            status.dwCurrentState == SERVICE_RUNNING,
            accounts::text((*config).lpBinaryPathName.0)?,
        ))
    }
}
fn signer(path: &std::path::Path) -> Result<(String, bool), Status> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut file = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(wide.as_ptr()),
            ..Default::default()
        };
        let mut data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            dwStateAction: WTD_STATEACTION_VERIFY,
            dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
            ..Default::default()
        };
        data.Anonymous.pFile = &mut file;
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let valid = WinVerifyTrust(
            HWND::default(),
            &mut action,
            (&mut data as *mut WINTRUST_DATA).cast(),
        ) == 0;
        let observed = (|| {
            let provider = WTHelperProvDataFromStateData(data.hWVTStateData);
            if provider.is_null() {
                return Err(Status::Missing);
            }
            let signer = WTHelperGetProvSignerFromChain(provider, 0, false, 0);
            if signer.is_null() {
                return Err(Status::Missing);
            }
            let cert = WTHelperGetProvCertFromChain(signer, 0);
            if cert.is_null() || (*cert).pCert.is_null() {
                return Err(Status::Missing);
            }
            let count = CertGetNameStringW((*cert).pCert, CERT_NAME_RDN_TYPE, 0, None, None);
            if count < 2 || count > 4096 {
                return Err(Status::Invalid);
            }
            let mut text = vec![0u16; count as usize];
            if CertGetNameStringW((*cert).pCert, CERT_NAME_RDN_TYPE, 0, None, Some(&mut text))
                != count
                || text.pop() != Some(0)
            {
                return Err(Status::Invalid);
            }
            String::from_utf16(&text).map_err(|_| Status::Invalid)
        })();
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        WinVerifyTrust(
            HWND::default(),
            &mut action,
            (&mut data as *mut WINTRUST_DATA).cast(),
        );
        observed.map(|name| (name, valid))
    }
}
fn application(
    target: &w::WindowsApplicationTarget,
    rows: &mut Rows<'_>,
    budget: &Budget,
) -> Result<(), Status> {
    let (identity, expected, path, running) = match target {
        w::WindowsApplicationTarget::Agent => (
            "rss-mdm-agent".to_owned(),
            None,
            std::env::current_exe().map_err(|_| Status::Failed)?,
            true,
        ),
        w::WindowsApplicationTarget::Signed {
            service: name,
            publisher,
        } => {
            let (running, command) = match service(name) {
                Ok(value) => value,
                Err(Status::Missing) => return Err(Status::Missing),
                Err(e) => return Err(e),
            };
            let path =
                super::super::system_values::service_image(&command).ok_or(Status::Partial)?;
            let path = std::path::PathBuf::from(path);
            if !path.is_absolute() || path.as_os_str().to_string_lossy().contains('%') {
                return Err(Status::Partial);
            }
            (name.clone(), Some(publisher.as_str()), path, running)
        }
    };
    budget.check()?;
    let (publisher, valid) = match signer(&path) {
        Ok(v) => v,
        Err(_) if expected.is_some() => return Err(Status::Partial),
        Err(_) => (String::new(), false),
    };
    if expected.is_some_and(|e| e != publisher) {
        return Err(Status::Invalid);
    }
    rows.push(Row::from([
        ("identity".into(), identity.into()),
        ("signer".into(), publisher.into()),
        ("presence".into(), "present".into()),
        (
            "signature".into(),
            if valid { "valid" } else { "invalid" }.into(),
        ),
        ("running".into(), enabled(Some(running)).into()),
        ("health".into(), "unknown".into()),
    ]))
}
pub(super) fn read(
    query: &w::NativeSystemQuery,
    rows: &mut Rows<'_>,
    budget: &Budget,
) -> Result<(), Status> {
    match query {
        w::NativeSystemQuery::WindowsBitlocker => wmi::bitlocker(budget, rows),
        w::NativeSystemQuery::WindowsFirewall => {
            let _apartment = wmi::Apartment::open()?;
            unsafe {
                let policy: INetFwPolicy2 =
                    CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER).map_err(error)?;
                let active = policy.CurrentProfileTypes().ok();
                let running = service("MpsSvc").ok().map(|v| v.0);
                for (id, profile) in [
                    ("domain", NET_FW_PROFILE2_DOMAIN),
                    ("private", NET_FW_PROFILE2_PRIVATE),
                    ("public", NET_FW_PROFILE2_PUBLIC),
                ] {
                    budget.check()?;
                    let configured = policy
                        .get_FirewallEnabled(profile)
                        .ok()
                        .map(|v| v.as_bool());
                    let effective = configured.zip(running).zip(active).map(
                        |((configured, running), active)| {
                            configured && running && active & profile.0 != 0
                        },
                    );
                    rows.push(Row::from([
                        ("id".into(), id.into()),
                        ("configured".into(), enabled(configured).into()),
                        ("running".into(), enabled(effective).into()),
                    ]))?;
                }
            }
            Ok(())
        }
        w::NativeSystemQuery::WindowsDefender => {
            use w::CimType::*;
            let facts = standard::query(
                "root\\Microsoft\\Windows\\Defender",
                "MSFT_MpComputerStatus",
                &[
                    ("AMServiceEnabled", Boolean),
                    ("AntivirusEnabled", Boolean),
                    ("RealTimeProtectionEnabled", Boolean),
                    ("AntivirusSignatureVersion", String),
                    ("DefenderSignaturesOutOfDate", Boolean),
                    ("AMRunningMode", String),
                ],
                budget,
            )?;
            if facts.len() != 1 {
                return Err(Status::Missing);
            }
            let fact = &facts[0];
            let preferences = standard::query(
                "root\\Microsoft\\Windows\\Defender",
                "MSFT_MpPreference",
                &[("DisableRealtimeMonitoring", Boolean)],
                budget,
            )
            .ok();
            let configured = preferences
                .as_ref()
                .filter(|v| v.len() == 1)
                .and_then(|v| v[0].get("DisableRealtimeMonitoring"))
                .and_then(|v| v.as_bool())
                .map(|v| !v);
            let realtime = fact
                .get("RealTimeProtectionEnabled")
                .and_then(|v| v.as_bool());
            let mode = match fact.get("AMRunningMode").and_then(|v| v.as_str()) {
                Some("Normal") => "normal",
                Some("Passive Mode" | "SxS Passive Mode" | "EDR Block Mode") => "passive",
                _ => "unknown",
            };
            let health = if mode == "passive" {
                "passive"
            } else {
                match (
                    realtime,
                    fact.get("AntivirusEnabled").and_then(|v| v.as_bool()),
                    fact.get("DefenderSignaturesOutOfDate")
                        .and_then(|v| v.as_bool()),
                ) {
                    (Some(true), Some(true), Some(false)) if mode == "normal" => "healthy",
                    (Some(false), _, _) | (_, Some(false), _) | (_, _, Some(true)) => "unhealthy",
                    _ => "unknown",
                }
            };
            let mut row = Row::from([
                ("presence".into(), "present".into()),
                ("configured".into(), enabled(configured).into()),
                ("running".into(), enabled(realtime).into()),
                ("mode".into(), mode.into()),
                ("health".into(), health.into()),
            ]);
            if let Some(version) = fact.get("AntivirusSignatureVersion") {
                row.insert("signature_version".into(), version.clone());
            }
            rows.push(row)
        }
        w::NativeSystemQuery::WindowsSecureBoot => {
            let key = registry::machine(
                "SYSTEM\\CurrentControlSet\\Control\\SecureBoot\\State",
                w::RegistryView::Registry64,
            )?;
            let state = registry::value(&key, "UEFISecureBootEnabled", w::RegistryType::Dword)?;
            rows.push(Row::from([(
                "state".into(),
                match state.as_u64() {
                    Some(0) => "disabled",
                    Some(1) => "enabled",
                    _ => "unknown",
                }
                .into(),
            )]))
        }
        w::NativeSystemQuery::WindowsApplication { target } => application(target, rows, budget),
        _ => Err(Status::Invalid),
    }
}
