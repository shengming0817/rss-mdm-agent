//! Homebrew JSON v2 only. Prefixes never select executable paths.
//! ref: Homebrew/brew@7cce6eac8d897b0b8440e16f33dbcb21770da7cf Library/Homebrew/cmd/info.rb.
use super::*;
use std::{collections::BTreeSet, os::unix::fs::MetadataExt};
pub(super) const SANDBOX: &str = "(version 1)(allow default)(deny file-write*)(allow file-write* (literal \"/dev/null\"))(deny network*)";
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
    let prefix_path = Path::new(prefix);
    if prefix_path.canonicalize()?.as_os_str() != prefix_path.as_os_str() {
        return Err(Status::Denied.into());
    }
    let sandbox = c
        .input
        .tools
        .get("sandbox-exec")
        .ok_or(Status::Unsupported)?;
    let home = home(uid)?;
    let home = home.to_str().ok_or(Status::Invalid)?;
    let env = [
        ("HOME", home),
        ("HOMEBREW_NO_AUTO_UPDATE", "1"),
        ("HOMEBREW_NO_ANALYTICS", "1"),
        ("HOMEBREW_NO_INSTALL_CLEANUP", "1"),
        ("HOMEBREW_NO_ENV_HINTS", "1"),
        ("HOMEBREW_NO_ENV_FILE", "1"),
        ("HOMEBREW_NO_GITHUB_API", "1"),
    ];
    let mut selected = None;
    for (name, tool) in &c.input.tools {
        if !name.starts_with("brew-") {
            continue;
        }
        let path = Path::new(&tool.path);
        let _lease = execution_runner::staging::verify_retained(path, &tool.artifact.sha256)
            .map_err(|_| Status::Denied)?;
        let code_root = path.parent().and_then(Path::parent).ok_or(Status::Denied)?;
        // Formula/cask loading executes Ruby. Every admitted installation entry must
        // remain administrator protected, including the installed .brew/.metadata code.
        protected_tree(c, code_root)?;
        if !prefix_path.starts_with(code_root) {
            protected_tree(c, prefix_path)?;
        }
        let observed = c.run(sandbox, &["-p", SANDBOX, &tool.path, "--prefix"], &env)?;
        if std::str::from_utf8(&observed)
            .map_err(|_| Status::Invalid)?
            .trim()
            == prefix
        {
            if selected.is_some() {
                return Err(Status::Invalid.into());
            }
            selected = Some(tool);
        }
    }
    let tool = selected.ok_or(Status::Unsupported)?;
    let version = c.run(sandbox, &["-p", SANDBOX, &tool.path, "--version"], &env)?;
    let version = std::str::from_utf8(&version)
        .map_err(|_| Status::Invalid)?
        .lines()
        .next()
        .ok_or(Status::Invalid)?;
    // The adapter's tested tool revision; other versions require explicit adapter validation.
    if version != "Homebrew 7.0.7" {
        return Err(Status::Unsupported.into());
    }
    let bytes = c.run(
        sandbox,
        &[
            "-p",
            SANDBOX,
            &tool.path,
            "info",
            "--json=v2",
            "--installed",
        ],
        &env,
    )?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| Status::Invalid)?;
    decode(&value, prefix, identity, kind, taps, rows)?;
    Ok(())
}
fn protected_tree(c: &Context<'_>, root: &Path) -> Result<()> {
    installation_security::protected(root).map_err(|_| Status::Denied)?;
    let mut pending = vec![root.to_owned()];
    let mut count = 0usize;
    while let Some(path) = pending.pop() {
        c.check()?;
        count += 1;
        if count > 100_000 {
            return Err(Status::Truncated.into());
        }
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.uid() != 0 || meta.mode() & 0o022 != 0 && !meta.file_type().is_symlink() {
            return Err(Status::Denied.into());
        }
        if meta.file_type().is_symlink() {
            let target = path.canonicalize()?;
            if !target.starts_with(root) {
                return Err(Status::Denied.into());
            }
            installation_security::protected(&target).map_err(|_| Status::Denied)?;
        } else if meta.is_dir() {
            installation_security::protected(&path).map_err(|_| Status::Denied)?;
            for entry in std::fs::read_dir(&path)? {
                pending.push(entry?.path());
                if pending.len() > 100_000 {
                    return Err(Status::Truncated.into());
                }
            }
        } else if !meta.is_file() {
            return Err(Status::Denied.into());
        } else {
            installation_security::protected(&path).map_err(|_| Status::Denied)?;
        }
    }
    Ok(())
}
pub(super) fn decode(
    v: &serde_json::Value,
    prefix: &str,
    owner: &str,
    kind: w::MacosBrewPackage,
    taps: &BTreeSet<String>,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let formula = kind == w::MacosBrewPackage::Formula;
    let field = if formula { "formulae" } else { "casks" };
    let packages = v
        .get(field)
        .and_then(serde_json::Value::as_array)
        .ok_or(Status::Invalid)?;
    for p in packages {
        let get = |key| {
            p.get(key)
                .and_then(serde_json::Value::as_str)
                .ok_or(Status::Missing)
        };
        let tap = get("tap")?;
        if !taps.is_empty() && !taps.contains(tap) {
            continue;
        }
        let name = get(if formula { "name" } else { "token" })?;
        let versions: Vec<&str> = if formula {
            p.get("installed")
                .and_then(serde_json::Value::as_array)
                .ok_or(Status::Invalid)?
                .iter()
                .map(|i| {
                    i.get("version")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(Status::Invalid)
                })
                .collect::<std::result::Result<_, _>>()?
        } else {
            match p.get("installed") {
                Some(serde_json::Value::String(s)) => vec![s],
                Some(serde_json::Value::Array(a)) => a
                    .iter()
                    .map(|v| v.as_str().ok_or(Status::Invalid))
                    .collect::<std::result::Result<_, _>>()?,
                Some(serde_json::Value::Null) => vec![],
                _ => return Err(Status::Invalid.into()),
            }
        };
        for version in versions {
            if name.is_empty()
                || name.contains('/')
                || version.is_empty()
                || version.contains('/')
                || name == ".."
                || version == ".."
            {
                return Err(Status::Invalid.into());
            }
            let native = format!("{tap}/{name}");
            let location = format!(
                "{prefix}/{}/{name}/{version}",
                if formula { "Cellar" } else { "Caskroom" }
            );
            let kind = if formula { "formula" } else { "cask" };
            rows.push(row(&[
                (
                    "id",
                    stable(&["homebrew", owner, prefix, tap, kind, name, version]).into(),
                ),
                ("native_id", native.into()),
                ("name", name.into()),
                ("version", version.into()),
                ("scope", "user".into()),
                ("owner", owner.into()),
                ("origin", "homebrew".into()),
                ("location", location.into()),
                ("prefix", prefix.into()),
                ("tap", tap.into()),
                ("package_kind", kind.into()),
            ]))?;
        }
    }
    Ok(())
}
