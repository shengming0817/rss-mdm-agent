// ref: apple-oss-distributions/xnu bsd/sys/un.h (LOCAL_PEERTOKEN).
use super::*;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

/// Owns the sole navigation receiver in this UID and kernel login incarnation.
pub struct NavigationInstance {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    _lock: File,
    socket: PathBuf,
}
impl Drop for NavigationInstance {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}
fn peer(stream: &UnixStream) -> Result<(), Error> {
    let mut token = [0u32; 8];
    let mut size = std::mem::size_of_val(&token) as libc::socklen_t;
    // SAFETY: the descriptor is owned by stream and the fixed audit-token buffer is writable.
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERTOKEN,
            token.as_mut_ptr().cast(),
            &mut size,
        )
    } != 0
        || size as usize != std::mem::size_of_val(&token)
        || token[1] != unsafe { libc::geteuid() }
        || token[6] != current_session()?
    {
        return Err(Error::Denied);
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| Error::Unavailable)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| Error::Unavailable)?;
    Ok(())
}
// A socket read timeout applies per syscall, so enforce the whole-frame deadline
// and shutdown independently even when a peer continuously dribbles bytes.
fn frame_read(
    stream: &mut UnixStream,
    mut bytes: &mut [u8],
    deadline: Instant,
    stop: &AtomicBool,
) -> Result<(), Error> {
    while !bytes.is_empty() {
        if stop.load(Ordering::Acquire) {
            return Err(Error::Unavailable);
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::Unavailable)?;
        stream
            .set_read_timeout(Some(remaining.min(Duration::from_millis(100))))
            .map_err(|_| Error::Unavailable)?;
        match stream.read(bytes) {
            Ok(0) => return Err(Error::Unavailable),
            Ok(count) => bytes = &mut bytes[count..],
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(Error::Unavailable),
        }
    }
    Ok(())
}
impl NavigationInstance {
    /// Elect a receiver or forward one bounded intent, exiting only after its queue ACK.
    /// An empty message requests focus only. Every nonempty message is parsed again.
    pub fn claim(
        initial: Option<&str>,
        receive: impl Fn(String) -> bool + Send + 'static,
    ) -> Result<Option<Self>, Error> {
        Self::claim_named(initial, receive, &name()?)
    }
    fn claim_named(
        initial: Option<&str>,
        receive: impl Fn(String) -> bool + Send + 'static,
        name: &str,
    ) -> Result<Option<Self>, Error> {
        if let Some(uri) = initial {
            EntryIntent::parse(uri).map_err(|_| Error::InvalidInput)?;
        }
        let root = PathBuf::from(format!("/private/tmp/rss-mdm-nav-{}", current_subject()?));
        match std::fs::DirBuilder::new().mode(0o700).create(&root) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(Error::Unavailable),
        }
        let meta = std::fs::symlink_metadata(&root).map_err(|_| Error::Denied)?;
        if !meta.is_dir()
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & 0o777 != 0o700
        {
            return Err(Error::Denied);
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root.join(format!("{}.lock", &name[..24])))
            .map_err(|_| Error::Denied)?;
        platform_private_storage::validate_file_handle(&lock).map_err(|_| Error::Denied)?;
        let socket = root.join(format!("{}.sock", &name[..24]));
        // SAFETY: flock applies to the uniquely owned open file descriptor.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let started = Instant::now();
            let mut stream = loop {
                match UnixStream::connect(&socket) {
                    Ok(s) => break s,
                    Err(_) if started.elapsed() < Duration::from_secs(2) => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => return Err(Error::Unavailable),
                }
            };
            peer(&stream)?;
            let body = initial.unwrap_or("").as_bytes();
            stream
                .write_all(&(body.len() as u32).to_be_bytes())
                .and_then(|_| stream.write_all(body))
                .map_err(|_| Error::Unavailable)?;
            let mut ack = [0u8];
            stream
                .read_exact(&mut ack)
                .map_err(|_| Error::Unavailable)?;
            return if ack == [1] {
                Ok(None)
            } else {
                Err(Error::Unavailable)
            };
        }
        // Only the lock holder may clean a stale socket belonging to this exact login.
        match std::fs::symlink_metadata(&socket) {
            Ok(meta) if meta.uid() == unsafe { libc::geteuid() } => {
                std::fs::remove_file(&socket).map_err(|_| Error::Denied)?
            }
            Ok(_) => return Err(Error::Denied),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Unavailable),
        }
        let listener = UnixListener::bind(&socket).map_err(|_| Error::Unavailable)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Denied)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| Error::Unavailable)?;
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let thread = std::thread::spawn(move || {
            while !cancelled.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let result = (|| -> Result<bool, Error> {
                            peer(&stream)?;
                            let deadline = Instant::now() + Duration::from_secs(2);
                            let mut header = [0; 4];
                            frame_read(&mut stream, &mut header, deadline, &cancelled)?;
                            let len = u32::from_be_bytes(header) as usize;
                            if len > execution_contract::ENTRY_URI_BYTES {
                                return Err(Error::InvalidInput);
                            }
                            let mut body = vec![0; len];
                            frame_read(&mut stream, &mut body, deadline, &cancelled)?;
                            let uri = String::from_utf8(body).map_err(|_| Error::InvalidInput)?;
                            if !uri.is_empty() {
                                EntryIntent::parse(&uri).map_err(|_| Error::InvalidInput)?;
                            }
                            Ok(receive(uri))
                        })();
                        let _ = stream.write_all(&[u8::from(matches!(result, Ok(true)))]);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Some(Self {
            stop,
            thread: Some(thread),
            _lock: lock,
            socket,
        }))
    }
}
use std::os::unix::fs::DirBuilderExt;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_slow_frame_cannot_extend_the_total_budget_or_ignore_shutdown() {
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now() + Duration::from_millis(160);
        let thread = std::thread::spawn(move || {
            for _ in 0..8 {
                if writer.write_all(&[1]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        });
        let mut frame = [0; 32];
        assert!(frame_read(&mut reader, &mut frame, deadline, &stop).is_err());
        assert!(Instant::now().duration_since(deadline) < Duration::from_millis(100));
        drop(reader);
        thread.join().unwrap();
        let (mut reader, _writer) = UnixStream::pair().unwrap();
        stop.store(true, Ordering::Release);
        assert!(frame_read(
            &mut reader,
            &mut frame,
            Instant::now() + Duration::from_secs(2),
            &stop
        )
        .is_err());
    }
    #[test]
    fn actual_kernel_login_handoff_is_acknowledged_and_does_not_submit() {
        // Isolate the real native transport from the user's production desktop endpoint.
        let name = uuid::Uuid::new_v4().simple().to_string();
        let (sender, receiver) = std::sync::mpsc::channel();
        let primary =
            NavigationInstance::claim_named(None, move |uri| sender.send(uri).is_ok(), &name)
                .unwrap()
                .unwrap();
        let uri = "rssmdm://view/v1/node/settings.account";
        assert!(NavigationInstance::claim_named(
            Some(uri),
            |_| panic!("secondary received"),
            &name
        )
        .unwrap()
        .is_none());
        assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).unwrap(), uri);
        drop(primary);
        // A closed owner releases its lock and socket, allowing the same login to recover.
        assert!(NavigationInstance::claim_named(None, |_| true, &name)
            .unwrap()
            .is_some());
    }
}
