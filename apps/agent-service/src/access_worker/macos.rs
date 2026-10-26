//! Root-only signed account execution. PSSO callbacks and private keys stay in the extension.
use super::*;
use std::{
    ffi::CString,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use w::{
    AccessAccount as Account, AccessAction as Action, AccessEffect as Effect,
    AccessObservation as Observation,
};
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NativeAccount {
    pub name: String,
    pub generated_uid: String,
    pub uid: u32,
    pub administrator: bool,
    pub secure_token: i32,
}
pub(crate) fn inspect(value: &str, by_name: bool) -> Result<NativeAccount, Error> {
    unsafe extern "C" {
        fn rss_access_account(
            value: *const std::ffi::c_char,
            by_name: i32,
            output: *mut u8,
            capacity: usize,
        ) -> i32;
    }
    let value = CString::new(value).map_err(|_| Error::Identity)?;
    let mut out = [0u8; 4096];
    // SAFETY: fixed owned buffers remain live; native function checks the supplied capacity.
    let count = unsafe {
        rss_access_account(
            value.as_ptr(),
            i32::from(by_name),
            out.as_mut_ptr(),
            out.len(),
        )
    };
    if count == -2 {
        return Err(Error::NotFound);
    }
    if count < 0 || count as usize > out.len() {
        return Err(Error::Identity);
    }
    let account: NativeAccount =
        serde_json::from_slice(&out[..count as usize]).map_err(|_| Error::Protocol)?;
    if account.uid < 501 || !w::generated_uid_valid(&account.generated_uid) {
        return Err(Error::Identity);
    }
    Ok(account)
}
/// Fixed macOS tools with a private no-echo controlling terminal. Passwords never enter argv/env.
fn tool(
    program: &str,
    args: &[&str],
    password: Option<&str>,
    seconds: u64,
) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes two descriptors; each is immediately owned by a File.
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } != 0
    {
        return Err(Error::Unavailable);
    }
    let mut master = unsafe { std::fs::File::from_raw_fd(master) };
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };
    let mut term = unsafe { std::mem::zeroed::<libc::termios>() };
    if unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut term) } != 0 {
        return Err(Error::Unavailable);
    }
    term.c_lflag &= !(libc::ECHO | libc::ECHONL);
    if unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &term) } != 0 {
        return Err(Error::Unavailable);
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LANG", "C")
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave));
    let _terminal_group = native_process::TerminalGroup::configure(&mut command)?;
    struct Child(std::process::Child);
    impl std::ops::Deref for Child {
        type Target = std::process::Child;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl std::ops::DerefMut for Child {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl Drop for Child {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    let mut child = Child(command.spawn()?);
    if unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } < 0 {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Error::Unavailable);
    }
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut output = Zeroizing::new(Vec::new());
    let mut sent = false;
    loop {
        let mut bytes = [0u8; 2048];
        match master.read(&mut bytes) {
            Ok(n) => output.extend_from_slice(&bytes[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(_) => (),
        }
        if output.len() > 65536 || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Unavailable);
        }
        if !sent
            && password.is_some()
            && output
                .windows(8)
                .any(|v| v.eq_ignore_ascii_case(b"password"))
            && output.trim_ascii_end().last() == Some(&b':')
        {
            let password = password.ok_or(Error::Protocol)?;
            master.write_all(password.as_bytes())?;
            master.write_all(b"\n")?;
            sent = true;
        }
        if let Some(status) = child.try_wait()? {
            loop {
                let mut tail = [0u8; 2048];
                match master.read(&mut tail) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => output.extend_from_slice(&tail[..n]),
                }
                if output.len() > 65536 {
                    return Err(Error::Capacity);
                }
            }
            return if status.success() {
                Ok(output)
            } else {
                Err(Error::Unavailable)
            };
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[derive(Serialize, Deserialize)]
struct Creation {
    task: w::AccessTaskSpec,
    generated_uid: Option<String>,
}
fn facts(account: &NativeAccount) -> w::AccessMacFacts {
    let secure_token = match account.secure_token {
        1 => Observation::Present,
        0 => Observation::NeedsAdministrator,
        _ => Observation::Unknown,
    };
    let filevault_unlock = match tool("/usr/bin/fdesetup", &["list"], None, 10) {
        Ok(bytes) => match std::str::from_utf8(&bytes) {
            Ok(text) => {
                if text.lines().any(|l| {
                    l.split_once(',').is_some_and(|(_, id)| {
                        id.trim().eq_ignore_ascii_case(&account.generated_uid)
                    })
                }) {
                    Observation::Present
                } else {
                    Observation::Absent
                }
            }
            Err(_) => Observation::Unknown,
        },
        Err(_) => Observation::Unknown,
    };
    let volume_owner = tool(
        "/usr/sbin/diskutil",
        &["apfs", "listUsers", "/", "-plist"],
        None,
        10,
    )
    .ok()
    .and_then(|b| plist::Value::from_reader(std::io::Cursor::new(&*b)).ok())
    .and_then(|v| {
        v.as_dictionary()
            .and_then(|d| d.get("Users"))
            .and_then(plist::Value::as_array)
            .cloned()
    })
    .and_then(|users| {
        users.into_iter().find(|v| {
            v.as_dictionary()
                .and_then(|d| d.get("APFSCryptoUserUUID"))
                .and_then(plist::Value::as_string)
                .is_some_and(|id| id.eq_ignore_ascii_case(&account.generated_uid))
        })
    })
    .and_then(|v| {
        v.as_dictionary()
            .and_then(|d| d.get("VolumeOwner"))
            .and_then(plist::Value::as_boolean)
    })
    .map_or(Observation::Unknown, |owner| {
        if owner {
            Observation::Present
        } else {
            Observation::Absent
        }
    });
    w::AccessMacFacts {
        platform_sso: None,
        secure_token,
        volume_owner,
        filevault_unlock,
    }
}
pub(super) fn execute(input: &Input) -> Result<w::AccessResult, Error> {
    let _lock = state::lock(&input.root)?;
    let task = &input.task;
    if let Some(result) = stored_result(&input.root, task)? {
        return Ok(result);
    }
    let result = apply(input).unwrap_or(Effect::Unknown);
    let result = report(task, result);
    // Persist pending and unknown outcomes too: a restarted worker must not repeat mutations.
    state::write(&input.root, &name(task, "result"), &result)?;
    Ok(result)
}
fn apply(input: &Input) -> Result<Effect, Error> {
    let task = &input.task;
    if matches!(&task.action, Action::Apply{configuration,..}|Action::Create{configuration,..} if configuration.enabled)
    {
        crate::service::platform_sso::enabled(&input.root, input.installation)?;
    }
    let deployment =
        crate::deployment::Deployment::metadata(&crate::deployment::Deployment::default_path()?)?;
    if deployment.service_id != input.installation
        || crate::access_worker::root(&deployment.execution) != input.root
    {
        return Err(Error::Identity);
    }
    if matches!(&task.action, Action::Apply{configuration,..}|Action::Create{configuration,..} if configuration.enabled)
    {
        deployment
            .platform_sso
            .as_ref()
            .ok_or(Error::Unsupported)?
            .verify()?;
    }
    let (account, configuration) = match &task.action {
        Action::Apply {
            account: Account::Macos { generated_uid },
            configuration,
        } => {
            let mut account = inspect(&generated_uid.to_string(), false)?;
            let mut password = Zeroizing::new(String::new());
            std::io::stdin().take(1025).read_to_string(&mut password)?;
            if !password.is_empty() {
                if password.len() > 1024
                    || password.chars().any(char::is_control)
                    || !execution_ipc::macos_identity::console_user(account.uid)
                {
                    return Err(Error::Identity);
                }
                // The OS's interactive authorization UI owns the qualified administrator credential.
                tool(
                    "/bin/launchctl",
                    &[
                        "asuser",
                        &account.uid.to_string(),
                        "/usr/sbin/sysadminctl",
                        "-secureTokenOn",
                        &account.name,
                        "-password",
                        "-",
                        "interactive",
                    ],
                    Some(&password),
                    120,
                )?;
                account = inspect(&generated_uid.to_string(), false)?;
            }
            (account, configuration)
        }
        Action::Create {
            name: account_name,
            configuration,
        } => {
            let marker = name(task, "creation");
            let id = match state::read_optional::<Creation>(&input.root, &marker)? {
                Some(record) if record.task == *task => {
                    record.generated_uid.ok_or(Error::Conflict)?
                }
                Some(_) => return Err(Error::Identity),
                None => {
                    if account_name.is_empty()
                        || account_name.starts_with('-')
                        || account_name.len() > 64
                        || !account_name.bytes().all(|b| {
                            b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.'
                        })
                    {
                        return Err(Error::Protocol);
                    }
                    match inspect(account_name, true) {
                        Ok(_) => {
                            return Ok(Effect::Failed {
                                reason: w::AccessFailure::NameConflict,
                            })
                        }
                        Err(Error::NotFound) => (),
                        Err(e) => return Err(e),
                    }
                    let mut password = Zeroizing::new(String::new());
                    std::io::stdin().take(1025).read_to_string(&mut password)?;
                    if password.is_empty()
                        || password.len() > 1024
                        || password.chars().any(char::is_control)
                    {
                        return Err(Error::Protocol);
                    }
                    state::write(
                        &input.root,
                        &marker,
                        &Creation {
                            task: task.clone(),
                            generated_uid: None,
                        },
                    )?;
                    tool(
                        "/usr/sbin/sysadminctl",
                        &["-addUser", account_name, "-password", "-"],
                        Some(&password),
                        60,
                    )?;
                    let account = inspect(account_name, true)?;
                    state::write(
                        &input.root,
                        &marker,
                        &Creation {
                            task: task.clone(),
                            generated_uid: Some(account.generated_uid.clone()),
                        },
                    )?;
                    account.generated_uid
                }
            };
            (inspect(&id, false)?, configuration)
        }
        Action::Unbind {
            account: Account::Macos { generated_uid },
            disposition,
        } => {
            if *disposition != w::AccessAccountDisposition::Keep {
                return Err(Error::Unsupported);
            }
            state::write(
                &input.root,
                &name(task, "prepared"),
                &w::AccessPrepared {
                    binding_id: task.binding_id,
                    binding_generation: task.binding_generation,
                    revision: task.revision,
                    definition_digest: task.definition_digest,
                    account: Account::Macos {
                        generated_uid: *generated_uid,
                    },
                },
            )?;
            let name = crate::service::platform_sso::binding_name(&generated_uid.to_string())?;
            if let Some(mut binding) =
                state::read_optional::<platform_sso::LocalBinding>(&input.root, &name)?
            {
                if binding.binding != task.binding_id
                    || binding.binding_generation > task.binding_generation
                {
                    return Err(Error::Identity);
                }
                binding.enabled = false;
                binding.binding_generation = task.binding_generation;
                binding.revision = task.revision;
                crate::service::platform_sso::save_binding(&input.root, &binding)?;
            }
            return Ok(Effect::Unbound {
                disposition: *disposition,
            });
        }
        _ => return Err(Error::Unsupported),
    };
    if let Some(role) = configuration.role {
        unsafe extern "C" {
            fn rss_access_role(uuid: *const std::ffi::c_char, administrator: i32) -> i32;
        }
        let id = CString::new(account.generated_uid.as_str()).map_err(|_| Error::Identity)?;
        let result = unsafe {
            rss_access_role(id.as_ptr(), i32::from(role == w::AccessRole::Administrator))
        };
        if result == -2 {
            return Ok(Effect::Failed {
                reason: w::AccessFailure::RecoveryAccountProtected,
            });
        }
        if result != 0 {
            return Err(Error::Unavailable);
        }
    }
    let account = inspect(&account.generated_uid, false)?;
    let identity = Account::Macos {
        generated_uid: uuid::Uuid::parse_str(&account.generated_uid)
            .map_err(|_| Error::Identity)?,
    };
    state::write(
        &input.root,
        &name(task, "prepared"),
        &w::AccessPrepared {
            binding_id: task.binding_id,
            binding_generation: task.binding_generation,
            revision: task.revision,
            definition_digest: task.definition_digest,
            account: identity.clone(),
        },
    )?;
    crate::service::platform_sso::save_binding(
        &input.root,
        &platform_sso::LocalBinding {
            installation: input.installation,
            tenant: task.tenant_id,
            agent_registration: task.registration_id,
            registration_generation: task.generation,
            binding: task.binding_id,
            binding_generation: task.binding_generation,
            revision: task.revision,
            generated_uid: account.generated_uid.clone(),
            enabled: configuration.enabled,
            configuration: configuration.mac.clone().ok_or(Error::Unsupported)?,
        },
    )?;
    Ok(Effect::Applied {
        account: identity,
        role: if account.administrator {
            w::AccessRole::Administrator
        } else {
            w::AccessRole::Standard
        },
        mac: Some(observed_facts(&input.root, &account)?),
        windows: None,
    })
}

fn observed_facts(
    root: &std::path::Path,
    account: &NativeAccount,
) -> Result<w::AccessMacFacts, Error> {
    let mut result = facts(account);
    if let Some(binding) = state::read_optional::<platform_sso::LocalBinding>(
        root,
        &crate::service::platform_sso::binding_name(&account.generated_uid)?,
    )? {
        result.platform_sso = crate::service::platform_sso::native_facts(root, &binding)?;
    }
    Ok(result)
}
/// Complete the original prepared operation by reading native state, never repeating mutations.
pub(crate) fn refresh(
    root: &std::path::Path,
    installation: uuid::Uuid,
    task: &w::AccessTaskSpec,
) -> Result<(), Error> {
    refresh_with(
        root,
        installation,
        task,
        (crate::wall()? / 1000) as i64,
        |id| {
            let account = inspect(&id.to_string(), false)?;
            Ok((
                if account.administrator {
                    w::AccessRole::Administrator
                } else {
                    w::AccessRole::Standard
                },
                observed_facts(root, &account)?,
            ))
        },
    )
}
pub(crate) fn refresh_with(
    root: &std::path::Path,
    installation: uuid::Uuid,
    task: &w::AccessTaskSpec,
    now: i64,
    observe: impl FnOnce(uuid::Uuid) -> Result<(w::AccessRole, w::AccessMacFacts), Error>,
) -> Result<(), Error> {
    // The worker publishes its result only after native mutation has finished. Do not wait
    // for its state lock while it may still be showing an authorization prompt.
    if stored_result(root, task)?.is_none() {
        return Ok(());
    }
    let _lock = state::lock(root)?;
    if task.platform != w::TaskPlatform::Macos || task.deadline <= now {
        return Ok(());
    }
    if stored_result(root, task)?
        .is_some_and(|r| complete(task, &r.effect) || matches!(r.effect, Effect::Failed { .. }))
    {
        return Ok(());
    }
    let Some(prepared) = super::prepared(root, task)? else {
        return Ok(());
    };
    if prepared.binding_id != task.binding_id
        || prepared.binding_generation != task.binding_generation
        || prepared.revision != task.revision
        || prepared.definition_digest != task.definition_digest
    {
        return Err(Error::Identity);
    }
    let Account::Macos { generated_uid } = prepared.account else {
        return Err(Error::Identity);
    };
    let effect = match &task.action {
        Action::Apply { configuration, .. } | Action::Create { configuration, .. } => {
            if let Action::Apply { account, .. } = &task.action {
                if account != &(Account::Macos { generated_uid }) {
                    return Err(Error::Identity);
                }
            }
            let next = platform_sso::LocalBinding {
                installation,
                tenant: task.tenant_id,
                agent_registration: task.registration_id,
                registration_generation: task.generation,
                binding: task.binding_id,
                binding_generation: task.binding_generation,
                revision: task.revision,
                generated_uid: generated_uid.to_string(),
                enabled: configuration.enabled,
                configuration: configuration.mac.clone().ok_or(Error::Unsupported)?,
            };
            crate::service::platform_sso::save_binding(root, &next)?;
            let (role, facts) = observe(generated_uid)?;
            Effect::Applied {
                account: Account::Macos { generated_uid },
                role,
                mac: Some(facts),
                windows: None,
            }
        }
        Action::Unbind {
            account,
            disposition,
        } => {
            if account != &(Account::Macos { generated_uid })
                || *disposition != w::AccessAccountDisposition::Keep
            {
                return Err(Error::Identity);
            }
            if let Some(mut binding) = state::read_optional::<platform_sso::LocalBinding>(
                root,
                &crate::service::platform_sso::binding_name(&generated_uid.to_string())?,
            )? {
                if binding.binding != task.binding_id
                    || binding.binding_generation > task.binding_generation
                {
                    return Err(Error::Identity);
                }
                binding.enabled = false;
                binding.binding_generation = task.binding_generation;
                binding.revision = task.revision;
                crate::service::platform_sso::save_binding(root, &binding)?;
            }
            Effect::Unbound {
                disposition: *disposition,
            }
        }
    };
    state::write(root, &name(task, "result"), &report(task, effect))
}
