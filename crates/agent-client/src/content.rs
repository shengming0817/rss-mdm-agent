use crate::{
    store::{encode, hash, private},
    wire, Client, Clock, Error, Offer, SecretProvider,
};
use fs2::FileExt;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
/// A locked, verified local artifact. No server-selected filesystem path is used.
pub struct ContentFile {
    pub(crate) file: File,
    path: PathBuf,
    length: u64,
    digest: [u8; 32],
    key: String,
}
impl ContentFile {
    /// Protocol artifact key, independent of the local filename.
    pub fn key(&self) -> &str {
        &self.key
    }
    /// Verified private path for the trusted host (never a UI/model field).
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Read-only cloned handle; caller may consume bytes while the owning lease is alive.
    pub fn reader(&self) -> Result<File, Error> {
        self.verify()?;
        let file = open(&self.path, false)?;
        FileExt::try_lock_shared(&file).map_err(|_| Error::Storage)?;
        Ok(file)
    }
    pub(crate) fn verify(&self) -> Result<(), Error> {
        private(&self.path)?;
        if self.file.metadata()?.len() != self.length {
            return Err(Error::Untrusted);
        }
        let current = open(&self.path, false)?;
        if current.metadata()?.len() != self.length {
            return Err(Error::Untrusted);
        }
        if digest(current)? != self.digest {
            return Err(Error::Untrusted);
        }
        if digest(self.file.try_clone()?)? != self.digest {
            return Err(Error::Untrusted);
        }
        Ok(())
    }
}
/// Complete prepared materials, bound to one immutable offer and remote attempt.
pub struct Materials {
    pub(crate) files: Vec<ContentFile>,
    pub(crate) task: uuid::Uuid,
    pub(crate) attempt: uuid::Uuid,
    pub(crate) input: Vec<u8>,
}
impl Materials {
    /// All required artifacts in producer order, including software prerequisites.
    pub fn files(&self) -> &[ContentFile] {
        &self.files
    }
    pub(crate) fn validate(&self, offer: &Offer) -> Result<(), Error> {
        if self.task != offer.task_id()
            || self.attempt != offer.attempt_id()
            || self.input != encode(offer.payload())?
        {
            return Err(Error::Untrusted);
        }
        for file in &self.files {
            file.verify()?;
        }
        Ok(())
    }
}
pub(crate) fn open(path: &Path, write: bool) -> Result<File, Error> {
    private(path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(write);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Storage);
    }
    Ok(file)
}
fn digest(mut file: File) -> Result<[u8; 32], Error> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hash.finalize().into())
}
impl<S: SecretProvider, C: Clock> Client<S, C> {
    /// Stream all signed materials, validating the entire set before a Start request.
    pub async fn prepare(&mut self, offer: &Offer) -> Result<Materials, Error> {
        self.verify(&offer.signed, wire::TaskPermit::Offer)?;
        let artifacts: Vec<(String, u64, [u8; 32])> = match offer.payload() {
            wire::TaskPayload::Script(v) => {
                vec![("script".into(), v.content.length, v.content.sha256)]
            }
            wire::TaskPayload::Software(v) => v
                .steps
                .iter()
                .flat_map(|v| {
                    v.artifacts
                        .iter()
                        .map(|a| (a.key.clone(), a.length, a.sha256))
                })
                .collect(),
            _ => return Err(Error::Unsupported),
        };
        let total = artifacts.iter().try_fold(0u64, |n, (_, length, _)| {
            n.checked_add(*length).ok_or(Error::Capacity)
        })?;
        if total > self.store.cfg.limits.cache_bytes
            || artifacts
                .iter()
                .any(|(_, n, _)| *n > self.store.cfg.limits.artifact_bytes)
        {
            return Err(Error::Capacity);
        }
        let root = self.store.root.join("content");
        native_process::private_storage::directory(&root)?;
        if root.canonicalize()? != root {
            return Err(Error::Storage);
        }
        let mut files = Vec::new();
        for (key, length, sha256) in artifacts {
            self.verify(&offer.signed, wire::TaskPermit::Offer)?;
            let name = hash(&encode(&(
                self.store.cfg.origin.as_str(),
                self.store.cfg.tenant,
                sha256,
                length,
            ))?);
            self.store.conn.execute(
                "INSERT OR IGNORE INTO cache_refs VALUES(?1,?2)",
                params![offer.task_id().to_string(), name],
            )?;
            files.push(
                self.download(offer, &root, &name, key, length, sha256)
                    .await?,
            );
        }
        Ok(Materials {
            files,
            task: offer.task_id(),
            attempt: offer.attempt_id(),
            input: encode(offer.payload())?,
        })
    }
    async fn download(
        &self,
        offer: &Offer,
        root: &Path,
        name: &str,
        key: String,
        length: u64,
        sha256: [u8; 32],
    ) -> Result<ContentFile, Error> {
        let path = root.join(format!("{name}.blob"));
        if path.try_exists()? {
            let file = open(&path, false)?;
            FileExt::try_lock_shared(&file).map_err(|_| Error::Storage)?;
            if file.metadata()?.len() == length && digest(file.try_clone()?)? == sha256 {
                return Ok(ContentFile {
                    file,
                    path,
                    length,
                    digest: sha256,
                    key,
                });
            }
            return Err(Error::Untrusted);
        }
        let partial = root.join(format!("{name}.partial"));
        let mut file = if partial.try_exists()? {
            open(&partial, true)?
        } else {
            native_process::private_storage::create_new(&partial)?
        };
        file.try_lock_exclusive().map_err(|_| Error::Storage)?;
        let mut offset = file.metadata()?.len();
        if offset > length {
            file.set_len(0)?;
            offset = 0;
        }
        let used = std::fs::read_dir(root)?.try_fold(0u64, |n, e| {
            let path = e?.path();
            private(&path)?;
            let size = std::fs::symlink_metadata(&path)?.len();
            n.checked_add(size).ok_or(Error::Capacity)
        })?;
        if used
            .checked_add(length - offset)
            .is_none_or(|n| n > self.store.cfg.limits.cache_bytes)
        {
            return Err(Error::Capacity);
        }
        if offset == length && digest(open(&partial, false)?)? != sha256 {
            file.set_len(0)?;
            offset = 0;
        }
        if offset < length {
            let mut url = self.url(&format!("tasks/{}/content", offer.task_id()))?;
            url.query_pairs_mut()
                .append_pair("attempt", &offer.attempt_id().to_string());
            if key != "script" {
                url.query_pairs_mut().append_pair("artifact", &key);
            }
            let etag = format!(
                "\"{}\"",
                sha256
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
            let remaining = std::time::Duration::from_secs(
                u64::try_from(offer.payload().expires_at() - self.now()?)
                    .map_err(|_| Error::Expired)?,
            );
            if remaining.is_zero() {
                return Err(Error::Expired);
            }
            let mut request = self
                .http
                .get(url)
                .bearer_auth(self.credential()?.expose())
                .header("accept-encoding", "identity")
                .timeout(self.store.cfg.limits.transfer_timeout.min(remaining));
            if offset > 0 {
                request = request
                    .header("range", format!("bytes={offset}-"))
                    .header("if-range", &etag);
            }
            let mut response = request.send().await.map_err(|_| Error::Unavailable)?;
            if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                return Err(Error::Protocol);
            }
            response = self.checked_response(response).await?;
            let partial_response = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
            let start = if partial_response {
                offset
            } else if response.status() == reqwest::StatusCode::OK {
                0
            } else {
                return Err(Error::Protocol);
            };
            let headers = response.headers();
            if headers
                .get("content-encoding")
                .is_some_and(|v| v != "identity")
                || headers.get("etag").and_then(|v| v.to_str().ok()) != Some(etag.as_str())
                || response.content_length() != Some(length - start)
            {
                return Err(Error::Untrusted);
            }
            if partial_response {
                let expected = format!("bytes {start}-{}/{}", length - 1, length);
                if headers.get("content-range").and_then(|v| v.to_str().ok())
                    != Some(expected.as_str())
                {
                    return Err(Error::Untrusted);
                }
            }
            if start == 0 {
                file.set_len(0)?;
            }
            file.seek(SeekFrom::Start(start))?;
            let mut count = start;
            while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unavailable)? {
                count = count
                    .checked_add(chunk.len() as u64)
                    .ok_or(Error::Capacity)?;
                if count > length {
                    return Err(Error::Untrusted);
                }
                file.write_all(&chunk)?;
            }
            file.sync_all()?;
            if count != length {
                return Err(Error::Untrusted);
            }
        }
        if digest(open(&partial, false)?)? != sha256 {
            file.set_len(0)?;
            file.sync_all()?;
            return Err(Error::Untrusted);
        }
        file.sync_all()?;
        native_process::private_storage::replace(&partial, &path)?;
        FileExt::unlock(&file)?;
        // create_new is write-only; return a separately opened read handle.
        let file = open(&path, false)?;
        FileExt::try_lock_shared(&file).map_err(|_| Error::Storage)?;
        Ok(ContentFile {
            file,
            path,
            length,
            digest: sha256,
            key,
        })
    }
    /// Bounded garbage collection. Referenced or leased content is never removed.
    pub fn cleanup(&mut self, limit: usize) -> Result<usize, Error> {
        if limit == 0 || limit > 128 {
            return Err(Error::Configuration);
        }
        let root = self.store.root.join("content");
        if !root.try_exists()? {
            return Ok(0);
        }
        private(&root)?;
        let mut removed = 0;
        for entry in std::fs::read_dir(&root)? {
            if removed == limit {
                break;
            }
            let path = entry?.path();
            let name = path
                .file_stem()
                .and_then(|v| v.to_str())
                .ok_or(Error::Storage)?;
            if name.len() != 64
                || !name.bytes().all(|v| v.is_ascii_hexdigit())
                || !matches!(
                    path.extension().and_then(|v| v.to_str()),
                    Some("blob" | "partial")
                )
            {
                return Err(Error::Storage);
            }
            let referenced: bool = self.store.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM cache_refs WHERE name=?1)",
                [name],
                |r| r.get(0),
            )?;
            if referenced {
                continue;
            }
            let file = open(&path, false)?;
            if file.try_lock_exclusive().is_err() {
                continue;
            }
            std::fs::remove_file(&path)?;
            removed += 1;
        }
        Ok(removed)
    }
}
