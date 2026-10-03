//! Attempt-local removal: isolate names atomically, then verify the retained object.
//! ref: rustix v1.1.2 src/fs/at.rs and src/backend/libc/fs/syscalls.rs:567-605.
//! unlinkat has no inode precondition; a pre-unlink pathname check is insufficient.
use super::*;

const ROOT_SLOT: &str = "removing.app";
const ENTRY_SLOT: &str = "deleting";

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObjectIdentity {
    device: u64,
    inode: u64,
    kind: u32,
}
impl ObjectIdentity {
    fn file(file: &File) -> Result<Self, Error> {
        let meta = file.metadata()?;
        Ok(Self {
            device: meta.dev(),
            inode: meta.ino(),
            kind: meta.mode() & u32::from(libc::S_IFMT),
        })
    }
    fn entry(parent: &File, name: &OsStr) -> Result<Self, Error> {
        let meta = stat_at(parent, name)?;
        Ok(Self {
            device: meta.st_dev as u64,
            inode: meta.st_ino,
            kind: u32::from(meta.st_mode & libc::S_IFMT),
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Disposition {
    DeleteVerified,
    RestoreOnly,
}

/// This is transient resource ownership for one exact attempt, never software provenance.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RemovalState {
    tree: Tree,
    objects: BTreeMap<PathBuf, ObjectIdentity>,
    pending: Option<PathBuf>,
    disposition: Disposition,
}

pub(super) fn same_entry(parent: &File, name: &OsStr, retained: &File) -> Result<(), Error> {
    if ObjectIdentity::entry(parent, name)? != ObjectIdentity::file(retained)? {
        return Err(Error::Untrusted);
    }
    Ok(())
}
pub(super) fn same_path(path: &Path, retained: &File) -> Result<(), Error> {
    let meta = fs::symlink_metadata(path)?;
    let actual = ObjectIdentity {
        device: meta.dev(),
        inode: meta.ino(),
        kind: meta.mode() & u32::from(libc::S_IFMT),
    };
    if actual != ObjectIdentity::file(retained)? {
        return Err(Error::Untrusted);
    }
    Ok(())
}
fn rename_at(source: &File, name: &OsStr, target: &File, slot: &OsStr) -> Result<(), Error> {
    let name = CString::new(name.as_bytes()).map_err(|_| Error::Protocol)?;
    let slot = CString::new(slot.as_bytes()).map_err(|_| Error::Protocol)?;
    // SAFETY: both owned directory descriptors and both NUL-terminated names remain live.
    if unsafe {
        libc::renameatx_np(
            source.as_raw_fd(),
            name.as_ptr(),
            target.as_raw_fd(),
            slot.as_ptr(),
            libc::RENAME_EXCL,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    source.sync_all()?;
    target.sync_all()?;
    Ok(())
}
fn unlink_at(parent: &File, name: &str, directory: bool) -> Result<(), Error> {
    let name = CString::new(name).map_err(|_| Error::Protocol)?;
    // SAFETY: the owned directory descriptor and NUL-terminated name remain live.
    if unsafe {
        libc::unlinkat(
            parent.as_raw_fd(),
            name.as_ptr(),
            if directory { libc::AT_REMOVEDIR } else { 0 },
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    parent.sync_all()?;
    Ok(())
}
fn has_entry(parent: &File, name: &str) -> Result<bool, Error> {
    let name = CString::new(name).map_err(|_| Error::Protocol)?;
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: live directory descriptor, terminated name and valid output buffer.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::NotFound {
        Ok(false)
    } else {
        Err(error.into())
    }
}
fn snapshot(root: &File) -> Result<RemovalState, Error> {
    let tree = inventory_from(root)?;
    let mut objects = BTreeMap::new();
    for (path, entry) in &tree {
        objects.insert(
            path.clone(),
            ObjectIdentity::file(&source_entry(root, path, entry)?)?,
        );
    }
    // Detect changes between the content observation and identity capture.
    if inventory_from(root)? != tree {
        return Err(Error::Untrusted);
    }
    Ok(RemovalState {
        tree,
        objects,
        pending: None,
        disposition: Disposition::DeleteVerified,
    })
}
fn parent_for(root: &File, relative: &Path, state: &RemovalState) -> Result<Option<File>, Error> {
    let mut parent = root.try_clone()?;
    let mut path = PathBuf::new();
    for component in relative.parent().ok_or(Error::Untrusted)?.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(Error::Untrusted);
        }
        path.push(component.as_os_str());
        if !has_entry(
            &parent,
            component.as_os_str().to_str().ok_or(Error::Untrusted)?,
        )? {
            return Ok(None);
        }
        parent = open_at(
            &parent,
            component.as_os_str(),
            libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )?;
        if state.objects.get(&path) != Some(&ObjectIdentity::file(&parent)?) {
            return Err(Error::Untrusted);
        }
    }
    Ok(Some(parent))
}
fn check_content(parent: &File, name: &str, expected: &Entry) -> Result<(), Error> {
    let file = open_entry(parent, OsStr::new(name))?;
    match expected {
        Entry::Directory { .. } if file.metadata()?.is_dir() => (),
        Entry::Link { target }
            if file.metadata()?.file_type().is_symlink()
                && link_at(parent, OsStr::new(name), &file)? == *target =>
        {
            ()
        }
        Entry::File { length, sha256, .. }
            if file.metadata()?.is_file() && file.metadata()?.len() == *length =>
        {
            use sha2::{Digest as _, Sha256};
            let mut hash = Sha256::new();
            let mut file = file.take(length.checked_add(1).ok_or(Error::Capacity)?);
            let mut buffer = [0u8; 65536];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            if format!("{:x}", hash.finalize()) != *sha256 {
                return Err(Error::Untrusted);
            }
        }
        _ => return Err(Error::Untrusted),
    }
    Ok(())
}

pub(super) fn is_removal(request: &WorkerRequest) -> Result<bool, Error> {
    let (_, current, next) = image_state_paths(request)?;
    if !present(&current)? && !present(&next)? {
        return Ok(true);
    }
    Ok(state(request)?.removal.is_some())
}

pub(super) fn authorize_version(
    operation: WorkerOperation,
    application: &Application,
    observed: &SoftwareState,
    before: Option<&SoftwareState>,
) -> Result<(), Error> {
    if !matches!(observed, SoftwareState::Present { .. }) || Some(observed) != before {
        return Err(Error::Untrusted);
    }
    if operation == WorkerOperation::Uninstall
        && *observed
            != (SoftwareState::Present {
                version: package_value(&application.version)?,
            })
    {
        return Err(Error::Untrusted);
    }
    Ok(())
}

pub(super) fn execute(
    request: &WorkerRequest,
    dmg: &Dmg,
    application: &Application,
    before: Option<&SoftwareState>,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    let installed = target(request, dmg, application)?;
    let parent = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(installed.parent().ok_or(Error::Untrusted)?)?;
    let root = open_at(
        &parent,
        OsStr::new(&application.target_name),
        libc::O_DIRECTORY | libc::O_NOFOLLOW,
    )?;
    let observed = bundle_from(request, &installed, application, &root)?;
    authorize_version(request.operation, application, &observed, before)?;
    require_not_running(&installed)?;
    let removal = snapshot(&root)?;
    same_path(&installed, &root)?;
    same_path(installed.parent().ok_or(Error::Untrusted)?, &parent)?;
    let mut record = ImageState {
        owner: owner(request)?,
        device: None,
        stage: None,
        alternate: None,
        partial_stage: false,
        group: unsafe { libc::getpgrp() },
        removal: Some(removal),
    };
    // SAFETY: getpgrp only observes the current process group; it dereferences no pointers.
    let (resource_parent, _, _) = image_state_paths(request)?;
    if resource_parent.metadata()?.dev() != parent.metadata()?.dev() {
        return Err(Error::Unsupported);
    }
    create_image_root(request, &record)?;
    let private = owned_root(request)?;
    rename_at(
        &parent,
        OsStr::new(&application.target_name),
        &private,
        OsStr::new(ROOT_SLOT),
    )?;
    drain(request, dmg, &mut record)?;
    Ok(result(0, None, String::new()))
}

fn restore(
    request: &WorkerRequest,
    dmg: &Dmg,
    record: &mut ImageState,
    private: &File,
) -> Result<(), Error> {
    let Payload::AppCopy { application, .. } = &dmg.payload else {
        return Err(Error::Untrusted);
    };
    let removal = record.removal.as_ref().ok_or(Error::Untrusted)?;
    if let Some(relative) = &removal.pending {
        if has_entry(private, ENTRY_SLOT)? {
            let root = open_at(
                private,
                OsStr::new(ROOT_SLOT),
                libc::O_DIRECTORY | libc::O_NOFOLLOW,
            )?;
            let parent = parent_for(&root, relative, removal)?.ok_or(Error::Untrusted)?;
            rename_at(
                private,
                OsStr::new(ENTRY_SLOT),
                &parent,
                relative.file_name().ok_or(Error::Untrusted)?,
            )?;
        }
        record.removal.as_mut().ok_or(Error::Untrusted)?.pending = None;
        save(request, record)?;
    }
    if has_entry(private, ROOT_SLOT)? {
        let installed = target(request, dmg, application)?;
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(installed.parent().ok_or(Error::Untrusted)?)?;
        rename_at(
            private,
            OsStr::new(ROOT_SLOT),
            &parent,
            OsStr::new(&application.target_name),
        )?;
    }
    remove_image_root(request)
}
fn refuse(
    request: &WorkerRequest,
    dmg: &Dmg,
    record: &mut ImageState,
    private: &File,
) -> Result<(), Error> {
    record.removal.as_mut().ok_or(Error::Untrusted)?.disposition = Disposition::RestoreOnly;
    save(request, record)?;
    // Recovery is no-overwrite; a failed restore retains the unapproved object and state.
    let _ = restore(request, dmg, record, private);
    Err(Error::Untrusted)
}
fn finish_pending(
    request: &WorkerRequest,
    dmg: &Dmg,
    record: &mut ImageState,
    private: &File,
) -> Result<(), Error> {
    let removal = record.removal.as_ref().ok_or(Error::Untrusted)?;
    let Some(relative) = &removal.pending else {
        return Ok(());
    };
    if has_entry(private, ENTRY_SLOT)? {
        let expected = removal.objects.get(relative).ok_or(Error::Untrusted)?;
        let entry = removal.tree.get(relative).ok_or(Error::Untrusted)?;
        if ObjectIdentity::entry(private, OsStr::new(ENTRY_SLOT))? != *expected
            || check_content(private, ENTRY_SLOT, entry).is_err()
        {
            return refuse(request, dmg, record, private);
        }
        unlink_at(
            private,
            ENTRY_SLOT,
            matches!(entry, Entry::Directory { .. }),
        )?;
    }
    record.removal.as_mut().ok_or(Error::Untrusted)?.pending = None;
    save(request, record)
}
fn drain(request: &WorkerRequest, dmg: &Dmg, record: &mut ImageState) -> Result<(), Error> {
    let private = owned_root(request)?;
    let removal = record.removal.as_ref().ok_or(Error::Untrusted)?;
    if removal.disposition == Disposition::RestoreOnly {
        return restore(request, dmg, record, &private);
    }
    if !has_entry(&private, ROOT_SLOT)? {
        if has_entry(&private, ENTRY_SLOT)? {
            return Err(Error::Untrusted);
        }
        return remove_image_root(request);
    }
    if removal.objects.get(Path::new(""))
        != Some(&ObjectIdentity::entry(&private, OsStr::new(ROOT_SLOT))?)
    {
        return refuse(request, dmg, record, &private);
    }
    let root = open_at(
        &private,
        OsStr::new(ROOT_SLOT),
        libc::O_DIRECTORY | libc::O_NOFOLLOW,
    )?;
    finish_pending(request, dmg, record, &private)?;
    let paths: Vec<_> = record
        .removal
        .as_ref()
        .ok_or(Error::Untrusted)?
        .tree
        .keys()
        .filter(|p| !p.as_os_str().is_empty())
        .rev()
        .cloned()
        .collect();
    for relative in paths {
        let removal = record.removal.as_ref().ok_or(Error::Untrusted)?;
        let parent = match parent_for(&root, &relative, removal)? {
            Some(parent) => parent,
            None => continue,
        };
        let name = relative.file_name().ok_or(Error::Untrusted)?.to_os_string();
        if !has_entry(&parent, name.to_str().ok_or(Error::Untrusted)?)? {
            continue;
        }
        // Commit recovery identity before the atomic isolation. The post-rename identity,
        // rather than the source name's earlier contents, decides deletion permission.
        record.removal.as_mut().ok_or(Error::Untrusted)?.pending = Some(relative);
        save(request, record)?;
        rename_at(&parent, &name, &private, OsStr::new(ENTRY_SLOT))?;
        finish_pending(request, dmg, record, &private)?;
    }
    same_entry(&private, OsStr::new(ROOT_SLOT), &root)?;
    unlink_at(&private, ROOT_SLOT, true)?;
    remove_image_root(request)
}

pub(super) fn cleanup(
    request: &WorkerRequest,
    dmg: &Dmg,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    if discard_unpublished_image_state(request)? {
        return Ok(result(0, None, String::new()));
    }
    let mut record = state(request)?;
    // An alive original group may still hold or mutate its private resources.
    if record.group > 1 && record.group != unsafe { libc::getpgrp() } {
        // SAFETY: signal 0 observes existence and does not signal a process.
        let exists = unsafe { libc::kill(-record.group, 0) };
        if exists == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            return Err(Error::Unavailable);
        }
    }
    drain(request, dmg, &mut record)?;
    Ok(result(0, None, String::new()))
}

#[cfg(test)]
mod tests {
    use super::super::tests::{image_request, initial_state};
    use super::*;

    fn isolated() -> (PathBuf, WorkerRequest, ImageState) {
        let (parent, request) = image_request();
        let source = parent.join("independent.app");
        fs::create_dir_all(source.join("Contents")).unwrap();
        fs::write(
            source.join("Contents/local-data"),
            b"current app, independent of image",
        )
        .unwrap();
        let source = File::open(&source).unwrap();
        let mut record = initial_state(&request);
        record.removal = Some(snapshot(&source).unwrap());
        create_image_root(&request, &record).unwrap();
        rename_at(
            &File::open(&parent).unwrap(),
            OsStr::new("independent.app"),
            &owned_root(&request).unwrap(),
            OsStr::new(ROOT_SLOT),
        )
        .unwrap();
        (parent, request, record)
    }

    #[test]
    fn ordinary_uninstall_binds_declared_version_while_remove_previous_binds_observed_old_version()
    {
        let application = Application {
            path: "new.app".into(),
            bundle_id: "org.rss.test".into(),
            version: "2.0".into(),
            target_name: "Target.app".into(),
        };
        let old = SoftwareState::Present {
            version: package_value("1.0").unwrap(),
        };
        let new = SoftwareState::Present {
            version: package_value("2.0").unwrap(),
        };
        assert!(
            authorize_version(WorkerOperation::Uninstall, &application, &old, Some(&old)).is_err()
        );
        assert!(
            authorize_version(WorkerOperation::Uninstall, &application, &new, Some(&new)).is_ok()
        );
        assert!(authorize_version(
            WorkerOperation::RemovePrevious,
            &application,
            &old,
            Some(&old)
        )
        .is_ok());
        assert!(authorize_version(
            WorkerOperation::RemovePrevious,
            &application,
            &new,
            Some(&old)
        )
        .is_err());
        assert!(authorize_version(
            WorkerOperation::Uninstall,
            &application,
            &SoftwareState::Absent {},
            Some(&old)
        )
        .is_err());
    }

    #[test]
    fn independent_current_tree_is_deleted_and_recovery_reopens_each_atomic_boundary() {
        // Begin is committed before rename; interruption can also follow rename or unlink.
        for boundary in 0..3 {
            let (parent, request, mut record) = isolated();
            let private = owned_root(&request).unwrap();
            let root = File::open(request.resource_root.join(ROOT_SLOT)).unwrap();
            record.removal.as_mut().unwrap().pending = Some("Contents/local-data".into());
            save(&request, &record).unwrap();
            if boundary > 0 {
                rename_at(
                    &open_at(
                        &root,
                        OsStr::new("Contents"),
                        libc::O_DIRECTORY | libc::O_NOFOLLOW,
                    )
                    .unwrap(),
                    OsStr::new("local-data"),
                    &private,
                    OsStr::new(ENTRY_SLOT),
                )
                .unwrap();
            }
            if boundary > 1 {
                unlink_at(&private, ENTRY_SLOT, false).unwrap();
            }
            drop(record);
            let wire::SoftwareTaskBehavior::Dmg(dmg) = &request.action.behavior else {
                unreachable!()
            };
            assert_eq!(cleanup(&request, dmg).unwrap().0, 0);
            assert!(!request.resource_root.exists());
            let (_, current, next) = image_state_paths(&request).unwrap();
            assert!(!current.exists() && !next.exists());
            assert_eq!(
                fs::read(parent.join("image.dmg")).unwrap(),
                b"unmounted test image"
            );
            fs::remove_dir_all(parent).unwrap();
        }
    }

    #[test]
    fn replaced_leaf_with_identical_bytes_is_preserved_and_cannot_be_deleted_by_cleanup() {
        let (parent, request, mut record) = isolated();
        let root = request.resource_root.join(ROOT_SLOT);
        fs::rename(root.join("Contents/local-data"), parent.join("original")).unwrap();
        fs::copy(parent.join("original"), root.join("Contents/local-data")).unwrap();
        let wire::SoftwareTaskBehavior::Dmg(dmg) = &request.action.behavior else {
            unreachable!()
        };
        assert!(matches!(
            drain(&request, dmg, &mut record),
            Err(Error::Untrusted)
        ));
        assert_eq!(
            fs::read(request.resource_root.join(ENTRY_SLOT)).unwrap(),
            fs::read(parent.join("original")).unwrap()
        );
        assert!(matches!(
            state(&request).unwrap().removal.unwrap().disposition,
            Disposition::RestoreOnly
        ));
        assert!(cleanup(&request, dmg).is_err());
        assert!(request.resource_root.join(ENTRY_SLOT).exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn replaced_root_or_parent_and_escaping_links_are_rejected() {
        let (parent, request, mut record) = isolated();
        let root_path = request.resource_root.join(ROOT_SLOT);
        let root = File::open(&root_path).unwrap();
        fs::rename(root_path.join("Contents"), parent.join("original-contents")).unwrap();
        fs::create_dir(root_path.join("Contents")).unwrap();
        assert!(parent_for(
            &root,
            Path::new("Contents/local-data"),
            record.removal.as_ref().unwrap()
        )
        .is_err());
        fs::rename(&root_path, parent.join("original-root")).unwrap();
        fs::create_dir(&root_path).unwrap();
        fs::write(root_path.join("unapproved"), b"preserve").unwrap();
        let wire::SoftwareTaskBehavior::Dmg(dmg) = &request.action.behavior else {
            unreachable!()
        };
        assert!(matches!(
            drain(&request, dmg, &mut record),
            Err(Error::Untrusted)
        ));
        assert_eq!(fs::read(root_path.join("unapproved")).unwrap(), b"preserve");
        std::os::unix::fs::symlink(&parent, root_path.join("escape")).unwrap();
        assert!(snapshot(&File::open(&root_path).unwrap()).is_err());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn unknown_private_entries_prevent_resource_closure() {
        let (parent, request, _) = isolated();
        fs::write(request.resource_root.join("unowned"), b"preserve").unwrap();
        let wire::SoftwareTaskBehavior::Dmg(dmg) = &request.action.behavior else {
            unreachable!()
        };
        assert!(cleanup(&request, dmg).is_err());
        assert_eq!(
            fs::read(request.resource_root.join("unowned")).unwrap(),
            b"preserve"
        );
        assert!(image_state_paths(&request).unwrap().1.exists());
        fs::remove_file(request.resource_root.join("unowned")).unwrap();
        assert_eq!(cleanup(&request, dmg).unwrap().0, 0);
        fs::remove_dir_all(parent).unwrap();
    }
}
