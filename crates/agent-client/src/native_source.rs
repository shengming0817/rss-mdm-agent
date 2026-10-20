//! Standard WinGet documents and bounded Git object inspection; no source configuration or install.
//! ref: git/git v2.51.0 Documentation/gitprotocol-pack.adoc (upload-pack without sideband).
use crate::{wire, Error};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
mod pack_budget;

/// Bound on one frozen Git pack, independently of the decoded document budget.
pub const MAX_PACK_BYTES: usize = 16 * 1024 * 1024;
/// Bound on the canonical native document or path/byte inventory.
pub const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

/// OS credential key for one exact target/reference pair, isolated from device bootstrap keys.
pub fn credential_key(binding: &wire::SoftwareReadCredential) -> Result<String, Error> {
    let bytes = serde_json::to_vec(&(binding.source.as_str(), binding.reference.as_str()))
        .map_err(|_| Error::Configuration)?;
    Ok(format!("software-read-{:x}", Sha256::digest(bytes)))
}

/// Check a provisioned read token without accepting it as device or execution authority.
pub fn validate_read_token(value: &str) -> Result<(), Error> {
    if !(32..=512).contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(Error::Identity);
    }
    Ok(())
}

/// Verify the REST document before handing it to a package manager.
/// The caller authorizes the URL and verifies the separate information endpoint's identifier.
pub fn verify_winget(
    bytes: &[u8],
    binding: &wire::SoftwareExportBinding,
    action: &wire::SoftwareTaskAction,
) -> Result<(), Error> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::Capacity);
    }
    let document: Value = serde_json::from_slice(bytes).map_err(|_| Error::Protocol)?;
    if wire::winget_document_digest(&document)? != binding.document_sha256
        || document["PackageIdentifier"].as_str() != Some(action.package.as_str())
    {
        return Err(Error::Untrusted);
    }
    let versions = document["Versions"].as_array().ok_or(Error::Protocol)?;
    if versions.len() != 1 || versions[0]["PackageVersion"].as_str() != Some(&action.version) {
        return Err(Error::Untrusted);
    }
    let installers = versions[0]["Installers"]
        .as_array()
        .ok_or(Error::Protocol)?;
    if installers.is_empty()
        || installers.iter().any(|i| {
            !binding.artifacts.iter().any(|a| {
                i["InstallerUrl"].as_str() == Some(&a.url)
                    && i["InstallerSha256"]
                        .as_str()
                        .is_some_and(|d| d.eq_ignore_ascii_case(&hex(&a.sha256)))
            })
        })
    {
        return Err(Error::Untrusted);
    }
    Ok(())
}

/// Build the narrow standard upload-pack request for the exact signed commit.
pub(crate) fn pack_request(commit: &str) -> Result<Vec<u8>, Error> {
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Protocol);
    }
    let want = format!("want {commit} ofs-delta\n");
    Ok(format!("{:04x}{want}00000009done\n", want.len() + 4).into_bytes())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Run one local Git plumbing command, with hooks, config inheritance and network protocols disabled.
