//! macOS query ownership: sandboxed no-fork query roots and broker-owned no-fork tool roots.
//! ref: rust-lang/rust@1.85.0 library/std/src/sys/pal/unix/process/process_unix.rs.
//! PGID emptiness and pipe EOF are deliberately not used as descendant closure evidence.
use super::*;
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
const CHANNEL_FD: i32 = 198;
const INPUT_FD: i32 = 199;
const REQUEST_CAP: usize = 16 * 1024;
const CLEANUP: Duration = Duration::from_millis(100);
const LEAF_PROFILE: &str =
    "(version 1)(allow default)(deny process-fork)(deny network*)(deny file-write*)";
static CHANNEL: OnceLock<Mutex<UnixStream>> = OnceLock::new();
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Request {
    BeginRead,
    EndRead,
    Budget,
    Observed(usize),
    Tool { name: String, args: Vec<String> },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reply {
    pub status: w::NativeQueryStatus,
    pub bytes: Vec<u8>,
    pub diagnostics: Vec<u8>,
    pub observed: usize,
}
#[link(name = "sandbox")]
unsafe extern "C" {
    fn sandbox_init(profile: *const libc::c_char, flags: u64, error: *mut *mut libc::c_char)
        -> i32;
    fn sandbox_free_error(error: *mut libc::c_char);
}
pub(super) fn confine_query() -> Result<(), Error> {
    let profile = std::ffi::CString::new(format!("{LEAF_PROFILE}(deny process-exec)"))
        .map_err(|_| Error::Configuration)?;
    let mut error = std::ptr::null_mut();
    // SAFETY: profile and error pointer remain valid for this synchronous native call.
    let rc = unsafe { sandbox_init(profile.as_ptr(), 0, &mut error) };
    if !error.is_null() {
        // SAFETY: the sandbox API allocated this error buffer.
        unsafe { sandbox_free_error(error) };
    }
    if rc != 0 {
        return Err(Error::Unavailable);
    }
    // SAFETY: the internal launch recipe exclusively transfers this inherited socket.
    let channel = unsafe { UnixStream::from_raw_fd(CHANNEL_FD) };
    CHANNEL
        .set(Mutex::new(channel))
        .map_err(|_| Error::Unavailable)
}
fn frame<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Protocol)?;
    let size = u32::try_from(bytes.len()).map_err(|_| Error::Capacity)?;
    let mut output = size.to_be_bytes().to_vec();
    output.extend(bytes);
    Ok(output)
}
pub(super) fn observe(n: usize) -> Result<(), Error> {
    acknowledge(Request::Observed(n))
}
pub(super) fn begin_read() -> Result<(), Error> {
    acknowledge(Request::BeginRead)
}
pub(super) fn end_read() -> Result<(), Error> {
    acknowledge(Request::EndRead)
}
pub(super) fn remaining_budget() -> Result<usize, Error> {
    let Some(channel) = CHANNEL.get() else {
        #[cfg(test)]
        return Ok(usize::MAX);
        #[cfg(not(test))]
        return Err(Error::Unavailable);
    };
    let mut channel = channel.lock().map_err(|_| Error::Unavailable)?;
    channel.write_all(&frame(&Request::Budget)?)?;
    let mut bytes = [0; 8];
    channel.read_exact(&mut bytes)?;
    usize::try_from(u64::from_be_bytes(bytes)).map_err(|_| Error::Capacity)
}
fn acknowledge(request: Request) -> Result<(), Error> {
    let Some(channel) = CHANNEL.get() else {
        #[cfg(test)]
        return Ok(());
        #[cfg(not(test))]
        return Err(Error::Unavailable);
    };
    let mut channel = channel.lock().map_err(|_| Error::Unavailable)?;
    channel.write_all(&frame(&request)?)?;
    let mut ack = [0];
    channel.read_exact(&mut ack)?;
    if ack != [0] {
        return Err(Error::Protocol);
    }
    Ok(())
}
pub(super) fn tool(name: &str, args: &[&str], cap: usize) -> Result<Reply, Error> {
    let mut channel = CHANNEL
        .get()
        .ok_or(Error::Unavailable)?
        .lock()
        .map_err(|_| Error::Unavailable)?;
    let request = frame(&Request::Tool {
        name: name.into(),
        args: args.iter().map(|s| (*s).into()).collect(),
    })?;
    if request.len() > REQUEST_CAP {
        return Err(Error::Capacity);
    }
    channel.write_all(&request)?;
    let mut header = [0; 4];
    channel.read_exact(&mut header)?;
    let len = u32::from_be_bytes(header) as usize;
    if len > cap.saturating_mul(4).saturating_add(1024) {
        return Err(Error::Capacity);
    }
    let mut bytes = vec![0; len];
    channel.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)
}
fn nonblocking(fd: i32) -> Result<(), Error> {
    // SAFETY: fcntl only changes flags on the borrowed live descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(Error::Unavailable);
    }
    Ok(())
}
struct OwnedChild {
    child: Child,
    until: Instant,
    reaped: bool,
}
impl OwnedChild {
    fn poll(&mut self) -> Result<Option<ExitStatus>, Error> {
        let status = self.child.try_wait()?;
        self.reaped |= status.is_some();
        Ok(status)
    }
    fn close(&mut self) -> Result<(), Error> {
        if !self.reaped {
            self.child.kill()?;
            while self.poll()?.is_none() {
                if Instant::now() >= self.until {
                    return Err(Error::Unavailable);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        Ok(())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Error paths never emit a completion receipt; still make a bounded cleanup attempt.
        let _ = self.close();
    }
}
fn drain(pipe: &mut impl Read, out: &mut Vec<u8>, cap: usize) -> Result<(usize, bool), Error> {
    let mut buffer = [0; 8192];
    let mut observed = 0;
    // Bound work per iteration even if a writer continuously floods the pipe.
    for _ in 0..8 {
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok((observed, true)),
            Ok(n) => {
                observed += n;
                out.extend_from_slice(&buffer[..n.min(cap.saturating_sub(out.len()))]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok((observed, false))
}
fn finish_pipe(
    pipe: &mut impl Read,
    out: &mut Vec<u8>,
    cap: usize,
    until: Instant,
) -> Result<usize, Error> {
    let mut total = 0usize;
    loop {
        let (n, eof) = drain(pipe, out, cap)?;
        total = total.saturating_add(n);
        if eof {
            return Ok(total);
        }
        if Instant::now() >= until {
            return Err(Error::Unavailable);
        }
    }
}
fn send(pipe: &mut impl Write, bytes: &[u8], until: Instant) -> Result<(), Error> {
    let mut offset = 0;
    while offset < bytes.len() {
        if Instant::now() >= until {
            return Err(Error::Unavailable);
        }
        match pipe.write(&bytes[offset..]) {
            Ok(0) => return Err(Error::Unavailable),
            Ok(n) => offset += n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1))
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn inherited_source(fd: i32) -> Result<File, Error> {
    // SAFETY: duplicate a live descriptor above both reserved destinations.
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, INPUT_FD + 1) };
    if copy < 0 {
        return Err(Error::Unavailable);
    }
    // SAFETY: the successful duplication transferred an exclusively owned descriptor.
    Ok(unsafe { File::from_raw_fd(copy) })
}
pub(super) fn query(
    material: &[u8],
    input: &Input,
    id: &str,
    q: &w::NativeQuery,
    remaining: Duration,
    raw_allowance: usize,
) -> Result<w::NativeQueryResult, Error> {
    let allowance = remaining
        .min(Duration::from_secs(q.budget.timeout_seconds.into()))
        .min(Duration::from_secs(
            u64::try_from(input.read.deadline.saturating_sub(now()?)).unwrap_or(0),
        ));
    if allowance <= CLEANUP {
        return Ok(failure(q, w::NativeQueryStatus::Timeout, now()?));
    }
    let until = Instant::now() + allowance;
    let work_until = until - CLEANUP;
    let image = std::env::current_exe()?;
    let _lease = execution_runner::staging::verify_retained(&image, &input.image_sha256)
        .map_err(crate::error::app_error)?;
    if Instant::now() >= work_until {
        return Ok(failure(q, w::NativeQueryStatus::Timeout, now()?));
    }
    let (channel, remote) = UnixStream::pair()?;
    channel.set_nonblocking(true)?;
    let mut fds = [-1; 2];
    // SAFETY: two writable descriptor slots; ownership moves immediately to Files.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(Error::Unavailable);
    }
    let read = unsafe { File::from_raw_fd(fds[0]) };
    let mut write = unsafe { File::from_raw_fd(fds[1]) };
    // SAFETY: both descriptors are live; close-on-exec prevents leaking the writer.
    for fd in fds {
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(Error::Unavailable);
        }
    }
    nonblocking(write.as_raw_fd())?;
    let mut command = Command::new(image);
    command
        .args(["--collection-query", "/dev/fd/199", id])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let remote_copy = inherited_source(remote.as_raw_fd())?;
    let read_copy = inherited_source(read.as_raw_fd())?;
    let remote_fd = remote_copy.as_raw_fd();
    let read_fd = read_copy.as_raw_fd();
    // SAFETY: only async-signal-safe descriptor operations run between fork and exec.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(remote_fd, CHANNEL_FD) < 0 || libc::dup2(read_fd, INPUT_FD) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut owner = OwnedChild {
        child: command.spawn()?,
        until,
        reaped: false,
    };
    drop(remote);
    drop(read);
    drop(remote_copy);
    drop(read_copy);
    let stdout = owner.child.stdout.take().ok_or(Error::Unavailable)?;
    let stderr = owner.child.stderr.take().ok_or(Error::Unavailable)?;
    nonblocking(stdout.as_raw_fd())?;
    nonblocking(stderr.as_raw_fd())?;
    if send(&mut write, material, work_until).is_err() {
        drop(write);
        owner.close()?;
        return Ok(failure(q, w::NativeQueryStatus::Timeout, now()?));
    }
    drop(write);
    supervise(
        owner,
        channel,
        stdout,
        stderr,
        material,
        input,
        q,
        work_until,
        until,
        raw_allowance,
    )
}
#[allow(clippy::too_many_arguments)]
fn supervise(
    mut owner: OwnedChild,
    mut channel: UnixStream,
    mut stdout: impl Read + AsRawFd,
    mut stderr: impl Read + AsRawFd,
    material: &[u8],
    input: &Input,
    q: &w::NativeQuery,
    work_until: Instant,
    until: Instant,
    raw_allowance: usize,
) -> Result<w::NativeQueryResult, Error> {
    let cap = q.budget.output_bytes as usize;
    let raw_cap = raw_allowance.min(cap);
    let mut inflight_read = false;
    let observed_before = input.observed.load(std::sync::atomic::Ordering::Relaxed);
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let mut requests = Vec::new();
    let mut output_seen = 0usize;
    let mut diagnostic_seen = 0usize;
    let mut stopped = None;
    let status = loop {
        output_seen = output_seen.saturating_add(drain(&mut stdout, &mut output, cap + 1)?.0);
        diagnostic_seen =
            diagnostic_seen.saturating_add(drain(&mut stderr, &mut diagnostics, 1024)?.0);
        let (received, _) = drain(&mut channel, &mut requests, REQUEST_CAP + 1)?;
        if received > REQUEST_CAP
            || requests.len() > REQUEST_CAP
            || output_seen > cap
            || diagnostic_seen > 1024
        {
            stopped = Some(w::NativeQueryStatus::Truncated);
            break None;
        }
        while requests.len() >= 4 {
            let len =
                u32::from_be_bytes(requests[..4].try_into().map_err(|_| Error::Protocol)?) as usize;
            if len > REQUEST_CAP - 4 {
                return Err(Error::Protocol);
            }
            if requests.len() < len + 4 {
                break;
            }
            let request: Request =
                serde_json::from_slice(&requests[4..4 + len]).map_err(|_| Error::Protocol)?;
            requests.drain(..4 + len);
            match request {
                Request::BeginRead | Request::EndRead => {
                    let begin = matches!(request, Request::BeginRead);
                    if begin == inflight_read {
                        return Err(Error::Protocol);
                    }
                    inflight_read = begin;
                    if send(&mut channel, &[0], work_until).is_err() {
                        stopped = Some(w::NativeQueryStatus::Timeout);
                        break;
                    }
                }
                Request::Budget => {
                    let remaining = raw_remaining(input, observed_before, raw_cap);
                    if send(&mut channel, &(remaining as u64).to_be_bytes(), work_until).is_err() {
                        stopped = Some(w::NativeQueryStatus::Timeout);
                        break;
                    }
                }
                Request::Observed(n) => {
                    if !inflight_read {
                        return Err(Error::Protocol);
                    }
                    input
                        .observed
                        .fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
                    if send(&mut channel, &[0], work_until).is_err() {
                        stopped = Some(w::NativeQueryStatus::Timeout);
                        break;
                    }
                }
                Request::Tool { name, args } => {
                    if inflight_read {
                        return Err(Error::Protocol);
                    }
                    let remaining = raw_remaining(input, observed_before, raw_cap);
                    let reply = run_tool(input, q, &name, &args, remaining, work_until, until)?;
                    if send(&mut channel, &frame(&reply)?, work_until).is_err() {
                        stopped = Some(w::NativeQueryStatus::Timeout);
                        break;
                    }
                }
            }
        }
        if input
            .observed
            .load(std::sync::atomic::Ordering::Relaxed)
            .saturating_sub(observed_before)
            > raw_cap as u64
        {
            stopped = Some(w::NativeQueryStatus::Truncated);
        }
        if stopped.is_some() {
            break None;
        }
        if let Some(status) = owner.poll()? {
            break Some(status);
        }
        if Instant::now() >= work_until {
            stopped = Some(w::NativeQueryStatus::Timeout);
            break None;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    owner.close()?;
    // No process can inherit these pipes: query fork/exec was denied before provider work.
    output_seen =
        output_seen.saturating_add(finish_pipe(&mut stdout, &mut output, cap + 1, until)?);
    diagnostic_seen =
        diagnostic_seen.saturating_add(finish_pipe(&mut stderr, &mut diagnostics, 1024, until)?);
    // A timeout can race a final file-read notification. Its acknowledged protocol
    // permits at most one outstanding request; drain and account it after reap.
    finish_pipe(&mut channel, &mut requests, REQUEST_CAP + 1, until)?;
    if !requests.is_empty() {
        if requests.len() < 4 {
            return Err(Error::Protocol);
        }
        let len =
            u32::from_be_bytes(requests[..4].try_into().map_err(|_| Error::Protocol)?) as usize;
        if len + 4 != requests.len() {
            return Err(Error::Protocol);
        }
        let request: Request =
            serde_json::from_slice(&requests[4..]).map_err(|_| Error::Protocol)?;
        match request {
            Request::Observed(n) => {
                if !inflight_read {
                    return Err(Error::Protocol);
                }
                input
                    .observed
                    .fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
            }
            Request::EndRead if inflight_read => {
                inflight_read = false;
            }
            // BeginRead was not acknowledged, so the child could not start reading.
            Request::BeginRead | Request::Budget | Request::Tool { .. } => {}
            Request::EndRead => return Err(Error::Protocol),
        }
    }
    if inflight_read {
        // Reap proves process closure, not that an interrupted read was fully counted.
        return Err(Error::Unavailable);
    }
    if output_seen > cap || diagnostic_seen > 1024 {
        stopped = Some(w::NativeQueryStatus::Truncated);
    }
    if let Some(status) = stopped {
        return Ok(failure(q, status, now()?));
    }
    if status.is_none_or(|s| !s.success()) {
        return Ok(failure(q, w::NativeQueryStatus::Invalid, now()?));
    }
    let receipt: CollectionCompletion =
        serde_json::from_slice(&diagnostics).map_err(|_| Error::Unavailable)?;
    if !receipt.closed
        || receipt.input_sha256 != hex(material)
        || receipt.output_sha256 != hex(&output)
        || receipt.native_output_bytes
            != input
                .observed
                .load(std::sync::atomic::Ordering::Relaxed)
                .saturating_sub(observed_before)
    {
        return Err(Error::Unavailable);
    }
    Ok(decode_query(q, &output, now()?))
}
fn raw_remaining(input: &Input, before: u64, cap: usize) -> usize {
    cap.saturating_sub(
        usize::try_from(
            input
                .observed
                .load(std::sync::atomic::Ordering::Relaxed)
                .saturating_sub(before),
        )
        .unwrap_or(usize::MAX),
    )
}
fn run_tool(
    input: &Input,
    query: &w::NativeQuery,
    name: &str,
    args: &[String],
    cap: usize,
    work_until: Instant,
    until: Instant,
) -> Result<Reply, Error> {
    if Instant::now() >= work_until {
        return Ok(Reply {
            status: w::NativeQueryStatus::Timeout,
            bytes: Vec::new(),
            diagnostics: Vec::new(),
            observed: 0,
        });
    }
    if cap == 0 {
        return Ok(Reply {
            status: w::NativeQueryStatus::Truncated,
            bytes: Vec::new(),
            diagnostics: Vec::new(),
            observed: 0,
        });
    }
    if !super::macos::tool::allowed(&query.definition, name, args, &input.read.execution) {
        return Err(Error::Untrusted);
    }
    let tool = input.tools.get(name).ok_or(Error::Unavailable)?;
    if Some(tool.path.as_str()) != super::macos::tool::path(name) {
        return Err(Error::Untrusted);
    }
    let sandbox = input.tools.get("sandbox-exec").ok_or(Error::Unavailable)?;
    if sandbox.path != "/usr/bin/sandbox-exec" {
        return Err(Error::Untrusted);
    }
    let _tool =
        execution_runner::staging::verify_retained(Path::new(&tool.path), &tool.artifact.sha256)
            .map_err(crate::error::app_error)?;
    let _sandbox = execution_runner::staging::verify_retained(
        Path::new(&sandbox.path),
        &sandbox.artifact.sha256,
    )
    .map_err(crate::error::app_error)?;
    if Instant::now() >= work_until {
        return Ok(Reply {
            status: w::NativeQueryStatus::Timeout,
            bytes: Vec::new(),
            diagnostics: Vec::new(),
            observed: 0,
        });
    }
    let mut command = Command::new(&sandbox.path);
    command.args(["-p", LEAF_PROFILE, &tool.path]);
    if name == "system_profiler" {
        // Its default dispatcher forks a reporter and silently omits _items when fork
        // is denied. Run that fixed reporter in the broker-owned leaf process instead.
        command.arg("-nospawn");
    }
    command
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    capture(command.spawn()?, cap, work_until, until, &input.observed)
}
fn read_native(
    pipe: &mut impl Read,
    out: &mut Vec<u8>,
    retain: bool,
    left: usize,
) -> Result<(usize, bool), Error> {
    if left == 0 {
        return Ok((0, false));
    }
    let mut buffer = [0; 8192];
    let limit = left.min(buffer.len());
    match pipe.read(&mut buffer[..limit]) {
        Ok(0) => Ok((0, true)),
        Ok(n) => {
            if retain {
                out.extend_from_slice(&buffer[..n]);
            }
            Ok((n, false))
        }
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok((0, false))
        }
        Err(e) => Err(e.into()),
    }
}
fn capture(
    child: Child,
    cap: usize,
    work_until: Instant,
    until: Instant,
    raw: &std::sync::atomic::AtomicU64,
) -> Result<Reply, Error> {
    let mut owner = OwnedChild {
        child,
        until,
        reaped: false,
    };
    let mut stdout = owner.child.stdout.take().ok_or(Error::Unavailable)?;
    let mut stderr = owner.child.stderr.take().ok_or(Error::Unavailable)?;
    nonblocking(stdout.as_raw_fd())?;
    nonblocking(stderr.as_raw_fd())?;
    let mut bytes = Vec::new();
    let mut discarded = Vec::new();
    let mut observed = 0usize;
    let mut status = w::NativeQueryStatus::Complete;
    loop {
        let (out, out_eof) =
            read_native(&mut stdout, &mut bytes, true, cap.saturating_sub(observed))?;
        observed += out;
        raw.fetch_add(out as u64, std::sync::atomic::Ordering::Relaxed);
        let (err, err_eof) = read_native(
            &mut stderr,
            &mut discarded,
            true,
            cap.saturating_sub(observed),
        )?;
        observed += err;
        raw.fetch_add(err as u64, std::sync::atomic::Ordering::Relaxed);
        // At the exact boundary no extra probe is allowed to spend the result reserve.
        if observed >= cap {
            status = w::NativeQueryStatus::Truncated;
            break;
        }
        if let Some(exit) = owner.poll()? {
            if !exit.success() {
                status = if std::str::from_utf8(&discarded).is_ok_and(|s| {
                    s.contains("Must be running as root")
                        || s.contains("Operation not permitted")
                        || s.contains("Permission denied")
                }) {
                    w::NativeQueryStatus::Denied
                } else {
                    w::NativeQueryStatus::Invalid
                };
                break;
            }
            if out_eof && err_eof {
                break;
            }
        }
        if Instant::now() >= work_until {
            status = w::NativeQueryStatus::Timeout;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    owner.close()?;
    // Drop unread pipe data. No-fork confinement and exact root reap prove closure;
    // reading discarded bytes here would spend the reserved result/receipt budget.
    if status != w::NativeQueryStatus::Complete {
        bytes.clear();
    }
    Ok(Reply {
        status,
        bytes,
        diagnostics: discarded,
        observed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const CHILD: &str = "RSS_COLLECTION_CONFINED_TEST";

    // Invoked in a separate process because a sandbox cannot be removed from its caller.
    #[test]
    fn confined_process_fixture() {
        let Ok(mode) = std::env::var(CHILD) else {
            return;
        };
        if mode.starts_with("broker") {
            broker_fixture(&mode);
        }
        let profile = std::ffi::CString::new(format!("{LEAF_PROFILE}(deny process-exec)")).unwrap();
        let mut error = std::ptr::null_mut();
        // SAFETY: valid profile/error storage for the native call.
        assert_eq!(unsafe { sandbox_init(profile.as_ptr(), 0, &mut error) }, 0);
        match mode.as_str() {
            "fork" => {
                // SAFETY: this isolated test expects the kernel to reject fork. If the
                // policy regresses, the child exits immediately and is explicitly reaped.
                let pid = unsafe { libc::fork() };
                if pid == 0 {
                    unsafe { libc::_exit(91) };
                }
                if pid > 0 {
                    unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) };
                }
                assert_eq!(pid, -1, "sandbox must prohibit descendant creation");
                assert!(Command::new("/usr/bin/true").spawn().is_err());
                let image = std::ffi::CString::new("/usr/bin/true").unwrap();
                let argv = [image.as_ptr(), std::ptr::null()];
                // SAFETY: a terminated argv and valid C string; policy must reject exec.
                assert_eq!(unsafe { libc::execv(image.as_ptr(), argv.as_ptr()) }, -1);
                println!("fork-and-exec-denied");
            }
            "hang" => {
                // Session escape changes no ownership fact: the exact Child is killed.
                unsafe { libc::setsid() };
                loop {
                    std::thread::sleep(Duration::from_secs(60));
                }
            }
            "flood" => {
                let buffer = [b'x'; 8192];
                loop {
                    std::io::stdout().write_all(&buffer).unwrap();
                }
            }
            _ => panic!("unknown fixture"),
        }
    }
    fn broker_fixture(mode: &str) -> ! {
        confine_query().unwrap();
        // SAFETY: the test launch exclusively transfers the two socket writers.
        assert!(unsafe { libc::dup2(196, 1) } >= 0);
        assert!(unsafe { libc::dup2(197, 2) } >= 0);
        let material =
            execution_runner::staging::read_worker_input(Path::new("/dev/fd/199"), 256 * 1024)
                .unwrap();
        let input: Input = serde_json::from_slice(&material).unwrap();
        let q = &input.read.plan.spec().queries["items"];
        if mode == "broker_read_hang" {
            begin_read().unwrap();
            let _bytes = std::fs::read("/System/Library/CoreServices/SystemVersion.plist").unwrap();
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
        let result = macos::execute(&input, q).unwrap();
        let output = serde_json::to_vec(&result).unwrap();
        let receipt = CollectionCompletion {
            input_sha256: hex(&material),
            output_sha256: hex(&output),
            closed: true,
            native_output_bytes: input.observed.load(std::sync::atomic::Ordering::Relaxed),
        };
        std::io::stdout().write_all(&output).unwrap();
        std::io::stderr()
            .write_all(&serde_json::to_vec(&receipt).unwrap())
            .unwrap();
        std::process::exit(0);
    }
    #[test]
    #[ignore = "actual macOS restricted worker performance acceptance; run explicitly"]
    fn host_performance_queries_through_confined_broker() {
        let _budget = crate::scenario_budget::enter();
        for section in [
            w::PerformanceSection::Cpu,
            w::PerformanceSection::Memory,
            w::PerformanceSection::FileSystems,
            w::PerformanceSection::Network,
        ] {
            let definition = w::NativeQueryDefinition::MacosPerformance { section };
            assert_eq!(
                engines().get(&definition.engine()),
                Some(&definition.engine().contract_version())
            );
            let input = macos::test_input(definition);
            let q = input.read.plan.spec().queries["items"].clone();
            assert!(input.read.plan.supported_by(&engines()));
            let began = Instant::now();
            let (result, observed) = broker_input("broker", 1024 * 1024, input).unwrap();
            assert_eq!(
                result.status,
                w::NativeQueryStatus::Complete,
                "{section:?}: {result:?}"
            );
            assert!(result.evidence_matches(&q));
            assert!(result.complete_for(&q, now().unwrap()));
            assert!(began.elapsed() < Duration::from_secs(q.budget.timeout_seconds.into()));
            assert!(observed <= u64::from(q.budget.output_bytes));
            assert!(serde_json::to_vec(&result).unwrap().len() <= q.budget.output_bytes as usize);
            if section == w::PerformanceSection::Cpu {
                assert!(began.elapsed() >= Duration::from_millis(w::PERFORMANCE_CPU_SAMPLE_MILLIS));
            }
            for row in &result.rows {
                assert!(q.definition.accepts_row(row));
            }
        }
    }
    fn broker_case(mode: &str, raw_allowance: usize) -> Result<(w::NativeQueryResult, u64), Error> {
        let mut input = macos::test_input(w::NativeQueryDefinition::MacosHardware {
            section: w::MacosHardwareSection::Identity,
        });
        for (name, path) in [
            ("system_profiler", "/usr/sbin/system_profiler"),
            ("sandbox-exec", "/usr/bin/sandbox-exec"),
        ] {
            input.tools.insert(
                name.into(),
                crate::backend::plan::pin(Path::new(path)).unwrap(),
            );
        }
        broker_input(mode, raw_allowance, input)
    }
    fn broker_input(
        mode: &str,
        raw_allowance: usize,
        input: Input,
    ) -> Result<(w::NativeQueryResult, u64), Error> {
        let material = serde_json::to_vec(&input).unwrap();
        let (channel, remote) = UnixStream::pair().unwrap();
        let (stdout, out_writer) = UnixStream::pair().unwrap();
        let (stderr, err_writer) = UnixStream::pair().unwrap();
        for stream in [&channel, &stdout, &stderr] {
            stream.set_nonblocking(true).unwrap();
        }
        let remote_copy = inherited_source(remote.as_raw_fd()).unwrap();
        let out_copy = inherited_source(out_writer.as_raw_fd()).unwrap();
        let err_copy = inherited_source(err_writer.as_raw_fd()).unwrap();
        drop(remote);
        drop(out_writer);
        drop(err_writer);
        let (remote, out_writer, err_writer) = (remote_copy, out_copy, err_copy);
        let source_fds = [
            remote.as_raw_fd(),
            out_writer.as_raw_fd(),
            err_writer.as_raw_fd(),
        ];
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "collection_worker::macos_supervisor::tests::confined_process_fixture",
                "--nocapture",
            ])
            .env(CHILD, mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // SAFETY: only async-signal-safe descriptor duplication runs after fork.
        unsafe {
            command.pre_exec(move || {
                for (source, target) in [
                    (source_fds[0], CHANNEL_FD),
                    (source_fds[1], 196),
                    (source_fds[2], 197),
                    (0, INPUT_FD),
                ] {
                    if libc::dup2(source, target) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        drop(remote);
        drop(out_writer);
        drop(err_writer);
        let until =
            Instant::now() + Duration::from_secs(if mode == "broker_read_hang" { 2 } else { 20 });
        let work_until = until - CLEANUP;
        let mut owner = OwnedChild {
            child,
            until,
            reaped: false,
        };
        let mut writer = owner.child.stdin.take().unwrap();
        nonblocking(writer.as_raw_fd()).unwrap();
        send(&mut writer, &material, work_until).unwrap();
        drop(writer);
        let q = &input.read.plan.spec().queries["items"];
        let result = supervise(
            owner,
            channel,
            stdout,
            stderr,
            &material,
            &input,
            q,
            work_until,
            until,
            raw_allowance,
        )?;
        Ok((
            result,
            input.observed.load(std::sync::atomic::Ordering::Relaxed),
        ))
    }
    #[test]
    fn confined_native_provider_and_broker_share_the_real_protocol() {
        let (result, observed) = broker_case("broker", 1024 * 1024).unwrap();
        assert_eq!(result.status, w::NativeQueryStatus::Complete);
        assert_eq!(result.rows.len(), 1);
        assert!(observed > 0);
    }
    #[test]
    fn killing_a_file_read_without_end_cannot_claim_complete_accounting() {
        assert!(matches!(
            broker_case("broker_read_hang", 1024 * 1024),
            Err(Error::Unavailable)
        ));
    }
    #[test]
    fn broker_raw_boundary_preserves_a_truncated_query_receipt() {
        let (result, observed) = broker_case("broker", 1024).unwrap();
        assert_eq!(result.status, w::NativeQueryStatus::Truncated);
        assert_eq!(observed, 1024);
    }
    #[test]
    #[ignore = "explicit read-only host provider evidence; no personal keychain"]
    fn host_system_queries_through_confined_broker() {
        use w::{MacosSecuritySection as S, NativeSystemQuery as Q};
        let current =
            crate::account_identity::generated_uid(&unsafe { libc::geteuid() }.to_string())
                .unwrap()
                .unwrap();
        for query in [
            Q::MacosApplication {
                target: w::MacosApplicationTarget::Signed {
                    bundle_id: "org.example.rss-absent".into(),
                    team_id: "ABCDE12345".into(),
                },
                permissions: Default::default(),
            },
            Q::MacosAccounts {
                section: w::AccountSection::Accounts,
                selection: w::AccountSelection::Selected {
                    identities: [current].into(),
                },
            },
            Q::MacosSecurity {
                section: S::Gatekeeper,
            },
            Q::MacosSecurity {
                section: S::Xprotect,
            },
            Q::MacosSecurity {
                section: S::SystemExtensions,
            },
            Q::MacosSecurity {
                section: S::Filevault,
            },
            Q::MacosSecurity {
                section: S::Firewall,
            },
            Q::MacosSecurity { section: S::Sip },
            Q::MacosCertificates {
                store: w::MacosCertificateStore::SystemRoots,
            },
            Q::MacosAccounts {
                section: w::AccountSection::Accounts,
                selection: w::AccountSelection::Selected {
                    identities: ["aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into()].into(),
                },
            },
            Q::MacosUpdates {
                section: w::UpdateSection::Available,
                mode: w::UpdateScanMode::Local,
            },
        ] {
            let mut input = macos::test_input(w::NativeQueryDefinition::System {
                query: query.clone(),
            });
            for name in macos::tool::required(&input.read.plan) {
                let path = Path::new(macos::tool::path(name).unwrap());
                if path.exists() {
                    input
                        .tools
                        .insert(name.into(), crate::backend::plan::pin(path).unwrap());
                }
            }
            let q = input.read.plan.spec().queries["items"].clone();
            let (result, observed) = broker_input("broker", 1024 * 1024, input).unwrap();
            eprintln!(
                "host {:?}: {:?}; rows={}; raw={observed}",
                query.engine(),
                result.status,
                result.row_count
            );
            assert!(result.evidence_matches(&q));
            let expected = match query {
                Q::MacosAccounts {
                    selection: w::AccountSelection::Selected { ref identities },
                    ..
                } if identities.contains("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa") => {
                    w::NativeQueryStatus::Partial
                }
                Q::MacosUpdates { .. } => w::NativeQueryStatus::Partial,
                _ => w::NativeQueryStatus::Complete,
            };
            assert_eq!(result.status, expected, "{:?}", query.engine());
            if expected == w::NativeQueryStatus::Complete {
                assert!(result.complete_for(&q, now().unwrap()));
            }
        }
    }
    fn fixture(mode: &str, cap: usize) -> (Reply, u64, Duration) {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "collection_worker::macos_supervisor::tests::confined_process_fixture",
                "--nocapture",
            ])
            .env(CHILD, mode)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().unwrap();
        let began = Instant::now();
        let raw = std::sync::atomic::AtomicU64::new(0);
        let result = capture(
            child,
            cap,
            began + Duration::from_secs(2),
            began + Duration::from_secs(3),
            &raw,
        )
        .unwrap();
        (
            result,
            raw.load(std::sync::atomic::Ordering::Relaxed),
            began.elapsed(),
        )
    }
    #[test]
    fn kernel_sandbox_rejects_fork_and_exec() {
        let (result, observed, _) = fixture("fork", 64 * 1024);
        assert_eq!(result.status, w::NativeQueryStatus::Complete);
        assert_eq!(result.observed as u64, observed);
        assert!(String::from_utf8(result.bytes)
            .unwrap()
            .contains("fork-and-exec-denied"));
    }
    #[test]
    fn escaped_session_root_is_reaped_within_query_budget() {
        let (result, _, elapsed) = fixture("hang", 64 * 1024);
        assert_eq!(result.status, w::NativeQueryStatus::Timeout);
        assert!(elapsed < Duration::from_secs(3));
    }
    #[test]
    fn output_flood_is_bounded_and_raw_bytes_are_counted() {
        let (result, observed, elapsed) = fixture("flood", 1024);
        assert_eq!(result.status, w::NativeQueryStatus::Truncated);
        assert!(result.bytes.is_empty());
        assert_eq!(observed, 1024);
        assert_eq!(result.observed as u64, observed);
        assert!(elapsed < Duration::from_secs(3));
    }
    #[test]
    fn real_system_profiler_runs_under_leaf_policy() {
        let began = Instant::now();
        let child = Command::new("/usr/bin/sandbox-exec")
            .args([
                "-p",
                LEAF_PROFILE,
                "/usr/sbin/system_profiler",
                "-nospawn",
                "-xml",
                "SPHardwareDataType",
                "-detailLevel",
                "basic",
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let raw = std::sync::atomic::AtomicU64::new(0);
        let reply = capture(
            child,
            1024 * 1024,
            began + Duration::from_secs(15),
            began + Duration::from_secs(16),
            &raw,
        )
        .unwrap();
        assert_eq!(reply.status, w::NativeQueryStatus::Complete);
        let value = plist::Value::from_reader(std::io::Cursor::new(reply.bytes)).unwrap();
        assert!(value.as_array().unwrap().iter().any(|block| {
            block
                .as_dictionary()
                .and_then(|d| d.get("_items"))
                .and_then(plist::Value::as_array)
                .is_some_and(|items| !items.is_empty())
        }));
        assert_eq!(
            raw.load(std::sync::atomic::Ordering::Relaxed),
            reply.observed as u64
        );
    }
    #[test]
    fn broker_rejects_non_recipe_commands() {
        use super::macos::tool::allowed;
        let input = macos::test_input(w::NativeQueryDefinition::MacosHardware {
            section: w::MacosHardwareSection::Disks,
        });
        let d = &input.read.plan.spec().queries["items"].definition;
        let binding = &input.read.execution;
        assert!(!allowed(d, "sh", &["-c".into(), "true".into()], binding));
        assert!(!allowed(
            d,
            "pkgutil",
            &["--forget".into(), "package".into()],
            binding
        ));
        assert!(!allowed(
            d,
            "diskutil",
            &["info".into(), "-plist".into(), "--help".into()],
            binding
        ));
        assert!(allowed(
            d,
            "diskutil",
            &["info".into(), "-plist".into(), "disk1s1".into()],
            binding
        ));
        assert!(!allowed(d, "spctl", &["--status".into()], binding));
    }
}
