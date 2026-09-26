use execution_app::Error;
use execution_contract::*;
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

pub(super) struct Expanded {
    pub root: PathBuf,
    pub manifest: BundleManifest,
    tree: Option<super::tree::Tree>,
}
impl Drop for Expanded {
    fn drop(&mut self) {
        if let Some(tree) = self.tree.take() {
            tree.cleanup();
        }
    }
}
fn relative(value: &str, depth: u32) -> Result<String, Error> {
    if value.is_empty()
        || value.len() > 1024
        || value.contains(['\\', ':'])
        || value.chars().any(char::is_control)
    {
        return Err(Error::Denied);
    }
    let parts: Vec<_> = value.split('/').collect();
    if parts.len() > depth as usize
        || parts.iter().any(|p| {
            let stem = p.split('.').next().unwrap_or("").to_ascii_uppercase();
            p.is_empty()
                || *p == "."
                || *p == ".."
                || p.ends_with(['.', ' '])
                || ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
                || (stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        })
    {
        return Err(Error::Denied);
    }
    Ok(value.to_ascii_lowercase())
}
// ref: zip-rs zip2 src/read.rs@771dfc534d2614158af5497ea3dff4d4208d7db1.
// Do not use extract(): it permits links and overwrites existing paths.
pub(super) fn extract(
    file: File,
    spec: &SoftwareSpec,
    root: PathBuf,
    control: &super::PreparationControl,
) -> Result<Expanded, Error> {
    control.check()?;
    let limits = spec.bundle.ok_or(Error::InvalidInput)?;
    if file.metadata().map_err(|_| Error::Unavailable)?.len() > limits.archive_bytes {
        return Err(Error::Capacity);
    }
    let mut archive = zip::ZipArchive::new(file).map_err(|_| Error::InvalidInput)?;
    if archive.len() > limits.files as usize + 1 {
        return Err(Error::Capacity);
    }
    let mut names = BTreeSet::new();
    for i in 0..archive.len() {
        control.check()?;
        let entry = archive.by_index(i).map_err(|_| Error::InvalidInput)?;
        if !entry.is_file()
            || entry.encrypted()
            || entry.is_symlink()
            || entry
                .unix_mode()
                .is_some_and(|m| m & 0o170000 != 0 && m & 0o170000 != 0o100000)
            || entry.size() > limits.file_bytes
            || !names.insert(relative(entry.name(), limits.depth)?)
        {
            return Err(Error::Denied);
        }
    }
    let manifest: BundleManifest = {
        let mut entry = archive
            .by_name("manifest.json")
            .map_err(|_| Error::InvalidInput)?;
        if entry.size() > 1024 * 1024 {
            return Err(Error::Capacity);
        }
        let mut bytes = Vec::new();
        entry
            .by_ref()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::InvalidInput)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Error::Capacity);
        }
        serde_json::from_slice(&bytes).map_err(|_| Error::InvalidInput)?
    };
    let version = match &spec.desired {
        DesiredState::Present { version, .. } => Some(version),
        DesiredState::Absent => None,
    };
    let install = if spec.adapter.platform() == Platform::Windows {
        "install.ps1"
    } else {
        "install.sh"
    };
    if manifest.package != spec.package
        || version.is_some_and(|v| v != &manifest.version)
        || manifest.detection != spec.detection
        || manifest.install != install
        || manifest.files.len() + 1 != archive.len()
    {
        return Err(Error::Denied);
    }
    let mut expected = BTreeSet::new();
    expected.insert("manifest.json".to_owned());
    for f in &manifest.files {
        if !expected.insert(relative(&f.path, limits.depth)?) {
            return Err(Error::Denied);
        }
    }
    if expected != names
        || !manifest.files.iter().any(|f| f.path == manifest.install)
        || manifest
            .uninstall
            .as_ref()
            .is_some_and(|p| !manifest.files.iter().any(|f| &f.path == p))
    {
        return Err(Error::Denied);
    }
    std::fs::create_dir(&root).map_err(|_| Error::Conflict)?;
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| Error::Unavailable)?;
    }
    let tree = super::tree::Tree::open(&root)?;
    let mut expanded = Expanded {
        root,
        manifest,
        tree: Some(tree),
    };
    let mut total = 0u64;
    for declared in &expanded.manifest.files {
        let mut entry = archive
            .by_name(&declared.path)
            .map_err(|_| Error::InvalidInput)?;
        let mut out = expanded.tree.as_mut().expect("tree").file(&declared.path)?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            control.check()?;
            let n = entry.read(&mut buffer).map_err(|_| Error::InvalidInput)?;
            if n == 0 {
                break;
            }
            size = size.checked_add(n as u64).ok_or(Error::Capacity)?;
            total = total.checked_add(n as u64).ok_or(Error::Capacity)?;
            if size > limits.file_bytes || total > limits.expanded_bytes {
                return Err(Error::Capacity);
            }
            hash.update(&buffer[..n]);
            out.write_all(&buffer[..n])
                .map_err(|_| Error::Unavailable)?;
        }
        if format!("{:x}", hash.finalize()) != declared.sha256.as_str() {
            return Err(Error::Denied);
        }
        out.sync_all().map_err(|_| Error::Unavailable)?;
    }
    if !expanded.tree.as_ref().expect("tree").intact() {
        return Err(Error::Denied);
    }
    Ok(expanded)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_portable_path_attacks() {
        for p in [
            "/root",
            "../x",
            "a/../x",
            "C:/x",
            "a\\b",
            "a//b",
            "NUL.txt",
            "foo.",
            "a/b/../../c",
            "COM1",
        ] {
            assert!(relative(p, 32).is_err(), "{p}");
        }
        assert_eq!(relative("a/b.txt", 32).unwrap(), "a/b.txt");
        assert!(relative("a/b/c", 2).is_err());
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn spec() -> SoftwareSpec {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../../../execution-contract/tests/fixtures/software.json"
        ))
        .unwrap();
        let mut s: SoftwareSpec =
            serde_json::from_value(v["execution"]["software"].clone()).unwrap();
        s.adapter = SoftwareKind::MacosBundle;
        s.bundle = Some(BundleLimits {
            archive_bytes: 1024 * 1024,
            files: 8,
            file_bytes: 8192,
            expanded_bytes: 16384,
            depth: 8,
        });
        s
    }
    fn exercise(attack: &str) -> bool {
        let mut s = spec();
        let base = std::env::temp_dir().join(format!(
            "rss-zip-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&base).unwrap();
        let archive = base.join("input.zip");
        let data = b"#!/bin/sh\nexit 0\n";
        let mut manifest = BundleManifest {
            package: s.package.clone(),
            version: PackageValue::new("1.0").unwrap(),
            files: vec![BundleFile {
                path: "install.sh".into(),
                sha256: Digest::new(format!("{:x}", Sha256::digest(data))).unwrap(),
            }],
            install: "install.sh".into(),
            uninstall: None,
            detection: s.detection.clone(),
        };
        let path = match attack {
            "traversal" => "../outside",
            "absolute" => "/outside",
            "drive" => "C:/outside",
            _ => "install.sh",
        };
        if attack == "hash" {
            manifest.files[0].sha256 = Digest::new("ff".repeat(32)).unwrap();
        }
        if attack == "manifest" {
            manifest.version = PackageValue::new("wrong").unwrap();
        }
        if attack == "file-limit" {
            s.bundle.as_mut().unwrap().file_bytes = 8;
        }
        if attack == "archive-limit" {
            s.bundle.as_mut().unwrap().archive_bytes = 8;
        }
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        if attack == "link" {
            zip.add_symlink(path, "../outside", options).unwrap();
        } else {
            zip.start_file(path, options).unwrap();
            zip.write_all(data).unwrap();
        }
        if attack == "extra" || attack == "collision" {
            zip.start_file(
                if attack == "extra" {
                    "extra.txt"
                } else {
                    "INSTALL.SH"
                },
                options,
            )
            .unwrap();
            zip.write_all(b"unexpected").unwrap();
        }
        zip.finish().unwrap();
        let root = base.join("staging");
        let result = extract(
            File::open(&archive).unwrap(),
            &s,
            root.clone(),
            &super::super::PreparationControl::test(),
        );
        let ok = result.is_ok();
        drop(result);
        assert!(!root.exists(), "staging leaked for {attack}");
        assert!(!base.join("outside").exists());
        std::fs::remove_dir_all(base).unwrap();
        ok
    }
    #[test]
    fn exact_manifest_extracts_and_cleans_staging() {
        assert!(exercise("valid"));
    }
    #[test]
    fn malicious_archives_never_escape_or_leave_partial_staging() {
        for attack in [
            "traversal",
            "absolute",
            "drive",
            "hash",
            "manifest",
            "file-limit",
            "archive-limit",
            "link",
            "extra",
            "collision",
        ] {
            assert!(!exercise(attack), "accepted {attack}");
        }
    }
}
