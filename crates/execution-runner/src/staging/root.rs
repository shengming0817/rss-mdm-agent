//! One exclusive material root; the caller supplies settlement from the authoritative journal.
use super::*;
mod reclamation;
use fs2::FileExt;
pub use reclamation::{ReclaimPhase, Reclamation};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
};

const MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 32_768;
type Identity = (u64, u64);

/// Exclusive service-owned publisher. Owner metadata binds authority/device and journal identity;
/// it records no task state. All mutations require this non-cloneable owner.
pub struct MaterialRoot {
    pub(super) path: PathBuf,
    _lease: crate::platform::PathLease,
    _owner: File,
    instance: std::sync::Arc<()>,
    generation: u64,
    #[cfg(any(test, feature = "test-support"))]
    fixture_owned: bool,
}
impl Drop for MaterialRoot {
    fn drop(&mut self) {
        // Release the capability at owner drop even if a concurrent POSIX fork briefly
        // inherited its close-on-exec descriptor. Closing only our copy can retain flock.
        let _ = FileExt::unlock(&self._owner);
    }
}
/// Bounded, path-free reclamation diagnostics.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Reclaimed {
    /// Removed file names, including abandoned publication staging names.
    pub files: usize,
    /// Unique physical bytes released after every known link was removed.
    pub bytes: u64,
    /// Removed empty managed directories.
    pub directories: usize,
}
struct Entry {
    path: PathBuf,
    identity: Option<Identity>,
    bytes: u64,
    links: u64,
}
impl MaterialRoot {
    /// Open a protected initialized root, exclusively bound to this owner and journal file.
    /// Existing mismatched or incomplete metadata is rejected and never rewritten.
    pub fn open(root: &Path, owner: &Digest, journal: &Path) -> Result<Self, Error> {
        require_system()?;
        let journal = journal.canonicalize().map_err(|_| Error::Storage)?;
        let identity = crate::platform::journal_identity(&journal)?;
        let binding = serde_json::to_vec(&(owner.as_str(), journal, identity))
            .map_err(|_| Error::Configuration)?;
        Self::open_bound(root, &format!("{:x}", Sha256::digest(binding)), true)
    }
    /// Explicit unprivileged filesystem fixture. This constructor is absent from default
    /// production builds and its objects cannot pass immutable producer verification.
    #[cfg(feature = "test-support")]
    pub fn open_fixture(root: &Path, owner: &Digest, journal: &Path) -> Result<Self, Error> {
        let journal = journal.canonicalize().map_err(|_| Error::Storage)?;
        let identity = crate::platform::journal_identity(&journal)?;
        let binding = serde_json::to_vec(&(owner.as_str(), journal, identity))
            .map_err(|_| Error::Configuration)?;
        Self::open_bound(root, &format!("{:x}", Sha256::digest(binding)), false)
    }
    fn open_bound(root: &Path, binding: &str, immutable: bool) -> Result<Self, Error> {
        let lease = crate::platform::PathLease::source(root, immutable)?;
        let path = root.canonicalize().map_err(|_| Error::Storage)?;
        let owner_path = path.join(".owner");
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            );
        }
        // ref: fs2@0.4.3 src/unix.rs and src/windows.rs: nonblocking exclusive file lock.
        let (mut owner, created) = match options.clone().create_new(true).open(&owner_path) {
            Ok(file) => (file, true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (
                options.open(&owner_path).map_err(|_| Error::Storage)?,
                false,
            ),
            Err(_) => return Err(Error::Storage),
        };
        if immutable {
            crate::platform::protected_file(&owner)?;
        }
        owner.try_lock_exclusive().map_err(|_| Error::Conflict)?;
        if created {
            owner
                .write_all(binding.as_bytes())
                .and_then(|_| owner.sync_all())
                .map_err(|_| Error::Storage)?;
            #[cfg(target_os = "macos")]
            File::open(&path)
                .and_then(|f| f.sync_all())
                .map_err(|_| Error::Storage)?;
        } else {
            let mut actual = Vec::new();
            Read::by_ref(&mut owner)
                .take(65)
                .read_to_end(&mut actual)
                .map_err(|_| Error::Storage)?;
            if actual != binding.as_bytes() {
                return Err(Error::Configuration);
            }
        }
        Ok(Self {
            path,
            _lease: lease,
            _owner: owner,
            instance: std::sync::Arc::new(()),
            generation: 0,
            #[cfg(any(test, feature = "test-support"))]
            fixture_owned: !immutable,
        })
    }
    /// Publish exact bytes under the selected immutable subject namespace.
    pub fn publish(
        &mut self,
        subject: Option<&str>,
        source: impl Read,
        expected: &Digest,
        length: u64,
    ) -> Result<PathBuf, Error> {
        self.generation = self.generation.checked_add(1).ok_or(Error::Capacity)?;
        super::publish(self, subject, source, expected, length)
    }
    /// Extract a verified manifest into immutable hard-linked members.
    pub fn bundle(
        &mut self,
        subject: Option<&str>,
        archive: &Path,
        digest: &Digest,
        manifest: &execution_contract::BundleManifest,
    ) -> Result<BTreeMap<String, PathBuf>, Error> {
        self.generation = self.generation.checked_add(1).ok_or(Error::Capacity)?;
        super::bundle(self, subject, archive, digest, manifest)
    }
    /// Create an immutable native filename alias of an owned exact artifact.
    pub fn native_export(
        &mut self,
        source: &Path,
        expected: &Digest,
        name: &str,
        subject: Option<&str>,
    ) -> Result<PathBuf, Error> {
        self.generation = self.generation.checked_add(1).ok_or(Error::Capacity)?;
        super::native_export(self, source, expected, name, subject)
    }
    pub(super) fn contained(&self, path: &Path) -> Result<(), Error> {
        if path == self.path
            || !path.starts_with(&self.path)
            || path.components().any(|p| {
                !matches!(
                    p,
                    std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                        | std::path::Component::Normal(_)
                )
            })
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    fn immutable(&self) -> bool {
        #[cfg(any(test, feature = "test-support"))]
        if self.fixture_owned {
            return false;
        }
        true
    }
    fn file(&self, path: &Path) -> Result<(File, crate::platform::PathLease), Error> {
        crate::platform::PathLease::source(path, self.immutable())?.into_file()
    }
    pub(super) fn directory_lease(&self, path: &Path) -> Result<crate::platform::PathLease, Error> {
        crate::platform::PathLease::directory(path, self.immutable())
    }
    pub(super) fn verified(
        &self,
        path: &Path,
        digest: &Digest,
    ) -> Result<(File, crate::platform::PathLease), Error> {
        crate::materialize::verify_material_kind(path, digest, self.immutable())
    }
    pub(super) fn verify(&self, path: &Path, digest: &Digest, length: u64) -> Result<(), Error> {
        let (file, _lease) = self.verified(path, digest)?;
        if file.metadata().map_err(|_| Error::Storage)?.len() != length {
            return Err(Error::Denied);
        }
        Ok(())
    }
    fn inventory(&self) -> Result<Vec<Entry>, Error> {
        let mut pending = vec![self.path.clone()];
        let mut entries = Vec::new();
        while let Some(directory) = pending.pop() {
            let _lease = crate::platform::PathLease::source(&directory, self.immutable())?;
            for item in std::fs::read_dir(directory).map_err(|_| Error::Storage)? {
                let path = item.map_err(|_| Error::Storage)?.path();
                if entries.len() == MAX_ENTRIES {
                    return Err(Error::Capacity);
                }
                self.contained(&path)?;
                let meta = path.symlink_metadata().map_err(|_| Error::Storage)?;
                let (identity, bytes, links) = if meta.is_dir() {
                    pending.push(path.clone());
                    (None, 0, 0)
                } else if meta.is_file() {
                    if path == self.path.join(".owner") {
                        (
                            Some(crate::platform::storage_identity(&self._owner)?),
                            self._owner.metadata().map_err(|_| Error::Storage)?.len(),
                            crate::platform::storage_links(&self._owner)?,
                        )
                    } else {
                        let (file, _lease) = self.file(&path)?;
                        (
                            Some(crate::platform::storage_identity(&file)?),
                            file.metadata().map_err(|_| Error::Storage)?.len(),
                            crate::platform::storage_links(&file)?,
                        )
                    }
                } else {
                    return Err(Error::Denied);
                };
                entries.push(Entry {
                    path,
                    identity,
                    bytes,
                    links,
                });
            }
        }
        Ok(entries)
    }
    pub(super) fn quota(&self, additional: u64, names: usize) -> Result<(), Error> {
        let entries = self.inventory()?;
        if entries
            .len()
            .checked_add(names)
            .is_none_or(|n| n > MAX_ENTRIES)
        {
            return Err(Error::Capacity);
        }
        let mut identities = BTreeSet::new();
        let mut bytes = additional;
        for entry in entries {
            if entry.identity.is_some_and(|id| identities.insert(id)) {
                bytes = bytes.checked_add(entry.bytes).ok_or(Error::Capacity)?;
            }
        }
        if bytes > MAX_BYTES {
            return Err(Error::Capacity);
        }
        Ok(())
    }
    pub(super) fn directory(&mut self, path: &Path) -> Result<(), Error> {
        self.contained(path)?;
        self.quota(0, 1)?;
        platform_private_storage::directory(path).map_err(|_| Error::Storage)
    }
    pub(super) fn link(&mut self, source: &Path, destination: &Path) -> Result<(), Error> {
        self.contained(source)?;
        self.contained(destination)?;
        self.quota(0, 1)?;
        std::fs::hard_link(source, destination).map_err(|_| Error::Storage)
    }
}

