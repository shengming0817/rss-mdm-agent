//! Candidate integrity, using scripts/ai-host-artifacts.mjs canonical tree format.
//! ref: constant_time_eq 0.4.2 src/lib.rs; sha2 0.10 Digest incremental API.
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Component, Path},
};

pub(super) fn digest(root: &Path) -> std::io::Result<String> {
    fn entry(hash: &mut Sha256, root: &Path, name: &Path) -> std::io::Result<()> {
        let path = root.join(name);
        let metadata = std::fs::symlink_metadata(&path)?;
        let name_text = name.to_str().ok_or_else(invalid)?;
        if metadata.is_symlink() {
            let target = std::fs::read_link(&path)?;
            let mut depth = 0usize;
            for component in name
                .parent()
                .unwrap_or(Path::new(""))
                .join(&target)
                .components()
            {
                match component {
                    Component::Normal(_) => depth += 1,
                    Component::ParentDir if depth > 0 => depth -= 1,
                    Component::CurDir => (),
                    _ => return Err(invalid()),
                }
            }
            let target = target.to_str().ok_or_else(invalid)?;
            hash.update(format!("link\0{name_text}\0{target}\0"));
        } else if metadata.is_dir() {
            hash.update(format!("directory\0{name_text}\0"));
            let mut children = std::fs::read_dir(&path)?
                .map(|e| e.map(|e| e.file_name()))
                .collect::<Result<Vec<_>, _>>()?;
            // JS Array.sort compares UTF-16 code units. Reject non-UTF-8 names.
            if children.iter().any(|n| n.to_str().is_none()) {
                return Err(invalid());
            }
            children.sort_by_cached_key(|n| n.to_str().unwrap().encode_utf16().collect::<Vec<_>>());
            for child in children {
                entry(hash, root, &name.join(child))?;
            }
        } else if metadata.is_file() {
            hash.update(format!(
                "file\0{name_text}\0{}\0{}\0",
                metadata.permissions().mode() & 0o777,
                metadata.len()
            ));
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)?;
            if !file.metadata()?.is_file() {
                return Err(invalid());
            }
            let mut buffer = [0; 65536];
            loop {
                let size = file.read(&mut buffer)?;
                if size == 0 {
                    break;
                }
                hash.update(&buffer[..size]);
            }
        } else {
            return Err(invalid());
        }
        Ok(())
    }
    let mut hash = Sha256::new();
    for name in [
        "bin",
        "node_modules",
        "NODE-LICENSE",
        "package.json",
        "pnpm-lock.yaml",
    ] {
        entry(&mut hash, root, Path::new(name))?;
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn invalid() -> std::io::Error {
    std::io::Error::other("invalid runtime tree")
}

pub(super) fn verify(root: &Path, manifest: &str, trusted: Option<&str>) -> std::io::Result<()> {
    if manifest.len() != 64
        || !manifest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    if trusted.is_some_and(|expected| {
        !constant_time_eq::constant_time_eq(expected.as_bytes(), manifest.as_bytes())
    }) {
        return Err(invalid());
    }
    if !constant_time_eq::constant_time_eq(digest(root)?.as_bytes(), manifest.as_bytes()) {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_digest_matches_packager_and_rejects_bytes_modes_links_and_forged_metadata() {
        let root = std::env::temp_dir().join(format!("rss-tree-{}", uuid::Uuid::new_v4()));
        for dir in ["bin", "node_modules"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for name in [
            "bin/node",
            "node_modules/data",
            "node_modules/\u{e000}",
            "node_modules/\u{10000}",
            "NODE-LICENSE",
            "package.json",
            "pnpm-lock.yaml",
        ] {
            std::fs::write(root.join(name), name).unwrap();
            std::fs::set_permissions(root.join(name), std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        std::os::unix::fs::symlink("../node_modules/data", root.join("bin/link")).unwrap();
        let expected = digest(&root).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap();
        let node = std::process::Command::new("node").args(["--input-type=module", "-e", "import{pathToFileURL}from'node:url';const m=await import(pathToFileURL(process.argv[1]).href);process.stdout.write(m.runtimeTreeSha256(process.argv[2]));"])
            .arg(repository.join("scripts/ai-host-artifacts.mjs")).arg(&root).output().unwrap();
        assert!(node.status.success());
        assert_eq!(String::from_utf8(node.stdout).unwrap(), expected);
        verify(&root, &expected, Some(&expected)).unwrap();
        std::fs::write(root.join("bin/node"), b"tampered").unwrap();
        assert!(verify(&root, &expected, Some(&expected)).is_err());
        assert!(
            verify(&root, &digest(&root).unwrap(), Some(&expected)).is_err(),
            "rewritten manifest cannot replace the compiled candidate"
        );
        std::fs::write(root.join("bin/node"), b"bin/node").unwrap();
        std::fs::set_permissions(
            root.join("bin/node"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        assert!(verify(&root, &expected, Some(&expected)).is_err());
        std::fs::set_permissions(
            root.join("bin/node"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        std::fs::remove_file(root.join("bin/link")).unwrap();
        std::os::unix::fs::symlink("../package.json", root.join("bin/link")).unwrap();
        assert!(verify(&root, &expected, Some(&expected)).is_err());
        std::fs::remove_file(root.join("bin/link")).unwrap();
        std::os::unix::fs::symlink("../../escape", root.join("bin/link")).unwrap();
        assert!(digest(&root).is_err());
        std::fs::remove_file(root.join("bin/link")).unwrap();
        let fifo = root.join("bin/fifo");
        assert!(std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        assert!(digest(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
