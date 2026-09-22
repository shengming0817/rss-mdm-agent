use super::*;
use std::{
    ffi::OsStr,
    os::windows::{ffi::OsStrExt, process::CommandExt},
    ptr::{null, null_mut},
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::{
    Foundation::*,
    System::{
        Diagnostics::ToolHelp::*, JobObjects::*, SystemServices::JOB_OBJECT_QUERY, Threading::*,
    },
};
fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain(Some(0)).collect()
}
struct Handle(HANDLE);
unsafe impl Send for Handle {}
// Kernel job/process query and termination operations support shared live handles.
unsafe impl Sync for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Job {
    handle: Handle,
    name: Option<String>,
}
impl Job {
    fn new(name: Option<String>) -> io::Result<Self> {
        let text = name.as_deref().map(wide);
        let handle =
            unsafe { CreateJobObjectW(null(), text.as_ref().map_or(null(), |s| s.as_ptr())) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let handle = Handle(handle);
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            return Err(io::Error::other("job already exists"));
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                handle.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle, name })
    }
    fn attach(&self, pid: u32) -> io::Result<()> {
        let process = Handle(unsafe {
            OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        });
        if process.0.is_null() {
            return Err(io::Error::last_os_error());
        }
        if unsafe { AssignProcessToJobObject(self.handle.0, process.0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // Only one initial thread exists: the child was created suspended and
        // has never executed user code before the job assignment.
        let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) });
        if snapshot.0 == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of_val(&entry) as u32;
        let mut found = false;
        let mut present = unsafe { Thread32First(snapshot.0, &mut entry) };
        while present != 0 {
            if entry.th32OwnerProcessID == pid {
                let thread =
                    Handle(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) });
                if thread.0.is_null() || unsafe { ResumeThread(thread.0) } == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                found = true;
            }
            present = unsafe { Thread32Next(snapshot.0, &mut entry) };
        }
        if !found {
            return Err(io::Error::other("missing initial thread"));
        }
        Ok(())
    }
    fn terminate(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.handle.0, 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    fn empty(&self) -> bool {
        let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
        unsafe {
            QueryInformationJobObject(
                self.handle.0,
                JobObjectBasicAccountingInformation,
                (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of_val(&info) as u32,
                null_mut(),
            ) != 0
                && info.ActiveProcesses == 0
        }
    }
}
pub fn absent(scope: &Scope) -> bool {
    let Scope::JobObject { name, session } = scope else {
        return false;
    };
    if current_session().ok().as_ref() != Some(session) {
        return false;
    }
    let Some(id) = name.strip_prefix("Local\\rss-mdm-worker-") else {
        return false;
    };
    if id.len() != 36 || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return false;
    }
    let handle = unsafe { OpenJobObjectW(JOB_OBJECT_QUERY, 0, wide(name).as_ptr()) };
    if !handle.is_null() {
        drop(Handle(handle));
        return false;
    }
    // A job is destroyed only after all associated processes AND handles are
    // gone. Access denied is not absence. This never obtains termination rights.
    unsafe { GetLastError() == ERROR_FILE_NOT_FOUND }
}
fn parent() -> io::Result<Handle> {
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) });
    if snapshot.0 == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of_val(&entry) as u32;
    let mut present = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while present != 0 {
        if entry.th32ProcessID == std::process::id() {
            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, entry.th32ParentProcessID) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            return Ok(Handle(handle));
        }
        present = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    Err(io::Error::other("missing parent"))
}
pub struct Owner {
    session: u32,
    child: Child,
    parent: Handle,
    job: Job,
}
impl Owner {
    pub fn spawn(command: &mut Command, id: &str) -> io::Result<Self> {
        let parent = parent()?;
        let session = current_session()?;
        let job = Job::new(Some(format!("Local\\rss-mdm-worker-{id}")))?;
        command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
        let mut child = command.spawn()?;
        if let Err(error) = job.attach(child.id()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok(Self {
            child,
            parent,
            job,
            session,
        })
    }
    pub fn child(&mut self) -> &mut Child {
        &mut self.child
    }
    pub fn scope(&self) -> Scope {
        Scope::JobObject {
            name: self.job.name.clone().unwrap(),
            session: self.session,
        }
    }
    pub fn parent_gone(&self) -> bool {
        unsafe { WaitForSingleObject(self.parent.0, 0) != WAIT_TIMEOUT }
    }
    pub fn terminate(&mut self) -> io::Result<()> {
        self.job.terminate()?;
        self.child.wait()?;
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while !self.job.empty() {
            if std::time::Instant::now() >= until {
                return Err(io::Error::other("job did not drain"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}
pub struct HostScope {
    job: Job,
}
impl HostScope {
    pub fn prepare(command: &mut Command) -> io::Result<Self> {
        command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
        Ok(Self {
            job: Job::new(None)?,
        })
    }
    pub fn attach(&mut self, pid: u32) -> io::Result<()> {
        self.job.attach(pid)
    }
    pub fn request_stop(&self) -> io::Result<()> {
        Ok(())
    }
    pub fn terminate(&self) -> io::Result<()> {
        self.job.terminate()
    }
    pub fn empty(&self) -> bool {
        self.job.empty()
    }
}

fn current_session() -> io::Result<u32> {
    let mut session = 0;
    if unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(session)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn another_session_is_unknown_even_if_its_local_job_name_is_absent_here() {
        let session = current_session().unwrap();
        assert!(!absent(&Scope::JobObject {
            name: "Local\\rss-mdm-worker-00000000-0000-0000-0000-000000000000".into(),
            session: session.wrapping_add(1)
        }));
    }
}
