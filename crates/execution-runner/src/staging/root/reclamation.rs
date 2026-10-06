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
}
/// One resumable pass, bound to its exclusive MaterialRoot. It holds no journal authority.
pub struct Reclamation {
    root: PathBuf,
    instance: std::sync::Weak<()>,
    generation: u64,
    phase: ReclaimPhase,
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
    proven: BTreeSet<Identity>,
    cursor: Option<PathBuf>,
    retained_done: bool,
    candidates_done: bool,
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
    /// Transfer a complete journal/reference proof. The producer remains responsible for its currency.
    pub fn prove(
        &mut self,
        candidates: BTreeMap<PathBuf, Digest>,
        retained: BTreeSet<PathBuf>,
    ) -> Result<(), Error> {
        if self.phase != ReclaimPhase::AwaitingProof {
            return Err(Error::Conflict);
        }
        if candidates.len() > MAX_ENTRIES || retained.len() > MAX_ENTRIES {
            return Err(Error::Capacity);
        }
        self.candidates = candidates;
        self.retained = retained;
        self.phase = ReclaimPhase::Verifying;
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
    /// Publication generation; any attempted publication invalidates previous inventories.
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
            proven: BTreeSet::new(),
            cursor: None,
            retained_done: false,
            candidates_done: false,
            hashing: None,
            result: Reclaimed::default(),
        }
    }
    /// Advance bounded metadata, hash or unlink work. OS calls themselves have no wall-clock guarantee.
    /// Caller checks the current journal and live-reference proof before every Files/Directories step.
    pub fn reclaim_step(&mut self, pass: &mut Reclamation) -> Result<(), Error> {
        if !pass
            .instance
            .ptr_eq(&std::sync::Arc::downgrade(&self.instance))
            || pass.root != self.path
            || pass.generation != self.generation
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
        if !pass.retained_done {
            for _ in 0..METADATA_STEP {
                let next = pass
                    .retained
                    .range::<PathBuf, _>((
                        pass.cursor.as_ref().map_or(Unbounded, Excluded),
                        Unbounded,
                    ))
                    .next()
                    .cloned();
                let Some(path) = next else {
                    pass.retained_done = true;
                    pass.cursor = None;
                    break;
                };
                pass.cursor = Some(path.clone());
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
        if !pass.candidates_done {
            let mut bytes_left = HASH_STEP;
            let mut names_left = METADATA_STEP;
            while bytes_left != 0 && names_left != 0 {
                if pass.hashing.is_none() {
                    let next = pass
                        .candidates
                        .range::<PathBuf, _>((
                            pass.cursor.as_ref().map_or(Unbounded, Excluded),
                            Unbounded,
                        ))
                        .next()
                        .map(|(p, d)| (p.clone(), d.clone()));
                    let Some((path, expected)) = next else {
                        pass.candidates_done = true;
                        pass.cursor = None;
                        break;
                    };
                    names_left -= 1;
                    pass.cursor = Some(path.clone());
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
                    pass.hashing = Some(Hashing {
                        path,
                        expected,
                        identity,
                        file,
                        _lease: lease,
                        hasher: Sha256::new(),
                        bytes: 0,
                    });
                }
                let hashing = pass.hashing.as_mut().unwrap();
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
                        || hashing.path != *pass.cursor.as_ref().unwrap()
                    {
                        return Err(Error::Conflict);
                    }
                    pass.proven.insert(hashing.identity);
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
            return Ok(());
        }
        for _ in 0..METADATA_STEP {
            let next = pass
                .entries
                .range::<PathBuf, _>((pass.cursor.as_ref().map_or(Unbounded, Excluded), Unbounded))
                .next()
                .map(|(p, e)| (p.clone(), e.identity, e.links));
            let Some((path, identity, links)) = next else {
                pass.phase = ReclaimPhase::Files;
                pass.cursor = None;
                return Ok(());
            };
            pass.cursor = Some(path.clone());
            if let Some(id) = identity {
                if pass.links[&id] != links
                    || (!pass.candidates.contains_key(&path) && !pending_name(&self.path, &path))
                    || path
                        .ancestors()
                        .any(|p| pass.protected_directories.contains(p))
                {
                    pass.protected.insert(id);
                }
            }
        }
        Ok(())
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
                || !(pass.proven.contains(&id) || pending_name(&self.path, &path))
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
            drop(file);
            drop(lease);
            std::fs::remove_file(&path).map_err(|_| Error::Storage)?;
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
                    ) =>
                {
                    ()
                }
                Err(_) => return Err(Error::Storage),
            }
        }
        Ok(())
    }
}
