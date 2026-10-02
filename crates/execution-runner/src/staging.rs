//! Immutable execution materials retained independently of transport-cache acknowledgements.
use execution_app::Error;
use execution_contract::Digest;
use sha2::{Digest as _, Sha256};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Create a new administrator-owned material root, or validate the existing protected root.
/// This contains only execution artifacts, never device credentials or a second execution ledger.
pub fn initialize(root: &Path) -> Result<(), Error> {
    require_system()?;
    let parent = root.parent().ok_or(Error::Configuration)?;
    installation_security::protected(parent).map_err(|_| Error::Denied)?;
    if !root.exists() {
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::fs::DirBuilderExt;
            use std::os::unix::fs::PermissionsExt;
            std::fs::DirBuilder::new()
                .mode(0o755)
                .create(root)
                .map_err(|_| Error::Storage)?;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                .map_err(|_| Error::Storage)?;
        }
        #[cfg(windows)]
        {
            std::fs::create_dir(root).map_err(|_| Error::Storage)?;
            crate::platform::grant_read(root, "S-1-5-11")?;
        }
    }
    let _ = crate::platform::PathLease::source(root, true)?;
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(root)
            .map_err(|_| Error::Storage)?
            .permissions()
            .mode()
            & 0o777
            != 0o755
        {
            return Err(Error::Configuration);
        }
    }
    Ok(())
}
fn require_system() -> Result<(), Error> {
    let expected = if cfg!(windows) { "S-1-5-18" } else { "0" };
    if crate::host::current_subject()? != expected {
        return Err(Error::Denied);
    }
    Ok(())
}
/// Publish exact bytes into an immutable, content-addressed namespace. Existing content is
/// verified and never replaced; a user receives only read access to their own artifact namespace.
pub fn publish(
    root: &Path,
    subject: Option<&str>,
    mut source: impl Read,
    expected: &Digest,
    length: u64,
) -> Result<PathBuf, Error> {
    require_system()?;
    if length > 8 * 1024 * 1024 * 1024 {
        return Err(Error::Capacity);
    }
    let _root = crate::platform::PathLease::source(root, true)?;
    let key = subject.map_or_else(
        || "system".into(),
        |subject| format!("{:x}", Sha256::digest(subject.as_bytes())),
    );
    let directory = root.join(key);
    if !directory.exists() {
        native_process::private_storage::directory(&directory).map_err(|_| Error::Storage)?;
    }
    let _directory = crate::platform::PathLease::source(&directory, true)?;
    if let Some(subject) = subject {
        crate::platform::grant_read(&directory, subject)?;
    }
    let path = directory.join(expected.as_str());
    if path.exists() {
        verify(&path, expected, length)?;
        if let Some(subject) = subject {
            crate::platform::grant_read(&path, subject)?;
        }
        return Ok(path);
    }
    quota(root, length)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Clock)?
        .as_nanos();
    let temporary = directory.join(format!(".pending-{}-{nonce}", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut output = options.open(&temporary).map_err(|_| Error::Storage)?;
    let result = (|| {
        let mut digest = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0; 65536];
        loop {
            let n = source.read(&mut buffer).map_err(|_| Error::Storage)?;
            if n == 0 {
                break;
            }
            size = size.checked_add(n as u64).ok_or(Error::Capacity)?;
            if size > length {
                return Err(Error::Denied);
            }
            digest.update(&buffer[..n]);
            output.write_all(&buffer[..n]).map_err(|_| Error::Storage)?;
        }
        if size != length || format!("{:x}", digest.finalize()) != expected.as_str() {
            return Err(Error::Denied);
        }
        output.sync_all().map_err(|_| Error::Storage)?;
        drop(output);
        if let Some(subject) = subject {
            crate::platform::grant_read(&temporary, subject)?;
        }
        std::fs::hard_link(&temporary, &path).map_err(|_| Error::Conflict)?;
        #[cfg(target_os = "macos")]
        std::fs::File::open(&directory)
            .and_then(|file| file.sync_all())
            .map_err(|_| Error::Storage)?;
        verify(&path, expected, length)?;
        Ok(path)
    })();
    // Only the exact newly created staging name is removed; published data is never reset.
    let _ = std::fs::remove_file(&temporary);
    result
}
fn quota(root: &Path, additional: u64) -> Result<(), Error> {
    let mut pending = vec![root.to_owned()];
    let mut count = 0usize;
    let mut bytes = additional;
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).map_err(|_| Error::Storage)? {
            let entry = entry.map_err(|_| Error::Storage)?;
            count += 1;
            if count > 32_768 {
                return Err(Error::Capacity);
            }
            let metadata = entry
                .path()
                .symlink_metadata()
                .map_err(|_| Error::Storage)?;
            if metadata.is_symlink() {
                return Err(Error::Denied);
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                bytes = bytes.checked_add(metadata.len()).ok_or(Error::Capacity)?;
            } else {
                return Err(Error::Denied);
            }
            if bytes > 8 * 1024 * 1024 * 1024 {
                return Err(Error::Capacity);
            }
        }
    }
    Ok(())
}
/// Expand an exact V4 Bundle into a protected content-addressed tree. Members retain their
/// relative layout; no path, link, undeclared member or existing file is overwritten.
pub fn bundle(
    root: &Path,
    subject: Option<&str>,
    archive_path: &Path,
    digest: &Digest,
    manifest: &execution_contract::BundleManifest,
) -> Result<std::collections::BTreeMap<String, PathBuf>, Error> {
    use std::collections::{BTreeMap, BTreeSet};
    require_system()?;
    if manifest.entries.is_empty() || manifest.entries.len() > 4096 {
        return Err(Error::Capacity);
    }
    let (archive, _lease) = crate::materialize::verify_material(archive_path, digest)?;
    let mut archive = zip::ZipArchive::new(archive).map_err(|_| Error::InvalidInput)?;
    if archive.len() != manifest.entries.len() + 1 {
        return Err(Error::Denied);
    }
    let mut names = BTreeSet::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|_| Error::InvalidInput)?;
        let name = file.name();
        if !file.is_file()
            || file.is_symlink()
            || file.encrypted()
            || file.size() > 1024 * 1024 * 1024
            || file
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 != 0 && mode & 0o170000 != 0o100000)
            || !portable_member(name)
            || !names.insert(name.to_ascii_lowercase())
        {
            return Err(Error::Denied);
        }
    }
    let mut declared = manifest
        .entries
        .keys()
        .map(|s| s.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    if declared.len() != manifest.entries.len()
        || !declared.insert("manifest.json".into())
        || declared != names
    {
        return Err(Error::Denied);
    }
    let mut bytes = Vec::new();
    archive
        .by_name("manifest.json")
        .map_err(|_| Error::InvalidInput)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Storage)?;
    if bytes.len() > 1024 * 1024
        || serde_json::from_slice::<execution_contract::BundleManifest>(&bytes)
            .map_err(|_| Error::InvalidInput)?
            != *manifest
    {
        return Err(Error::Denied);
    }
    let namespace = subject.map_or_else(
        || "system".into(),
        |s| format!("{:x}", Sha256::digest(s.as_bytes())),
    );
    let parent = root.join(namespace);
    if !parent.exists() {
        native_process::private_storage::directory(&parent).map_err(|_| Error::Storage)?;
    }
    let _parent = crate::platform::PathLease::source(&parent, true)?;
    if let Some(subject) = subject {
        crate::platform::grant_read(&parent, subject)?;
    }
    let tree = parent.join(format!("bundle-{}", digest.as_str()));
    if !tree.exists() {
        native_process::private_storage::directory(&tree).map_err(|_| Error::Storage)?;
    }
    let _tree = crate::platform::PathLease::source(&tree, true)?;
    if let Some(subject) = subject {
        crate::platform::grant_read(&tree, subject)?;
    }
    let mut result = BTreeMap::new();
    let mut total = 0u64;
    for (name, member) in &manifest.entries {
        if !portable_member(name) {
            return Err(Error::Denied);
        }
        total = total.checked_add(member.length).ok_or(Error::Capacity)?;
        if member.length > 1024 * 1024 * 1024 || total > 8 * 1024 * 1024 * 1024 {
            return Err(Error::Capacity);
        }
        let mut file = archive.by_name(name).map_err(|_| Error::InvalidInput)?;
        if file.size() != member.length {
            return Err(Error::Denied);
        }
        let expected = Digest::new(
            member
                .sha256
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        )
        .map_err(|_| Error::InvalidInput)?;
        let published = publish(root, subject, &mut file, &expected, member.length)?;
        let path = tree.join(name);
        let mut directory = tree.clone();
        let components: Vec<_> = name.split('/').collect();
        for part in &components[..components.len() - 1] {
            directory.push(part);
            if !directory.exists() {
                native_process::private_storage::directory(&directory)
                    .map_err(|_| Error::Storage)?;
            }
            let _ = crate::platform::PathLease::source(&directory, true)?;
            if let Some(subject) = subject {
                crate::platform::grant_read(&directory, subject)?;
            }
        }
        if !path.exists() {
            std::fs::hard_link(&published, &path).map_err(|_| Error::Storage)?;
        }
        verify(&path, &expected, member.length)?;
        result.insert(name.clone(), path);
    }
    Ok(result)
}
fn portable_member(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 1024
        && !name.contains(['\\', ':'])
        && !name.chars().any(char::is_control)
        && name.split('/').count() <= 32
        && name.split('/').all(|part| {
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        })
}
/// Retain the native export filename (notably Homebrew's formula class lookup) under
/// the exact payload digest. No mutable source name or default-source search is created.
pub fn native_export(
    source: &Path,
    expected: &Digest,
    name: &str,
    subject: Option<&str>,
) -> Result<PathBuf, Error> {
    require_system()?;
    if !portable_member(name) || name.contains('/') {
        return Err(Error::InvalidInput);
    }
    let (_, _lease) = crate::materialize::verify_material(source, expected)?;
    let parent = source.parent().ok_or(Error::Configuration)?;
    let directory = parent.join(format!("export-{}", expected.as_str()));
    if !directory.exists() {
        native_process::private_storage::directory(&directory).map_err(|_| Error::Storage)?;
    }
    let _directory = crate::platform::PathLease::source(&directory, true)?;
    if let Some(subject) = subject {
        crate::platform::grant_read(&directory, subject)?;
    }
    let named = directory.join(name);
    if !named.exists() {
        std::fs::hard_link(source, &named).map_err(|_| Error::Storage)?;
    }
    crate::materialize::verify_material(&named, expected)?;
    Ok(named)
}
fn verify(path: &Path, expected: &Digest, length: u64) -> Result<(), Error> {
    let _lease = crate::platform::PathLease::source(path, true)?;
    let mut file = crate::platform::open_file(path)?;
    if file.metadata().map_err(|_| Error::Storage)?.len() != length {
        return Err(Error::Denied);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|_| Error::Storage)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    if format!("{:x}", digest.finalize()) != expected.as_str() {
        return Err(Error::Denied);
    }
    Ok(())
}
