//! SCM and Task Scheduler retain native ownership; installer input never contains argv secrets.
//! ref: windows-sys 0.61.2 Win32/System/Services; windows 0.61.3 TaskScheduler.
use super::*;
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
pub(super) fn preflight_desktop_entry(_: &Candidate) -> Result<(), Error> {
    Ok(())
}
pub(super) fn desktop_entry(_: &Candidate) -> Result<(), Error> {
    Ok(())
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
        return Ok(());
    }
    let xml = format!(
        r#"<?xml version="1.0"?><Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><Triggers><LogonTrigger><UserId>{}</UserId></LogonTrigger></Triggers><Principals><Principal id="User"><UserId>{}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><ExecutionTimeLimit>PT0S</ExecutionTimeLimit></Settings><Actions Context="User"><Exec><Command>{}</Command><Arguments>{}</Arguments></Exec></Actions></Task>"#,
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
    unsafe { task.Run(&empty) }.map_err(|_| Error::Unavailable)?;
    Ok(())
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
        let args: Vec<String> = serde_json::from_slice(
            &installation_security::read_protected(path).map_err(|_| Error::Identity)?,
        )
        .map_err(|_| Error::Configuration)?;
        super::run(&args.into_iter().map(OsString::from).collect::<Vec<_>>())
    })();
    value.dwCurrentState = SERVICE_STOPPED;
    value.dwWin32ExitCode = if result.is_ok() {
        0
    } else {
        ERROR_SERVICE_SPECIFIC_ERROR
    };
    value.dwServiceSpecificExitCode = if result.is_ok() { 0 } else { 1 };
    unsafe {
        SetServiceStatus(status, &value);
    }
}
pub fn worker(name: String, path: PathBuf) -> Result<(), Error> {
    if execution_ipc::host::current_subject().map_err(crate::error::app_error)? != "S-1-5-18" {
        return Err(Error::Identity);
    }
    WORKER
        .set((name.clone(), path))
        .map_err(|_| Error::Conflict)?;
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
    let root = Deployment::default_path()?
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-operations");
    public_directory(&root)?;
    let id = uuid::Uuid::new_v4();
    let path = root.join(format!("{id}.json"));
    let input = args
        .iter()
        .map(|s| s.to_str().map(str::to_owned).ok_or(Error::Configuration))
        .collect::<Result<Vec<_>, _>>()?;
    write_new(&path, &json(&input)?)?;
    let name = format!("RssExecution-Install-{id}");
    let manager = manager()?;
    let image = wide(format!(
        "{} --installation-worker {} {}",
        quote(&binary)?,
        name,
        quote(&path)?
    ));
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
        return Err(Error::Unavailable);
    }
    let handle = Sc(raw);
    let result = (|| {
        if unsafe { StartServiceW(handle.0, 0, null()) } == 0 {
            return Err(Error::Unavailable);
        }
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            let status = state(&handle)?;
            if status.dwCurrentState == SERVICE_STOPPED {
                if status.dwWin32ExitCode != 0 {
                    return Err(Error::Unavailable);
                }
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Err(Error::Unavailable);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    })();
    if state(&handle)?.dwCurrentState == SERVICE_STOPPED {
        if unsafe { DeleteService(handle.0) } == 0 {
            return Err(Error::Unavailable);
        }
        std::fs::remove_file(path)?;
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

pub(super) fn uninstall_payload(_: &Candidate) -> Result<(), Error> {
    Err(Error::Unsupported)
}
