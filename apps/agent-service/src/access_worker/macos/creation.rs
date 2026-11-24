//! Original create intent and native attribution. Callers hold the shared account worker lock.
use super::*;
use execution_contract::Id;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub(in crate::access_worker) struct Creation {
    task: w::AccessTaskSpec,
    boot: Id,
    group: Option<i32>,
    generated_uid: Option<String>,
}

pub(crate) enum Evidence {
    Unstarted,
    Unknown,
    Absent,
    Created(NativeAccount),
}

pub(crate) fn evidence(root: &Path, task: &w::AccessTaskSpec, short_name: &str) -> Result<Evidence, Error> {
    let Some(intent) = state::read_optional::<Creation>(root, &name(task, "creation"))? else { return Ok(Evidence::Unstarted); };
    if intent.task != *task { return Err(Error::Identity); }
    if let Some(uid) = intent.generated_uid {
        return Ok(inspect(&uid, false).map_or(Evidence::Unknown, Evidence::Created));
    }
    // Owner death drops the terminal watchdog. A negative read only proves absence after
    // that process group has actually disappeared, or the machine has rebooted.
    let boot = execution_ipc::host::boot_generation().map_err(|_| Error::Identity)?;
    let stopped = intent.boot != boot || intent.group.is_some_and(|group| {
        group > 1 && unsafe { libc::kill(-group, 0) } == -1 &&
            std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    });
    if stopped && matches!(inspect(short_name, true), Err(Error::NotFound)) { Ok(Evidence::Absent) }
    else { Ok(Evidence::Unknown) }
}

pub(crate) enum Outcome {
    Created(NativeAccount),
    NameConflict,
}

fn available_uid() -> Result<u32, Error> {
    // sysadminctl has no GeneratedUID input. Give this invocation an unused POSIX UID so a
    // simultaneous same-name creation by another administrator cannot be adopted by name.
    for _ in 0..8 {
        let mut random = [0u8; 4];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut random).map_err(|_| Error::Unavailable)?;
        let uid = 501 + u32::from_ne_bytes(random) % 59000;
        let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let mut buffer = [0u8; 16384];
        // SAFETY: libc receives writable caller-owned storage and returns a borrowed pointer.
        let err = unsafe { libc::getpwuid_r(uid, record.as_mut_ptr(), buffer.as_mut_ptr().cast(), buffer.len(), &mut result) };
        if err != 0 { return Err(Error::Unavailable); }
        if result.is_null() { return Ok(uid); }
    }
    Err(Error::Conflict)
}

pub(crate) fn create(
    root: &Path,
    task: &w::AccessTaskSpec,
    short_name: &str,
    input: impl FnOnce() -> Result<Zeroizing<String>, Error>,
) -> Result<Outcome, Error> {
    let marker = name(task, "creation");
    match evidence(root, task, short_name)? {
        Evidence::Created(account) => return Ok(Outcome::Created(account)),
        Evidence::Absent | Evidence::Unknown => return Err(Error::Unavailable),
        Evidence::Unstarted => (),
    }
    match inspect(short_name, true) {
        Ok(_) => return Ok(Outcome::NameConflict),
        Err(Error::NotFound) => (),
        Err(e) => return Err(e),
    }
    let uid = available_uid()?;
    let mut intent = Creation {
        task: task.clone(),
        boot: execution_ipc::host::boot_generation().map_err(|_| Error::Identity)?,
        group: None, generated_uid: None,
    };
    let password = input()?;
    if password.is_empty() || password.len() > 1024 || password.chars().any(char::is_control) { return Err(Error::Denied); }
    if task.expires_at.min(task.deadline) <= (crate::wall()?/1000) as i64 { return Err(Error::Expired); }
    state::write(root, &marker, &intent)?;
    terminal_tool("/usr/sbin/sysadminctl", &["-addUser",short_name,"-UID",&uid.to_string(),"-password","-"], Some(&password), 60, |pid| {
        intent.group = Some(i32::try_from(pid).map_err(|_|Error::Identity)?);
        state::write(root, &marker, &intent)
    })?;
    let account = inspect(short_name, true)?;
    if account.uid != uid || !account.name.eq_ignore_ascii_case(short_name) { return Err(Error::Identity); }
    intent.generated_uid = Some(account.generated_uid.clone());
    state::write(root, &marker, &intent)?;
    Ok(Outcome::Created(account))
}
