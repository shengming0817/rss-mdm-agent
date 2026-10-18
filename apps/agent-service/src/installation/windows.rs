//! SCM and Task Scheduler retain native ownership; installer input never contains argv secrets.
//! ref: windows-sys 0.61.2 Win32/System/Services; windows 0.61.3 TaskScheduler.
use super::worker_evidence::WorkerEvidence;
use super::*;
use agent_client::protocol::*;
use std::{
    os::windows::ffi::{OsStrExt, OsStringExt},
    ptr::{null, null_mut},
    sync::OnceLock,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, WinTrust::*, *},
    Storage::FileSystem::*,
    System::{Diagnostics::ToolHelp::*, Services::*, Threading::*},
};
const NAME: &str = "RssExecution";
fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
unsafe fn text(value: *const u16) -> Result<String, Error> {
    if value.is_null() {
        return Err(Error::Identity);
    }
    let mut len = 0;
    while len < 32768 && unsafe { *value.add(len) } != 0 {
        len += 1;
    }
    if len == 32768 {
        return Err(Error::Identity);
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(value, len) })
        .map_err(|_| Error::Identity)
}
struct Sc(SC_HANDLE);
impl Drop for Sc {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn manager() -> Result<Sc, Error> {
    let handle = unsafe {
        OpenSCManagerW(
            null(),
            null(),
            SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE,
        )
    };
    if handle.is_null() {
        return Err(Error::Identity);
    }
    Ok(Sc(handle))
}
fn service(name: &str) -> Result<Option<Sc>, Error> {
    let manager = manager()?;
    let handle = unsafe {
        OpenServiceW(
            manager.0,
            wide(name).as_ptr(),
            SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS | SERVICE_START | SERVICE_STOP | DELETE,
        )
    };
    if handle.is_null() {
        if unsafe { GetLastError() } == ERROR_SERVICE_DOES_NOT_EXIST {
            return Ok(None);
        }
        return Err(Error::Identity);
    }
    Ok(Some(Sc(handle)))
}
fn state(handle: &Sc) -> Result<SERVICE_STATUS_PROCESS, Error> {
    let mut status = SERVICE_STATUS_PROCESS::default();
    let mut needed = 0;
    if unsafe {
        QueryServiceStatusEx(
            handle.0,
            SC_STATUS_PROCESS_INFO,
            (&mut status as *mut SERVICE_STATUS_PROCESS).cast(),
            std::mem::size_of_val(&status) as u32,
            &mut needed,
        )
    } == 0
    {
        return Err(Error::Unavailable);
    }
    Ok(status)
}
fn quote(path: &Path) -> Result<String, Error> {
    let text = path.to_str().ok_or(Error::Configuration)?;
    if !path.is_absolute() || text.contains(['"', '\0']) {
        return Err(Error::Configuration);
    }
    Ok(format!("\"{text}\""))
}
fn program(binary: &Path, config: &Path) -> Result<String, Error> {
    Ok(format!("{} --config {}", quote(binary)?, quote(config)?))
}
fn owned(handle: &Sc, binary: &Path, config: &Path) -> Result<(), Error> {
    let mut buffer = vec![0usize; 8192];
    let mut needed = 0;
    if unsafe {
        QueryServiceConfigW(
            handle.0,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * std::mem::size_of::<usize>()) as u32,
            &mut needed,
        )
    } == 0
    {
        return Err(Error::Unavailable);
    }
    let value = unsafe { &*(buffer.as_ptr() as *const QUERY_SERVICE_CONFIGW) };
    if unsafe { text(value.lpBinaryPathName) }? != program(binary, config)?
        || unsafe { text(value.lpServiceStartName) }? != "LocalSystem"
    {
        return Err(Error::Identity);
    }
    Ok(())
}
fn wait_stopped(handle: &Sc) -> Result<(), Error> {
    let before = state(handle)?;
    let process = if before.dwProcessId != 0 {
        let h = unsafe { OpenProcess(SYNCHRONIZE, 0, before.dwProcessId) };
        if h.is_null() {
            return Err(Error::Unavailable);
        }
        Some(Handle(h))
    } else {
        None
    };
    if before.dwCurrentState != SERVICE_STOPPED {
        let mut receipt = SERVICE_STATUS::default();
        if unsafe { ControlService(handle.0, SERVICE_CONTROL_STOP, &mut receipt) } == 0
            && unsafe { GetLastError() } != ERROR_SERVICE_NOT_ACTIVE
        {
            return Err(Error::Unavailable);
        }
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if state(handle)?.dwCurrentState == SERVICE_STOPPED
            && process
                .as_ref()
                .is_none_or(|h| unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0)
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub(super) fn register(system: bool, binary: &Path, config: &Path) -> Result<(), Error> {
    if !system {
        return task_register(binary, config);
    }
    let deployment = Deployment::load(config)?;
    if deployment.service.path != binary {
        return Err(Error::Identity);
    }
    register_verified(binary, config)
}
/// Compensation only: the retained executable validates its own persistent format.
pub(super) fn restore_registration(binary: &Path, config: &Path) -> Result<(), Error> {
    let deployment = Deployment::load_persistent(config)?;
    if deployment.service.path != binary {
        return Err(Error::Identity);
    }
    super::command(
        binary,
        &[
            "--config".into(),
            config.into(),
            "--validate-persistent".into(),
        ],
    )?;
    register_verified(binary, config)
}
fn register_verified(binary: &Path, config: &Path) -> Result<(), Error> {
    let handle = if let Some(handle) = service(NAME)? {
        owned(&handle, binary, config)?;
        handle
    } else {
        let manager = manager()?;
        let image = wide(program(binary, config)?);
        let handle = unsafe {
            CreateServiceW(
                manager.0,
                wide(NAME).as_ptr(),
                wide("RSS MDM Agent").as_ptr(),
                SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS | SERVICE_START | SERVICE_STOP | DELETE,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_AUTO_START,
                SERVICE_ERROR_NORMAL,
                image.as_ptr(),
                null(),
                null_mut(),
                null(),
                null(),
                null(),
            )
        };
        if handle.is_null() {
            return Err(Error::Unavailable);
        }
        Sc(handle)
    };
    if state(&handle)?.dwCurrentState != SERVICE_RUNNING
        && unsafe { StartServiceW(handle.0, 0, null()) } == 0
    {
        return Err(Error::Unavailable);
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let value = state(&handle)?;
        if value.dwCurrentState == SERVICE_RUNNING && value.dwProcessId != 0 {
            return Ok(());
        }
        if value.dwCurrentState == SERVICE_STOPPED || Instant::now() >= deadline {
            return Err(Error::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub(super) fn stop(system: bool, binary: &Path, config: &Path) -> Result<(), Error> {
    if !system {
        return task_stop(binary, config, false);
    }
    if let Some(handle) = service(NAME)? {
        owned(&handle, binary, config)?;
        wait_stopped(&handle)?;
    }
    Ok(())
}
pub(super) fn remove(system: bool, binary: &Path, config: &Path) -> Result<(), Error> {
    if !system {
        return task_stop(binary, config, true);
    }
    if let Some(handle) = service(NAME)? {
        owned(&handle, binary, config)?;
        wait_stopped(&handle)?;
        if unsafe { DeleteService(handle.0) } == 0 {
            return Err(Error::Unavailable);
        }
    }
    Ok(())
}
pub(super) fn status(system: bool) -> Result<(), Error> {
    if !system {
        return Err(Error::Unsupported);
    }
    println!(
        "{}",
        if service(NAME)?.is_some() {
            "registered"
        } else {
            "absent"
        }
    );
    Ok(())
}
pub(super) fn public_directory(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() {
        return Err(Error::Configuration);
    }
    if !path.exists() {
        public_directory(path.parent().ok_or(Error::Configuration)?)?;
        let mut descriptor = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide("O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU)").as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(Error::Identity);
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let created = unsafe { CreateDirectoryW(wide(path).as_ptr(), &attributes) };
        unsafe {
            LocalFree(descriptor);
        }
        if created == 0 {
            return Err(Error::Unavailable);
        }
    }
    installation_security::protected(path).map_err(|_| Error::Identity)
}
pub(super) fn replace(staged: &Path, target: &Path) -> Result<(), Error> {
    if unsafe {
        MoveFileExW(
            wide(staged).as_ptr(),
            wide(target).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}
pub(super) fn interactive_profile() -> Result<Profile, Error> {
    use windows_sys::Win32::System::Registry::*;
    let (subject, _) =
        execution_ipc::host::active_user_session().map_err(crate::error::app_error)?;
    let key = wide(format!(
        "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\ProfileList\\{subject}"
    ));
    let mut buffer = vec![0u16; 32768];
    let mut bytes = (buffer.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            wide("ProfileImagePath").as_ptr(),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status != 0 || bytes < 2 || bytes as usize > buffer.len() * 2 {
        return Err(Error::Identity);
    }
    let home =
        String::from_utf16(&buffer[..bytes as usize / 2 - 1]).map_err(|_| Error::Identity)?;
    if home.contains('%') || !Path::new(&home).is_absolute() {
        return Err(Error::Configuration);
    }
    Ok(Profile {
        subjects: vec![subject.clone()],
        helper_work_roots: BTreeMap::from([(
            subject,
            PathBuf::from(home).join("AppData\\Local\\RSS MDM Agent\\helper"),
        )]),
    })
}
pub(super) fn verify_signatures(candidate: &Candidate) -> Result<(), Error> {
    if candidate.signing_mode == SigningMode::Development {
        return Ok(());
    }
    for image in [&candidate.service, &candidate.desktop] {
        let name = wide(&image.path);
        let mut file = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: name.as_ptr(),
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
        let status = unsafe {
            WinVerifyTrust(
                null_mut(),
                &mut action,
                (&mut data as *mut WINTRUST_DATA).cast(),
            )
        };
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        unsafe {
            WinVerifyTrust(
                null_mut(),
                &mut action,
                (&mut data as *mut WINTRUST_DATA).cast(),
            );
        }
        if status != 0 {
            return Err(Error::Identity);
        }
    }
    Ok(())
}
pub(super) fn execution_resources(
    interpreters: &mut Vec<crate::Interpreter>,
    managers: &mut Vec<crate::SoftwareManager>,
) -> Result<(), Error> {
    let system = std::env::var_os("SystemRoot").ok_or(Error::Configuration)?;
    let path = PathBuf::from(system).join("System32\\msiexec.exe");
    installation_security::protected(&path).map_err(|_| Error::Identity)?;
    let powershell = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .ok_or(Error::Configuration)?
        .join("PowerShell\\7\\pwsh.exe");
    if powershell.exists()
        && Target::current()
            .map_err(|_| Error::Unsupported)?
            .verify_image(&powershell)
            .is_ok()
    {
        installation_security::protected(&powershell).map_err(|_| Error::Identity)?;
        interpreters.push(crate::Interpreter {
            profile: rss_mdm_agent_wire::runtime::ExecutorProfile::PowerShell7,
            image: Artifact {
                sha256: hash(&std::fs::read(&powershell)?),
                path: powershell,
                cdhash: None,
            },
        });
    }
    managers.push(crate::SoftwareManager {
        executor: crate::SoftwareManagerKind::Msi,
        image: Artifact {
            sha256: hash(&std::fs::read(&path)?),
            path,
            cdhash: None,
        },
    });
    Ok(())
}
const UNINSTALL_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\RSS MDM Agent";
fn known_folder(id: &::windows::core::GUID) -> Result<PathBuf, Error> {
    use ::windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT},
    };
    let value = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }
        .map_err(|_| Error::Unavailable)?;
    let result = unsafe { text(value.0) }.map(PathBuf::from);
    unsafe {
        CoTaskMemFree(Some(value.0.cast()));
    }
    result
}
pub(super) fn payload_parent() -> Result<PathBuf, Error> {
    Ok(
        known_folder(&::windows::Win32::UI::Shell::FOLDERID_ProgramFiles)?
            .join("RSS MDM Agent\\versions"),
    )
}
fn shortcut() -> Result<PathBuf, Error> {
    Ok(
        known_folder(&::windows::Win32::UI::Shell::FOLDERID_CommonPrograms)?
            .join("RSS MDM Agent.lnk"),
    )
}
fn shell_link() -> Result<::windows::Win32::UI::Shell::IShellLinkW, Error> {
    use ::windows::Win32::{System::Com::*, UI::Shell::*};
    unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .map_err(|_| Error::Unavailable)
}
fn registry_value(name: &str) -> Result<Option<String>, Error> {
    use windows_sys::Win32::System::Registry::*;
    let mut buffer = vec![0u16; 32768];
    let mut length = (buffer.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(UNINSTALL_KEY).as_ptr(),
            wide(name).as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut length,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != 0 || length < 2 || length as usize > buffer.len() * 2 {
        return Err(Error::Identity);
    }
    Ok(Some(
        String::from_utf16(&buffer[..length as usize / 2 - 1]).map_err(|_| Error::Identity)?,
    ))
}
fn retained_image(path: &Path, desktop: bool) -> Result<(), Error> {
    let directory = Deployment::default_path()?
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-manifests");
    if directory.exists() {
        for entry in std::fs::read_dir(directory)?.take(128) {
            let entry = entry?;
            let bytes = installation_security::read_protected(&entry.path())
                .map_err(|_| Error::Identity)?;
            if entry.file_name() != OsString::from(format!("{}.json", hash(&bytes))) {
                return Err(Error::Identity);
            }
            let candidate: Candidate =
                serde_json::from_slice(&bytes).map_err(|_| Error::Identity)?;
            if candidate.root.parent() == Some(payload_parent()?.as_path())
                && path
                    == if desktop {
                        &candidate.desktop.path
                    } else {
                        &candidate.service.path
                    }
            {
                return Ok(());
            }
        }
    }
    Err(Error::Identity)
}
pub(super) fn preflight_desktop_entry(candidate: &Candidate) -> Result<(), Error> {
    use ::windows::{
        core::{Interface, PCWSTR},
        Win32::System::Com::{IPersistFile, STGM_READ},
    };
    if candidate.format()? == "windows_msix" {
        return if super::msix::root()? == candidate.root {
            Ok(())
        } else {
            Err(Error::Identity)
        };
    }
    if candidate.root.parent() != Some(payload_parent()?.as_path()) {
        return Err(Error::Identity);
    }
    if let Some(image) = registry_value("ServiceImage")? {
        retained_image(Path::new(&image), false)?;
    } else if registry_value("DisplayName")?.is_some() {
        return Err(Error::Identity);
    }
    let path = shortcut()?;
    if path.exists() {
        installation_security::protected(&path).map_err(|_| Error::Identity)?;
        let link = shell_link()?;
        let persist: IPersistFile = link.cast().map_err(|_| Error::Unavailable)?;
        let mut buffer = vec![0u16; 32768];
        unsafe {
            persist
                .Load(PCWSTR(wide(&path).as_ptr()), STGM_READ)
                .map_err(|_| Error::Unavailable)?;
            link.GetPath(&mut buffer, null_mut(), 0)
                .map_err(|_| Error::Unavailable)?;
        }
        retained_image(Path::new(&unsafe { text(buffer.as_ptr()) }?), true)?;
    }
    Ok(())
}
pub(super) fn desktop_entry(candidate: &Candidate) -> Result<(), Error> {
    use windows_sys::Win32::System::Registry::*;
    preflight_desktop_entry(candidate)?;
    if matches!(candidate.format()?.as_str(), "windows_msix" | "windows_msi") {
        // The package manager owns its shortcut lifecycle, including uninstall and rollback.
        return Ok(());
    }
    let mut key = null_mut();
    if unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            wide(UNINSTALL_KEY).as_ptr(),
            0,
            null(),
            0,
            KEY_SET_VALUE | KEY_WOW64_64KEY,
            null(),
            &mut key,
            null_mut(),
        )
    } != 0
    {
        return Err(Error::Unavailable);
    }
    let result = (|| {
        let uninstaller = candidate.root.join("uninstall.exe");
        for (name, value) in [
            (
                "ServiceImage",
                candidate.service.path.to_string_lossy().into_owned(),
            ),
            ("DisplayName", "RSS MDM Agent".into()),
            ("DisplayVersion", candidate.product_version.clone()),
            ("UninstallString", quote(&uninstaller)?),
            (
                "QuietUninstallString",
                format!("{} /S", quote(&uninstaller)?),
            ),
        ] {
            let bytes = wide(value);
            if unsafe {
                RegSetValueExW(
                    key,
                    wide(name).as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr().cast(),
                    (bytes.len() * 2) as u32,
                )
            } != 0
            {
                return Err(Error::Unavailable);
            }
        }
        desktop_shortcut(candidate)
    })();
    unsafe {
        RegCloseKey(key);
    }
    result
}
fn desktop_shortcut(candidate: &Candidate) -> Result<(), Error> {
    use ::windows::{
        core::{Interface, PCWSTR},
        Win32::System::Com::IPersistFile,
    };
    let path = shortcut()?;
    public_directory(path.parent().ok_or(Error::Configuration)?)?;
    let staged = path.with_extension(format!("pending-{}", uuid::Uuid::new_v4()));
    let link = shell_link()?;
    unsafe { link.SetPath(PCWSTR(wide(&candidate.desktop.path).as_ptr())) }
        .map_err(|_| Error::Unavailable)?;
    let persist: IPersistFile = link.cast().map_err(|_| Error::Unavailable)?;
    unsafe { persist.Save(PCWSTR(wide(&staged).as_ptr()), true) }
        .map_err(|_| Error::Unavailable)?;
    replace(&staged, &path)
}
pub(super) fn quiescent(old: &Deployment, config: &Path) -> Result<(), Error> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(Error::Unavailable);
    }
    let snapshot = Handle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while found != 0 {
        let handle =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID) };
        if !handle.is_null() {
            let handle = Handle(handle);
            let mut bytes = vec![0u16; 32768];
            let mut length = bytes.len() as u32;
            if unsafe { QueryFullProcessImageNameW(handle.0, 0, bytes.as_mut_ptr(), &mut length) }
                != 0
            {
                let path = PathBuf::from(OsString::from_wide(&bytes[..length as usize]));
                if old
                    .clients
                    .images
                    .iter()
                    .any(|i| i.path != old.service.path && i.path == path)
                {
                    return Err(Error::Conflict);
                }
            }
        }
        found = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    for subject in old.helper_work_roots.keys() {
        task_remove_subject(subject, &old.service.path, config)?;
    }
    Ok(())
}

fn task_folder() -> Result<::windows::Win32::System::TaskScheduler::ITaskFolder, Error> {
    use ::windows::Win32::System::{Com::*, TaskScheduler::*, Variant::VARIANT};
    unsafe {
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| Error::Unavailable)?;
        let empty = VARIANT::default();
        service
            .Connect(&empty, &empty, &empty, &empty)
            .map_err(|_| Error::Unavailable)?;
        service
            .GetFolder(&"\\".into())
            .map_err(|_| Error::Unavailable)
    }
}
fn task_get(
    subject: &str,
) -> Result<Option<::windows::Win32::System::TaskScheduler::IRegisteredTask>, Error> {
    let result = unsafe { task_folder()?.GetTask(&format!("RssExecution-{subject}").into()) };
    match result {
        Ok(task) => Ok(Some(task)),
        Err(e) if e.code().0 as u32 == 0x80070002 => Ok(None),
        Err(_) => Err(Error::Unavailable),
    }
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn task_owned(
    task: &::windows::Win32::System::TaskScheduler::IRegisteredTask,
    subject: &str,
    binary: &Path,
    config: &Path,
) -> Result<(), Error> {
    let xml = unsafe { task.Xml() }
        .map_err(|_| Error::Unavailable)?
        .to_string();
    super::task_document::owned(
        &xml,
        subject,
        binary.to_str().ok_or(Error::Configuration)?,
        &format!("--config {} --user-helper", quote(config)?),
    )
}
fn task_register(binary: &Path, config: &Path) -> Result<(), Error> {
    use ::windows::Win32::System::{TaskScheduler::*, Variant::VARIANT};
    let subject = execution_ipc::host::current_subject().map_err(crate::error::app_error)?;
    let deployment = Deployment::load(config)?;
    if !deployment.helper_work_roots.contains_key(&subject) {
        return Err(Error::Identity);
    }
    if let Some(task) = task_get(&subject)? {
        task_owned(&task, &subject, binary, config)?;
        return task_start(&task);
    }
    let xml = format!(
        r#"<?xml version="1.0"?><Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><Triggers><LogonTrigger><UserId>{}</UserId></LogonTrigger></Triggers><Principals><Principal id="User"><UserId>{}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Settings><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><Enabled>true</Enabled><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><ExecutionTimeLimit>PT0S</ExecutionTimeLimit></Settings><Actions Context="User"><Exec><Command>{}</Command><Arguments>{}</Arguments></Exec></Actions></Task>"#,
        escape(&subject),
        escape(&subject),
        escape(binary.to_str().ok_or(Error::Configuration)?),
        escape(&format!("--config {} --user-helper", quote(config)?))
    );
    let empty = VARIANT::default();
    let task = unsafe {
        task_folder()?.RegisterTask(
            &format!("RssExecution-{subject}").into(),
            &xml.into(),
            TASK_CREATE.0,
            &VARIANT::from(subject.as_str()),
            &empty,
            TASK_LOGON_INTERACTIVE_TOKEN,
            &empty,
        )
    }
    .map_err(|_| Error::Unavailable)?;
    task_start(&task)
}
fn task_start(
    task: &::windows::Win32::System::TaskScheduler::IRegisteredTask,
) -> Result<(), Error> {
    use ::windows::Win32::System::{TaskScheduler::TASK_STATE_RUNNING, Variant::VARIANT};
    if unsafe { task.State() }.map_err(|_| Error::Unavailable)? != TASK_STATE_RUNNING {
        unsafe { task.Run(&VARIANT::default()) }.map_err(|_| Error::Unavailable)?;
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if unsafe { task.State() }.map_err(|_| Error::Unavailable)? == TASK_STATE_RUNNING {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn task_remove_subject(subject: &str, binary: &Path, config: &Path) -> Result<(), Error> {
    use ::windows::Win32::System::TaskScheduler::TASK_STATE_RUNNING;
    if let Some(task) = task_get(subject)? {
        task_owned(&task, subject, binary, config)?;
        unsafe { task.Stop(0) }.map_err(|_| Error::Unavailable)?;
        let deadline = Instant::now() + Duration::from_secs(20);
        while unsafe { task.State() }.map_err(|_| Error::Unavailable)? == TASK_STATE_RUNNING {
            if Instant::now() >= deadline {
                return Err(Error::Unavailable);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        unsafe { task_folder()?.DeleteTask(&format!("RssExecution-{subject}").into(), 0) }
            .map_err(|_| Error::Unavailable)?;
    }
    Ok(())
}
fn task_stop(binary: &Path, config: &Path, remove: bool) -> Result<(), Error> {
    let subject = execution_ipc::host::current_subject().map_err(crate::error::app_error)?;
    if remove {
        return task_remove_subject(&subject, binary, config);
    }
    if let Some(task) = task_get(&subject)? {
        task_owned(&task, &subject, binary, config)?;
        unsafe { task.Stop(0) }.map_err(|_| Error::Unavailable)?;
    }
    Ok(())
}

// A temporary SCM process inherits LocalSystem rather than treating an elevated user as SYSTEM.
static WORKER: OnceLock<(String, PathBuf)> = OnceLock::new();
pub(super) fn record(value: &outcome::Report) -> Result<(), Error> {
    if let Some((_, path)) = WORKER.get() {
        publish(&path.with_extension("result.json"), &json(value)?)?;
    }
    Ok(())
}
fn operation_root() -> Result<PathBuf, Error> {
    Ok(Deployment::default_path()?
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-operations"))
}
fn read_result(path: &Path) -> Result<Option<outcome::Report>, Error> {
    let result = path.with_extension("result.json");
    if !result.exists() {
        return Ok(None);
    }
    let bytes = installation_security::read_protected(&result).map_err(|_| Error::Identity)?;
    if bytes.len() > 4096 {
        return Err(Error::Capacity);
    }
    let report: outcome::Report =
        serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
    if report.version != 1 {
        return Err(Error::Configuration);
    }
    Ok(Some(report))
}
fn worker_owned(handle: &Sc, name: &str, path: &Path) -> Result<(), Error> {
    let mut buffer = vec![0usize; 8192];
    let mut needed = 0;
    if unsafe {
        QueryServiceConfigW(
            handle.0,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * std::mem::size_of::<usize>()) as u32,
            &mut needed,
        )
    } == 0
    {
        return Err(Error::Unavailable);
    }
    let value = unsafe { &*(buffer.as_ptr() as *const QUERY_SERVICE_CONFIGW) };
    let image = unsafe { text(value.lpBinaryPathName) }?;
    let suffix = format!(" --installation-worker {name} {}", quote(path)?);
    if !image.ends_with(&suffix)
        || !image.starts_with('"')
        || unsafe { text(value.lpServiceStartName) }? != "LocalSystem"
    {
        return Err(Error::Identity);
    }
    // Requests/results are retained under the protected UUID operation root until native exit.
    installation_security::protected(path).map_err(|_| Error::Identity)
}
fn read_worker_evidence(path: &Path) -> Result<Option<WorkerEvidence>, Error> {
    let evidence = path.with_extension("worker.json");
    if !evidence.exists() {
        return Ok(None);
    }
    let bytes = installation_security::read_protected(&evidence).map_err(|_| Error::Identity)?;
    if bytes.len() > 512 {
        return Err(Error::Capacity);
    }
    let evidence: WorkerEvidence =
        serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
    if !evidence.valid() {
        return Err(Error::Configuration);
    }
    Ok(Some(evidence))
}
fn worker_exited(process_id: u32) -> Result<(), Error> {
    let process = unsafe { OpenProcess(SYNCHRONIZE, 0, process_id) };
    if process.is_null() {
        return if unsafe { GetLastError() } == ERROR_INVALID_PARAMETER {
            Ok(())
        } else {
            Err(Error::Unavailable)
        };
    }
    let process = Handle(process);
    if unsafe { WaitForSingleObject(process.0, 20_000) } != WAIT_OBJECT_0 {
        return Err(Error::Unavailable);
    }
    Ok(())
}
fn reclaim_worker(handle: &Sc, path: &Path) -> Result<bool, Error> {
    if state(handle)?.dwCurrentState != SERVICE_STOPPED {
        return Ok(false);
    }
    let Some(evidence) = read_worker_evidence(path)? else {
        return Ok(false);
    };
    // Never read STOPPED's invalid SCM PID, and never require a business terminal
    // receipt to prove native exit. Missing/crashed outcomes remain unknown.
    let exited = match evidence.process_id {
        Some(pid) => worker_exited(pid).is_ok(),
        None => false,
    };
    if !evidence.reclaimable(true, exited) {
        return Ok(false);
    }
    if unsafe { DeleteService(handle.0) } == 0 {
        return Err(Error::Unavailable);
    }
    std::fs::remove_file(path)?;
    // Keep both the native lifetime evidence and any business result for diagnosis.
    Ok(true)
}
pub(super) fn operation_status(id: uuid::Uuid) -> Result<(), Error> {
    let path = operation_root()?.join(format!("{id}.json"));
    let name = format!("RssExecution-Install-{id}");
    let report = read_result(&path)?;
    let evidence = read_worker_evidence(&path)?;
    let native = if let Some(handle) = service(&name)? {
        worker_owned(&handle, &name, &path)?;
        if reclaim_worker(&handle, &path)? {
            "stopped"
        } else if state(&handle)?.dwCurrentState == SERVICE_STOPPED {
            "exit-unconfirmed"
        } else {
            "active"
        }
    } else {
        "absent"
    };
    let result_state = report
        .as_ref()
        .filter(|r| r.terminal)
        .map(|r| r.state.as_str())
        .unwrap_or("outcome-unknown");
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({"operation":id,"worker":native,
            "resultState":result_state,"result":report,"native":evidence}))
        .map_err(|_| Error::Protocol)?
    );
    Ok(())
}
unsafe extern "system" fn control(
    _: u32,
    _: u32,
    _: *mut std::ffi::c_void,
    _: *mut std::ffi::c_void,
) -> u32 {
    0
}
unsafe extern "system" fn worker_main(_: u32, _: *mut *mut u16) {
    let Some((name, path)) = WORKER.get() else {
        return;
    };
    let status =
        unsafe { RegisterServiceCtrlHandlerExW(wide(name).as_ptr(), Some(control), null()) };
    if status.is_null() {
        return;
    }
    let mut value = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_RUNNING,
        ..Default::default()
    };
    unsafe {
        SetServiceStatus(status, &value);
    }
    let result = (|| {
        record(&outcome::snapshot())?;
        let args: Vec<String> = serde_json::from_slice(
            &installation_security::read_protected(path).map_err(|_| Error::Identity)?,
        )
        .map_err(|_| Error::Configuration)?;
        super::run(&args.into_iter().map(OsString::from).collect::<Vec<_>>())
    })();
    if let Err(error) = result {
        outcome::failed(error);
    } else {
        outcome::succeeded();
    }
    outcome::terminal();
    let recorded = record(&outcome::snapshot());
    value.dwCurrentState = SERVICE_STOPPED;
    value.dwWin32ExitCode = if result.is_ok() && recorded.is_ok() {
        0
    } else {
        ERROR_SERVICE_SPECIFIC_ERROR
    };
    value.dwServiceSpecificExitCode = match result.and(recorded) {
        Ok(()) => 0,
        Err(error) => error as u32 + 1,
    };
    unsafe {
        SetServiceStatus(status, &value);
    }
}
pub fn worker(name: String, path: PathBuf) -> Result<(), Error> {
    if execution_ipc::host::current_subject().map_err(crate::error::app_error)? != "S-1-5-18" {
        return Err(Error::Identity);
    }
    WORKER
        .set((name.clone(), path.clone()))
        .map_err(|_| Error::Conflict)?;
    // Publish actual process identity before entering the dispatcher or doing any
    // installation work. A crash/final-result write failure cannot erase this proof.
    write_new(
        &path.with_extension("worker.json"),
        &json(&WorkerEvidence::process(std::process::id()))?,
    )?;
    let mut name = wide(name);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_mut_ptr(),
            lpServiceProc: Some(worker_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: null_mut(),
            lpServiceProc: None,
        },
    ];
    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
        return Err(Error::Unavailable);
    }
    Ok(())
}
pub(super) fn dispatch(args: &[OsString]) -> Result<bool, Error> {
    if execution_ipc::host::current_subject().map_err(crate::error::app_error)? == "S-1-5-18" {
        return Ok(false);
    }
    let binary = std::env::current_exe()?;
    installation_security::protected(&binary).map_err(|_| Error::Identity)?;
    Target::current()
        .map_err(|_| Error::Unsupported)?
        .verify_image(&binary)
        .map_err(|_| Error::Identity)?;
    let root = operation_root()?;
    public_directory(&root)?;
    let id = uuid::Uuid::new_v4();
    let path = root.join(format!("{id}.json"));
    let input = args
        .iter()
        .map(|s| s.to_str().map(str::to_owned).ok_or(Error::Configuration))
        .collect::<Result<Vec<_>, _>>()?;
    let name = format!("RssExecution-Install-{id}");
    let manager = manager()?;
    let image = wide(format!(
        "{} --installation-worker {} {}",
        quote(&binary)?,
        name,
        quote(&path)?
    ));
    write_new(&path, &json(&input)?)?;
    eprintln!(
        "{}",
        serde_json::json!({"operation":id,"state":"submitted"})
    );
    let raw = unsafe {
        CreateServiceW(
            manager.0,
            wide(&name).as_ptr(),
            wide(&name).as_ptr(),
            SERVICE_QUERY_STATUS | SERVICE_START | SERVICE_STOP | DELETE,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_NORMAL,
            image.as_ptr(),
            null(),
            null_mut(),
            null(),
            null(),
            null(),
        )
    };
    if raw.is_null() {
        std::fs::remove_file(&path)?;
        return Err(Error::Unavailable);
    }
    let handle = Sc(raw);
    let result = (|| {
        if unsafe { StartServiceW(handle.0, 0, null()) } == 0 {
            let error = unsafe { GetLastError() };
            if let Some(evidence) = WorkerEvidence::not_started(error) {
                // Exclusive publication prevents overwriting actual worker PID proof.
                write_new(&path.with_extension("worker.json"), &json(&evidence)?)?;
            }
            eprintln!(
                "{}",
                serde_json::json!({"operation":id,"state":"outcome-unknown",
                "native_exit_code":error,"query":"--installation operation-status --operation <UUID>"})
            );
            return Err(Error::Unavailable);
        }
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            let status = state(&handle)?;
            if status.dwCurrentState == SERVICE_STOPPED {
                if let Some(report) = read_result(&path)? {
                    if report.terminal {
                        return if status.dwWin32ExitCode == 0 {
                            Ok(true)
                        } else {
                            Err(Error::Unavailable)
                        };
                    }
                }
                if reclaim_worker(&handle, &path)? {
                    return Err(Error::Unavailable);
                }
            }
            if Instant::now() >= deadline {
                eprintln!(
                    "{}",
                    serde_json::json!({"operation":id,"state":"outcome-unknown","worker":"may-still-be-active","query":"--installation operation-status --operation <UUID>"})
                );
                return Err(Error::Unavailable);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    })();
    let report = read_result(&path)?;
    // The loop may already have reclaimed a crashed worker. A remaining request
    // is reclaimed after confirmed native exit even without a terminal report.
    let reclaimed = !path.exists() || reclaim_worker(&handle, &path)?;
    eprintln!(
        "{}",
        serde_json::json!({"operation":id,
        "worker":if reclaimed { "stopped" } else { "exit-unconfirmed" },
        "resultState":report.as_ref().filter(|r| r.terminal).map(|r| r.state.as_str()).unwrap_or("outcome-unknown"),
        "result":report})
    );
    if !reclaimed {
        return Err(Error::Unavailable);
    }
    result
}

pub(super) fn read_secret(path: &Path) -> Result<Vec<u8>, Error> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    installation_security::protected(path).map_err(|_| Error::Identity)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut acl = null_mut();
    let mut descriptor = null_mut();
    if unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut acl,
            null_mut(),
            &mut descriptor,
        )
    } != 0
    {
        return Err(Error::Identity);
    }
    let result = (|| {
        if acl.is_null() {
            return Err(Error::Identity);
        }
        for index in 0..unsafe { (*acl).AceCount } as u32 {
            let mut ace = null_mut();
            if unsafe { GetAce(acl, index, &mut ace) } == 0 {
                return Err(Error::Identity);
            }
            let header = unsafe { &*(ace as *const ACE_HEADER) };
            if header.AceFlags & INHERIT_ONLY_ACE as u8 != 0 || header.AceType == 1 {
                continue;
            }
            if header.AceType != 0 {
                return Err(Error::Identity);
            }
            let allowed = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
            let mut sid = null_mut();
            if unsafe {
                ConvertSidToStringSidW(
                    (&allowed.SidStart as *const u32).cast_mut().cast(),
                    &mut sid,
                )
            } == 0
            {
                return Err(Error::Identity);
            }
            let value = unsafe { text(sid) };
            unsafe {
                LocalFree(sid.cast());
            }
            if allowed.Mask != 0 && !matches!(value?.as_str(), "S-1-5-18" | "S-1-5-32-544") {
                return Err(Error::Identity);
            }
        }
        if !file.metadata()?.is_file() || file.metadata()?.len() > 64 {
            return Err(Error::Capacity);
        }
        let mut bytes = Vec::new();
        file.take(65).read_to_end(&mut bytes)?;
        if bytes.len() > 64 {
            return Err(Error::Capacity);
        }
        Ok(bytes)
    })();
    unsafe {
        LocalFree(descriptor);
    }
    result
}

pub(super) struct ComApartment;
impl ComApartment {
    pub(super) fn new() -> Result<Self, Error> {
        unsafe {
            ::windows::Win32::System::Com::CoInitializeEx(
                None,
                ::windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
        }
        .ok()
        .map_err(|_| Error::Unavailable)?;
        Ok(Self)
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            ::windows::Win32::System::Com::CoUninitialize();
        }
    }
}

pub(super) fn uninstall_payload(candidate: &Candidate) -> Result<(), Error> {
    use windows_sys::Win32::System::Registry::*;
    preflight_desktop_entry(candidate)?;
    if let Some(image) = registry_value("ServiceImage")? {
        if Path::new(&image) != candidate.service.path {
            return Err(Error::Identity);
        }
    }
    retire_payload(candidate)?;
    let path = shortcut()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    let status = unsafe {
        RegDeleteKeyExW(
            HKEY_LOCAL_MACHINE,
            wide(UNINSTALL_KEY).as_ptr(),
            KEY_WOW64_64KEY,
            0,
        )
    };
    if status != 0 && status != ERROR_FILE_NOT_FOUND {
        return Err(Error::Unavailable);
    }
    Ok(())
}
