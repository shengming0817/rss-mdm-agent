//! Cooperative reclamation under the non-cloneable material publisher.
use super::*;
use std::{
    cmp::Reverse,
    fs::ReadDir,
    ops::Bound::{Excluded, Unbounded},
};

const METADATA_STEP: usize = 64;
const HASH_STEP: usize = 1024 * 1024;
const DELETE_STEP: usize = 16;

/// Current phase of one owner-local reclamation pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReclaimPhase {
    /// Walk the protected material root incrementally.
    Inventory,
    /// The product must finish its authenticated journal/reference proof.
    AwaitingProof,
    /// Validate retained coordinates and candidate bytes before deleting anything.
    Verifying,
    /// Remove only proved unprotected file identities.
    Files,
    /// Remove deepest empty managed directories.
    Directories,
    /// All selected work is finished.
    Complete,
}
struct Hashing {
    path: PathBuf,
    expected: Digest,
    file: File,
    _lease: crate::platform::PathLease,
    identity: Identity,
    hasher: Sha256,
    bytes: u64,
    stamp: Stamp,
}
#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    length: u64,
    modified: std::time::SystemTime,
}
impl Stamp {
    fn read(file: &File) -> Result<Self, Error> {
        let metadata = file.metadata().map_err(|_| Error::Storage)?;
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified().map_err(|_| Error::Storage)?,
        })
    }
}
struct Verified {
    digest: Digest,
    stamp: Stamp,
}
/// One resumable pass, bound to its exclusive MaterialRoot. It holds no journal authority.
pub struct Reclamation {
    root: PathBuf,
    instance: std::sync::Weak<()>,
    generation: u64,
    phase: ReclaimPhase,
    resume: ReclaimPhase,
    pending: Vec<PathBuf>,
    directory: Option<(ReadDir, crate::platform::PathLease)>,
    entries: BTreeMap<PathBuf, Entry>,
    directories: BTreeSet<(Reverse<usize>, PathBuf)>,
    links: BTreeMap<Identity, u64>,
    removed: BTreeMap<Identity, u64>,
    candidates: BTreeMap<PathBuf, Digest>,
    retained: BTreeSet<PathBuf>,
    protected: BTreeSet<Identity>,
    protected_directories: BTreeSet<PathBuf>,
    proven: BTreeMap<Identity, Verified>,
    cursor: Option<PathBuf>,
    pending_retained: BTreeSet<PathBuf>,
    pending_candidates: BTreeMap<PathBuf, Digest>,
    allowed_links: BTreeMap<Identity, u64>,
    hashing: Option<Hashing>,
    result: Reclaimed,
}
impl Reclamation {
    /// Current phase; an AwaitingProof pass never mutates the filesystem.
    pub fn phase(&self) -> ReclaimPhase {
        self.phase
    }
    /// Names from the completed inventory, without opening data handles.
    pub fn files(&self) -> impl Iterator<Item = &PathBuf> {
        self.entries
            .values()
            .filter(|e| e.identity.is_some())
            .map(|e| &e.path)
    }
    /// Whether an exact file was present in this physical inventory.
    pub fn contains_file(&self, path: &Path) -> bool {
        self.entries.get(path).is_some_and(|e| e.identity.is_some())
    }
    /// Suspend deletion for a fresh journal/reference proof without discarding object verification.
    pub fn recheck_proof(&mut self) -> Result<(), Error> {
        if self.phase == ReclaimPhase::Complete {
            return Err(Error::Conflict);
        }
        if matches!(self.phase, ReclaimPhase::Files | ReclaimPhase::Directories) {
            self.resume = self.phase;
        }
        if self.phase != ReclaimPhase::Inventory {
            self.phase = ReclaimPhase::AwaitingProof;
        }
        Ok(())
    }
    /// Completed physical result. Consuming an unfinished pass cannot claim successful reclamation.
    pub fn into_result(self) -> Result<Reclaimed, Error> {
        if self.phase != ReclaimPhase::Complete {
            return Err(Error::Conflict);
        }
        Ok(self.result)
    }
}
impl MaterialRoot {
    /// Publication generation; new names require a later inventory and a fresh deletion proof.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Begin a new pass without walking or hashing the material tree synchronously.
    pub fn begin_reclamation(&self) -> Reclamation {
        Reclamation {
            root: self.path.clone(),
            instance: std::sync::Arc::downgrade(&self.instance),
            generation: self.generation,
            phase: ReclaimPhase::Inventory,
            resume: ReclaimPhase::Files,
            pending: vec![self.path.clone()],
            directory: None,
            entries: BTreeMap::new(),
            directories: BTreeSet::new(),
            links: BTreeMap::new(),
            removed: BTreeMap::new(),
            candidates: BTreeMap::new(),
            retained: BTreeSet::new(),
            protected: BTreeSet::new(),
            protected_directories: BTreeSet::new(),
            proven: BTreeMap::new(),
            cursor: None,
            pending_retained: BTreeSet::new(),
            pending_candidates: BTreeMap::new(),
            allowed_links: BTreeMap::new(),
            hashing: None,
            result: Reclaimed::default(),
        }
    }
    /// Attach a complete current journal/reference proof, preserving immutable object progress.
    /// The caller authenticates references; this owner binds deletion to current publication state.
    pub fn prove_reclamation(
        &self,
        pass: &mut Reclamation,
        candidates: &BTreeMap<PathBuf, Digest>,
        retained: &BTreeSet<PathBuf>,
    ) -> Result<(), Error> {
        if !pass
            .instance
            .ptr_eq(&std::sync::Arc::downgrade(&self.instance))
            || pass.phase != ReclaimPhase::AwaitingProof
        {
            return Err(Error::Conflict);
        }
        if candidates.len() > MAX_ENTRIES || retained.len() > MAX_ENTRIES {
            return Err(Error::Capacity);
        }
        if pass
            .hashing
            .as_ref()
            .is_some_and(|h| candidates.get(&h.path) != Some(&h.expected))
        {
            pass.hashing = None;
        }
        // Preserve verification cursors. Only changed coordinates join the work queues;
        // old retained identities stay protected for the rest of this physical pass.
        pass.pending_retained
            .extend(retained.difference(&pass.retained).cloned());
        pass.pending_candidates
            .retain(|p, d| candidates.get(p) == Some(d));
        for (path, digest) in candidates {
            if pass.candidates.get(path) != Some(digest) {
                pass.pending_candidates.insert(path.clone(), digest.clone());
            }
            if !pass.candidates.contains_key(path) {
                if let Some(id) = pass.entries.get(path).and_then(|e| e.identity) {
                    *pass.allowed_links.entry(id).or_default() += 1;
                }
            }
        }
        for path in pass
            .candidates
            .keys()
            .filter(|p| !candidates.contains_key(*p))
        {
            if let Some(id) = pass.entries.get(path).and_then(|e| e.identity) {
                let count = pass.allowed_links.get_mut(&id).ok_or(Error::Conflict)?;
                *count = count.checked_sub(1).ok_or(Error::Conflict)?;
            }
        }
        pass.generation = self.generation;
        pass.candidates.clone_from(candidates);
        pass.retained.extend(retained.iter().cloned());
        pass.phase = ReclaimPhase::Verifying;
        Ok(())
    }
    /// Advance bounded metadata, hash or unlink work. OS calls themselves have no wall-clock guarantee.
    /// Caller checks the current journal and live-reference proof before every Files/Directories step.
    pub fn reclaim_step(&mut self, pass: &mut Reclamation) -> Result<(), Error> {
        if !pass
            .instance
            .ptr_eq(&std::sync::Arc::downgrade(&self.instance))
            || pass.root != self.path
            || (matches!(pass.phase, ReclaimPhase::Files | ReclaimPhase::Directories)
                && pass.generation != self.generation)
        {
            return Err(Error::Conflict);
        }
        match pass.phase {
            ReclaimPhase::Inventory => self.inventory_step(pass),
            ReclaimPhase::AwaitingProof | ReclaimPhase::Complete => Ok(()),
            ReclaimPhase::Verifying => self.verification_step(pass),
            ReclaimPhase::Files => self.files_step(pass),
            ReclaimPhase::Directories => self.directories_step(pass),
        }
    }
    fn inventory_step(&self, pass: &mut Reclamation) -> Result<(), Error> {
        for _ in 0..METADATA_STEP {
            if pass.directory.is_none() {
                let Some(path) = pass.pending.pop() else {
                    pass.phase = ReclaimPhase::AwaitingProof;
                    return Ok(());
                };
                let lease = crate::platform::PathLease::source(&path, self.immutable())?;
                pass.directory =
                    Some((std::fs::read_dir(path).map_err(|_| Error::Storage)?, lease));
            }
            let Some(item) = pass.directory.as_mut().unwrap().0.next() else {
                pass.directory = None;
                continue;
            };
            let path = item.map_err(|_| Error::Storage)?.path();
            if pass.entries.len() == MAX_ENTRIES {
                return Err(Error::Capacity);
            }
            self.contained(&path)?;
            let meta = path.symlink_metadata().map_err(|_| Error::Storage)?;
            let (identity, bytes, links) = if meta.is_dir() {
                pass.pending.push(path.clone());
                pass.directories
                    .insert((Reverse(path.components().count()), path.clone()));
                (None, 0, 0)
            } else if meta.is_file() {
                let (identity, bytes, links) = if path == self.path.join(".owner") {
                    (
                        crate::platform::storage_identity(&self._owner)?,
                        self._owner.metadata().map_err(|_| Error::Storage)?.len(),
                        crate::platform::storage_links(&self._owner)?,
                    )
                } else {
                    let (file, _lease) = self.file(&path)?;
                    (
                        crate::platform::storage_identity(&file)?,
                        file.metadata().map_err(|_| Error::Storage)?.len(),
                        crate::platform::storage_links(&file)?,
                    )
                };
                *pass.links.entry(identity).or_default() += 1;
                if pending_name(&self.path, &path) {
                    *pass.allowed_links.entry(identity).or_default() += 1;
                }
                (Some(identity), bytes, links)
            } else {
                return Err(Error::Denied);
            };
            if pass
                .entries
                .insert(
                    path.clone(),
                    Entry {
                        path,
                        identity,
                        bytes,
                        links,
                    },
                )
                .is_some()
            {
                return Err(Error::Conflict);
            }
        }
        Ok(())
    }
    fn verification_step(&self, pass: &mut Reclamation) -> Result<(), Error> {
        if !pass.pending_retained.is_empty() {
            for _ in 0..METADATA_STEP {
                let Some(path) = pass.pending_retained.pop_first() else {
                    break;
                };
                if !path.starts_with(&self.path) {
                    continue;
                }
                self.contained(&path)?;
                if path
                    .symlink_metadata()
                    .map_err(|_| Error::Storage)?
                    .is_dir()
                {
                    let _lease = crate::platform::PathLease::source(&path, self.immutable())?;
                    pass.protected_directories.insert(path);
                } else {
                    let (file, _lease) = self.file(&path)?;
                    pass.protected
                        .insert(crate::platform::storage_identity(&file)?);
                }
            }
            return Ok(());
        }
        {
            let mut bytes_left = HASH_STEP;
            let mut names_left = METADATA_STEP;
            while bytes_left != 0 && names_left != 0 {
                if pass.hashing.is_none() {
                    let Some((path, expected)) = pass.pending_candidates.pop_first() else {
                        pass.phase = pass.resume;
                        break;
                    };
                    names_left -= 1;
                    self.contained(&path)?;
                    if !managed_file(&self.path, &path, &expected) {
                        return Err(Error::Denied);
                    }
                    match path.symlink_metadata() {
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(_) => return Err(Error::Storage),
                        Ok(_) => (),
                    }
                    let (file, lease) = self.file(&path)?;
                    let identity = crate::platform::storage_identity(&file)?;
                    if pass
                        .entries
                        .get(&path)
                        .is_none_or(|e| e.identity != Some(identity))
                    {
                        return Err(Error::Conflict);
                    }
                    let stamp = Stamp::read(&file)?;
                    if pass
                        .proven
                        .get(&identity)
                        .is_some_and(|p| p.digest == expected && p.stamp == stamp)
                    {
                        continue;
                    }
                    pass.proven.remove(&identity);
                    pass.hashing = Some(Hashing {
                        path,
                        expected,
                        identity,
                        file,
                        _lease: lease,
                        hasher: Sha256::new(),
                        bytes: 0,
                        stamp,
                    });
                }
                let hashing = pass.hashing.as_mut().unwrap();
                if Stamp::read(&hashing.file)? != hashing.stamp {
                    return Err(Error::Conflict);
                }
                let mut buffer = [0u8; 65536];
                let amount = hashing
                    .file
                    .read(&mut buffer[..bytes_left.min(65536)])
                    .map_err(|_| Error::Storage)?;
                if amount == 0 {
                    let hashing = pass.hashing.take().unwrap();
                    if format!("{:x}", hashing.hasher.finalize()) != hashing.expected.as_str() {
                        return Err(Error::Denied);
                    }
                    // The retained source lease pins the object throughout its incremental hash.
                    if crate::platform::storage_identity(&hashing.file)? != hashing.identity
                        || Stamp::read(&hashing.file)? != hashing.stamp
                    {
                        return Err(Error::Conflict);
                    }
                    pass.proven.insert(
                        hashing.identity,
                        Verified {
                            digest: hashing.expected,
                            stamp: hashing.stamp,
                        },
                    );
                } else {
                    bytes_left -= amount;
                    hashing.bytes = hashing
                        .bytes
                        .checked_add(amount as u64)
                        .ok_or(Error::Capacity)?;
                    if hashing.bytes > MAX_BYTES {
                        return Err(Error::Capacity);
                    }
                    hashing.hasher.update(&buffer[..amount]);
                }
            }
            Ok(())
        }
    }
    fn files_step(&self, pass: &mut Reclamation) -> Result<(), Error> {
        for _ in 0..DELETE_STEP {
            let next = pass
                .entries
                .range::<PathBuf, _>((pass.cursor.as_ref().map_or(Unbounded, Excluded), Unbounded))
                .next()
                .map(|(p, e)| (p.clone(), e.identity, e.bytes, e.links));
            let Some((path, identity, bytes, links)) = next else {
                pass.phase = ReclaimPhase::Directories;
                pass.cursor = None;
                return Ok(());
            };
            pass.cursor = Some(path.clone());
            let Some(id) = identity else {
                continue;
            };
            if pass.protected.contains(&id)
                || pass.links[&id] != links
                || pass.allowed_links.get(&id).copied().unwrap_or(0) != pass.links[&id]
                || path
                    .ancestors()
                    .any(|p| pass.protected_directories.contains(p))
                || !(pass.proven.contains_key(&id) || pending_name(&self.path, &path))
            {
                continue;
            }
            let (file, lease) = self.file(&path)?;
            let removed = pass.removed.get(&id).copied().unwrap_or(0);
            if crate::platform::storage_identity(&file)? != id
                || crate::platform::storage_links(&file)? != links.saturating_sub(removed)
            {
                return Err(Error::Conflict);
            }
            if let Some(proven) = pass.proven.get(&id) {
                if Stamp::read(&file)? != proven.stamp {
                    return Err(Error::Conflict);
                }
            }
            drop(file);
            drop(lease);
            std::fs::remove_file(&path).map_err(|_| Error::Storage)?;
            pass.entries.remove(&path);
            pass.result.files += 1;
            *pass.removed.entry(id).or_default() += 1;
            if pass.removed[&id] == pass.links[&id] {
                pass.result.bytes += bytes;
            }
        }
        Ok(())
    }
    fn directories_step(&self, pass: &mut Reclamation) -> Result<(), Error> {
        for _ in 0..DELETE_STEP {
            let Some((_, path)) = pass.directories.pop_first() else {
                pass.phase = ReclaimPhase::Complete;
                return Ok(());
            };
            if pass
                .protected_directories
                .range(path.clone()..)
                .next()
                .is_some_and(|p| p.starts_with(&path))
                || !managed_directory(&self.path, &path)
            {
                continue;
            }
            match std::fs::remove_dir(&path) {
                Ok(()) => pass.result.directories += 1,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(_) => return Err(Error::Storage),
            }
        }
        Ok(())
    }
}
