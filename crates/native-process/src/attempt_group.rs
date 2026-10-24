//! A dedicated group leader retains the PGID until cleanup; parent death closes its pipe.
//! ref: tokio tokio/src/process/unix/mod.rs@tokio-1.43.0 (owned child/pipe lifetimes).
use std::{
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    process::Command,
};

/// Live cooperative process-group owner, never reconstructed from persisted PIDs.
pub struct AttemptGroup {
    leader: i32,
    control: Option<OwnedFd>,
}
impl AttemptGroup {
    /// Fork a minimal watchdog. The child calls only async-signal-safe libc functions.
    pub fn new() -> io::Result<Self> {
        let mut fds = [0; 2];
        // SAFETY: writable two-element descriptor array.
        if unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: socketpair returned two uniquely owned descriptors.
        let parent = unsafe { OwnedFd::from_raw_fd(fds[0]) };
        let child = unsafe { OwnedFd::from_raw_fd(fds[1]) };
        for fd in &fds {
            if unsafe { libc::fcntl(*fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        let max_fd = unsafe { libc::getdtablesize() };
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(io::Error::last_os_error());
        }
        if pid == 0 {
            // No Rust allocation, unwinding, destructor, mutex or inherited runtime in the child.
            unsafe {
                // Never retain another attempt's owner pipe across fork.
                for fd in 0..max_fd {
                    if fd != fds[1] {
                        libc::close(fd);
                    }
                }
                libc::signal(libc::SIGTERM, libc::SIG_IGN);
                if libc::setpgid(0, 0) != 0 {
                    libc::_exit(125);
                }
                let ready: u8 = 1;
                if libc::write(fds[1], (&ready as *const u8).cast(), 1) != 1 {
                    libc::_exit(125);
                }
                let mut byte: u8 = 0;
                loop {
                    let count = libc::read(fds[1], (&mut byte as *mut u8).cast(), 1);
                    if count >= 0 {
                        break;
                    }
                    if *libc::__error() != libc::EINTR {
                        break;
                    }
                }
                libc::kill(-libc::getpid(), libc::SIGKILL);
                libc::_exit(126);
            }
        }
        drop(child);
        let mut guard = Self {
            leader: pid,
            control: Some(parent),
        };
        let mut poll = libc::pollfd {
            fd: guard.control.as_ref().unwrap().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let mut ready = 0u8;
        let ok = unsafe {
            libc::poll(&mut poll, 1, 2000) > 0
                && libc::read(poll.fd, (&mut ready as *mut u8).cast(), 1) == 1
                && ready == 1
        };
        if !ok {
            guard.terminate();
            return Err(io::Error::other("process owner unavailable"));
        }
        Ok(guard)
    }
    /// Assign a child before it executes any user code.
    pub fn configure(&self, command: &mut Command) {
        command.process_group(self.leader);
    }
    /// Stable group identity while this live owner is held.
    pub fn group(&self) -> u32 {
        self.leader as u32
    }
    /// Cooperative stop. Watchdog also receives TERM; EOF still closes the group on owner death.
    pub fn request_stop(&self) {
        unsafe {
            libc::kill(-self.leader, libc::SIGTERM);
        }
    }
    /// Closing the owner kills the group. Reap only the watchdog this instance created.
    pub fn terminate(&mut self) {
        if self.control.take().is_some() {
            unsafe {
                libc::kill(-self.leader, libc::SIGKILL);
            }
            let mut status = 0;
            loop {
                let rc = unsafe { libc::waitpid(self.leader, &mut status, 0) };
                if rc >= 0 || io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                    break;
                }
            }
        }
    }
}
impl Drop for AttemptGroup {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Owns a fixed tool's private controlling terminal group, including on worker death.
/// A terminal requires a new session, so the outer attempt group alone cannot contain it.
pub struct TerminalGroup {
    _control: OwnedFd,
}
impl TerminalGroup {
    /// Configure a command whose stdin is already a private PTY slave.
    /// The short-lived EOF watchdog uses the same lifetime rule as `AttemptGroup`.
    pub fn configure(command: &mut Command) -> io::Result<Self> {
        let mut fds = [0; 2];
        if unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let parent = unsafe { OwnedFd::from_raw_fd(fds[0]) };
        let child = unsafe { OwnedFd::from_raw_fd(fds[1]) };
        for fd in fds {
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        let max_fd = unsafe { libc::getdtablesize() };
        // SAFETY: the post-fork path invokes only async-signal-safe libc operations.
        unsafe {
            command.pre_exec(move || {
                libc::close(fds[0]);
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as libc::c_ulong, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                let leader = libc::getpid();
                // Inherit ignore before fork: an immediately exiting session leader cannot
                // deliver SIGHUP before the watchdog has installed its own disposition.
                let old_hup = libc::signal(libc::SIGHUP, libc::SIG_IGN);
                if old_hup == libc::SIG_ERR {
                    return Err(io::Error::last_os_error());
                }
                let watchdog = libc::fork();
                if watchdog < 0 {
                    return Err(io::Error::last_os_error());
                }
                if watchdog == 0 {
                    let control = child.as_raw_fd();
                    for fd in 0..max_fd {
                        if fd != control {
                            libc::close(fd);
                        }
                    }
                    libc::signal(libc::SIGHUP, libc::SIG_IGN);
                    libc::signal(libc::SIGTERM, libc::SIG_IGN);
                    let mut byte = 0u8;
                    loop {
                        if libc::read(control, (&mut byte as *mut u8).cast(), 1) >= 0
                            || *libc::__error() != libc::EINTR
                        {
                            break;
                        }
                    }
                    // The watchdog still occupies this group, preventing PGID reuse.
                    libc::kill(-leader, libc::SIGKILL);
                    libc::_exit(126);
                }
                libc::signal(libc::SIGHUP, old_hup);
                Ok(())
            });
        }
        Ok(Self { _control: parent })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::File,
        io::{BufRead, BufReader},
        process::Stdio,
        time::{Duration, Instant},
    };
    #[test]
    fn terminal_owner_eof_stops_the_detached_tool() {
        let (mut master, mut slave) = (-1, -1);
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let _master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "test -r /dev/tty || exit 4; printf 'ready\\n'; read value </dev/tty",
            ])
            .stdin(Stdio::from(slave))
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let owner = TerminalGroup::configure(&mut command).unwrap();
        let mut child = command.spawn().unwrap();
        let mut line = String::new();
        let stdout = child.stdout.take().unwrap();
        let mut ready = libc::pollfd {
            fd: stdout.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert!(unsafe { libc::poll(&mut ready, 1, 3000) } > 0);
        BufReader::new(stdout).read_line(&mut line).unwrap();
        assert_eq!(line, "ready\n");
        drop(owner); // Identical EOF when a killed worker loses its only owner descriptor.
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("terminal tool escaped owner");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
