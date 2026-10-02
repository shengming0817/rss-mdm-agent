// ref: Munki installer/dmg.py and dmgutils.py; Apple copyfile.h and renameatx_np(2).
use super::*;
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        },
    },
    path::{Component, Path},
};
use wire::{
    SoftwareTaskDmg as Dmg, SoftwareTaskDmgPayload as Payload,
    SoftwareTaskMacApplication as Application,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Entry {
    Directory {
        mode: u32,
    },
    File {
        mode: u32,
        length: u64,
        sha256: String,
    },
    Link {
        target: PathBuf,
    },
}
type Tree = BTreeMap<PathBuf, Entry>;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageState {
    owner: String,
    device: Option<String>,
    stage: Option<Tree>,
    alternate: Option<Tree>,
}
fn owner(request: &WorkerRequest) -> Result<String, Error> {
    use sha2::{Digest as _, Sha256};
    let bytes = serde_json::to_vec(&(
        &request.action,
        &request.materials,
        &request.tools,
        &request.resource_root,
        &request.run_as,
        &request.session,
        request.architecture,
    ))
    .map_err(|_| Error::Protocol)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn owned_root(request: &WorkerRequest) -> Result<File, Error> {
    let meta = fs::symlink_metadata(&request.resource_root)?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o077 != 0
    {
        return Err(Error::Untrusted);
    }
    Ok(OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&request.resource_root)?)
}
fn state(request: &WorkerRequest) -> Result<ImageState, Error> {
    let _root = owned_root(request)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(request.resource_root.join("owner.json"))?;
    if file.metadata()?.len() > 4 * 1024 * 1024 {
        return Err(Error::Capacity);
    }
    let mut bytes = Vec::new();
    file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    let state: ImageState = serde_json::from_slice(&bytes).map_err(|_| Error::Untrusted)?;
    if state.owner != owner(request)? {
        return Err(Error::Untrusted);
    }
    Ok(state)
}
fn save(request: &WorkerRequest, state: &ImageState) -> Result<(), Error> {
    let _root = owned_root(request)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(request.resource_root.join("owner.next"))?;
    file.write_all(&serde_json::to_vec(state).map_err(|_| Error::Protocol)?)?;
    file.sync_all()?;
    fs::rename(
        request.resource_root.join("owner.next"),
        request.resource_root.join("owner.json"),
    )?;
    _root.sync_all()?;
    Ok(())
}
fn plist(bytes: &[u8]) -> Result<plist::Value, Error> {
    plist::Value::from_reader(std::io::Cursor::new(bytes)).map_err(|_| Error::Protocol)
}
fn native(
    request: &WorkerRequest,
    tool: &str,
    args: &[&str],
) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
    run_tool(
        request,
        &request.tool(tool)?,
        &args.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>(),
    )
}
fn attach(request: &WorkerRequest, dmg: &Dmg) -> Result<(i32, SoftwareWorkerResult), Error> {
    let parent = request.resource_root.parent().ok_or(Error::Configuration)?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o022 != 0 {
        return Err(Error::Untrusted);
    }
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&request.resource_root)?;
    let mut record = ImageState {
        owner: owner(request)?,
        device: None,
        stage: None,
        alternate: None,
    };
    save(request, &record)?;
    let mount = request.resource_root.join("volume");
    fs::DirBuilder::new().mode(0o700).create(&mount)?;
    let image = request.material(&dmg.image)?;
    if !request.action.signatures.is_empty() {
        verify_code(request, &image, "open")?;
    }
    let (code, output, error) = native(
        request,
        "hdiutil",
        &[
            "attach",
            "-readonly",
            "-nobrowse",
            "-noautoopen",
            "-owners",
            "on",
            "-mountpoint",
            text(&mount)?,
            "-plist",
            text(&image)?,
        ],
    )?;
    if code != 0 {
        return Ok(result(
            code,
            None,
            String::from_utf8_lossy(&error).into_owned(),
        ));
    }
    let data = plist(&output)?;
    let entities = data
        .as_dictionary()
        .and_then(|d| d.get("system-entities"))
        .and_then(plist::Value::as_array)
        .ok_or(Error::Protocol)?;
    let mounted = entities
        .iter()
        .filter(|e| {
            e.as_dictionary()
                .is_some_and(|d| d.contains_key("mount-point"))
        })
        .collect::<Vec<_>>();
    let roots = entities
        .iter()
        .filter_map(|e| e.as_dictionary()?.get("dev-entry")?.as_string())
        .filter(|d| disk_root(d))
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(Error::Untrusted);
    }
    record.device = Some(roots[0].into());
    save(request, &record)?;
    if mounted.len() != 1
        || mounted[0]
            .as_dictionary()
            .and_then(|d| d.get("mount-point"))
            .and_then(plist::Value::as_string)
            != Some(text(&mount)?)
    {
        return Err(Error::Untrusted);
    }
    readonly(&mount)?;
    let (code, info, _) = native(request, "diskutil", &["info", "-plist", text(&mount)?])?;
    if code != 0
        || plist(&info)?
            .as_dictionary()
            .and_then(|d| d.get("VolumeName"))
            .and_then(plist::Value::as_string)
            != Some(dmg.volume.as_str())
    {
        return Err(Error::Untrusted);
    }
    Ok(result(0, None, String::new()))
}
fn disk_root(device: &str) -> bool {
    device
        .strip_prefix("/dev/disk")
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}
fn readonly(path: &Path) -> Result<(), Error> {
    let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| Error::Protocol)?;
    let mut state = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(name.as_ptr(), state.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if unsafe { state.assume_init() }.f_flags & libc::MNT_RDONLY as u32 == 0 {
        return Err(Error::Untrusted);
    }
    Ok(())
}
fn selected(request: &WorkerRequest, relative: &str) -> Result<PathBuf, Error> {
    if !relative_path(relative) {
        return Err(Error::Untrusted);
    }
    state(request)?;
    let volume = request.resource_root.join("volume");
    readonly(&volume)?;
    let selected = volume.join(relative);
    // The selected payload itself and each directory component must be real objects.
    let mut current = volume.clone();
    for component in Path::new(relative).components() {
        current.push(component);
        if fs::symlink_metadata(&current)?.file_type().is_symlink() {
            return Err(Error::Untrusted);
        }
    }
    if !selected.canonicalize()?.starts_with(volume.canonicalize()?) {
        return Err(Error::Untrusted);
    }
    Ok(selected)
}
fn target(request: &WorkerRequest, dmg: &Dmg, application: &Application) -> Result<PathBuf, Error> {
    if !relative_path(&application.target_name)
        || application.target_name.contains('/')
        || !application.target_name.ends_with(".app")
    {
        return Err(Error::Protocol);
    }
    let root = match dmg.scope {
        wire::SoftwareTaskScope::System => {
            if !matches!(request.run_as, RunAs::System { .. }) {
                return Err(Error::Identity);
            }
            PathBuf::from("/Applications")
        }
        wire::SoftwareTaskScope::User => {
            let RunAs::User { account } = &request.run_as else {
                return Err(Error::Identity);
            };
            let uid = unsafe { libc::geteuid() };
            if account.subject.as_str() != uid.to_string() {
                return Err(Error::Identity);
            }
            let mut pwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
            let mut buffer = [0u8; 16384];
            let mut found = std::ptr::null_mut();
            if unsafe {
                libc::getpwuid_r(
                    uid,
                    pwd.as_mut_ptr(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    &mut found,
                )
            } != 0
                || found.is_null()
            {
                return Err(Error::Identity);
            }
            let pwd = unsafe { pwd.assume_init() };
            let home = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) }
                .to_str()
                .map_err(|_| Error::Identity)?;
            let root = PathBuf::from(home).join("Applications");
            // A missing Applications directory is not created by read-only detection.
            if !root.exists() && request.operation != WorkerOperation::Detect {
                fs::DirBuilder::new().mode(0o700).create(&root)?;
            }
            root
        }
    };
    if let Ok(meta) = fs::symlink_metadata(&root) {
        let expected_uid = match dmg.scope {
            wire::SoftwareTaskScope::System => 0,
            wire::SoftwareTaskScope::User => unsafe { libc::geteuid() },
        };
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.uid() != expected_uid
            || meta.mode() & 0o002 != 0
            || (meta.mode() & 0o020 != 0
                && !(dmg.scope == wire::SoftwareTaskScope::System && meta.gid() == 80))
        {
            return Err(Error::Untrusted);
        }
    }
    Ok(root.join(&application.target_name))
}
fn text(path: &Path) -> Result<&str, Error> {
    path.to_str().ok_or(Error::Protocol)
}
fn relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
fn bundle(
    request: &WorkerRequest,
    path: &Path,
    application: &Application,
) -> Result<SoftwareState, Error> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SoftwareState::Absent {}),
        Err(e) => return Err(e.into()),
        Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
            return Err(Error::Untrusted)
        }
        _ => (),
    }
    let metadata = path.join("Contents/Info.plist");
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(metadata)?;
    if file.metadata()?.len() > 1024 * 1024 {
        return Err(Error::Capacity);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    let data = plist(&bytes)?;
    let dictionary = data.as_dictionary().ok_or(Error::Protocol)?;
    if dictionary
        .get("CFBundleIdentifier")
        .and_then(plist::Value::as_string)
        != Some(application.bundle_id.as_str())
    {
        return Err(Error::Untrusted);
    }
    let version = dictionary
        .get("CFBundleShortVersionString")
        .and_then(plist::Value::as_string)
        .ok_or(Error::Protocol)?;
    verify_code(request, path, "execute")?;
    let executable = dictionary
        .get("CFBundleExecutable")
        .and_then(plist::Value::as_string)
        .ok_or(Error::Protocol)?;
    if !relative_path(executable) || executable.contains('/') {
        return Err(Error::Untrusted);
    }
    let binary = path.join("Contents/MacOS").join(executable);
    let (code, architecture, _) = native(request, "lipo", &["-archs", text(&binary)?])?;
    let expected = match request.architecture {
        wire::TaskArchitecture::Aarch64 => "arm64",
        wire::TaskArchitecture::X86_64 => "x86_64",
    };
    if code != 0
        || !std::str::from_utf8(&architecture)
            .map_err(|_| Error::Protocol)?
            .split_whitespace()
            .any(|a| a == expected)
    {
        return Err(Error::Untrusted);
    }
    Ok(SoftwareState::Present {
        version: package_value(version)?,
    })
}
fn package_value(value: &str) -> Result<PackageValue, Error> {
    PackageValue::new(value).map_err(|_| Error::Protocol)
}
fn verify_code(request: &WorkerRequest, path: &Path, kind: &str) -> Result<(), Error> {
    if kind == "execute" || kind == "open" {
        if native(
            request,
            "codesign",
            &["--verify", "--deep", "--strict", text(path)?],
        )?
        .0 != 0
        {
            return Err(Error::Untrusted);
        }
        let (code, _, details) = native(
            request,
            "codesign",
            &["--display", "--verbose=4", text(path)?],
        )?;
        if code != 0 {
            return Err(Error::Untrusted);
        }
        let details = std::str::from_utf8(&details).map_err(|_| Error::Protocol)?;
        for signature in &request.action.signatures {
            if signature.mechanism != wire::SoftwareTaskSignatureMechanism::AppleDeveloperId
                || !details
                    .lines()
                    .any(|line| line == format!("TeamIdentifier={}", signature.publisher))
            {
                return Err(Error::Untrusted);
            }
        }
    }
    if kind == "install" {
        let (code, certificate, _) =
            native(request, "pkgutil", &["--check-signature", text(path)?])?;
        let certificate = std::str::from_utf8(&certificate).map_err(|_| Error::Protocol)?;
        if code != 0
            || request.action.signatures.iter().any(|s| {
                s.mechanism != wire::SoftwareTaskSignatureMechanism::AppleDeveloperId
                    || !certificate.lines().any(|line| {
                        line.contains("Developer ID Installer:")
                            && line.trim_end().ends_with(&format!("({})", s.publisher))
                    })
            })
        {
            return Err(Error::Untrusted);
        }
    }
    let args = if kind == "open" {
        vec![
            "--assess",
            "--type",
            "open",
            "--context",
            "context:primary-signature",
            text(path)?,
        ]
    } else {
        vec!["--assess", "--type", kind, text(path)?]
    };
    if native(request, "spctl", &args)?.0 != 0 {
        return Err(Error::Untrusted);
    }
    Ok(())
}
fn inventory(root: &Path) -> Result<Tree, Error> {
    fn visit(
        root: &Path,
        path: &Path,
        device: u64,
        tree: &mut Tree,
        total: &mut u64,
    ) -> Result<(), Error> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| Error::Untrusted)?
            .to_path_buf();
        if relative.components().count() > 32 || tree.len() >= 4096 {
            return Err(Error::Capacity);
        }
        let metadata = fs::symlink_metadata(path)?;
        if metadata.dev() != device {
            return Err(Error::Untrusted);
        }
        let entry = if metadata.file_type().is_symlink() {
            let target = fs::read_link(path)?;
            if target.is_absolute() || !path.canonicalize()?.starts_with(root.canonicalize()?) {
                return Err(Error::Untrusted);
            }
            Entry::Link { target }
        } else if metadata.is_dir() {
            let entry = Entry::Directory {
                mode: metadata.mode() & 0o755,
            };
            tree.insert(relative.clone(), entry.clone());
            for child in fs::read_dir(path)? {
                visit(root, &child?.path(), device, tree, total)?;
            }
            entry
        } else if metadata.is_file() {
            if metadata.len() > 1024 * 1024 * 1024 {
                return Err(Error::Capacity);
            }
            *total = total.checked_add(metadata.len()).ok_or(Error::Capacity)?;
            if *total > 8 * 1024 * 1024 * 1024 {
                return Err(Error::Capacity);
            }
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)?;
            use sha2::{Digest as _, Sha256};
            let mut hash = Sha256::new();
            let mut file = file;
            let mut buffer = [0u8; 65536];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            Entry::File {
                mode: metadata.mode() & 0o755,
                length: metadata.len(),
                sha256: format!("{:x}", hash.finalize()),
            }
        } else {
            return Err(Error::Untrusted);
        };
        tree.insert(relative, entry);
        Ok(())
    }
    let mut tree = Tree::new();
    let device = fs::symlink_metadata(root)?.dev();
    visit(root, root, device, &mut tree, &mut 0)?;
    Ok(tree)
}
fn copy_tree(source: &Path, target: &Path, tree: &Tree) -> Result<(), Error> {
    // Copy data and extended attributes through retained no-follow descriptors. Ownership
    // belongs to the actual invocation account; quarantine is preserved, never cleared.
    unsafe extern "C" {
        fn fcopyfile(from: i32, to: i32, state: *mut libc::c_void, flags: u32) -> i32;
    }
    for (relative, entry) in tree {
        let original = source.join(relative);
        let copy = target.join(relative);
        match entry {
            Entry::Directory { mode } => {
                fs::DirBuilder::new().mode(*mode).create(&copy)?;
                let source = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&original)?;
                let target = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&copy)?;
                if unsafe {
                    fcopyfile(
                        source.as_raw_fd(),
                        target.as_raw_fd(),
                        std::ptr::null_mut(),
                        1 << 2,
                    )
                } != 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            Entry::Link { target } => {
                std::os::unix::fs::symlink(target, &copy)?;
                unsafe extern "C" {
                    fn copyfile(
                        from: *const libc::c_char,
                        to: *const libc::c_char,
                        state: *mut libc::c_void,
                        flags: u32,
                    ) -> i32;
                }
                let from =
                    CString::new(original.as_os_str().as_bytes()).map_err(|_| Error::Protocol)?;
                let to = CString::new(copy.as_os_str().as_bytes()).map_err(|_| Error::Protocol)?;
                if unsafe {
                    copyfile(
                        from.as_ptr(),
                        to.as_ptr(),
                        std::ptr::null_mut(),
                        (1 << 2) | (1 << 18) | (1 << 19),
                    )
                } != 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            Entry::File { mode, .. } => {
                let source = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(original)?;
                let target = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(*mode)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&copy)?;
                if unsafe {
                    fcopyfile(
                        source.as_raw_fd(),
                        target.as_raw_fd(),
                        std::ptr::null_mut(),
                        (1 << 2) | (1 << 3),
                    )
                } != 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                target.sync_all()?;
                fs::set_permissions(copy, fs::Permissions::from_mode(*mode))?;
            }
        }
    }
    if inventory(target)? != *tree {
        return Err(Error::Untrusted);
    }
    Ok(())
}
fn rename(source: &Path, target: &Path, swap: bool) -> Result<(), Error> {
    let parent = |p: &Path| -> Result<File, Error> {
        Ok(OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(p.parent().ok_or(Error::Protocol)?)?)
    };
    let source_parent = parent(source)?;
    let target_parent = parent(target)?;
    let name = |p: &Path| {
        CString::new(p.file_name().ok_or(Error::Protocol)?.as_bytes()).map_err(|_| Error::Protocol)
    };
    if unsafe {
        libc::renameatx_np(
            source_parent.as_raw_fd(),
            name(source)?.as_ptr(),
            target_parent.as_raw_fd(),
            name(target)?.as_ptr(),
            if swap {
                libc::RENAME_SWAP
            } else {
                libc::RENAME_EXCL
            },
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    source_parent.sync_all()?;
    target_parent.sync_all()?;
    Ok(())
}
fn remove_tree(root: &Path, expected: &Tree) -> Result<(), Error> {
    if inventory(root)? != *expected {
        return Err(Error::Untrusted);
    }
    let root_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    for (relative, entry) in expected
        .iter()
        .rev()
        .filter(|(p, _)| !p.as_os_str().is_empty())
    {
        let mut parent = root_file.try_clone()?;
        let parts = relative.components().collect::<Vec<_>>();
        for part in &parts[..parts.len() - 1] {
            let name = CString::new(part.as_os_str().as_bytes()).map_err(|_| Error::Protocol)?;
            let fd = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            parent = unsafe { File::from_raw_fd(fd) };
        }
        let leaf = CString::new(parts.last().ok_or(Error::Protocol)?.as_os_str().as_bytes())
            .map_err(|_| Error::Protocol)?;
        if unsafe {
            libc::unlinkat(
                parent.as_raw_fd(),
                leaf.as_ptr(),
                if matches!(entry, Entry::Directory { .. }) {
                    libc::AT_REMOVEDIR
                } else {
                    0
                },
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    fs::remove_dir(root)?;
    Ok(())
}
fn cleanup(request: &WorkerRequest, dmg: &Dmg) -> Result<(i32, SoftwareWorkerResult), Error> {
    let record = state(request)?;
    let staged = request.resource_root.join("application.app");
    if staged.exists() {
        let actual = inventory(&staged)?;
        if record.stage.as_ref() != Some(&actual) && record.alternate.as_ref() != Some(&actual) {
            return Err(Error::Untrusted);
        }
        remove_tree(&staged, &actual)?;
    }
    let device = record.device.clone().or(owned_device(request, dmg)?);
    if let Some(device) = &device {
        if !disk_root(device) {
            return Err(Error::Untrusted);
        }
        if owns_device(request, dmg, device)? {
            let (code, _, error) = native(request, "hdiutil", &["detach", device])?;
            if code != 0 {
                return Ok(result(
                    code,
                    None,
                    String::from_utf8_lossy(&error).into_owned(),
                ));
            }
            if owns_device(request, dmg, device)? {
                return Err(Error::Untrusted);
            }
        }
    }
    fs::remove_dir(request.resource_root.join("volume"))?;
    fs::remove_file(request.resource_root.join("owner.json"))?;
    fs::remove_dir(&request.resource_root)?;
    Ok(result(0, None, String::new()))
}
fn owns_device(request: &WorkerRequest, dmg: &Dmg, device: &str) -> Result<bool, Error> {
    let (code, output, _) = native(request, "hdiutil", &["info", "-plist"])?;
    if code != 0 {
        return Err(Error::Unavailable);
    }
    let info = plist(&output)?;
    let images = info
        .as_dictionary()
        .and_then(|d| d.get("images"))
        .and_then(plist::Value::as_array)
        .ok_or(Error::Protocol)?;
    let image = request.material(&dmg.image)?;
    let mount = request.resource_root.join("volume");
    let mount_text = text(&mount)?;
    let mut matching = 0;
    for entry in images {
        let d = entry.as_dictionary().ok_or(Error::Protocol)?;
        let entities = d
            .get("system-entities")
            .and_then(plist::Value::as_array)
            .ok_or(Error::Protocol)?;
        if entities.iter().any(|e| {
            e.as_dictionary()
                .and_then(|d| d.get("dev-entry"))
                .and_then(plist::Value::as_string)
                == Some(device)
        }) {
            if d.get("image-path").and_then(plist::Value::as_string) != Some(text(&image)?)
                || !entities.iter().any(|e| {
                    e.as_dictionary()
                        .and_then(|d| d.get("mount-point"))
                        .and_then(plist::Value::as_string)
                        == Some(mount_text)
                })
            {
                return Err(Error::Untrusted);
            }
            matching += 1;
        }
    }
    if matching > 1 {
        return Err(Error::Untrusted);
    }
    Ok(matching == 1)
}
pub(super) fn execute(
    request: &WorkerRequest,
    before: Option<&SoftwareState>,
) -> Result<(i32, SoftwareWorkerResult), Error> {
    let wire::SoftwareTaskBehavior::Dmg(dmg) = &request.action.behavior else {
        return Err(Error::Unsupported);
    };
    match request.operation {
        WorkerOperation::Attach => return attach(request, dmg),
        WorkerOperation::Cleanup => return cleanup(request, dmg),
        _ => (),
    }
    match &dmg.payload {
        Payload::AppCopy {
            application,
            uninstall,
        } => {
            let installed = target(request, dmg, application)?;
            if request.operation == WorkerOperation::Detect {
                return Ok(result(
                    0,
                    Some(bundle(request, &installed, application)?),
                    String::new(),
                ));
            }
            let payload = selected(request, &application.path)?;
            let staged = request.resource_root.join("application.app");
            if request.operation == WorkerOperation::Stage {
                if bundle(request, &payload, application)?
                    != (SoftwareState::Present {
                        version: package_value(&application.version)?,
                    })
                {
                    return Err(Error::Untrusted);
                }
                let tree = inventory(&payload)?;
                copy_tree(&payload, &staged, &tree)?;
                if bundle(request, &staged, application)?
                    != (SoftwareState::Present {
                        version: package_value(&application.version)?,
                    })
                {
                    return Err(Error::Untrusted);
                }
                let mut record = state(request)?;
                record.stage = Some(tree);
                save(request, &record)?;
                return Ok(result(0, None, String::new()));
            }
            let observed = bundle(request, &installed, application)?;
            if installed.exists() {
                require_not_running(&installed)?;
            }
            if Some(&observed) != before {
                return Err(Error::Untrusted);
            }
            if matches!(
                request.operation,
                WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
            ) {
                if !uninstall {
                    return Err(Error::Unsupported);
                }
                let actual = inventory(&installed)?;
                // Exact-version removal compares complete bytes/links to the approved read-only image.
                if request.operation == WorkerOperation::Uninstall && actual != inventory(&payload)?
                {
                    return Err(Error::Untrusted);
                }
                remove_tree(&installed, &actual)?;
                return Ok(result(0, None, String::new()));
            }
            let new = inventory(&staged)?;
            if new != inventory(&payload)? {
                return Err(Error::Untrusted);
            }
            let mut record = state(request)?;
            let old = if matches!(observed, SoftwareState::Present { .. }) {
                Some(inventory(&installed)?)
            } else {
                None
            };
            // Persist the role of the staging slot before the atomic publication/swap.
            record.alternate = old.clone();
            save(request, &record)?;
            rename(&staged, &installed, old.is_some())?;
            record.stage = old.clone();
            record.alternate = None;
            save(request, &record)?;
            if inventory(&installed)? != new
                || bundle(request, &installed, application)?
                    != (SoftwareState::Present {
                        version: package_value(&application.version)?,
                    })
            {
                return Err(Error::Untrusted);
            }
            if let Some(old) = old {
                if inventory(&staged)? != old {
                    return Err(Error::Untrusted);
                }
            }
            Ok(result(0, None, String::new()))
        }
        Payload::ContainedPkg {
            path,
            length,
            sha256,
            receipt,
            uninstall,
        } => {
            if request.operation == WorkerOperation::Detect {
                let detection = execution_runner::software::receipt_state(receipt)?;
                return Ok(result(0, Some(detection), String::new()));
            }
            if before != Some(&execution_runner::software::receipt_state(receipt)?) {
                return Err(Error::Conflict);
            }
            let package = if matches!(
                request.operation,
                WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
            ) {
                request.material(&uninstall.as_ref().ok_or(Error::Unsupported)?.installer)?
            } else {
                let package = selected(request, path)?;
                if fs::metadata(&package)?.len() != *length {
                    return Err(Error::Untrusted);
                }
                let digest = Digest::new(crate::plan::hex(sha256)).map_err(|_| Error::Protocol)?;
                // Image contents are read-only; inner selected PKG bytes are checked independently.
                if file_hash(&package)? != digest.as_str() {
                    return Err(Error::Untrusted);
                }
                package
            };
            if !request.action.signatures.is_empty() {
                verify_code(request, &package, "install")?;
            }
            let mut args = vec![
                "-pkg".into(),
                text(&package)?.into(),
                "-target".into(),
                "/".into(),
            ];
            if matches!(
                request.operation,
                WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
            ) {
                args.extend(
                    uninstall
                        .as_ref()
                        .ok_or(Error::Unsupported)?
                        .invocation
                        .arguments
                        .clone(),
                );
            }
            let (code, stdout, stderr) = run_tool(request, &request.tool("installer")?, &args)?;
            Ok(result(
                code,
                None,
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&stdout),
                    String::from_utf8_lossy(&stderr)
                ),
            ))
        }
    }
}
fn file_hash(path: &Path) -> Result<String, Error> {
    use sha2::{Digest as _, Sha256};
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

// ref: Apple xnu bsd/sys/proc_info.h and libproc.h. Never terminate an application.
fn require_not_running(application: &Path) -> Result<(), Error> {
    unsafe extern "C" {
        fn proc_listpids(kind: u32, kind_info: u32, buffer: *mut libc::c_void, size: i32) -> i32;
        fn proc_pidpath(pid: i32, buffer: *mut libc::c_void, size: u32) -> i32;
    }
    let root = application.canonicalize()?;
    let mut pids = [0i32; 16384];
    let bytes = unsafe {
        proc_listpids(
            1,
            0,
            pids.as_mut_ptr().cast(),
            std::mem::size_of_val(&pids) as i32,
        )
    };
    if bytes <= 0 || bytes as usize >= std::mem::size_of_val(&pids) {
        return Err(Error::Unavailable);
    }
    for pid in pids.into_iter().take(bytes as usize / 4).filter(|p| *p > 0) {
        let mut path = [0u8; 4096];
        let length = unsafe { proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
        if length <= 0 {
            continue;
        }
        let end = path
            .iter()
            .position(|b| *b == 0)
            .ok_or(Error::Unavailable)?;
        let executable = Path::new(std::ffi::OsStr::from_bytes(&path[..end]));
        if executable.starts_with(&root) {
            return Err(Error::Unavailable);
        }
    }
    Ok(())
}

fn owned_device(request: &WorkerRequest, dmg: &Dmg) -> Result<Option<String>, Error> {
    let (code, output, _) = native(request, "hdiutil", &["info", "-plist"])?;
    if code != 0 {
        return Err(Error::Unavailable);
    }
    let data = plist(&output)?;
    let images = data
        .as_dictionary()
        .and_then(|d| d.get("images"))
        .and_then(plist::Value::as_array)
        .ok_or(Error::Protocol)?;
    let mount = request.resource_root.join("volume");
    let image = request.material(&dmg.image)?;
    let mut devices = Vec::new();
    for entry in images {
        let object = entry.as_dictionary().ok_or(Error::Protocol)?;
        let entities = object
            .get("system-entities")
            .and_then(plist::Value::as_array)
            .ok_or(Error::Protocol)?;
        if entities.iter().any(|e| {
            e.as_dictionary()
                .and_then(|d| d.get("mount-point"))
                .and_then(plist::Value::as_string)
                == Some(text(&mount).unwrap_or(""))
        }) {
            if object.get("image-path").and_then(plist::Value::as_string) != Some(text(&image)?) {
                return Err(Error::Untrusted);
            }
            for entity in entities {
                if let Some(device) = entity
                    .as_dictionary()
                    .and_then(|d| d.get("dev-entry"))
                    .and_then(plist::Value::as_string)
                    .filter(|d| disk_root(d))
                {
                    devices.push(device.to_owned());
                }
            }
        }
    }
    if devices.len() > 1 {
        return Err(Error::Untrusted);
    }
    Ok(devices.pop())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framework_links_metadata_and_atomic_replacement_use_actual_filesystem() {
        let root = std::env::temp_dir().join(format!("rss-dmg-files-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let source = root.join("source.app");
        fs::create_dir_all(source.join("Contents/Frameworks/Test.framework/Versions/A")).unwrap();
        fs::write(
            source.join("Contents/Frameworks/Test.framework/Versions/A/Test"),
            b"approved framework",
        )
        .unwrap();
        std::os::unix::fs::symlink(
            "A",
            source.join("Contents/Frameworks/Test.framework/Versions/Current"),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            "Versions/Current/Test",
            source.join("Contents/Frameworks/Test.framework/Test"),
        )
        .unwrap();
        let path = CString::new(source.as_os_str().as_bytes()).unwrap();
        let attribute = c"com.apple.quarantine";
        let quarantine = b"0081;native-proof;RSS;";
        assert_eq!(
            unsafe {
                libc::setxattr(
                    path.as_ptr(),
                    attribute.as_ptr(),
                    quarantine.as_ptr().cast(),
                    quarantine.len(),
                    0,
                    0,
                )
            },
            0
        );
        let tree = inventory(&source).unwrap();
        let stage = root.join("stage.app");
        copy_tree(&source, &stage, &tree).unwrap();
        let staged_path = CString::new(stage.as_os_str().as_bytes()).unwrap();
        let mut observed = [0u8; 128];
        let length = unsafe {
            libc::getxattr(
                staged_path.as_ptr(),
                attribute.as_ptr(),
                observed.as_mut_ptr().cast(),
                observed.len(),
                0,
                0,
            )
        };
        assert_eq!(&observed[..length as usize], quarantine);
        let installed = root.join("installed.app");
        rename(&stage, &installed, false).unwrap();
        assert_eq!(inventory(&installed).unwrap(), tree);
        copy_tree(&source, &stage, &tree).unwrap();
        fs::write(
            installed.join("Contents/Frameworks/Test.framework/Versions/A/Test"),
            b"old version",
        )
        .unwrap();
        let old = inventory(&installed).unwrap();
        rename(&stage, &installed, true).unwrap();
        assert_eq!(inventory(&installed).unwrap(), tree);
        assert_eq!(inventory(&stage).unwrap(), old);
        remove_tree(&stage, &old).unwrap();
        remove_tree(&installed, &tree).unwrap();
        std::os::unix::fs::symlink("/tmp", source.join("escape")).unwrap();
        assert!(inventory(&source).is_err());
        fs::remove_file(source.join("escape")).unwrap();
        remove_tree(&source, &tree).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
