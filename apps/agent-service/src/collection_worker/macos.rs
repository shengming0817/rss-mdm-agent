//! Closed macOS recipes, not a command interpreter or an Inventory mapper.
//! ref: osquery/osquery@5.23.1 specs/darwin/{apps,package_receipts}.table.
use super::values::{Row, Rows, Status};
use super::*;
use std::{
    cell::Cell,
    ffi::CString,
    io::Read,
    path::PathBuf,
    time::{Duration, Instant},
};
mod accounts;
mod application;
mod applications;
mod brew;
mod certificates;
mod hardware;
mod native;
mod receipts;
mod security;
mod signature;
pub(crate) fn local_app_identity(
    path: &Path,
    bundle: &str,
    team: &str,
) -> std::result::Result<(), Error> {
    let evidence = signature::inspect(path, Some((bundle, team))).map_err(|_| Error::Untrusted)?;
    if evidence.status != "signed"
        || evidence.identifier.as_deref() != Some(bundle)
        || evidence.team.as_deref() != Some(team)
    {
        return Err(Error::Untrusted);
    }
    Ok(())
}
pub(crate) fn local_user_app_root(subject: &str) -> std::result::Result<PathBuf, Error> {
    home(subject.parse().map_err(|_| Error::Identity)?)
        .map(|p| p.join("Applications"))
        .map_err(|_| Error::Identity)
}
#[cfg(test)]
mod tests;
pub(super) mod tool;
mod updates;

