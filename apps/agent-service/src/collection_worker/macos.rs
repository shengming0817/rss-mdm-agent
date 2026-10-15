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
mod applications;
mod brew;
mod hardware;
mod receipts;
#[cfg(test)]
mod tests;
mod tool;

pub(super) fn tools(
    _config: &crate::ExecutionConfig,
    plan: &w::NativeCollectionPlan,
) -> std::result::Result<BTreeMap<String, SoftwareMaterial>, Error> {
    let mut names = std::collections::BTreeSet::from(["sandbox-exec"]);
    for q in plan.spec().queries.values() {
        use w::NativeQueryDefinition::*;
        match q.definition {
            MacosHardware { .. } => {
                names.extend(["ioreg", "system_profiler", "diskutil"]);
            }
            MacosPkgReceipts {} => {
                names.insert("pkgutil");
            }
            MacosApplications { .. } => {}
            MacosBrew { .. } => {}
            _ => return Err(Error::Unsupported),
        }
    }
    let mut result = BTreeMap::new();
    for name in names {
        let dir = if name != "sandbox-exec" {
            "/usr/sbin"
        } else {
            "/usr/bin"
        };
        let path = PathBuf::from(dir).join(name);
        // Missing tools are recorded by the query, never interpreted as an empty inventory.
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
                    .task
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
            .task
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
            _ => Err(Status::Unsupported.into()),
        }?;
        context.check()
    })();
    let status = match status {
        Ok(()) => Ok(()),
        Err(Fault::Query(s)) => Err(s),
        Err(Fault::Unclosed) => return Err(Error::Unavailable),
    };
    let result = rows.finish(status);
    if let w::SoftwareExecutionTarget::User { identity, .. } = &input.task.execution.target {
        if result
            .rows
            .iter()
            .any(|r| r.get("owner").and_then(serde_json::Value::as_str) != Some(identity.as_str()))
        {
            return Ok(failure(q, Status::Denied, time));
        }
    }
    Ok(result)
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
