use crate::{
    store::{encode, hash, private},
    wire, Client, Clock, Error, IdentityProvider, Offer,
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
    pub(crate) path: PathBuf,
    pub(crate) length: u64,
    pub(crate) digest: [u8; 32],
    pub(crate) key: String,
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
    /// The verified frozen native document or Git pack for one signed software step.
    /// The file remains leased with the complete original offer's material set.
    pub fn native_source(&self, index: usize) -> Result<&ContentFile, Error> {
        self.files
            .iter()
            .find(|f| f.key == format!("native/{index}"))
            .ok_or(Error::Untrusted)
    }
    /// All required artifacts in producer order, including software prerequisites.
    pub fn files(&self) -> &[ContentFile] {
        &self.files
    }
    /// Reverify all materials against one exact original Offer.
    pub fn validate_offer(&self, offer: &Offer) -> Result<(), Error> {
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
    /// Reverify the original material payload against a verified Start without exposing fields.
    pub fn validate_start(&self, start: &crate::Start) -> Result<(), Error> {
        let original: wire::TaskPayload = crate::store::decode(&self.input)?;
        if self.task != start.payload().task_id()
            || self.attempt != start.payload().attempt_id()
            || !crate::client::same_input(&original, start.payload())
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
pub(crate) fn digest(file: File) -> Result<[u8; 32], Error> {
    digest_handle(&file)
}
// Windows byte-range locks also exclude another handle in the same process.
fn digest_handle(mut file: &File) -> Result<[u8; 32], Error> {
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
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    pub(crate) fn recover_materials(&self, offer: &Offer) -> Result<Materials, Error> {
        self.active()?;
        self.now()?;
        let artifacts = artifacts(offer.payload())?;
        let root = self.store.root.join("content");
        crate::store::private_root(&root)?;
        let mut total = 0u64;
        let mut files = Vec::new();
        for (key, length, sha256) in artifacts {
            total = total.checked_add(length).ok_or(Error::Capacity)?;
            if length > self.store.cfg.limits.artifact_bytes
                || total > self.store.cfg.limits.cache_bytes
            {
                return Err(Error::Capacity);
            }
            let name = hash(&encode(&(
                self.store.cfg.service_id,
                self.store.cfg.tenant,
                sha256,
                length,
            ))?);
            let path = root.join(format!("{name}.blob"));
            let file = open(&path, false)?;
            FileExt::try_lock_shared(&file).map_err(|_| Error::Storage)?;
            let content = ContentFile {
                file,
                path,
                length,
                digest: sha256,
                key,
            };
            content.verify()?;
            files.push(content);
        }
        self.recover_native_sources(offer, &root, &mut files)?;
        Ok(Materials {
            files,
            task: offer.task_id(),
            attempt: offer.attempt_id(),
            input: encode(offer.payload())?,
        })
    }
    /// Stream all signed materials, validating the entire set before a Start request.
    pub async fn prepare(&mut self, offer: &Offer) -> Result<Materials, Error> {
        self.verify(&offer.signed, wire::TaskPermit::Offer)?;
        for task in self.pending_tasks(self.configuration().limits.pending_tasks)? {
            let Ok(peer) = self.offer(task) else { continue };
            for (key, length, sha256) in artifacts(peer.payload())? {
                let name = hash(&encode(&(
                    self.store.cfg.service_id,
                    self.store.cfg.tenant,
                    sha256,
                    length,
                ))?);
                self.transfers.attach(
                    peer.task_id(),
                    peer.attempt_id(),
                    &key,
                    &name,
                    Some(length),
                    true,
                )?;
                self.store.conn.execute(
                    "INSERT OR IGNORE INTO cache_refs VALUES(?1,?2)",
                    params![peer.task_id().to_string(), name],
                )?;
            }
        }
        let artifacts = artifacts(offer.payload())?;
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
        platform_private_storage::directory(&root)?;
        crate::store::private_root(&root)?;
        let mut files = Vec::new();
        for (key, length, sha256) in artifacts {
            self.verify(&offer.signed, wire::TaskPermit::Offer)?;
            let name = hash(&encode(&(
                self.store.cfg.service_id,
                self.store.cfg.tenant,
                sha256,
                length,
            ))?);
            self.transfers.attach(
                offer.task_id(),
                offer.attempt_id(),
                &key,
                &name,
                Some(length),
                true,
            )?;
            self.store.conn.execute(
                "INSERT OR IGNORE INTO cache_refs VALUES(?1,?2)",
                params![offer.task_id().to_string(), name],
            )?;
            files.push(
                self.download(offer, &root, &name, key, length, sha256)
                    .await?,
            );
        }
        self.prepare_native_sources(offer, &root, &mut files)
            .await?;
        if !self
            .transfers
            .consumer_ready(offer.task_id(), offer.attempt_id())?
        {
            return Err(Error::TransferPaused);
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
        if let Some(content) = cached_content(&path, length, sha256, &key)? {
            self.transfers.publish(name, length, "complete")?;
            return Ok(content);
        }
        let partial = root.join(format!("{name}.partial"));
        let mut file = if partial.try_exists()? {
            open(&partial, true)?
        } else {
            // The private creator intentionally returns an exclusive write-only handle.
            // Reopen with the verified read/write path before taking the durable lease.
            drop(platform_private_storage::create_new(&partial)?);
            open(&partial, true)?
        };
        file.try_lock_exclusive().map_err(|_| Error::Storage)?;
        let mut offset = file.metadata()?.len();
        if !self.transfers.active(name)? {
            file.sync_all()?;
            FileExt::unlock(&file)?;
            self.transfers.publish(name, offset, "paused")?;
            return Err(Error::TransferPaused);
        }
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
        if offset == length && digest_handle(&file)? != sha256 {
            file.set_len(0)?;
            offset = 0;
        }
        if offset < length {
            let mut url = crate::http::url::<crate::http::TaskContent>(
                &self.connection_origin()?,
                &[&offer.task_id().to_string()],
                None,
            )?;
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
            let remaining = offer
                .preparation_deadline()
                .map(|until| {
                    u64::try_from(until - self.now()?)
                        .map(std::time::Duration::from_secs)
                        .map_err(|_| Error::Expired)
                })
                .transpose()?
                .unwrap_or(self.store.cfg.limits.transfer_timeout);
            if remaining.is_zero() {
                return Err(Error::Expired);
            }
            let mut request = self
                .call::<crate::http::TaskContent>(
                    &[&offer.task_id().to_string()],
                    url.query(),
                    None,
                )?
                .header("accept-encoding", "identity")
                .timeout(self.store.cfg.limits.transfer_timeout.min(remaining));
            if offset > 0 {
                request = request
                    .header("range", format!("bytes={offset}-"))
                    .header("if-range", &etag);
            }
            let mut response = self
                .content_response(request, &mut file, name, offset)
                .await?;
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
            self.stream_content(response, &mut file, name, start, length)
                .await?;
        }

        self.transfers.publish(name, length, "verifying")?;
        if digest_handle(&file)? != sha256 {
            self.transfers.publish(name, 0, "failed")?;
            file.set_len(0)?;
            file.sync_all()?;
            return Err(Error::Untrusted);
        }
        file.sync_all()?;
        platform_private_storage::replace(&partial, &path)?;
        FileExt::unlock(&file)?;
        // create_new is write-only; return a separately opened read handle.
        let file = open(&path, false)?;
        FileExt::try_lock_shared(&file).map_err(|_| Error::Storage)?;
        self.transfers.publish(name, length, "complete")?;
        Ok(ContentFile {
            file,
            path,
            length,
            digest: sha256,
            key,
        })
    }
    async fn content_response(
        &self,
        request: crate::http::Call<crate::http::TaskContent>,
        file: &mut File,
        name: &str,
        offset: u64,
    ) -> Result<reqwest::Response, Error> {
        let mut pending = Box::pin(self.agent_send(request));
        loop {
            if !self.transfers.active(name)? {
                drop(pending);
                file.sync_all()?;
                FileExt::unlock(&*file)?;
                self.transfers.publish(name, offset, "paused")?;
                return Err(Error::TransferPaused);
            }
            tokio::select! {biased;
                _=self.transfers.changed()=>{},
                response=&mut pending=>return response,
            }
        }
    }
    async fn stream_content(
        &self,
        mut response: reqwest::Response,
        file: &mut File,
        name: &str,
        mut count: u64,
        length: u64,
    ) -> Result<(), Error> {
        self.transfers.publish(name, count, "downloading")?;
        loop {
            if !self.transfers.active(name)? {
                file.sync_all()?;
                drop(response);
                FileExt::unlock(&*file)?;
                self.transfers.publish(name, count, "paused")?;
                return Err(Error::TransferPaused);
            }
            let chunk = tokio::select! {
                biased;
                _ = self.transfers.changed() => continue,
                chunk = response.chunk() => chunk.map_err(crate::client::transport_error).map_err(|error| self.handle_transport_error(error))?,
            };
            let Some(chunk) = chunk else { break };

            count = count
                .checked_add(chunk.len() as u64)
                .ok_or(Error::Capacity)?;
            if count > length {
                return Err(Error::Untrusted);
            }
            file.write_all(&chunk)?;
            self.transfers.publish(name, count, "downloading")?;
        }
        file.sync_all()?;
        if count != length {
            return Err(Error::Untrusted);
        }
        Ok(())
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
            self.store
                .conn
                .execute("DELETE FROM state WHERE key=?1", [format!("native/{name}")])?;
            removed += 1;
        }
        Ok(removed)
    }
}

fn artifacts(payload: &wire::TaskPayload) -> Result<Vec<(String, u64, [u8; 32])>, Error> {
    Ok(match payload {
        wire::TaskPayload::DeviceAccess(_) => vec![],
        wire::TaskPayload::Credential(_) => vec![],
        wire::TaskPayload::Collection(_)
        | wire::TaskPayload::Enrollment(_)
        | wire::TaskPayload::Unmanage(_) => vec![],
        wire::TaskPayload::Script(v) => {
            vec![("script".into(), v.content.length, v.content.sha256)]
        }
        wire::TaskPayload::Software(v) => v
            .steps
            .iter()
            .enumerate()
            .flat_map(|(index, step)| {
                let keys = execution_contract::software_artifact_keys(step, v.intent);
                step.artifacts
                    .iter()
                    .filter(move |a| {
                        a.key
                            .strip_prefix(&format!("{index}/"))
                            .is_some_and(|key| keys.contains(key))
                    })
                    .map(|a| (a.key.clone(), a.length, a.sha256))
            })
            .collect(),
    })
}

fn cached_content(
    path: &Path,
    length: u64,
    sha256: [u8; 32],
    key: &str,
) -> Result<Option<ContentFile>, Error> {
    if !path.try_exists()? {
        return Ok(None);
    }
    let file = open(path, false)?;
    FileExt::try_lock_shared(&file).map_err(|_| Error::Storage)?;
    if file.metadata()?.len() != length || digest(file.try_clone()?)? != sha256 {
        return Err(Error::Untrusted);
    }
    Ok(Some(ContentFile {
        file,
        path: path.to_owned(),
        length,
        digest: sha256,
        key: key.into(),
    }))
}

/// The original partial-file lease distinguishes an idle/future artifact from its active writer.
pub(crate) fn pause_ready(root: &Path, name: &str) -> Result<Option<u64>, Error> {
    if name.len() != 64 || !name.bytes().all(|v| v.is_ascii_hexdigit()) {
        return Err(Error::Storage);
    }
    let path = root.join("content").join(format!("{name}.partial"));
    if !path.try_exists()? {
        return Ok(Some(0));
    }
    let file = open(&path, true)?;
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => {
            let bytes = file.metadata()?.len();
            file.sync_all()?;
            Ok(Some(bytes))
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::WouldBlock
                || error.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
        {
            Ok(None)
        }
        Err(_) => Err(Error::Storage),
    }
}
