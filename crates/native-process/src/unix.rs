use super::*;
use std::os::unix::process::CommandExt;
pub fn absent(scope: &Scope) -> bool {
    let Scope::ProcessGroup { root } = scope else {
        return false;
    };
    if *root <= 1 || *root > i32::MAX as u32 {
        return false;
    }
    // SAFETY: signal zero only probes the process group; it never signals it.
    unsafe {
        libc::kill(-(*root as i32), 0) != 0
            && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }
}
pub struct Owner {
    child: Child,
    parent: i32,
}
impl Owner {
    pub fn spawn(command: &mut Command, _: &str) -> io::Result<Self> {
        // SAFETY: launcher is a dedicated single-threaded process before spawn.
        unsafe {
            if libc::setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        let child = command.spawn()?;
        Ok(Self {
            child,
            parent: unsafe { libc::getppid() },
        })
    }
    pub fn child(&mut self) -> &mut Child {
        &mut self.child
    }
    pub fn scope(&self) -> Scope {
        Scope::ProcessGroup {
            root: std::process::id(),
        }
    }
    pub fn parent_gone(&self) -> bool {
        unsafe { libc::getppid() != self.parent }
    }
    pub fn terminate(&mut self) -> io::Result<()> {
        // This launcher is the still-live group root; signaling includes itself.
        unsafe {
            libc::kill(-(std::process::id() as i32), libc::SIGKILL);
        }
        Err(io::Error::last_os_error())
    }
}
pub struct HostScope {
    root: Option<u32>,
}
impl HostScope {
    pub fn prepare(command: &mut Command) -> io::Result<Self> {
        command.process_group(0);
        Ok(Self { root: None })
    }
    pub fn attach(&mut self, pid: u32) -> io::Result<()> {
        self.root = Some(pid);
        Ok(())
    }
    pub fn request_stop(&self) -> io::Result<()> {
        if let Some(root) = self.root {
            if unsafe { libc::kill(-(root as i32), libc::SIGTERM) } != 0
                && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
    pub fn terminate(&self) -> io::Result<()> {
        if let Some(root) = self.root {
            // Scope is held only while the caller still owns the unreaped Child.
            if unsafe { libc::kill(-(root as i32), libc::SIGKILL) } != 0
                && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
    pub fn empty(&self) -> bool {
        self.root
            .is_none_or(|root| absent(&Scope::ProcessGroup { root }))
    }
}
