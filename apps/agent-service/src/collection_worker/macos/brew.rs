//! Read installed receipts only; never execute Homebrew, Ruby, or shell configuration.
//! ref: Homebrew/brew@7cce6eac8d897b0b8440e16f33dbcb21770da7cf
//! Library/Homebrew/{tab.rb,cask/{tab.rb,metadata.rb,caskroom.rb}}.
use super::*;
use std::{
    collections::BTreeSet,
    fs::{File, Metadata},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
};

const MAX_ENTRIES: usize = 10_000;
const MAX_RECEIPT: usize = 1024 * 1024;

// Reopen every observed object from the held prefix after scanning. This detects
// replacements as well as in-place writes, including changes after a row was read.
struct Scan<'a, 'b> {
    context: &'a Context<'b>,
    root: File,
    seen: Vec<(Vec<String>, Metadata)>,
}
impl Scan<'_, '_> {
    fn open(&mut self, parts: &[String], directory: bool) -> Result<File> {
        self.context.check()?;
        if self.seen.len() >= MAX_ENTRIES {
            return Err(Status::Truncated.into());
        }
        let file = relative(&self.root, parts, directory)?;
        self.seen.push((parts.to_vec(), file.metadata()?));
        Ok(file)
    }
    fn entries(&mut self, parts: &[String]) -> Result<Vec<String>> {
        let dir = self.open(parts, true)?;
        let mut names = Vec::new();
        for item in std::fs::read_dir(format!("/dev/fd/{}", dir.as_raw_fd()))? {
            self.context.check()?;
            let name = item?
                .file_name()
                .into_string()
                .map_err(|_| Status::Invalid)?;
            component(&name)?;
            names.push(name);
            if names.len() >= MAX_ENTRIES {
                return Err(Status::Truncated.into());
            }
        }
        names.sort();
        Ok(names)
    }
    fn json(&mut self, parts: &[String]) -> Result<serde_json::Value> {
        let file = self.open(parts, false)?;
        let bytes = self.context.read_bounded(file, MAX_RECEIPT)?;
        serde_json::from_slice(&bytes).map_err(|_| Status::Invalid.into())
    }
    fn verify(&self) -> Result<()> {
        for (parts, before) in &self.seen {
            self.context.check()?;
            let after = relative(&self.root, parts, before.is_dir())?.metadata()?;
            unchanged(before, &after)?;
        }
        Ok(())
    }
}
fn unchanged(a: &Metadata, b: &Metadata) -> Result<()> {
    if a.dev() != b.dev()
        || a.ino() != b.ino()
        || a.mode() != b.mode()
        || a.uid() != b.uid()
        || a.gid() != b.gid()
        || a.len() != b.len()
        || a.mtime() != b.mtime()
        || a.mtime_nsec() != b.mtime_nsec()
        || a.ctime() != b.ctime()
        || a.ctime_nsec() != b.ctime_nsec()
    {
        return Err(Status::Invalid.into());
    }
    Ok(())
}
fn component(s: &str) -> Result<()> {
    if s.is_empty()
        || s == "."
        || s == ".."
        || s.len() > 255
        || s.contains('/')
        || s.chars().any(char::is_control)
    {
        return Err(Status::Invalid.into());
    }
    Ok(())
}
fn child(parts: &[String], name: &str) -> Vec<String> {
    let mut result = parts.to_vec();
    result.push(name.into());
    result
}
fn openat(parent: &File, name: &str, directory: bool) -> Result<File> {
    component(name)?;
    let name = CString::new(name).map_err(|_| Status::Invalid)?;
    // SAFETY: valid parent fd and NUL-terminated single component; ownership of
    // the returned descriptor transfers to File exactly once.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | if directory { libc::O_DIRECTORY } else { 0 },
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let meta = file.metadata()?;
    if !(if directory {
        meta.is_dir()
    } else {
        meta.is_file()
    }) {
        return Err(Status::Invalid.into());
    }
    Ok(file)
}
fn relative(root: &File, parts: &[String], directory: bool) -> Result<File> {
    let mut file = root.try_clone()?;
    for (i, part) in parts.iter().enumerate() {
        file = openat(&file, part, i + 1 < parts.len() || directory)?;
    }
    Ok(file)
}
fn absolute(path: &Path) -> Result<File> {
    if !path.is_absolute() {
        return Err(Status::Denied.into());
    }
    let mut dir = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open("/")?;
    for part in path.components() {
        match part {
            std::path::Component::RootDir => {}
            std::path::Component::Normal(s) => {
                dir = openat(&dir, s.to_str().ok_or(Status::Invalid)?, true)?;
            }
            _ => return Err(Status::Denied.into()),
        }
    }
    Ok(dir)
}
pub(super) fn collect(
    c: &Context<'_>,
    prefix: &str,
    kind: w::MacosBrewPackage,
    taps: &BTreeSet<String>,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let w::SoftwareExecutionTarget::User { identity, .. } = &c.input.task.execution.target else {
        return Err(Status::Denied.into());
    };
    let uid = identity.parse::<u32>().map_err(|_| Status::Denied)?;
    if uid == 0 || unsafe { libc::geteuid() } != uid {
        return Err(Status::Denied.into());
    }
    let root = absolute(Path::new(prefix))?;
    let before = root.metadata()?;
    let mut scan = Scan {
        context: c,
        root,
        seen: Vec::new(),
    };
    installed(&mut scan, prefix, identity, uid, kind, taps, rows)?;
    scan.verify()?;
    unchanged(&before, &absolute(Path::new(prefix))?.metadata()?)
}
fn installed(
    scan: &mut Scan<'_, '_>,
    prefix: &str,
    owner: &str,
    uid: u32,
    kind: w::MacosBrewPackage,
    taps: &BTreeSet<String>,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let formula = kind == w::MacosBrewPackage::Formula;
    let base = vec![if formula { "Cellar" } else { "Caskroom" }.into()];
    // Missing roots/receipts are incomplete evidence, never a complete empty set.
    for name in scan.entries(&base)? {
        let package = child(&base, &name);
        if scan.open(&package, true)?.metadata()?.uid() != uid {
            return Err(Status::Denied.into());
        }
        if formula {
            let versions = scan.entries(&package)?;
            if versions.is_empty() {
                return Err(Status::Missing.into());
            }
            for version in versions {
                let installation = child(&package, &version);
                if scan.open(&installation, true)?.metadata()?.uid() != uid {
                    return Err(Status::Denied.into());
                }
                let receipt = scan.json(&child(&installation, "INSTALL_RECEIPT.json"))?;
                emit(&receipt, prefix, owner, &name, &version, kind, taps, rows)?;
            }
        } else {
            let metadata = child(&package, ".metadata");
            let receipt = scan.json(&child(&metadata, "INSTALL_RECEIPT.json"))?;
            let version = source(&receipt, "version")?;
            component(version)?;
            // Match Homebrew's latest timestamp selection, without loading any Ruby.
            let mut latest: Option<(String, String)> = None;
            for candidate in scan.entries(&metadata)? {
                if matches!(
                    candidate.as_str(),
                    "INSTALL_RECEIPT.json" | "config.json" | "LATEST_DOWNLOAD_SHA256"
                ) {
                    continue;
                }
                for stamp in scan.entries(&child(&metadata, &candidate))? {
                    if stamp.len() != 18
                        || stamp.as_bytes()[14] != b'.'
                        || !stamp
                            .bytes()
                            .enumerate()
                            .all(|(i, b)| i == 14 || b.is_ascii_digit())
                    {
                        return Err(Status::Unsupported.into());
                    }
                    if latest.as_ref().is_some_and(|(time, _)| stamp == *time) {
                        return Err(Status::Invalid.into());
                    }
                    if latest.as_ref().is_none_or(|(time, _)| stamp > *time) {
                        latest = Some((stamp, candidate.clone()));
                    }
                }
            }
            let (stamp, installed_version) = latest.ok_or(Status::Missing)?;
            if installed_version != version {
                return Err(Status::Invalid.into());
            }
            let casks = child(&child(&child(&metadata, version), &stamp), "Casks");
            let marker = child(&casks, &format!("{name}.json"));
            let saved = match scan.json(&marker) {
                Err(Fault::Query(Status::Missing)) => return Err(Status::Unsupported.into()),
                other => other?,
            };
            let saved = saved.as_object().ok_or(Status::Invalid)?;
            if saved
                .keys()
                .any(|key| !matches!(key.as_str(), "artifacts" | "url_specs" | "version"))
            {
                return Err(Status::Unsupported.into());
            }
            if saved
                .get("version")
                .is_some_and(|v| v.as_str() != Some(version))
                || saved.get("artifacts").is_some_and(|v| !v.is_array())
                || saved.get("url_specs").is_some_and(|v| !v.is_object())
            {
                return Err(Status::Invalid.into());
            }
            // Staged payload may be removed by a valid cask; installed metadata is
            // authoritative, not the current catalog or the presence of an app bundle.
            emit(&receipt, prefix, owner, &name, version, kind, taps, rows)?;
        }
    }
    Ok(())
}
fn source<'a>(receipt: &'a serde_json::Value, field: &str) -> Result<&'a str> {
    receipt
        .get("source")
        .and_then(|v| v.get(field))
        .and_then(serde_json::Value::as_str)
        .ok_or(Status::Unsupported.into())
}
#[allow(clippy::too_many_arguments)]
fn emit(
    receipt: &serde_json::Value,
    prefix: &str,
    owner: &str,
    name: &str,
    version: &str,
    kind: w::MacosBrewPackage,
    taps: &BTreeSet<String>,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let tap = source(receipt, "tap")?;
    // Validate even filtered records through the producer's sole selector grammar.
    // A malformed tap cannot become an apparently complete empty selection.
    w::NativeQueryDefinition::MacosBrew {
        prefix: prefix.into(),
        package: kind,
        taps: [tap.into()].into(),
    }
    .validate()
    .map_err(|_| Status::Invalid)?;
    if !taps.is_empty() && !taps.contains(tap) {
        return Ok(());
    }
    let formula = kind == w::MacosBrewPackage::Formula;
    let package_kind = if formula { "formula" } else { "cask" };
    let location = format!(
        "{prefix}/{}/{name}/{version}",
        if formula { "Cellar" } else { "Caskroom" }
    );
    rows.push(row(&[
        (
            "id",
            stable(&["homebrew", owner, prefix, tap, package_kind, name, version]).into(),
        ),
        ("native_id", format!("{tap}/{name}").into()),
        ("name", name.into()),
        ("version", version.into()),
        ("scope", "user".into()),
        ("owner", owner.into()),
        ("origin", "homebrew".into()),
        ("location", location.into()),
        ("prefix", prefix.into()),
        ("tap", tap.into()),
        ("package_kind", package_kind.into()),
    ]))?;
    Ok(())
}

#[cfg(test)]
mod tests;