fn hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn parts<'a>(root: &Path, path: &'a Path) -> Vec<&'a str> {
    path.strip_prefix(root)
        .ok()
        .into_iter()
        .flat_map(|p| p.components())
        .filter_map(|p| p.as_os_str().to_str())
        .collect()
}
fn namespace(name: &str) -> bool {
    name == "system" || hex(name)
}
fn managed_file(root: &Path, path: &Path, digest: &Digest) -> bool {
    let parts = parts(root, path);
    if parts.len() < 2 || !namespace(parts[0]) {
        return false;
    }
    if parts.len() == 2 {
        return parts[1] == digest.as_str();
    }
    if let Some(id) = parts[1].strip_prefix("export-") {
        return parts.len() == 3 && id == digest.as_str() && super::portable_member(parts[2]);
    }
    parts[1].strip_prefix("bundle-").is_some_and(hex)
        && super::portable_member(&parts[2..].join("/"))
}
fn managed_directory(root: &Path, path: &Path) -> bool {
    let parts = parts(root, path);
    !parts.is_empty()
        && namespace(parts[0])
        && (parts.len() == 1
            || parts[1]
                .strip_prefix("bundle-")
                .or_else(|| parts[1].strip_prefix("export-"))
                .is_some_and(hex)
                && (parts.len() == 2 || super::portable_member(&parts[2..].join("/"))))
}
fn pending_name(root: &Path, path: &Path) -> bool {
    let parts = parts(root, path);
    if parts.len() != 2 || !namespace(parts[0]) {
        return false;
    }
    let Some(name) = parts[1].strip_prefix(".pending-") else {
        return false;
    };
    let Some((pid, nonce)) = name.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && !nonce.is_empty()
        && pid.bytes().chain(nonce.bytes()).all(|b| b.is_ascii_digit())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    fn fixture() -> (MaterialRoot, PathBuf) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.cache");
        std::fs::create_dir_all(&cache).unwrap();
        let path = cache.canonicalize().unwrap().join(format!(
            "material-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        platform_private_storage::directory(&path).unwrap();
        let path = path.canonicalize().unwrap();
        (
            MaterialRoot::open_bound(&path, &"a".repeat(64), false).unwrap(),
            path,
        )
    }
    fn registry_paths(registry: &crate::MaterialRegistry) -> BTreeSet<PathBuf> {
        let mut snapshot = registry.retained_materials().unwrap();
        let mut paths = BTreeSet::new();
        while !snapshot.complete() {
            paths.extend(snapshot.step().unwrap());
        }
        paths
    }
    fn finish_reclamation(
        root: &mut MaterialRoot,
        candidates: &BTreeMap<PathBuf, Digest>,
        retained: &BTreeSet<PathBuf>,
    ) -> Result<Reclaimed, Error> {
        let mut pass = root.begin_reclamation();
        while pass.phase() == ReclaimPhase::Inventory {
            root.reclaim_step(&mut pass)?;
        }
        pass.prove(candidates.clone(), retained.clone())?;
        while pass.phase() != ReclaimPhase::Complete {
            root.reclaim_step(&mut pass)?;
        }
        pass.into_result()
    }
    fn digest(bytes: &[u8]) -> Digest {
        Digest::new(format!("{:x}", Sha256::digest(bytes))).unwrap()
    }
    fn publish(root: &mut MaterialRoot, bytes: &[u8]) -> (PathBuf, Digest) {
        let digest = digest(bytes);
        (
            root.publish(None, bytes, &digest, bytes.len() as u64)
                .unwrap(),
            digest,
        )
    }
    #[test]
    fn incremental_hash_preserves_candidates_until_complete_and_rejects_late_aliases() {
        let (mut root, path) = fixture();
        let bytes = vec![7; 3 * 1024 * 1024];
        let (source, digest) = publish(&mut root, &bytes);
        let mut pass = root.begin_reclamation();
        while pass.phase() == ReclaimPhase::Inventory {
            root.reclaim_step(&mut pass).unwrap();
        }
        pass.prove(BTreeMap::from([(source.clone(), digest)]), BTreeSet::new())
            .unwrap();
        root.reclaim_step(&mut pass).unwrap(); // retained coordinates
        root.reclaim_step(&mut pass).unwrap(); // first MiB
        assert_eq!(pass.phase(), ReclaimPhase::Verifying);
        assert!(source.exists());
        while pass.phase() == ReclaimPhase::Verifying {
            root.reclaim_step(&mut pass).unwrap();
        }
        let outside = path.with_extension("late-alias");
        std::fs::hard_link(&source, &outside).unwrap();
        assert!(matches!(root.reclaim_step(&mut pass), Err(Error::Conflict)));
        assert!(source.exists());
        std::fs::remove_file(outside).unwrap();
        drop(pass);
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn inventories_cannot_cross_publication_or_owner_lifetimes() {
        let (mut root, path) = fixture();
        let mut before_publish = root.begin_reclamation();
        publish(&mut root, b"new material");
        assert!(matches!(
            root.reclaim_step(&mut before_publish),
            Err(Error::Conflict)
        ));
        drop(before_publish);
        drop(root);
        let root = MaterialRoot::open_bound(&path, &"a".repeat(64), false).unwrap();
        let mut before_reopen = root.begin_reclamation();
        drop(root);
        let mut root = MaterialRoot::open_bound(&path, &"a".repeat(64), false).unwrap();
        assert!(matches!(
            root.reclaim_step(&mut before_reopen),
            Err(Error::Conflict)
        ));
        drop(before_reopen);
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn exclusive_owner_and_binding_survive_reopen_without_overwrite() {
        let (root, path) = fixture();
        let before = std::fs::read(path.join(".owner")).unwrap();
        assert!(matches!(
            MaterialRoot::open_bound(&path, &"a".repeat(64), false),
            Err(Error::Conflict)
        ));
        drop(root);
        assert!(matches!(
            MaterialRoot::open_bound(&path, &"b".repeat(64), false),
            Err(Error::Configuration)
        ));
        assert_eq!(std::fs::read(path.join(".owner")).unwrap(), before);
        drop(MaterialRoot::open_bound(&path, &"a".repeat(64), false).unwrap());
        std::fs::write(path.join(".owner"), b"partial").unwrap();
        assert!(matches!(
            MaterialRoot::open_bound(&path, &"a".repeat(64), false),
            Err(Error::Configuration)
        ));
        assert_eq!(std::fs::read(path.join(".owner")).unwrap(), b"partial");
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn hardlinks_charge_bytes_once_and_live_aliases_protect_settled_material() {
        let (mut root, path) = fixture();
        let bytes = vec![b'x'; 256];
        let (source, digest) = publish(&mut root, &bytes);
        let alias = root
            .native_export(&source, &digest, "formula.rb", None)
            .unwrap();
        // owner + namespace + source + export directory + alias; exactly 256 physical bytes.
        root.quota(MAX_BYTES - 64 - 256, MAX_ENTRIES - 5).unwrap();
        assert!(matches!(
            root.quota(MAX_BYTES - 64 - 255, 0),
            Err(Error::Capacity)
        ));
        assert!(matches!(
            root.quota(0, MAX_ENTRIES - 4),
            Err(Error::Capacity)
        ));
        let candidates =
            BTreeMap::from([(source.clone(), digest.clone()), (alias.clone(), digest)]);
        assert_eq!(
            finish_reclamation(&mut root, &candidates, &BTreeSet::from([alias.clone()]))
                .unwrap()
                .files,
            0
        );
        assert!(source.exists() && alias.exists());
        let result = finish_reclamation(&mut root, &candidates, &BTreeSet::new()).unwrap();
        assert_eq!((result.files, result.bytes), (2, 256));
        root.quota(MAX_BYTES - 64, MAX_ENTRIES - 1).unwrap();
        assert_eq!(
            finish_reclamation(&mut root, &candidates, &BTreeSet::new()).unwrap(),
            Reclaimed::default()
        );
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn bundle_members_share_quota_and_reclaim_with_their_digest_sources() {
        let (mut root, path) = fixture();
        let bytes = b"printf bundle";
        let member_digest: [u8; 32] = Sha256::digest(bytes).into();
        let manifest = execution_contract::BundleManifest {
            schema: execution_contract::V1,
            platform: execution_contract::Platform::Macos,
            architecture: execution_contract::PackageValue::new("aarch64").unwrap(),
            entries: BTreeMap::from([(
                "nested/install.sh".into(),
                execution_contract::BundleFile {
                    length: bytes.len() as u64,
                    sha256: member_digest,
                },
            )]),
        };
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        archive.start_file("manifest.json", options).unwrap();
        archive
            .write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        archive.start_file("nested/install.sh", options).unwrap();
        archive.write_all(bytes).unwrap();
        let archive = archive.finish().unwrap().into_inner();
        let (source, archive_digest) = publish(&mut root, &archive);
        let members = root
            .bundle(None, &source, &archive_digest, &manifest)
            .unwrap();
        let member = members["nested/install.sh"].clone();
        let digest = digest(bytes);
        let base = path.join("system").join(digest.as_str());
        root.quota(
            MAX_BYTES - 64 - archive.len() as u64 - bytes.len() as u64,
            0,
        )
        .unwrap();
        let candidates = BTreeMap::from([
            (source, archive_digest),
            (base, digest.clone()),
            (member, digest),
        ]);
        let result = finish_reclamation(&mut root, &candidates, &BTreeSet::new()).unwrap();
        assert_eq!(
            (result.files, result.bytes),
            (3, archive.len() as u64 + bytes.len() as u64)
        );
        root.quota(MAX_BYTES - 64, MAX_ENTRIES - 1).unwrap();
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn physical_quota_refuses_publication_and_settlement_restores_capacity() {
        let (mut root, path) = fixture();
        let (source, old_digest) = publish(&mut root, b"old material");
        let filler = path.join("unknown-sparse-object");
        File::create(&filler)
            .unwrap()
            .set_len(MAX_BYTES - 64 - 12)
            .unwrap();
        let fresh = digest(b"new");
        assert!(matches!(
            root.publish(None, b"new".as_slice(), &fresh, 3),
            Err(Error::Capacity)
        ));
        assert!(!path.join("system").join(fresh.as_str()).exists());
        assert_eq!(
            finish_reclamation(
                &mut root,
                &BTreeMap::from([(source, old_digest)]),
                &BTreeSet::new()
            )
            .unwrap()
            .bytes,
            12
        );
        assert!(
            filler.exists(),
            "unknown-origin bytes cannot be reclaimed to evade the limit"
        );
        root.publish(None, b"new".as_slice(), &fresh, 3).unwrap();
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn unknown_origin_and_unenumerated_links_are_preserved() {
        let (mut root, path) = fixture();
        let (source, digest) = publish(&mut root, b"settled");
        let (unknown, _) = publish(&mut root, b"no journal proof");
        let alias = path.join("unknown-origin");
        std::fs::hard_link(&source, &alias).unwrap();
        let candidates = BTreeMap::from([(source.clone(), digest)]);
        assert_eq!(
            finish_reclamation(&mut root, &candidates, &BTreeSet::new())
                .unwrap()
                .files,
            0
        );
        std::fs::remove_file(alias).unwrap();
        let outside = path.with_extension("alias");
        std::fs::hard_link(&source, &outside).unwrap();
        assert_eq!(
            finish_reclamation(&mut root, &candidates, &BTreeSet::new())
                .unwrap()
                .files,
            0
        );
        std::fs::remove_file(outside).unwrap();
        assert_eq!(
            finish_reclamation(&mut root, &candidates, &BTreeSet::new())
                .unwrap()
                .files,
            1
        );
        assert!(unknown.exists());
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn failed_proof_preserves_every_candidate_and_partial_reclaim_is_restartable() {
        let (mut root, path) = fixture();
        let (first, digest) = publish(&mut root, b"first");
        let alias = root
            .native_export(&first, &digest, "payload.pkg", None)
            .unwrap();
        let (second, second_digest) = publish(&mut root, b"second");
        let mut candidates = BTreeMap::from([
            (first.clone(), digest.clone()),
            (alias.clone(), digest),
            (second.clone(), second_digest),
        ]);
        let missing = path.join("system").join("0".repeat(64));
        assert!(finish_reclamation(&mut root, &candidates, &BTreeSet::from([missing])).is_err());
        assert!(first.exists() && second.exists());
        std::fs::write(&second, b"corruption").unwrap();
        assert!(finish_reclamation(&mut root, &candidates, &BTreeSet::new()).is_err());
        assert!(first.exists() && alias.exists());
        candidates.remove(&second);
        // Simulate interruption after one alias was deleted. The next owner completes the
        // same candidate set without reconstructing task state or resetting the root.
        std::fs::remove_file(&first).unwrap();
        drop(root);
        let mut root = MaterialRoot::open_bound(&path, &"a".repeat(64), false).unwrap();
        assert_eq!(
            finish_reclamation(&mut root, &candidates, &BTreeSet::new())
                .unwrap()
                .files,
            1
        );
        assert!(!alias.exists() && second.exists());
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn live_registry_lease_blocks_retirement_and_disk_reclamation() {
        let (mut root, path) = fixture();
        let (source, digest) = publish(&mut root, b"live helper material");
        let plan = execution_contract::FrozenExecution::freeze(
            execution_contract::decode_execution(
                include_bytes!("../../../execution-contract/tests/fixtures/plan.json"),
                &execution_app::test_execution_limits(),
            )
            .unwrap(),
            &execution_app::test_execution_limits(),
        )
        .unwrap();
        let registry = crate::MaterialRegistry::from_materials(
            BTreeMap::from([(
                plan.digest().as_str().to_owned(),
                crate::Artifacts {
                    interpreter: "/bin/sh".into(),
                    content: source.clone(),
                    work_root: path.parent().unwrap().to_owned(),
                    program: vec![],
                    delegate: None,
                    controlled_input: None,
                    fixture_owned: true,
                },
            )]),
            1,
        )
        .unwrap();
        let running = registry.get(plan.digest().as_str()).unwrap().unwrap();
        assert!(matches!(
            registry.retire(plan.digest()),
            Err(Error::Conflict)
        ));
        let candidates = BTreeMap::from([(source.clone(), digest)]);
        assert_eq!(
            finish_reclamation(&mut root, &candidates, &registry_paths(&registry))
                .unwrap()
                .files,
            0
        );
        drop(running);
        registry.retire(plan.digest()).unwrap();
        assert_eq!(
            finish_reclamation(&mut root, &candidates, &registry_paths(&registry))
                .unwrap()
                .files,
            1
        );
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn abandoned_pending_cleanup_obeys_the_same_reference_and_owner_lease() {
        let (mut root, path) = fixture();
        let (source, _) = publish(&mut root, b"pending");
        let pending = source.parent().unwrap().join(".pending-123-456");
        std::fs::hard_link(&source, &pending).unwrap();
        assert_eq!(
            finish_reclamation(
                &mut root,
                &BTreeMap::new(),
                &BTreeSet::from([source.clone()])
            )
            .unwrap()
            .files,
            0
        );
        // Unknown source remains, so even its temporary alias is preserved conservatively.
        assert_eq!(
            finish_reclamation(&mut root, &BTreeMap::new(), &BTreeSet::new())
                .unwrap()
                .files,
            0
        );
        std::fs::remove_file(source).unwrap();
        let malformed = path.join("system/.pending-unknown");
        std::fs::write(&malformed, b"unknown").unwrap();
        assert_eq!(
            finish_reclamation(&mut root, &BTreeMap::new(), &BTreeSet::new())
                .unwrap()
                .files,
            1
        );
        assert!(malformed.exists());
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
}
