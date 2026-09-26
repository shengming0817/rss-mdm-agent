//! Atomic kernel Job membership and explicitly inherited pipes; no PID/thread enumeration.
//! ref: Microsoft UpdateProcThreadAttribute PROC_THREAD_ATTRIBUTE_JOB_LIST / HANDLE_LIST.
use super::*;
use std::{
    io,
    os::windows::{io::IntoRawHandle, process::ExitStatusExt},
    process::ExitStatus,
    time::{Duration, Instant},
};
use tokio::net::windows::named_pipe::NamedPipeServer;
use windows_sys::Win32::{
    Storage::FileSystem::*,
    System::{JobObjects::*, Pipes::*},
};

pub(crate) struct Owner {
    job: OwnedHandle,
    name: String,
    session: u32,
}
impl Owner {
    pub(crate) fn prepare(_: &mut tokio::process::Command) -> Result<Self, Error> {
        let (subject, session) = token_identity()?;
        let name = format!("Local\\rss-mdm-attempt-{}", nonce()?);
        let descriptor = security(&format!("D:P(A;;GA;;;{subject})(A;;GA;;;SY)(A;;GA;;;BA)"))?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let handle = unsafe { CreateJobObjectW(&attributes, wide(&name).as_ptr()) };
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let job = own(handle)?;
        if existed {
            return Err(Error::Conflict);
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                raw(&job),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(Error::Unavailable);
        }
        Ok(Self { job, name, session })
    }
    pub(crate) fn scope(&self) -> ProcessScope {
        ProcessScope::JobObject {
            name: self.name.clone(),
            session: self.session,
        }
    }
    pub(crate) fn stop(&self) {
        unsafe {
            TerminateJobObject(raw(&self.job), 1);
        }
    }
    pub(crate) fn terminate(&mut self) {
        self.stop();
        let until = Instant::now() + Duration::from_millis(500);
        while !self.quiescent() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub(crate) fn quiescent(&self) -> bool {
        let mut value: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
        unsafe {
            QueryInformationJobObject(
                raw(&self.job),
                JobObjectBasicAccountingInformation,
                (&mut value as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of_val(&value) as u32,
                null_mut(),
            ) != 0
                && value.ActiveProcesses == 0
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.stop();
    }
}
struct Attributes {
    buffer: Vec<usize>,
    initialized: bool,
}
impl Attributes {
    fn new() -> io::Result<Self> {
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut result = Self {
            buffer: vec![0; bytes.div_ceil(size_of::<usize>())],
            initialized: false,
        };
        if unsafe { InitializeProcThreadAttributeList(result.pointer(), 2, 0, &mut bytes) } == 0 {
            return Err(io::Error::last_os_error());
        }
        result.initialized = true;
        Ok(result)
    }
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_mut_ptr().cast()
    }
    fn set(&mut self, key: usize, value: &[HANDLE]) -> io::Result<()> {
        if unsafe {
            UpdateProcThreadAttribute(
                self.pointer(),
                0,
                key,
                value.as_ptr().cast(),
                size_of_val(value),
                null_mut(),
                null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            if self.initialized {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}
async fn pipe(parent_writes: bool) -> io::Result<(NamedPipeServer, OwnedHandle)> {
    let (subject, _) = token_identity().map_err(io::Error::other)?;
    let descriptor =
        security(&format!("D:P(A;;GA;;;{subject})(A;;GA;;;SY)")).map_err(io::Error::other)?;
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let name = wide(format!(
        "\\\\.\\pipe\\rss-attempt-{}",
        nonce().map_err(io::Error::other)?
    ));
    let direction = if parent_writes {
        PIPE_ACCESS_OUTBOUND
    } else {
        PIPE_ACCESS_INBOUND
    };
    let server = own(unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            direction | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            16384,
            16384,
            1000,
            &attributes,
        )
    })
    .map_err(io::Error::other)?;
    let server = unsafe { NamedPipeServer::from_raw_handle(server.into_raw_handle()) }?;
    attributes.bInheritHandle = 1;
    let client = own(unsafe {
        CreateFileW(
            name.as_ptr(),
            if parent_writes {
                GENERIC_READ
            } else {
                GENERIC_WRITE
            },
            0,
            &attributes,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    })
    .map_err(io::Error::other)?;
    tokio::time::timeout(Duration::from_secs(1), server.connect())
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))??;
    Ok((server, client))
}
pub(crate) struct Child {
    process: OwnedHandle,
    pub stdin: Option<NamedPipeServer>,
    pub stdout: Option<NamedPipeServer>,
    pub stderr: Option<NamedPipeServer>,
}
impl Child {
    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match unsafe { WaitForSingleObject(raw(&self.process), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(raw(&self.process), &mut code) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(Some(ExitStatus::from_raw(code)))
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
    pub(crate) fn start_kill(&mut self) -> io::Result<()> {
        if self.try_wait()?.is_none() && unsafe { TerminateProcess(raw(&self.process), 1) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    pub(crate) async fn wait(&mut self) -> io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            tokio::time::sleep(Duration::from_millis(5)).await
        }
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.start_kill();
    }
}
pub(crate) async fn spawn(
    command: &mut tokio::process::Command,
    owner: &mut Owner,
    cancel: &std::sync::atomic::AtomicBool,
    deadline: Instant,
) -> io::Result<Child> {
    let (stdinput, childin) = pipe(true).await?;
    let (stdout, childout) = pipe(false).await?;
    let (stderr, childerr) = pipe(false).await?;
    let command = command.as_std();
    super::files::executable(std::path::Path::new(command.get_program()))
        .map_err(io::Error::other)?;
    let application = wide(command.get_program());
    let program = command
        .get_program()
        .to_str()
        .ok_or(io::ErrorKind::InvalidInput)?;
    let mut line = crate::windows_argv::quote(program);
    for argument in command.get_args() {
        line.push(' ');
        line.push_str(&crate::windows_argv::quote(
            argument.to_str().ok_or(io::ErrorKind::InvalidInput)?,
        ));
    }
    let mut line = wide(line);
    if line.len() > 32767 || line[..line.len() - 1].contains(&0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut environment = Vec::new();
    for (key, value) in command.get_envs() {
        let Some(value) = value else { continue };
        let mut entry: Vec<u16> = key.encode_wide().collect();
        entry.push('=' as u16);
        entry.extend(value.encode_wide());
        if entry.contains(&0) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        environment.extend(entry);
        environment.push(0u16);
    }
    environment.push(0);
    if environment.len() == 1 {
        environment.push(0)
    }
    let cwd = wide(
        command
            .get_current_dir()
            .ok_or(io::ErrorKind::InvalidInput)?,
    );
    let inherited = [raw(&childin), raw(&childout), raw(&childerr)];
    let jobs = [raw(&owner.job)];
    let mut attributes = Attributes::new()?;
    attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize, &inherited)?;
    attributes.set(PROC_THREAD_ATTRIBUTE_JOB_LIST as usize, &jobs)?;
    let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = inherited[0];
    startup.StartupInfo.hStdOutput = inherited[1];
    startup.StartupInfo.hStdError = inherited[2];
    startup.lpAttributeList = attributes.pointer();
    let mut information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    if cancel.load(std::sync::atomic::Ordering::Acquire) || Instant::now() >= deadline {
        return Err(io::ErrorKind::TimedOut.into());
    }
    // JOB_LIST assigns the exact job before any target thread can run. No attach/reopen fallback.
    if unsafe {
        CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            null(),
            null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT
                | CREATE_UNICODE_ENVIRONMENT
                | CREATE_NO_WINDOW
                | CREATE_SUSPENDED,
            environment.as_ptr().cast(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut information,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let process = unsafe { OwnedHandle::from_raw_handle(information.hProcess) };
    let thread = unsafe { OwnedHandle::from_raw_handle(information.hThread) };
    let mut member = 0;
    if unsafe { IsProcessInJob(raw(&process), raw(&owner.job), &mut member) } == 0
        || member == 0
        || cancel.load(std::sync::atomic::Ordering::Acquire)
        || Instant::now() >= deadline
        || unsafe { ResumeThread(raw(&thread)) } == u32::MAX
    {
        unsafe {
            TerminateProcess(raw(&process), 1);
        }
        owner.terminate();
        return Err(io::Error::other("suspended process job/resume failed"));
    }
    drop(thread);
    drop((childin, childout, childerr));
    Ok(Child {
        process,
        stdin: Some(stdinput),
        stdout: Some(stdout),
        stderr: Some(stderr),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    #[test]
    #[ignore = "subprocess fixture invoked by job_owns_descendants_and_cleans_up"]
    #[expect(
        clippy::zombie_processes,
        reason = "fixture intentionally exits before its Windows descendant; the owning job must reap it"
    )]
    fn child_fixture() {
        use std::os::windows::process::CommandExt;
        if std::env::var_os("RSS_JOB_LEAF").is_some() {
            std::thread::sleep(Duration::from_secs(30));
            return;
        }
        assert!(std::env::var_os("RSS_PARENT_SECRET").is_none());
        let exe = std::env::current_exe().unwrap();
        let args = [
            "--exact",
            "windows::process::tests::child_fixture",
            "--ignored",
            "--nocapture",
        ];
        let escaped = std::process::Command::new(&exe)
            .args(args)
            .env("RSS_JOB_LEAF", "1")
            .creation_flags(CREATE_BREAKAWAY_FROM_JOB)
            .spawn();
        assert!(escaped.is_err(), "Job must reject breakaway");
        let child = std::process::Command::new(exe)
            .args(args)
            .env("RSS_JOB_LEAF", "1")
            .spawn()
            .unwrap();
        println!("descendant={}", child.id());
        // Child handle closes; the kernel job retains the live descendant.
    }
    #[tokio::test]
    async fn job_owns_descendants_and_cleans_up() {
        let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "windows::process::tests::child_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env_clear()
            .current_dir(std::env::current_dir().unwrap());
        let mut owner = Owner::prepare(&mut command).unwrap();
        let mut child = spawn(
            &mut command,
            &mut owner,
            &std::sync::atomic::AtomicBool::new(false),
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
        drop(child.stdin.take());
        let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success());
        assert!(!owner.quiescent(), "root exit is not whole-job exit");
        owner.terminate();
        assert!(owner.quiescent());
        let mut bytes = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(2),
            child.stdout.take().unwrap().read_to_end(&mut bytes),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("descendant="));
    }
    #[tokio::test]
    async fn expired_or_cancelled_allowance_never_starts_a_target() {
        for cancelled in [false, true] {
            let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
            command
                .env_clear()
                .current_dir(std::env::current_dir().unwrap());
            let mut owner = Owner::prepare(&mut command).unwrap();
            let deadline = if cancelled {
                Instant::now() + Duration::from_secs(5)
            } else {
                Instant::now()
            };
            assert!(spawn(
                &mut command,
                &mut owner,
                &std::sync::atomic::AtomicBool::new(cancelled),
                deadline
            )
            .await
            .is_err());
            assert!(owner.quiescent());
        }
    }
    #[tokio::test]
    async fn dropping_owner_kills_the_process_without_pid_reopening() {
        let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "windows::process::tests::child_fixture",
                "--ignored",
            ])
            .env_clear()
            .env("RSS_JOB_LEAF", "1")
            .current_dir(std::env::current_dir().unwrap());
        let mut owner = Owner::prepare(&mut command).unwrap();
        let mut child = spawn(
            &mut command,
            &mut owner,
            &std::sync::atomic::AtomicBool::new(false),
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(child.try_wait().unwrap().is_none());
        drop(owner);
        assert!(!tokio::time::timeout(Duration::from_secs(2), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success());
    }
}

#[cfg(test)]
mod crash_tests {
    use super::*;
    #[test]
    #[ignore = "subprocess-only crash owner"]
    fn crash_owner_fixture() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "windows::process::tests::child_fixture",
                    "--ignored",
                ])
                .env_clear()
                .env("RSS_JOB_LEAF", "1")
                .current_dir(std::env::current_dir().unwrap());
            let mut owner = Owner::prepare(&mut command).unwrap();
            let child = spawn(
                &mut command,
                &mut owner,
                &std::sync::atomic::AtomicBool::new(false),
                Instant::now() + Duration::from_secs(5),
            )
            .await
            .unwrap();
            let pid = unsafe { GetProcessId(raw(&child.process)) };
            std::fs::write(
                std::env::var_os("RSS_JOB_PID_FILE").unwrap(),
                pid.to_string(),
            )
            .unwrap();
            std::thread::sleep(Duration::from_secs(30));
            // The parent must kill this fixture without running Rust destructors.
        });
    }
    #[test]
    fn kernel_closes_job_when_owner_process_is_killed() {
        let path = std::env::temp_dir().join(format!("rss-job-{}.txt", nonce().unwrap()));
        let mut owner = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows::process::crash_tests::crash_owner_fixture",
                "--ignored",
            ])
            .env("RSS_JOB_PID_FILE", &path)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        let pid = loop {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(pid) = text.parse::<u32>() {
                    break pid;
                }
            }
            if Instant::now() >= until {
                let _ = owner.kill();
                let _ = owner.wait();
                panic!("owner did not publish child")
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let process = own(unsafe { OpenProcess(SYNCHRONIZE, 0, pid) }).unwrap();
        owner.kill().unwrap();
        owner.wait().unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(raw(&process), 3000) },
            WAIT_OBJECT_0
        );
        std::fs::remove_file(path).unwrap();
    }
}
