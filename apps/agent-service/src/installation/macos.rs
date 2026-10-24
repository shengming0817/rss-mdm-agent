//! launchd is the sole process lifecycle authority. No Python on installed devices.
//! ref: apple launchd-842.92.1 man/launchd.plist.5.
use super::*;
use std::os::unix::fs::MetadataExt;
const LABEL: &str = "com.rss-mdm.agent.execution";
const EXIT_TIMEOUT_SECS: u64 = 35;
const STOP_TIMEOUT: Duration = Duration::from_secs(45);
fn require_exit_budget(job: &plist::Dictionary) -> Result<(), Error> {
    if job
        .get("ExitTimeOut")
        .and_then(plist::Value::as_unsigned_integer)
        != Some(EXIT_TIMEOUT_SECS)
    {
        return Err(Error::Configuration);
    }
    Ok(())
}
fn label(system: bool) -> String {
    format!("{LABEL}{}", if system { "" } else { ".user" })
}
fn plist_path(system: bool) -> PathBuf {
    Path::new(if system {
        "/Library/LaunchDaemons"
    } else {
        "/Library/LaunchAgents"
    })
    .join(format!("{}.plist", label(system)))
}
fn endpoint(system: bool) -> Result<String, Error> {
    Ok(if system {
        format!("system/{LABEL}")
    } else {
        format!(
            "gui/{}/{}.user",
            execution_ipc::host::current_subject().map_err(crate::error::app_error)?,
            LABEL
        )
    })
}
pub(super) fn dispatch(_: &[OsString]) -> Result<bool, Error> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(Error::Identity);
    }
    Ok(false)
}
pub(super) fn operation_status(_: uuid::Uuid) -> Result<(), Error> {
    Err(Error::Unsupported)
}
pub(super) fn public_directory(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() {
        return Err(Error::Configuration);
    }
    if !path.exists() {
        public_directory(path.parent().ok_or(Error::Configuration)?)?;
        std::fs::create_dir(path)?;
    }
    installation_security::protected(path).map_err(|_| Error::Identity)
}
pub(super) fn replace(staged: &Path, target: &Path) -> Result<(), Error> {
    std::fs::rename(staged, target)?;
    std::fs::File::open(target.parent().ok_or(Error::Configuration)?)?.sync_all()?;
    Ok(())
}
fn program(binary: &Path, config: &Path, system: bool) -> Vec<String> {
    let mut args = vec![
        binary.to_string_lossy().into_owned(),
        "--config".into(),
        config.to_string_lossy().into_owned(),
    ];
    if !system {
        args.push("--user-helper".into());
    }
    args
}
fn read_job(system: bool, binary: &Path, config: &Path) -> Result<plist::Dictionary, Error> {
    let bytes =
        installation_security::read_protected(&plist_path(system)).map_err(|_| Error::Identity)?;
    let value =
        plist::Value::from_reader(std::io::Cursor::new(bytes)).map_err(|_| Error::Configuration)?;
    let dictionary = value.into_dictionary().ok_or(Error::Identity)?;
    let args = dictionary
        .get("ProgramArguments")
        .and_then(|v| v.as_array())
        .ok_or(Error::Identity)?;
    let actual = args
        .iter()
        .map(|v| v.as_string().map(str::to_owned).ok_or(Error::Identity))
        .collect::<Result<Vec<_>, _>>()?;
    if dictionary.get("Label").and_then(|v| v.as_string()) != Some(label(system).as_str())
        || actual != program(binary, config, system)
    {
        return Err(Error::Identity);
    }
    Ok(dictionary)
}
fn registered(endpoint: &str) -> Result<Option<u32>, Error> {
    let (status, bytes) = utility(
        Path::new("/bin/launchctl"),
        &["print".into(), endpoint.into()],
    )?;
    if status.code() == Some(113) {
        return Ok(None);
    }
    if !status.success() {
        return Err(Error::Unavailable);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::Protocol)?;
    // A loaded job without a live PID is still a registration and must not be overwritten.
    Ok(Some(
        text.lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("pid = ")
                    .and_then(|p| p.parse().ok())
            })
            .unwrap_or(0),
    ))
}
fn stop_endpoint(endpoint: &str) -> Result<(), Error> {
    let Some(pid) = registered(endpoint)? else {
        return Ok(());
    };
    let deadline = Instant::now() + STOP_TIMEOUT;
    let (status, _) = utility_with_timeout(
        Path::new("/bin/launchctl"),
        &["bootout".into(), endpoint.into()],
        STOP_TIMEOUT,
    )?;
    if !status.success() {
        outcome::native_failure(status.code());
        return Err(Error::Unavailable);
    }
    loop {
        let live = pid != 0 && unsafe { libc::kill(pid as i32, 0) } == 0;
        if registered(endpoint)?.is_none() && !live {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn write_job(system: bool, binary: &Path, config: &Path) -> Result<(), Error> {
    let path = plist_path(system);
    public_directory(path.parent().ok_or(Error::Configuration)?)?;
    let dict = job(system, binary, config);
    let mut bytes = Vec::new();
    plist::Value::Dictionary(dict)
        .to_writer_xml(&mut bytes)
        .map_err(|_| Error::Configuration)?;
    write_new(&path, &bytes)
}
fn job(system: bool, binary: &Path, config: &Path) -> plist::Dictionary {
    let mut dict = plist::Dictionary::new();
    dict.insert("Label".into(), label(system).into());
    dict.insert(
        "ProgramArguments".into(),
        plist::Value::Array(
            program(binary, config, system)
                .into_iter()
                .map(plist::Value::String)
                .collect(),
        ),
    );
    let mut services = plist::Dictionary::new();
    services.insert(label(system), true.into());
    dict.insert("MachServices".into(), services.into());
    dict.insert("RunAtLoad".into(), true.into());
    dict.insert("KeepAlive".into(), false.into());
    dict.insert("ExitTimeOut".into(), EXIT_TIMEOUT_SECS.into());
    dict.insert("ProcessType".into(), "Background".into());
    dict
}
pub(super) fn register(system: bool, binary: &Path, config: &Path) -> Result<(), Error> {
    let deployment = Deployment::load(config)?;
    if deployment.service.path != binary {
        return Err(Error::Identity);
    }
    if !system {
        let subject = execution_ipc::host::current_subject().map_err(crate::error::app_error)?;
        if !deployment.helper_work_roots.contains_key(&subject) {
            return Err(Error::Identity);
        }
    }
    let path = plist_path(system);
    let endpoint = endpoint(system)?;
    if path.exists() {
        require_exit_budget(&read_job(system, binary, config)?)?;
    } else if system {
        write_job(true, binary, config)?;
    } else {
        return Err(Error::Configuration);
    }
    let domain = if system {
        "system".into()
    } else {
        format!(
            "gui/{}",
            execution_ipc::host::current_subject().map_err(crate::error::app_error)?
        )
    };
    let result = match registered(&endpoint)? {
        None => command(
            Path::new("/bin/launchctl"),
            &["bootstrap".into(), domain.into(), path.into()],
        ),
        Some(0) => command(
            Path::new("/bin/launchctl"),
            &["kickstart".into(), endpoint.clone().into()],
        ),
        Some(_) => Ok(Vec::new()),
    };
    if let Err(error) = result {
        stop_endpoint(&endpoint)?;
        return Err(error);
    }
    if system {
        let helper = plist_path(false);
        if helper.exists() {
            require_exit_budget(&read_job(false, binary, config)?)?;
        } else if !deployment.helper_work_roots.is_empty() {
            write_job(false, binary, config)?;
        }
    }
    // Registration and process startup are independent. A configured daemon must acquire a PID.
    {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if registered(&endpoint)?.is_some_and(|pid| pid != 0) {
                break;
            }
            if Instant::now() >= deadline {
                return Err(Error::Unavailable);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(())
}
pub(super) fn stop(system: bool, binary: &Path, config: &Path) -> Result<(), Error> {
    let path = plist_path(system);
    if !path.exists() {
        if registered(&endpoint(system)?)?.is_some() {
            return Err(Error::Identity);
        }
        return Ok(());
    }
    read_job(system, binary, config)?;
    stop_endpoint(&endpoint(system)?)
}
pub(super) fn remove(system: bool, binary: &Path, config: &Path) -> Result<(), Error> {
    stop(system, binary, config)?;
    let path = plist_path(system);
    if system && path.exists() {
        read_job(system, binary, config)?;
        std::fs::remove_file(path)?;
    }
    if system && plist_path(false).exists() {
        read_job(false, binary, config)?;
        std::fs::remove_file(plist_path(false))?;
    }
    Ok(())
}
pub(super) fn status(system: bool) -> Result<(), Error> {
    println!(
        "{}",
        if registered(&endpoint(system)?)?.is_some() {
            "registered"
        } else {
            "absent"
        }
    );
    Ok(())
}
pub(super) fn quiescent(old: &Deployment, config: &Path) -> Result<(), Error> {
    let processes = command(
        Path::new("/bin/ps"),
        &["-ax".into(), "-o".into(), "comm=".into()],
    )?;
    let text = std::str::from_utf8(&processes).map_err(|_| Error::Protocol)?;
    if old
        .clients
        .images
        .iter()
        .filter(|i| i.path != old.service.path)
        .any(|i| {
            text.lines()
                .any(|line| line.trim() == i.path.to_string_lossy())
        })
    {
        return Err(Error::Conflict);
    }
    for subject in old.helper_work_roots.keys() {
        // A retained profile can belong to a user who is currently logged out.
        let (status, _) = utility(
            Path::new("/bin/launchctl"),
            &["print".into(), format!("gui/{subject}").into()],
        )?;
        if status.code() == Some(125) {
            continue;
        }
        if !status.success() {
            return Err(Error::Unavailable);
        }
        let endpoint = format!("gui/{subject}/{LABEL}.user");
        if registered(&endpoint)?.is_some() {
            read_job(false, &old.service.path, config)?;
            stop_endpoint(&endpoint)?;
        }
    }
    Ok(())
}
pub(super) fn interactive_profile() -> Result<Profile, Error> {
    let subject = execution_ipc::host::active_user_subject().map_err(crate::error::app_error)?;
    let uid: libc::uid_t = subject.parse().map_err(|_| Error::Identity)?;
    let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0i8; 16384];
    if unsafe {
        libc::getpwuid_r(
            uid,
            entry.as_mut_ptr(),
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut result,
        )
    } != 0
        || result.is_null()
    {
        return Err(Error::Identity);
    }
    let home = PathBuf::from(
        unsafe { std::ffi::CStr::from_ptr((*result).pw_dir) }
            .to_str()
            .map_err(|_| Error::Identity)?,
    );
    Ok(Profile {
        subjects: vec![subject.clone()],
        helper_work_roots: BTreeMap::from([(
            subject,
            home.join("Library/Application Support/RSS MDM Agent/helper"),
        )]),
    })
}
pub(super) fn verify_signatures(candidate: &Candidate) -> Result<(), Error> {
    for image in [&candidate.service, &candidate.desktop] {
        let cdhash = image.cdhash.as_ref().ok_or(Error::Identity)?;
        if cdhash.len() != 40 || !cdhash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Identity);
        }
        let requirement = format!(
            "=cdhash H\"{cdhash}\"{}",
            if candidate.signing_mode == SigningMode::Release {
                " and anchor apple generic"
            } else {
                ""
            }
        );
        command(
            Path::new("/usr/bin/codesign"),
            &[
                "--verify".into(),
                "--strict".into(),
                "-R".into(),
                requirement.into(),
                image.path.clone().into(),
            ],
        )?;
    }
    Ok(())
}
pub(super) fn execution_resources(
    interpreters: &mut Vec<crate::Interpreter>,
    managers: &mut Vec<crate::SoftwareManager>,
) -> Result<(), Error> {
    fn artifact(path: &str) -> Result<Artifact, Error> {
        let path = PathBuf::from(path);
        installation_security::protected(&path).map_err(|_| Error::Identity)?;
        Ok(Artifact {
            sha256: hash(&std::fs::read(&path)?),
            path,
            cdhash: None,
        })
    }
    interpreters.push(crate::Interpreter {
        profile: rss_mdm_agent_wire::runtime::ExecutorProfile::PosixSh,
        image: artifact("/bin/sh")?,
    });
    managers.push(crate::SoftwareManager {
        executor: crate::SoftwareManagerKind::PackageInstaller,
        image: artifact("/usr/sbin/installer")?,
    });
    Ok(())
}
pub(super) fn preflight_desktop_entry(candidate: &Candidate) -> Result<(), Error> {
    let alias = Path::new("/Applications/RSS MDM Agent.app");
    installation_security::applications_directory().map_err(|_| Error::Identity)?;
    if let Ok(metadata) = std::fs::symlink_metadata(alias) {
        if !metadata.is_symlink()
            || metadata.uid() != 0
            || !std::fs::read_link(alias)?
                .starts_with(candidate.root.parent().ok_or(Error::Configuration)?)
        {
            return Err(Error::Identity);
        }
    }
    Ok(())
}
pub(super) fn desktop_entry(candidate: &Candidate) -> Result<(), Error> {
    preflight_desktop_entry(candidate)?;
    let app = candidate
        .desktop
        .path
        .ancestors()
        .find(|p| p.extension().is_some_and(|s| s == "app"))
        .ok_or(Error::Configuration)?;
    let alias = Path::new("/Applications/RSS MDM Agent.app");
    let staged = alias.with_extension(format!("pending-{}", uuid::Uuid::new_v4()));
    std::os::unix::fs::symlink(app, &staged)?;
    let result = replace(&staged, alias);
    if result.is_err() {
        let _ = std::fs::remove_file(staged);
    }
    result
}

pub(super) fn read_secret(path: &Path) -> Result<Vec<u8>, Error> {
    Ok(platform_private_storage::read(path, 64)?)
}

pub(super) fn uninstall_payload(candidate: &Candidate) -> Result<(), Error> {
    preflight_desktop_entry(candidate)?;
    let alias = Path::new("/Applications/RSS MDM Agent.app");
    if let Ok(target) = std::fs::read_link(alias) {
        if !target.starts_with(&candidate.root) {
            return Err(Error::Identity);
        }
        std::fs::remove_file(alias)?;
    }
    let retired = retire_payload(candidate)?;
    if retired.exists() {
        std::fs::remove_dir_all(retired)?;
    }
    Ok(())
}
pub(super) fn payload_parent() -> Result<PathBuf, Error> {
    Ok(Deployment::default_path()?
        .parent()
        .ok_or(Error::Configuration)?
        .join("versions"))
}

fn unmanage_job(task: uuid::Uuid) -> PathBuf {
    Path::new("/Library/LaunchDaemons").join(format!("{LABEL}.unmanage.{task}.plist"))
}
pub(super) fn launch_unmanage(deployment: &Deployment, task: uuid::Uuid) -> Result<(), Error> {
    let config = Deployment::default_path()?;
    let label = format!("{LABEL}.unmanage.{task}");
    let path = unmanage_job(task);
    let args = vec![
        unmanage_binary(deployment, task)?
            .to_string_lossy()
            .into_owned(),
        "--installation".into(),
        "uninstall".into(),
        "--config".into(),
        config.to_string_lossy().into_owned(),
        "--binary".into(),
        deployment.service.path.to_string_lossy().into_owned(),
        "--operation".into(),
        task.to_string(),
    ];
    let mut dict = plist::Dictionary::new();
    dict.insert("Label".into(), label.clone().into());
    dict.insert(
        "ProgramArguments".into(),
        plist::Value::Array(args.into_iter().map(plist::Value::String).collect()),
    );
    dict.insert("RunAtLoad".into(), true.into());
    let mut restart = plist::Dictionary::new();
    restart.insert("SuccessfulExit".into(), false.into());
    dict.insert("KeepAlive".into(), plist::Value::Dictionary(restart));
    dict.insert("ThrottleInterval".into(), 30.into());
    let mut bytes = Vec::new();
    plist::Value::Dictionary(dict)
        .to_writer_xml(&mut bytes)
        .map_err(|_| Error::Configuration)?;
    retain_public(&path, &bytes)?;
    if registered(&format!("system/{label}"))?.is_none() {
        command(
            Path::new("/bin/launchctl"),
            &["bootstrap".into(), "system".into(), path.into()],
        )?;
    }
    Ok(())
}
pub(super) fn finish_unmanage(deployment: &Deployment, task: uuid::Uuid) -> Result<bool, Error> {
    let carrier = unmanage_binary(deployment, task)?;
    let original = unmanage_job(task);
    let label = format!("{LABEL}.unmanage.{task}.cleanup");
    let cleanup = Path::new("/Library/LaunchDaemons").join(format!("{label}.plist"));
    let absent = |p: &Path| matches!(std::fs::symlink_metadata(p),Err(e) if e.kind()==std::io::ErrorKind::NotFound);
    if absent(&carrier) && absent(&original) && absent(&cleanup) {
        return Ok(true);
    }
    // ref: launchd.plist(5) ProgramArguments / KeepAlive SuccessfulExit.
    // The finalizer uses the OS image, so deleting our last executable cannot
    // remove its own recovery entry. Fixed script + separate exact UUID paths;
    // each deletion must succeed before the finalizer removes its durable plist.
    let args = cleanup_arguments(&carrier, &original, &cleanup);
    let mut dict = plist::Dictionary::new();
    dict.insert("Label".into(), label.clone().into());
    dict.insert(
        "ProgramArguments".into(),
        plist::Value::Array(args.into_iter().map(plist::Value::String).collect()),
    );
    dict.insert("RunAtLoad".into(), true.into());
    let mut restart = plist::Dictionary::new();
    restart.insert("SuccessfulExit".into(), false.into());
    dict.insert("KeepAlive".into(), plist::Value::Dictionary(restart));
    dict.insert("ThrottleInterval".into(), 30.into());
    let mut bytes = Vec::new();
    plist::Value::Dictionary(dict)
        .to_writer_xml(&mut bytes)
        .map_err(|_| Error::Configuration)?;
    retain_public(&cleanup, &bytes)?;
    if registered(&format!("system/{label}"))?.is_none() {
        command(
            Path::new("/bin/launchctl"),
            &["bootstrap".into(), "system".into(), cleanup.clone().into()],
        )?;
    }
    for _ in 0..10 {
        if absent(&carrier) && absent(&original) && absent(&cleanup) {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // Durable OS finalizer owns the remaining exact files; registration is not proof.
    Ok(false)
}
pub(super) fn unmanage_binary(_: &Deployment, task: uuid::Uuid) -> Result<PathBuf, Error> {
    Ok(Deployment::default_path()?
        .parent()
        .ok_or(Error::Configuration)?
        .join("install-operations")
        .join(format!("{task}.retirement")))
}

fn cleanup_arguments(carrier: &Path, original: &Path, cleanup: &Path) -> Vec<String> {
    vec![
        "/bin/sh".to_owned(),
        "-c".into(),
        r#"/bin/rm -f -- "$1" && /bin/rm -f -- "$2" && /bin/rm -f -- "$3""#.into(),
        "rss-retirement-cleanup".into(),
        carrier.to_string_lossy().into_owned(),
        original.to_string_lossy().into_owned(),
        cleanup.to_string_lossy().into_owned(),
    ]
}
#[cfg(test)]
mod retirement_tests {
    use super::*;
    #[test]
    fn launchd_budget_covers_service_stop_and_rejects_short_existing_jobs() {
        assert!(Duration::from_secs(EXIT_TIMEOUT_SECS) > crate::service::SHUTDOWN_WAIT);
        assert!(STOP_TIMEOUT > Duration::from_secs(EXIT_TIMEOUT_SECS));
        for system in [false, true] {
            let mut rendered = job(system, Path::new("/service"), Path::new("/configuration"));
            require_exit_budget(&rendered).unwrap();
            rendered.insert("ExitTimeOut".into(), 15u64.into());
            assert!(matches!(
                require_exit_budget(&rendered),
                Err(Error::Configuration)
            ));
            rendered.remove("ExitTimeOut");
            assert!(matches!(
                require_exit_budget(&rendered),
                Err(Error::Configuration)
            ));
        }
    }
    #[test]
    fn retirement_finalizer_retains_retry_entry_on_failure_and_uses_exact_paths() {
        let root = std::env::temp_dir().join(format!("carrier-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let carrier = root.join("carrier $(touch escaped) with spaces");
        let original = root.join("original.plist");
        let cleanup = root.join("cleanup.plist");
        std::fs::create_dir(&carrier).unwrap();
        std::fs::write(&original, b"original").unwrap();
        std::fs::write(&cleanup, b"retry").unwrap();
        let args = cleanup_arguments(&carrier, &original, &cleanup);
        let run = || {
            Command::new(&args[0])
                .args(&args[1..])
                .current_dir(&root)
                .output()
                .unwrap()
        };
        assert!(!run().status.success());
        assert!(original.exists() && cleanup.exists());
        assert!(!root.join("escaped").exists());
        std::fs::remove_dir(&carrier).unwrap();
        std::fs::write(&carrier, b"image").unwrap();
        assert!(run().status.success());
        assert!(!carrier.exists() && !original.exists() && !cleanup.exists());
        assert!(run().status.success());
        std::fs::remove_dir(&root).unwrap();
    }
}