/// Cancellation/timeout kills the child; every pipe is independently bounded and stderr is discarded.
async fn git(
    directory: &Path,
    arguments: &[&str],
    input: &[u8],
    limit: usize,
    account: Option<(&std::sync::atomic::AtomicU64, u64)>,
) -> Result<Vec<u8>, Error> {
    let mut child = tokio::process::Command::new("/usr/bin/git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.allow=never",
        ])
        .args(arguments)
        .current_dir(directory)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_ALLOC_LIMIT", "33554432")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| Error::Unavailable)?;
    let mut stdin = child.stdin.take().ok_or(Error::Unavailable)?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or(Error::Unavailable)?
        .take(limit as u64 + 1);
    let mut stderr = child.stderr.take().ok_or(Error::Unavailable)?.take(4097);
    let operation = async {
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let (_, _, _, status) = tokio::try_join!(
            async {
                stdin.write_all(input).await?;
                stdin.shutdown().await
            },
            stdout.read_to_end(&mut output),
            stderr.read_to_end(&mut errors),
            child.wait(),
        )
        .map_err(|_| Error::Untrusted)?;
        if let Some((counter, maximum)) = account {
            let count = (output.len() + errors.len()) as u64;
            let before = counter.fetch_add(count, std::sync::atomic::Ordering::AcqRel);
            if before.saturating_add(count) > maximum {
                return Err(Error::Capacity);
            }
        }
        if output.len() > limit || errors.len() > 4096 {
            return Err(Error::Capacity);
        }
        if !status.success() {
            return Err(Error::Untrusted);
        }
        Ok(output)
    };
    match tokio::time::timeout(Duration::from_secs(10), operation).await {
        Ok(Ok(bytes)) => Ok(bytes),
        result => {
            let _ = child.kill().await;
            match result {
                Ok(Err(error)) => Err(error),
                _ => Err(Error::Unavailable),
            }
        }
    }
}

/// Import a bounded pack into an account-private repository, checking the exact parentless commit
/// and its complete regular Ruby document inventory before any checkout or Ruby evaluation.
pub async fn import_brew_pack(
    repository: &Path,
    pack: &[u8],
    commit: &str,
    expected: &[u8; 32],
    account: Option<(&std::sync::atomic::AtomicU64, u64)>,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    pack_request(commit)?;
    if pack.len() > MAX_PACK_BYTES || !pack.starts_with(b"PACK") {
        return Err(Error::Capacity);
    }
    pack_budget::check(pack)?;
    git(repository, &["init", "--bare", "."], &[], 4096, account).await?;
    git(
        repository,
        &[
            "index-pack",
            "--stdin",
            "--strict",
            "--threads=1",
            "--max-input-size=16777216",
        ],
        pack,
        4096,
        account,
    )
    .await?;
    let lineage = git(
        repository,
        &["rev-list", "--parents", "-n", "1", commit],
        &[],
        128,
        account,
    )
    .await?;
    if lineage != format!("{commit}\n").as_bytes() {
        return Err(Error::Untrusted);
    }
    let tree = git(
        repository,
        &["ls-tree", "-rlz", "--full-tree", commit],
        &[],
        128 * 1024,
        account,
    )
    .await?;
    let mut documents: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total = 0usize;
    for record in tree.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        if documents.len() >= 256 {
            return Err(Error::Capacity);
        }
        let record = std::str::from_utf8(record).map_err(|_| Error::Protocol)?;
        let (metadata, path) = record.split_once('\t').ok_or(Error::Protocol)?;
        let fields = metadata.split_whitespace().collect::<Vec<_>>();
        let (folder, leaf) = path.split_once('/').ok_or(Error::Protocol)?;
        if fields.len() != 4
            || fields[0] != "100644"
            || fields[1] != "blob"
            || !matches!(folder, "Formula" | "Casks")
            || !leaf.ends_with(".rb")
            || leaf.len() > 128
            || !leaf
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"@+_.-".contains(&b))
        {
            return Err(Error::Untrusted);
        }
        let length = fields[3].parse::<usize>().map_err(|_| Error::Protocol)?;
        total = total.checked_add(length).ok_or(Error::Capacity)?;
        if total > MAX_DOCUMENT_BYTES {
            return Err(Error::Capacity);
        }
        let bytes = git(
            repository,
            &["show", &format!("{commit}:{path}")],
            &[],
            length,
            account,
        )
        .await?;
        if bytes.len() != length {
            return Err(Error::Untrusted);
        }
        documents.push((path.into(), bytes));
    }
    if wire::brew_documents_digest(documents.iter().map(|(p, b)| (p.as_str(), b.as_slice())))?
        != *expected
    {
        return Err(Error::Untrusted);
    }
    Ok(documents)
}
