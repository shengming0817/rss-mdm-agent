// ref: Munki installer/dmg.py and dmgutils.py; Apple copyfile.c, copyfile.h,
// renameatx_np(2), rename(2) and fsync(2).
use super::*;
use std::{
    collections::VecDeque,
    ffi::{CStr, CString, OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
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
    partial_stage: bool,
    group: i32,
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
fn present(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}
fn image_state_paths(request: &WorkerRequest) -> Result<(File, PathBuf, PathBuf), Error> {
    let parent = request.resource_root.parent().ok_or(Error::Configuration)?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o022 != 0
    {
        return Err(Error::Untrusted);
    }
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent)?;
    let name = request
        .resource_root
        .file_name()
        .ok_or(Error::Configuration)?;
    let sidecar = |suffix: &str| {
        let mut name = name.to_os_string();
        name.push(suffix);
        parent.join(name)
    };
    Ok((directory, sidecar(".owner.json"), sidecar(".owner.next")))
}
// An incomplete next file is never ownership proof. Only its exact protected name can
// be discarded before the initial publication, when neither root nor current exists.
fn read_image_state(request: &WorkerRequest, path: &Path) -> Result<Option<ImageState>, Error> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() > 4 * 1024 * 1024
    {
        return Err(Error::Untrusted);
    }
    let mut bytes = Vec::new();
    file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    let Ok(state) = serde_json::from_slice::<ImageState>(&bytes) else {
        return Ok(None);
    };
    if state.owner != owner(request)? {
        return Err(Error::Untrusted);
    }
    Ok(Some(state))
}
fn state(request: &WorkerRequest) -> Result<ImageState, Error> {
    if present(&request.resource_root)? {
        owned_root(request)?;
    }
    let (parent, current, next) = image_state_paths(request)?;
    if present(&next)? {
        if read_image_state(request, &next)?.is_some() {
            if present(&current)? {
                read_image_state(request, &current)?.ok_or(Error::Untrusted)?;
            }
            fs::rename(&next, &current)?;
        } else {
            read_image_state(request, &current)?.ok_or(Error::Untrusted)?;
            fs::remove_file(&next)?;
        }
        parent.sync_all()?;
    }
    read_image_state(request, &current)?.ok_or(Error::Untrusted)
}
fn save(request: &WorkerRequest, state: &ImageState) -> Result<(), Error> {
    let (parent, current, next) = image_state_paths(request)?;
    if state.owner != owner(request)? {
        return Err(Error::Untrusted);
    }
    if present(&current)? {
        read_image_state(request, &current)?.ok_or(Error::Untrusted)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&next)?;
    file.write_all(&serde_json::to_vec(state).map_err(|_| Error::Protocol)?)?;
    file.sync_all()?;
    fs::rename(next, current)?;
    parent.sync_all()?;
    Ok(())
}
fn create_image_root(request: &WorkerRequest, record: &ImageState) -> Result<(), Error> {
    let (parent, current, next) = image_state_paths(request)?;
    if present(&request.resource_root)? || present(&current)? || present(&next)? {
        return Err(Error::Untrusted);
    }
    // Publish the sole ownership record before creating anything it must recover.
    save(request, record)?;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&request.resource_root)?;
    owned_root(request)?.sync_all()?;
    parent.sync_all()?;
    Ok(())
}
fn discard_unpublished_image_state(request: &WorkerRequest) -> Result<bool, Error> {
    let (parent, current, next) = image_state_paths(request)?;
    if present(&request.resource_root)? || present(&current)? {
        return Ok(false);
    }
    if present(&next)? {
        if read_image_state(request, &next)?.is_some() {
            return Ok(false);
        }
        fs::remove_file(next)?;
        parent.sync_all()?;
    }
    Ok(true)
}
fn remove_image_root(request: &WorkerRequest) -> Result<(), Error> {
    let (parent, current, next) = image_state_paths(request)?;
    read_image_state(request, &current)?.ok_or(Error::Untrusted)?;
    if present(&next)? {
        return Err(Error::Untrusted);
    }
    if present(&request.resource_root)? {
        owned_root(request)?;
        fs::remove_dir(&request.resource_root)?;
    }
    // Keep ownership until the empty root removal is durable, including a restart
    // after rmdir but before unlinking this same state record.
    parent.sync_all()?;
    fs::remove_file(current)?;
    parent.sync_all()?;
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
    let mut record = ImageState {
        owner: owner(request)?,
        device: None,
        stage: None,
        alternate: None,
        partial_stage: false,
        group: unsafe { libc::getpgrp() },
    };
    create_image_root(request, &record)?;
    let mount = request.resource_root.join("volume");
    fs::DirBuilder::new().mode(0o700).create(&mount)?;
    let image = request.material(&dmg.image)?;
    if request
        .action
        .signatures
        .iter()
        .any(|signature| signature.artifact == dmg.image)
    {
        verify_code(request, &image, "open", Some(&dmg.image))?;
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
    record.device = Some(image_device(entities)?.ok_or(Error::Untrusted)?);
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
// APFS exposes its synthetic container alongside the image's physical disk. Detach the
// partition-scheme disk, retaining the single-device fallback for unpartitioned images.
fn image_device(entities: &[plist::Value]) -> Result<Option<String>, Error> {
    let mut roots = Vec::new();
    let mut schemes = Vec::new();
    for entity in entities {
        let object = entity.as_dictionary().ok_or(Error::Protocol)?;
        if let Some(device) = object
            .get("dev-entry")
            .and_then(plist::Value::as_string)
            .filter(|device| disk_root(device))
        {
            roots.push(device);
            if matches!(
                object.get("content-hint").and_then(plist::Value::as_string),
                Some("GUID_partition_scheme" | "Apple_partition_scheme")
            ) {
                schemes.push(device);
            }
        }
    }
    let candidates = if schemes.is_empty() { roots } else { schemes };
    match candidates.as_slice() {
        [] => Ok(None),
        [device] => Ok(Some((*device).to_owned())),
        _ => Err(Error::Untrusted),
    }
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
struct SelectedPayload {
    path: PathBuf,
    _volume: File,
    file: File,
    _parents: Vec<File>,
}
impl std::ops::Deref for SelectedPayload {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}
impl SelectedPayload {
    fn anchored(&self) -> PathBuf {
        // Only the contained PKG installer consumes a file-descriptor pathname.
        // macOS /dev/fd does not support traversing retained directory descriptors.
        PathBuf::from(format!("/dev/fd/{}", self.file.as_raw_fd()))
    }
    fn verify(&self, request: &WorkerRequest, dmg: &Dmg) -> Result<(), Error> {
        let record = state(request)?;
        if !record
            .device
            .as_ref()
            .is_some_and(|device| owns_device(request, dmg, device).unwrap_or(false))
        {
            return Err(Error::Untrusted);
        }
        let observed = fs::symlink_metadata(&self.path)?;
        let retained = self.file.metadata()?;
        if observed.dev() != retained.dev() || observed.ino() != retained.ino() {
            return Err(Error::Untrusted);
        }
        readonly(&request.resource_root.join("volume"))
    }
}
fn selected(request: &WorkerRequest, relative: &str) -> Result<SelectedPayload, Error> {
    if !relative_path(relative) {
        return Err(Error::Untrusted);
    }
    let wire::SoftwareTaskBehavior::Dmg(dmg) = &request.action.behavior else {
        return Err(Error::Untrusted);
    };
    let record = state(request)?;
    let device = record.device.as_deref().ok_or(Error::Untrusted)?;
    if !owns_device(request, dmg, device)? {
        return Err(Error::Untrusted);
    }
    let volume = request.resource_root.join("volume");
    readonly(&volume)?;
    let selected = volume.join(relative);
    // The selected payload itself and each directory component must be real objects.
    let volume_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&volume)?;
    let mut parents = Vec::new();
    for component in Path::new(relative).components() {
        let parent = parents.last().unwrap_or(&volume_file);
        parents.push(open_at(
            parent,
            component.as_os_str(),
            libc::O_NOFOLLOW | libc::O_NONBLOCK,
        )?);
    }
    if !selected.canonicalize()?.starts_with(volume.canonicalize()?) {
        return Err(Error::Untrusted);
    }
    let file = parents.pop().ok_or(Error::Untrusted)?;
    let payload = SelectedPayload {
        path: selected,
        _volume: volume_file,
        file,
        _parents: parents,
    };
    payload.verify(request, dmg)?;
    Ok(payload)
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
            return Err(Error::Untrusted);
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
    verify_code(request, path, "execute", None)?;
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
fn verify_code(
    request: &WorkerRequest,
    path: &Path,
    kind: &str,
    artifact: Option<&str>,
) -> Result<(), Error> {
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
        for signature in request
            .action
            .signatures
            .iter()
            .filter(|signature| artifact == Some(signature.artifact.as_str()))
        {
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
            || request
                .action
                .signatures
                .iter()
                .filter(|signature| artifact == Some(signature.artifact.as_str()))
                .any(|s| {
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
fn open_at(parent: &File, name: &OsStr, flags: i32) -> Result<File, Error> {
    let name = CString::new(name.as_bytes()).map_err(|_| Error::Protocol)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | flags,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn stat_at(parent: &File, name: &OsStr) -> Result<libc::stat, Error> {
    let name = CString::new(name.as_bytes()).map_err(|_| Error::Protocol)?;
    let mut observed = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            observed.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { observed.assume_init() })
}
fn open_entry(parent: &File, name: &OsStr) -> Result<File, Error> {
    let observed = stat_at(parent, name)?;
    let flags = match observed.st_mode & libc::S_IFMT {
        libc::S_IFDIR => libc::O_DIRECTORY | libc::O_NOFOLLOW,
        libc::S_IFREG => libc::O_NOFOLLOW | libc::O_NONBLOCK,
        libc::S_IFLNK => libc::O_SYMLINK,
        _ => return Err(Error::Untrusted),
    };
    let file = open_at(parent, name, flags)?;
    let retained = file.metadata()?;
    if observed.st_dev as u64 != retained.dev() || observed.st_ino != retained.ino() {
        return Err(Error::Untrusted);
    }
    Ok(file)
}
fn directory_names(directory: &File) -> Result<Vec<OsString>, Error> {
    struct Directory(*mut libc::DIR);
    impl Drop for Directory {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.0) };
        }
    }
    // A fresh open description avoids sharing/reusing the retained directory's cursor.
    let fd = open_at(
        directory,
        OsStr::new("."),
        libc::O_DIRECTORY | libc::O_NOFOLLOW,
    )?
    .into_raw_fd();
    let directory = unsafe { libc::fdopendir(fd) };
    if directory.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(error.into());
    }
    let directory = Directory(directory);
    let mut names = Vec::new();
    loop {
        unsafe { *libc::__error() = 0 };
        let entry = unsafe { libc::readdir(directory.0) };
        if entry.is_null() {
            if unsafe { *libc::__error() } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            return Ok(names);
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if names.len() >= 4096 {
            return Err(Error::Capacity);
        }
        names.push(OsString::from_vec(name.to_vec()));
    }
}
fn link_at(parent: &File, name: &OsStr, retained: &File) -> Result<PathBuf, Error> {
    let name_c = CString::new(name.as_bytes()).map_err(|_| Error::Protocol)?;
    let mut target = [0u8; 4096];
    let length = unsafe {
        libc::readlinkat(
            parent.as_raw_fd(),
            name_c.as_ptr(),
            target.as_mut_ptr().cast(),
            target.len(),
        )
    };
    if length < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if length as usize == target.len() {
        return Err(Error::Capacity);
    }
    let observed = stat_at(parent, name)?;
    let metadata = retained.metadata()?;
    if observed.st_dev as u64 != metadata.dev() || observed.st_ino != metadata.ino() {
        return Err(Error::Untrusted);
    }
    Ok(PathBuf::from(OsString::from_vec(
        target[..length as usize].to_vec(),
    )))
}
fn validate_links(tree: &Tree) -> Result<(), Error> {
    for (path, entry) in tree {
        let Entry::Link { target } = entry else {
            continue;
        };
        if target.is_absolute() {
            return Err(Error::Untrusted);
        }
        let mut current = path.parent().ok_or(Error::Untrusted)?.to_path_buf();
        let mut pending: VecDeque<_> = target
            .components()
            .map(|part| part.as_os_str().to_os_string())
            .collect();
        let mut followed = 0;
        while let Some(part) = pending.pop_front() {
            if part == OsStr::new(".") {
                continue;
            }
            if part == OsStr::new("..") {
                if !current.pop() {
                    return Err(Error::Untrusted);
                }
                continue;
            }
            current.push(part);
            match tree.get(&current).ok_or(Error::Untrusted)? {
                Entry::Link { target } => {
                    if target.is_absolute() || followed >= 32 {
                        return Err(Error::Untrusted);
                    }
                    followed += 1;
                    current.pop();
                    // Resolve .. after following each link, just as the filesystem does.
                    for part in target.components().rev() {
                        pending.push_front(part.as_os_str().to_os_string());
                    }
                }
                Entry::File { .. } if !pending.is_empty() => return Err(Error::Untrusted),
                _ => {}
            }
        }
        if !tree.contains_key(&current) {
            return Err(Error::Untrusted);
        }
    }
    Ok(())
}
fn inventory(root: &Path) -> Result<Tree, Error> {
    let root = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    inventory_from(&root)
}
fn inventory_from(root: &File) -> Result<Tree, Error> {
    fn visit(
        file: File,
        relative: &Path,
        parent: Option<&File>,
        device: u64,
        tree: &mut Tree,
        total: &mut u64,
    ) -> Result<(), Error> {
        if relative.components().count() > 32 || tree.len() >= 4096 {
            return Err(Error::Capacity);
        }
        let metadata = file.metadata()?;
        if metadata.dev() != device {
            return Err(Error::Untrusted);
        }
        let entry = if metadata.file_type().is_symlink() {
            let target = link_at(
                parent.ok_or(Error::Untrusted)?,
                relative.file_name().ok_or(Error::Untrusted)?,
                &file,
            )?;
            Entry::Link { target }
        } else if metadata.is_dir() {
            let entry = Entry::Directory {
                mode: metadata.mode() & 0o755,
            };
            tree.insert(relative.to_path_buf(), entry.clone());
            for name in directory_names(&file)? {
                let child = open_entry(&file, &name)?;
                visit(
                    child,
                    &relative.join(name),
                    Some(&file),
                    device,
                    tree,
                    total,
                )?;
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
        tree.insert(relative.to_path_buf(), entry);
        Ok(())
    }
    if !root.metadata()?.is_dir() {
        return Err(Error::Untrusted);
    }
    let mut tree = Tree::new();
    visit(
        root.try_clone()?,
        Path::new(""),
        None,
        root.metadata()?.dev(),
        &mut tree,
        &mut 0,
    )?;
    validate_links(&tree)?;
    Ok(tree)
}
fn source_entry(root: &File, relative: &Path, entry: &Entry) -> Result<File, Error> {
    if relative.as_os_str().is_empty() {
        return Ok(root.try_clone()?);
    }
    let mut parent = root.try_clone()?;
    for component in relative.parent().ok_or(Error::Untrusted)?.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(Error::Untrusted);
        }
        parent = open_at(
            &parent,
            component.as_os_str(),
            libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )?;
    }
    let file = open_entry(&parent, relative.file_name().ok_or(Error::Untrusted)?)?;
    let metadata = file.metadata()?;
    if metadata.dev() != root.metadata()?.dev()
        || match entry {
            Entry::Directory { .. } => !metadata.is_dir(),
            Entry::File { .. } => !metadata.is_file(),
            Entry::Link { target } => {
                !metadata.file_type().is_symlink()
                    || link_at(
                        &parent,
                        relative.file_name().ok_or(Error::Untrusted)?,
                        &file,
                    )? != *target
            }
        }
    {
        return Err(Error::Untrusted);
    }
    Ok(file)
}
#[cfg(test)]
fn copy_tree(source: &Path, target: &Path, tree: &Tree) -> Result<(), Error> {
    let source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(source)?;
    copy_tree_from(&source, target, tree)
}
fn copy_tree_from(source: &File, target: &Path, tree: &Tree) -> Result<(), Error> {
    // Data and extended attributes (including quarantine) come from retained source
    // descriptors. O_SYMLINK lets fcopyfile preserve link metadata without following it.
    unsafe extern "C" {
        fn fcopyfile(from: i32, to: i32, state: *mut libc::c_void, flags: u32) -> i32;
    }
    for (relative, entry) in tree {
        let source = source_entry(source, relative, entry)?;
        let copy = target.join(relative);
        let (output, flags) = match entry {
            Entry::Directory { mode } => {
                fs::DirBuilder::new().mode(*mode).create(&copy)?;
                (
                    OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                        .open(&copy)?,
                    1 << 2,
                )
            }
            Entry::Link { target } => {
                std::os::unix::fs::symlink(target, &copy)?;
                (
                    OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_SYMLINK | libc::O_CLOEXEC)
                        .open(&copy)?,
                    1 << 2,
                )
            }
            Entry::File { mode, .. } => (
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(*mode)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&copy)?,
                (1 << 2) | (1 << 3),
            ),
        };
        if unsafe {
            fcopyfile(
                source.as_raw_fd(),
                output.as_raw_fd(),
                std::ptr::null_mut(),
                flags,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        if let Entry::File { mode, .. } = entry {
            output.sync_all()?;
            output.set_permissions(fs::Permissions::from_mode(*mode))?;
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
    unlink_tree(root, expected)
}
fn unlink_tree(root: &Path, expected: &Tree) -> Result<(), Error> {
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
fn remove_owned_stage(root: &Path, expected: &Tree, partial: bool) -> Result<(), Error> {
    fn visit(
        root: &Path,
        path: &Path,
        expected: &Tree,
        partial: bool,
        actual: &mut Tree,
    ) -> Result<(), Error> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| Error::Untrusted)?
            .to_path_buf();
        let approved = expected.get(&relative).ok_or(Error::Untrusted)?;
        let metadata = fs::symlink_metadata(path)?;
        if metadata.dev() != fs::symlink_metadata(root)?.dev() {
            return Err(Error::Untrusted);
        }
        match approved {
            Entry::Directory { .. } if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                actual.insert(relative, approved.clone());
                for child in fs::read_dir(path)? {
                    visit(root, &child?.path(), expected, partial, actual)?;
                }
            }
            Entry::Link { target }
                if metadata.file_type().is_symlink() && fs::read_link(path)? == *target =>
            {
                actual.insert(relative, approved.clone());
            }
            Entry::File { length, sha256, .. }
                if metadata.is_file()
                    && !metadata.file_type().is_symlink()
                    && metadata.nlink() == 1 =>
            {
                if !(partial
                    && metadata.uid() == unsafe { libc::geteuid() }
                    && metadata.len() <= *length)
                    && (metadata.len() != *length || file_hash(path)? != *sha256)
                {
                    return Err(Error::Untrusted);
                }
                actual.insert(relative, approved.clone());
            }
            _ => return Err(Error::Untrusted),
        }
        Ok(())
    }
    let mut remaining = Tree::new();
    visit(root, root, expected, partial, &mut remaining)?;
    unlink_tree(root, &remaining)
}
fn cleanup(request: &WorkerRequest, dmg: &Dmg) -> Result<(i32, SoftwareWorkerResult), Error> {
    if discard_unpublished_image_state(request)? {
        return Ok(result(0, None, String::new()));
    }
    let mut record = state(request)?;
    if record.group != unsafe { libc::getpgrp() } && record.group > 1 {
        let exists = unsafe { libc::kill(-record.group, 0) };
        if exists == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            return Err(Error::Unavailable);
        }
    }
    record.group = unsafe { libc::getpgrp() };
    save(request, &record)?;
    let staged = request.resource_root.join("application.app");
    if present(&staged)? {
        let candidates = record.stage.iter().chain(record.alternate.iter());
        let mut accepted = false;
        for expected in candidates {
            if remove_owned_stage(&staged, expected, record.partial_stage).is_ok() {
                accepted = true;
                break;
            }
        }
        if !accepted {
            return Err(Error::Untrusted);
        }
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
    let volume = request.resource_root.join("volume");
    if present(&volume)? {
        fs::remove_dir(volume)?;
    }
    remove_image_root(request)?;
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
    let standalone_removal = request.operation == WorkerOperation::Uninstall
        && matches!(dmg.payload, Payload::ContainedPkg { .. });
    if request.operation != WorkerOperation::Detect && !standalone_removal {
        let mut record = state(request)?;
        record.group = unsafe { libc::getpgrp() };
        save(request, &record)?;
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
                payload.verify(request, dmg)?;
                let tree = inventory_from(&payload.file)?;
                let mut record = state(request)?;
                record.stage = Some(tree.clone());
                record.partial_stage = true;
                record.group = unsafe { libc::getpgrp() };
                save(request, &record)?;
                copy_tree_from(&payload.file, &staged, &tree)?;
                payload.verify(request, dmg)?;
                if bundle(request, &staged, application)?
                    != (SoftwareState::Present {
                        version: package_value(&application.version)?,
                    })
                {
                    return Err(Error::Untrusted);
                }
                let mut record = state(request)?;
                record.stage = Some(tree);
                record.partial_stage = false;
                save(request, &record)?;
                return Ok(result(0, None, String::new()));
            }
            payload.verify(request, dmg)?;
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
                if request.operation == WorkerOperation::Uninstall
                    && actual != inventory_from(&payload.file)?
                {
                    return Err(Error::Untrusted);
                }
                remove_tree(&installed, &actual)?;
                return Ok(result(0, None, String::new()));
            }
            let new = inventory(&staged)?;
            if new != inventory_from(&payload.file)? {
                return Err(Error::Untrusted);
            }
            let mut record = state(request)?;
            let old = if matches!(observed, SoftwareState::Present { .. }) {
                Some(inventory(&installed)?)
            } else {
                None
            };
            // Persist the role of the staging slot before the atomic publication/swap.
            record.group = unsafe { libc::getpgrp() };
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
                let detection = execution_runner::software::receipt_state(receipt)
                    .map_err(crate::error::app_error)?;
                return Ok(result(0, Some(detection), String::new()));
            }
            if before
                != Some(
                    &execution_runner::software::receipt_state(receipt)
                        .map_err(crate::error::app_error)?,
                )
            {
                return Err(Error::Conflict);
            }
            let mut selected_package = None;
            let package = if matches!(
                request.operation,
                WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
            ) {
                request.material(&uninstall.as_ref().ok_or(Error::Unsupported)?.installer)?
            } else {
                let package = selected(request, path)?;
                if package.file.metadata()?.len() != *length {
                    return Err(Error::Untrusted);
                }
                let digest =
                    Digest::new(crate::backend::plan::hex(sha256)).map_err(|_| Error::Protocol)?;
                // Image contents are read-only; inner selected PKG bytes are checked independently.
                if file_hash(&package)? != digest.as_str() {
                    return Err(Error::Untrusted);
                }
                package.verify(request, dmg)?;
                if unsafe { libc::fcntl(package.file.as_raw_fd(), libc::F_SETFD, 0) } < 0 {
                    return Err(Error::Unavailable);
                }
                // The owned read-only FD is deliberately inherited by the pinned installer only.
                let anchored = package.anchored();
                selected_package = Some(package);
                anchored
            };
            let artifact = if matches!(
                request.operation,
                WorkerOperation::Uninstall | WorkerOperation::RemovePrevious
            ) {
                Some(
                    uninstall
                        .as_ref()
                        .ok_or(Error::Unsupported)?
                        .installer
                        .as_str(),
                )
            } else {
                None
            };
            verify_code(request, &package, "install", artifact)?;
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
            drop(selected_package);
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
            if let Some(device) = image_device(entities)? {
                devices.push(device);
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
    fn material(path: &Path) -> SoftwareMaterial {
        SoftwareMaterial {
            path: path.to_str().unwrap().into(),
            artifact: ExactArtifactRef {
                resource: VersionedRef {
                    id: Id::new("dmg-regression").unwrap(),
                    revision: Id::new("1").unwrap(),
                },
                sha256: Digest::new(file_hash(path).unwrap()).unwrap(),
            },
        }
    }
    fn image_request() -> (PathBuf, WorkerRequest) {
        let parent = std::env::temp_dir().join(format!("rss-dmg-owner-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&parent).unwrap();
        let image = parent.join("image.dmg");
        let removal = parent.join("removal.pkg");
        fs::write(&image, b"unmounted test image").unwrap();
        fs::write(&removal, b"invalid removal package").unwrap();
        let invocation = wire::SoftwareTaskInvocation {
            run_as: wire::ExecutionIdentity::System,
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            timeout_seconds: 30,
            output_bytes: 1_048_576,
            exit_codes: wire::SoftwareTaskExitCodes {
                success: [0].into(),
                reboot: Default::default(),
            },
        };
        let action = wire::SoftwareTaskAction {
            package: "org.rss.dmg-regression".into(),
            version: "1.0".into(),
            behavior: wire::SoftwareTaskBehavior::Dmg(Dmg {
                image: "image".into(),
                volume: "RSS regression".into(),
                scope: wire::SoftwareTaskScope::System,
                invocation: invocation.clone(),
                upgrade: wire::SoftwareTaskUpgrade::InPlace,
                payload: Payload::ContainedPkg {
                    path: "payload.pkg".into(),
                    length: 1,
                    sha256: [1; 32],
                    receipt: format!("org.rss.dmg-regression.{}", uuid::Uuid::new_v4()),
                    uninstall: Some(wire::SoftwareTaskRemoval {
                        installer: "removal".into(),
                        invocation,
                    }),
                },
            }),
            signatures: Vec::new(),
            reboot: wire::SoftwareTaskReboot::Report,
            downgrade: wire::SoftwareTaskDowngrade::Deny,
            ownership: wire::SoftwareTaskOwnership::ManagedOnly,
        };
        let request = WorkerRequest {
            native_output: Default::default(),
            native_pending: Default::default(),
            external_pending: Default::default(),
            action,
            operation: WorkerOperation::Cleanup,
            materials: [
                ("image".into(), material(&image)),
                ("removal".into(), material(&removal)),
            ]
            .into(),
            tools: [
                ("hdiutil".into(), material(Path::new("/usr/bin/hdiutil"))),
                ("pkgutil".into(), material(Path::new("/usr/sbin/pkgutil"))),
            ]
            .into(),
            resource_root: parent.join("attempt"),
            run_as: RunAs::System {
                platform: Platform::Macos,
            },
            session: SessionRequirement::NotRequired {},
            architecture: wire::TaskArchitecture::Aarch64,
            output_bytes: 1_048_576,
            step: 0,
        };
        (parent, request)
    }
    fn initial_state(request: &WorkerRequest) -> ImageState {
        ImageState {
            owner: owner(request).unwrap(),
            device: None,
            stage: None,
            alternate: None,
            partial_stage: false,
            group: unsafe { libc::getpgrp() },
        }
    }
    fn write_next(request: &WorkerRequest, bytes: &[u8]) {
        let (_, _, next) = image_state_paths(request).unwrap();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(next)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
    fn assert_image_cleanup(request: &WorkerRequest) {
        assert_eq!(execute(request, None).unwrap().0, 0);
        let (_, current, next) = image_state_paths(request).unwrap();
        assert!(!present(&request.resource_root).unwrap());
        assert!(!present(&current).unwrap());
        assert!(!present(&next).unwrap());
    }
    #[test]
    fn standalone_pkg_uninstall_rechecks_receipt_and_frozen_package_without_image_state() {
        let (parent, mut request) = image_request();
        request.operation = WorkerOperation::Uninstall;
        let mismatched = SoftwareState::Present {
            version: package_value("1.0").unwrap(),
        };
        assert!(matches!(
            execute(&request, Some(&mismatched)),
            Err(Error::Conflict)
        ));
        assert_eq!(
            request
                .native_output
                .load(std::sync::atomic::Ordering::Acquire),
            0
        );
        // Real pkgutil rejects these invalid frozen bytes; no installer is available.
        assert!(matches!(
            execute(&request, Some(&SoftwareState::Absent {})),
            Err(Error::Untrusted)
        ));
        assert!(
            request
                .native_output
                .load(std::sync::atomic::Ordering::Acquire)
                > 0
        );
        let (_, current, next) = image_state_paths(&request).unwrap();
        assert!(!present(&request.resource_root).unwrap());
        assert!(!present(&current).unwrap());
        assert!(!present(&next).unwrap());
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn unpublished_partial_next_cleanup_preserves_other_parent_names() {
        let (parent, request) = image_request();
        write_next(&request, b"{\"owner\":");
        let unrelated = parent.join("other.owner.next");
        fs::write(&unrelated, b"must remain").unwrap();
        assert_image_cleanup(&request);
        assert_eq!(fs::read(unrelated).unwrap(), b"must remain");
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn published_ownership_reopens_before_root_creation_and_cleans_up() {
        let (parent, request) = image_request();
        let record = initial_state(&request);
        save(&request, &record).unwrap();
        let reopened = state(&request).unwrap();
        assert_eq!(reopened.owner, record.owner);
        assert_eq!(reopened.group, record.group);
        assert_image_cleanup(&request);
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn ownership_reopens_after_root_removal_before_state_unlink() {
        let (parent, request) = image_request();
        let record = initial_state(&request);
        create_image_root(&request, &record).unwrap();
        fs::remove_dir(&request.resource_root).unwrap();
        assert_eq!(state(&request).unwrap().owner, record.owner);
        assert_image_cleanup(&request);
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn cleanup_preserves_unknown_root_entries_and_ownership_for_retry() {
        let (parent, request) = image_request();
        create_image_root(&request, &initial_state(&request)).unwrap();
        let unknown = request.resource_root.join("unowned");
        fs::write(&unknown, b"must remain").unwrap();
        assert!(execute(&request, None).is_err());
        assert_eq!(fs::read(&unknown).unwrap(), b"must remain");
        assert_eq!(state(&request).unwrap().owner, owner(&request).unwrap());
        fs::remove_file(unknown).unwrap();
        assert_image_cleanup(&request);
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn ownerless_root_is_never_adopted_even_with_partial_next() {
        let (parent, request) = image_request();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&request.resource_root)
            .unwrap();
        write_next(&request, b"{\"owner\":");
        assert!(execute(&request, None).is_err());
        assert!(create_image_root(&request, &initial_state(&request)).is_err());
        let (_, _, next) = image_state_paths(&request).unwrap();
        assert!(present(&next).unwrap());
        assert!(present(&request.resource_root).unwrap());
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn pending_next_reopens_only_exact_owner_and_preserves_partial_stage_proof() {
        let (parent, request) = image_request();
        let mut record = initial_state(&request);
        create_image_root(&request, &record).unwrap();
        record.partial_stage = true;
        write_next(&request, &serde_json::to_vec(&record).unwrap());
        assert!(state(&request).unwrap().partial_stage);
        write_next(&request, b"{\"owner\":");
        assert!(state(&request).unwrap().partial_stage);
        record.owner = "wrong frozen request".into();
        write_next(&request, &serde_json::to_vec(&record).unwrap());
        assert!(matches!(state(&request), Err(Error::Untrusted)));
        let (_, current, next) = image_state_paths(&request).unwrap();
        assert!(present(&current).unwrap());
        assert!(present(&next).unwrap());
        fs::remove_file(next).unwrap();
        assert_image_cleanup(&request);
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn symlinked_next_or_root_is_rejected_without_touching_target() {
        let (parent, request) = image_request();
        let (_, _, next) = image_state_paths(&request).unwrap();
        let target = parent.join("unowned");
        fs::write(&target, b"must remain").unwrap();
        std::os::unix::fs::symlink(&target, &next).unwrap();
        assert!(execute(&request, None).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"must remain");
        fs::remove_file(next).unwrap();
        save(&request, &initial_state(&request)).unwrap();
        std::os::unix::fs::symlink(&parent, &request.resource_root).unwrap();
        assert!(matches!(state(&request), Err(Error::Untrusted)));
        assert!(execute(&request, None).is_err());
        fs::remove_file(&request.resource_root).unwrap();
        assert_image_cleanup(&request);
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn apfs_image_uses_physical_partition_disk_for_attach_and_cleanup() {
        let entity = |device: &str, hint: &str| {
            let mut object = plist::Dictionary::new();
            object.insert("dev-entry".into(), device.into());
            object.insert("content-hint".into(), hint.into());
            plist::Value::Dictionary(object)
        };
        let entities = [
            entity("/dev/disk4", "GUID_partition_scheme"),
            entity("/dev/disk4s1", "7C3457EF-0000-11AA-AA11-00306543ECAC"),
            entity("/dev/disk5", "EF57347C-0000-11AA-AA11-00306543ECAC"),
            entity("/dev/disk5s1", "41504653-0000-11AA-AA11-00306543ECAC"),
        ];
        assert_eq!(
            image_device(&entities).unwrap().as_deref(),
            Some("/dev/disk4")
        );
        assert_eq!(
            image_device(&[entity("/dev/disk6", "Apple_HFS")])
                .unwrap()
                .as_deref(),
            Some("/dev/disk6")
        );
        assert!(image_device(&[
            entity("/dev/disk4", "GUID_partition_scheme"),
            entity("/dev/disk6", "Apple_partition_scheme")
        ])
        .is_err());
    }
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
        let quarantine = b"0081;00000001;RSS;";
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
        let source_file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&source)
            .unwrap();
        let tree = inventory_from(&source_file).unwrap();
        let retained_source = root.join("retained.app");
        fs::rename(&source, &retained_source).unwrap();
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("replacement"),
            b"unapproved pathname replacement",
        )
        .unwrap();
        assert_ne!(inventory(&source).unwrap(), tree);
        assert_eq!(inventory_from(&source_file).unwrap(), tree);
        let stage = root.join("stage.app");
        copy_tree_from(&source_file, &stage, &tree).unwrap();
        assert_eq!(
            fs::read(stage.join("Contents/Frameworks/Test.framework/Test")).unwrap(),
            b"approved framework"
        );
        assert!(!stage.join("replacement").exists());
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
        assert!(length >= 0, "{}", std::io::Error::last_os_error());
        let quarantine = std::str::from_utf8(&observed[..length as usize]).unwrap();
        let flags = u32::from_str_radix(quarantine.split(';').next().unwrap(), 16).unwrap();
        // Apple's copyfile may update quarantine metadata; it must retain the original flags.
        assert_eq!(flags & 0x0081, 0x0081);
        let installed = root.join("installed.app");
        rename(&stage, &installed, false).unwrap();
        assert_eq!(inventory(&installed).unwrap(), tree);
        copy_tree_from(&source_file, &stage, &tree).unwrap();
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
        std::os::unix::fs::symlink("/tmp", retained_source.join("escape")).unwrap();
        assert!(inventory_from(&source_file).is_err());
        fs::remove_file(retained_source.join("escape")).unwrap();
        std::os::unix::fs::symlink("../source.app", retained_source.join("escape")).unwrap();
        assert!(inventory_from(&source_file).is_err());
        fs::remove_file(retained_source.join("escape")).unwrap();
        remove_tree(&retained_source, &tree).unwrap();
        fs::remove_file(source.join("replacement")).unwrap();
        fs::remove_dir(source).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn interrupted_owned_stage_cleanup_is_resumable_and_rejects_unknown_names() {
        let root = std::env::temp_dir().join(format!("rss-dmg-partial-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let source = root.join("source.app");
        fs::create_dir_all(source.join("Contents")).unwrap();
        fs::write(source.join("Contents/first"), b"approved complete bytes").unwrap();
        fs::write(source.join("Contents/second"), b"approved second file").unwrap();
        let tree = inventory(&source).unwrap();
        let stage = root.join("stage.app");
        fs::create_dir_all(stage.join("Contents")).unwrap();
        fs::write(stage.join("Contents/first"), b"approved").unwrap();
        fs::write(stage.join("unowned"), b"must remain").unwrap();
        assert!(remove_owned_stage(&stage, &tree, true).is_err());
        assert!(stage.join("unowned").exists());
        fs::remove_file(stage.join("unowned")).unwrap();
        remove_owned_stage(&stage, &tree, true).unwrap();
        copy_tree(&source, &stage, &tree).unwrap();
        fs::remove_file(stage.join("Contents/first")).unwrap();
        remove_owned_stage(&stage, &tree, false).unwrap();
        remove_tree(&source, &tree).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
