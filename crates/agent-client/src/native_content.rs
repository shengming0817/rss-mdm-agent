//! Read-only native preparation joins the existing content cache and original offer.
use crate::native_source::{self as native, MAX_DOCUMENT_BYTES, MAX_PACK_BYTES};
use crate::{
    content::{self, ContentFile},
    store::{encode, hash, private},
    wire, Client, Clock, Error, IdentityProvider, Offer,
};
use fs2::FileExt;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cached {
    length: u64,
    digest: [u8; 32],
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn name(
    service_id: uuid::Uuid,
    tenant: uuid::Uuid,
    step: &wire::SoftwareTaskStep,
) -> Result<String, Error> {
    Ok(hash(&encode(&(
        service_id,
        tenant,
        &step.export,
        &step.action.package,
        &step.action.version,
    ))?))
}
fn load(root: &Path, name: &str, index: usize, cached: Cached) -> Result<ContentFile, Error> {
    if cached.length > MAX_PACK_BYTES as u64 {
        return Err(Error::Capacity);
    }
    let path = root.join(format!("{name}.blob"));
    let file = content::open(&path, false)?;
    FileExt::try_lock_shared(&file).map_err(|_| Error::Storage)?;
    let file = ContentFile {
        file,
        path,
        length: cached.length,
        digest: cached.digest,
        key: format!("native/{index}"),
    };
    file.verify()?;
    Ok(file)
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    pub(crate) fn recover_native_sources(
        &self,
        offer: &Offer,
        root: &Path,
        files: &mut Vec<ContentFile>,
    ) -> Result<(), Error> {
        let wire::TaskPayload::Software(spec) = offer.payload() else {
            return Ok(());
        };
        for (index, step) in spec.steps.iter().enumerate() {
            if spec.intent == wire::SoftwareTaskIntent::Detect
                || matches!(step.export, wire::SoftwareTaskExport::Direct)
            {
                continue;
            }
            let name = name(self.store.cfg.service_id, self.store.cfg.tenant, step)?;
            let cached = self
                .store
                .get(&format!("native/{name}"))?
                .ok_or(Error::Storage)?;
            files.push(load(root, &name, index, cached)?);
        }
        Ok(())
    }

    pub(crate) async fn prepare_native_sources(
        &self,
        offer: &Offer,
        root: &Path,
        files: &mut Vec<ContentFile>,
    ) -> Result<(), Error> {
        let wire::TaskPayload::Software(spec) = offer.payload() else {
            return Ok(());
        };
        for (index, step) in spec.steps.iter().enumerate() {
            if spec.intent == wire::SoftwareTaskIntent::Detect
                || matches!(step.export, wire::SoftwareTaskExport::Direct)
            {
                continue;
            }
            self.verify(&offer.signed, wire::TaskPermit::Offer)?;
            let name = name(self.store.cfg.service_id, self.store.cfg.tenant, step)?;
            self.store.conn.execute(
                "INSERT OR IGNORE INTO cache_refs VALUES(?1,?2)",
                params![offer.task_id().to_string(), name],
            )?;
            if let Some(cached) = self.store.get(&format!("native/{name}"))? {
                if root.join(format!("{name}.blob")).try_exists()? {
                    files.push(load(root, &name, index, cached)?);
                    continue;
                }
                // A prior unreferenced GC may have removed the file before deleting its receipt.
                // Only a current verified Offer may fetch again; Start recovery never enters here.
                self.store
                    .conn
                    .execute("DELETE FROM state WHERE key=?1", [format!("native/{name}")])?;
            }
            let bytes = match &step.export {
                wire::SoftwareTaskExport::Direct => unreachable!(),
                wire::SoftwareTaskExport::Winget {
                    uri,
                    identifier,
                    binding,
                } => {
                    let base = url::Url::parse(uri).map_err(|_| Error::Protocol)?;
                    let info = self
                        .native_read(
                            offer,
                            self.native_http
                                .get(base.join("information").map_err(|_| Error::Protocol)?),
                            None,
                            65536,
                        )
                        .await?;
                    let info: serde_json::Value =
                        serde_json::from_slice(&info).map_err(|_| Error::Protocol)?;
                    if info["Data"]["SourceIdentifier"].as_str() != Some(identifier) {
                        return Err(Error::Untrusted);
                    }
                    let mut url = base
                        .join("packageManifests/")
                        .map_err(|_| Error::Protocol)?;
                    url.path_segments_mut()
                        .map_err(|_| Error::Protocol)?
                        .pop_if_empty()
                        .push(&step.action.package);
                    url.query_pairs_mut()
                        .append_pair("Version", &step.action.version);
                    let body = self
                        .native_read(offer, self.native_http.get(url), None, MAX_DOCUMENT_BYTES)
                        .await?;
                    let body: serde_json::Value =
                        serde_json::from_slice(&body).map_err(|_| Error::Protocol)?;
                    let bytes = serde_json::to_vec(&body["Data"]).map_err(|_| Error::Protocol)?;
                    native::winget_manifest(&bytes, binding, &step.action)?;
                    bytes
                }
                wire::SoftwareTaskExport::Brew {
                    uri,
                    commit,
                    credential_reference,
                    binding,
                    ..
                } => {
                    let credential = wire::SoftwareReadCredential {
                        source: binding.target.clone(),
                        reference: credential_reference.clone(),
                    };
                    let base = url::Url::parse(uri).map_err(|_| Error::Protocol)?;
                    let grant = self
                        .store
                        .cfg
                        .software_read_grants
                        .iter()
                        .find(|g| g.credential == credential)
                        .ok_or(Error::Denied)?;
                    if url::Url::parse(&grant.origin)
                        .map_err(|_| Error::Configuration)?
                        .origin()
                        != base.origin()
                    {
                        return Err(Error::Denied);
                    }
                    let token = self.secrets.software_read_token(&credential)?;
                    native::validate_read_token(&token)?;
                    let url = format!("{}/git-upload-pack", uri.trim_end_matches('/'));
                    let response = self
                        .native_read(
                            offer,
                            self.native_http
                                .post(url)
                                .header("content-type", "application/x-git-upload-pack-request")
                                .body(native::pack_request(commit)?),
                            Some(&token),
                            MAX_PACK_BYTES + 8,
                        )
                        .await?;
                    let bytes = response
                        .strip_prefix(b"0008NAK\n")
                        .ok_or(Error::Protocol)?
                        .to_vec();
                    let scratch = Scratch(
                        std::env::temp_dir()
                            .canonicalize()?
                            .join(format!("rss-native-{}", uuid::Uuid::new_v4())),
                    );
                    platform_private_storage::directory(&scratch.0)?;
                    native::import_brew_pack(
                        &scratch.0,
                        &bytes,
                        commit,
                        &binding.document_sha256,
                        None,
                    )
                    .await?;
                    bytes
                }
            };
            self.verify(&offer.signed, wire::TaskPermit::Offer)?;
            let used = std::fs::read_dir(root)?.try_fold(0u64, |total, e| {
                let path = e?.path();
                private(&path)?;
                total
                    .checked_add(std::fs::symlink_metadata(path)?.len())
                    .ok_or(Error::Capacity)
            })?;
            if used
                .checked_add(bytes.len() as u64)
                .is_none_or(|n| n > self.store.cfg.limits.cache_bytes)
            {
                return Err(Error::Capacity);
            }
            let partial = root.join(format!("{name}.partial"));
            let mut file = if partial.try_exists()? {
                content::open(&partial, true)?
            } else {
                platform_private_storage::create_new(&partial)?
            };
            file.try_lock_exclusive().map_err(|_| Error::Storage)?;
            file.set_len(0)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            platform_private_storage::replace(&partial, &root.join(format!("{name}.blob")))?;
            let cached = Cached {
                length: bytes.len() as u64,
                digest: Sha256::digest(&bytes).into(),
            };
            self.store.put(&format!("native/{name}"), &cached)?;
            FileExt::unlock(&file)?;
            files.push(load(root, &name, index, cached)?);
        }
        Ok(())
    }

    async fn native_read(
        &self,
        offer: &Offer,
        mut request: reqwest::RequestBuilder,
        token: Option<&str>,
        limit: usize,
    ) -> Result<Vec<u8>, Error> {
        if let Some(token) = token {
            request = request.basic_auth("rss", Some(token));
        }
        let remaining = u64::try_from(offer.payload().expires_at() - self.now()?)
            .map_err(|_| Error::Expired)?;
        if remaining == 0 {
            return Err(Error::Expired);
        }
        let mut response = request
            .header("accept-encoding", "identity")
            .timeout(
                self.store
                    .cfg
                    .limits
                    .transfer_timeout
                    .min(std::time::Duration::from_secs(remaining)),
            )
            .send()
            .await
            .map_err(|_| Error::Unavailable)?;
        match response.status().as_u16() {
            200 => (),
            401 | 403 => return Err(Error::Denied),
            404 => return Err(Error::NotFound),
            429 | 500..=599 => return Err(Error::Unavailable),
            _ => return Err(Error::Protocol),
        }
        if response
            .headers()
            .get("content-encoding")
            .is_some_and(|v| v != "identity")
            || response.content_length().is_some_and(|n| n > limit as u64)
        {
            return Err(Error::Capacity);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unavailable)? {
            if bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|n| n > limit)
            {
                return Err(Error::Capacity);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}