pub(super) fn tools(
    _config: &crate::ExecutionConfig,
    plan: &w::NativeCollectionPlan,
) -> std::result::Result<BTreeMap<String, SoftwareMaterial>, Error> {
    let mut result = BTreeMap::new();
    for name in tool::required(plan) {
        let path = PathBuf::from(tool::path(name).ok_or(Error::Unsupported)?);
        if path.try_exists()? {
            result.insert(name.into(), crate::backend::plan::pin(&path)?);
        }
    }
    Ok(result)
}
#[derive(Debug)]
enum Fault {
    Query(Status),
    Unclosed,
}
impl From<Status> for Fault {
    fn from(v: Status) -> Self {
        Self::Query(v)
    }
}
impl From<std::io::Error> for Fault {
    fn from(e: std::io::Error) -> Self {
        Self::Query(match e.kind() {
            std::io::ErrorKind::PermissionDenied => Status::Denied,
            std::io::ErrorKind::NotFound => Status::Missing,
            _ => Status::Invalid,
        })
    }
}
type Result<T> = std::result::Result<T, Fault>;
struct Context<'a> {
    input: &'a Input,
    until: Instant,
    bytes: Cell<usize>,
    cap: usize,
}
impl Context<'_> {
    fn check(&self) -> Result<()> {
        if Instant::now() >= self.until {
            Err(Status::Timeout.into())
        } else {
            Ok(())
        }
    }
    fn consume(&self, n: usize) -> Result<()> {
        super::macos_supervisor::observe(n).map_err(|_| Fault::Unclosed)?;
        self.bytes.set(self.bytes.get().saturating_add(n));
        self.input
            .observed
            .fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
        self.check()?;
        if self.bytes.get() > self.cap
            || self
                .input
                .observed
                .load(std::sync::atomic::Ordering::Relaxed)
                > self
                    .input
                    .read
                    .plan
                    .spec()
                    .budget
                    .output_bytes
                    .saturating_sub(1024) as u64
        {
            Err(Status::Truncated.into())
        } else {
            Ok(())
        }
    }
    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        use std::os::unix::fs::OpenOptionsExt;
        self.check()?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(Status::Invalid.into());
        }
        self.read_bounded(&file, usize::MAX)
    }
    fn read_bounded(&self, mut reader: impl Read, maximum: usize) -> Result<Vec<u8>> {
        self.check()?;
        let limit = self.cap.saturating_sub(self.bytes.get()).min(maximum);
        super::macos_supervisor::begin_read().map_err(|_| Fault::Unclosed)?;
        let mut bytes = Vec::new();
        let read = (&mut reader).take(limit as u64 + 1).read_to_end(&mut bytes);
        // Account partial reads even on I/O failure. EndRead follows the observation ACK;
        // a forced termination in between leaves an explicit incomplete accounting state.
        let accounted = self.consume(bytes.len());
        let ended = super::macos_supervisor::end_read().map_err(|_| Fault::Unclosed);
        accounted?;
        ended?;
        read?;
        if bytes.len() > limit {
            return Err(Status::Truncated.into());
        }
        Ok(bytes)
    }
    fn plist(&self, path: &Path) -> Result<plist::Value> {
        plist::Value::from_reader(std::io::Cursor::new(self.read(path)?))
            .map_err(|_| Status::Invalid.into())
    }
    fn command(&self, name: &str, args: &[&str]) -> Result<Vec<u8>> {
        let material = self.input.tools.get(name).ok_or(Status::Unsupported)?;
        self.run(material, args, &[])
    }
    fn command_plist(&self, name: &str, args: &[&str]) -> Result<plist::Value> {
        plist::Value::from_reader(std::io::Cursor::new(self.command(name, args)?))
            .map_err(|_| Status::Invalid.into())
    }
}
pub(super) fn execute(
    input: &Input,
    q: &w::NativeQuery,
) -> std::result::Result<w::NativeQueryResult, Error> {
    let time = now()?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Clock)?
        .as_millis() as u64;
    let remaining = Duration::from_millis(
        input
            .finish_by_unix_ms
            .saturating_sub(millis)
            .saturating_sub(250),
    )
    .min(Duration::from_secs(q.budget.timeout_seconds.into()));
    let native_cap = super::macos_supervisor::remaining_budget()?.saturating_sub(1);
    let context = Context {
        input,
        until: Instant::now() + remaining,
        bytes: Cell::new(0),
        cap: input
            .read
            .plan
            .spec()
            .budget
            .output_bytes
            .saturating_sub(512)
            .min(q.budget.output_bytes) as usize,
    };
    let context = Context {
        cap: context.cap.min(native_cap),
        ..context
    };
    let mut rows = Rows::new(q, time);
    let status = (|| {
        context.check()?;
        use w::NativeQueryDefinition::*;
        match &q.definition {
            MacosPerformance { section } => {
                super::performance::read(*section, context.until, |row| rows.push(row))
                    .map_err(Fault::Query)
            }
            MacosHardware { section } => hardware::collect(&context, *section, &mut rows),
            MacosApplications { root, max_depth } => {
                applications::collect(&context, *root, *max_depth, &mut rows)
            }
            MacosPkgReceipts {} => receipts::collect(&context, &mut rows),
            MacosBrew {
                prefix,
                package,
                taps,
            } => brew::collect(&context, prefix, *package, taps, &mut rows),
            System { query } => match query {
                w::NativeSystemQuery::MacosSecurity { section } => {
                    security::collect(&context, *section, &mut rows)
                }
                w::NativeSystemQuery::MacosAccounts { section, selection } => {
                    accounts::collect(&context, *section, selection, &mut rows)
                }
                w::NativeSystemQuery::MacosCertificates { store } => {
                    certificates::collect(&context, *store, &mut rows)
                }
                w::NativeSystemQuery::MacosUpdates { section, mode } => {
                    updates::collect(&context, *section, *mode, &mut rows)
                }
                w::NativeSystemQuery::MacosApplication {
                    target,
                    permissions,
                } => application::collect(&context, target, permissions, &mut rows),
                _ => Err(Status::Unsupported.into()),
            },
            _ => Err(Status::Unsupported.into()),
        }?;
        context.check()
    })();
    let status = match status {
        Ok(()) => Ok(()),
        Err(Fault::Query(s)) => Err(s),
        Err(Fault::Unclosed) => return Err(Error::Unavailable),
    };
    rows.result.collected_at = now()?;
    let result = rows.finish(status);
    if !owner_matches(input, q, &result) {
        return Ok(failure(q, Status::Denied, now()?));
    }
    Ok(result)
}
fn owner_matches(input: &Input, q: &w::NativeQuery, result: &w::NativeQueryResult) -> bool {
    let w::SoftwareExecutionTarget::User { identity, .. } = &input.read.execution.target else {
        return true;
    };
    let expected = match &q.definition {
        w::NativeQueryDefinition::System {
            query: w::NativeSystemQuery::MacosApplication { .. },
        } => return true,
        w::NativeQueryDefinition::System { .. } => input.read.execution.generated_uid.as_deref(),
        _ => Some(identity.as_str()),
    };
    expected.is_some_and(|owner| {
        result
            .rows
            .iter()
            .all(|r| r.get("owner").and_then(serde_json::Value::as_str) == Some(owner))
    })
}
// fdopendir owns an independently opened directory description. macOS /dev/fd
// cannot be reopened by std::fs::read_dir, and dup would share its scan offset.
fn directory_names(c: &Context<'_>, dir: &std::fs::File) -> Result<Vec<std::ffi::OsString>> {
    use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
    struct Directory(*mut libc::DIR);
    impl Drop for Directory {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.0) };
        }
    }
    c.check()?;
    // SAFETY: live directory fd and a fixed NUL-terminated relative component.
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            c".".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(error.into());
    }
    let stream = Directory(stream);
    let mut names = Vec::new();
    loop {
        c.check()?;
        // SAFETY: this module is macOS-only. errno distinguishes EOF from failure;
        // readdir's borrowed name is copied before the next call.
        unsafe { *libc::__error() = 0 };
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                return Err(error.into());
            }
            return Ok(names);
        }
        let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        names.push(std::ffi::OsStr::from_bytes(name).to_owned());
        if names.len() >= 10_000 {
            return Err(Status::Truncated.into());
        }
    }
}
fn object(v: &plist::Value) -> Result<&plist::Dictionary> {
    v.as_dictionary().ok_or(Status::Invalid.into())
}
fn text<'a>(v: &'a plist::Dictionary, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(plist::Value::as_string)
        .ok_or(Status::Missing.into())
}
fn array<'a>(v: &'a plist::Dictionary, key: &str) -> Result<&'a Vec<plist::Value>> {
    v.get(key)
        .and_then(plist::Value::as_array)
        .ok_or(Status::Missing.into())
}
fn uint(v: &plist::Dictionary, key: &str) -> Result<u64> {
    v.get(key)
        .and_then(plist::Value::as_unsigned_integer)
        .ok_or(Status::Missing.into())
}
fn row(fields: &[(&str, serde_json::Value)]) -> Row {
    fields
        .iter()
        .map(|(k, v)| ((*k).into(), v.clone()))
        .collect()
}
fn stable(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    for p in parts {
        hash.update((p.len() as u64).to_be_bytes());
        hash.update(p.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
fn home(uid: u32) -> Result<PathBuf> {
    let mut storage = vec![0u8; 65536];
    let mut pwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut found = std::ptr::null_mut();
    // SAFETY: bounded writable passwd/storage, copied before either allocation is dropped.
    let rc = unsafe {
        libc::getpwuid_r(
            uid,
            pwd.as_mut_ptr(),
            storage.as_mut_ptr().cast(),
            storage.len(),
            &mut found,
        )
    };
    if rc != 0 || found.is_null() {
        return Err(Status::Denied.into());
    }
    let path = unsafe { std::ffi::CStr::from_ptr((*found).pw_dir) }
        .to_str()
        .map_err(|_| Status::Invalid)?;
    Ok(PathBuf::from(path))
}

#[cfg(test)]
pub(super) fn test_input(definition: w::NativeQueryDefinition) -> Input {
    tests::input(tests::query(definition))
}

fn account_name(identity: &str) -> Result<String> {
    let uid = identity.parse::<u32>().map_err(|_| Status::Denied)?;
    if uid == 0 {
        return Err(Status::Denied.into());
    }
    let mut storage = vec![0u8; 16384];
    let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut found = std::ptr::null_mut();
    // SAFETY: owned writable buffers; strings copied before storage is released.
    let rc = unsafe {
        libc::getpwuid_r(
            uid,
            entry.as_mut_ptr(),
            storage.as_mut_ptr().cast(),
            storage.len(),
            &mut found,
        )
    };
    if rc != 0 || found.is_null() {
        return Err(Status::Denied.into());
    }
    let name = unsafe { std::ffi::CStr::from_ptr((*found).pw_name) }
        .to_str()
        .map_err(|_| Status::Invalid)?;
    if name.is_empty() || name.starts_with('-') {
        return Err(Status::Invalid.into());
    }
    Ok(name.into())
}
