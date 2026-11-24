//! Original create intent and native attribution. Callers hold the shared account worker lock.
use super::*;
use execution_contract::Id;
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct Creation {
    task: w::AccessTaskSpec,
    boot: Id,
    expected_uid: u32,
    group: Option<i32>,
    generated_uid: Option<String>,
}

pub(crate) enum Evidence {
    Unstarted,
    Unknown,
    Absent,
    Created(NativeAccount),
}

pub(crate) trait Credential {
    fn admit<T>(&self, action: impl FnOnce() -> Result<T, Error>) -> Result<T, Error>;
    fn send(&mut self, sink: &mut dyn Write) -> Result<(), Error>;
}
impl Credential for Zeroizing<String> {
    fn admit<T>(&self, action: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        action()
    }
    fn send(&mut self, sink: &mut dyn Write) -> Result<(), Error> {
        write_password(self, sink)
    }
}
trait Directory {
    fn guid_for_uid(&self, uid: u32) -> Result<Option<String>, Error>;
    fn exists(&self, name: &str) -> Result<bool, Error>;
    fn inspect(&self, key: &str, by_name: bool) -> Result<NativeAccount, Error>;
    fn add(
        &self,
        name: &str,
        uid: u32,
        admin: bool,
        input: &mut dyn TerminalInput,
    ) -> Result<(), Error>;
}
struct Local;
impl Directory for Local {
    fn guid_for_uid(&self, uid: u32) -> Result<Option<String>, Error> {
        crate::account_identity::generated_uid(&uid.to_string())
    }
    fn exists(&self, name: &str) -> Result<bool, Error> {
        unsafe extern "C" {
            fn rss_access_name_exists(name: *const std::ffi::c_char) -> i32;
        }
        let name = CString::new(name).map_err(|_| Error::Identity)?;
        // SAFETY: the native local-directory query borrows the NUL-terminated name.
        match unsafe { rss_access_name_exists(name.as_ptr()) } {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Unavailable),
        }
    }
    fn inspect(&self, key: &str, by_name: bool) -> Result<NativeAccount, Error> {
        inspect(key, by_name)
    }
    fn add(
        &self,
        name: &str,
        uid: u32,
        admin: bool,
        input: &mut dyn TerminalInput,
    ) -> Result<(), Error> {
        let numeric = uid.to_string();
        let mut args = vec!["-addUser", name, "-UID", &numeric, "-password", "-"];
        if admin {
            args.push("-admin");
        }
        terminal_tool("/usr/sbin/sysadminctl", &args, Some(input), 60).map(|_| ())
    }
}
pub(crate) fn evidence(
    root: &Path,
    task: &w::AccessTaskSpec,
    short_name: &str,
) -> Result<Evidence, Error> {
    evidence_with(&Local, root, task, short_name)
}
fn evidence_with(
    native: &impl Directory,
    root: &Path,
    task: &w::AccessTaskSpec,
    short_name: &str,
) -> Result<Evidence, Error> {
    let Some(mut intent) = state::read_optional::<Creation>(root, &name(task, "creation"))? else {
        return Ok(Evidence::Unstarted);
    };
    if intent.task != *task {
        return Err(Error::Identity);
    }
    if let Some(uid) = &intent.generated_uid {
        return Ok(native
            .inspect(uid, false)
            .map_or(Evidence::Unknown, Evidence::Created));
    }
    // Owner death drops the terminal watchdog. A negative read only proves absence after
    // that process group has actually disappeared, or the machine has rebooted.
    let boot = execution_ipc::host::boot_generation().map_err(|_| Error::Identity)?;
    let stopped = intent.boot != boot
        || intent.group.is_some_and(|group| {
            group > 1
                && unsafe { libc::kill(-group, 0) } == -1
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        });
    if !stopped {
        return Ok(Evidence::Unknown);
    }
    if !native.exists(short_name)? {
        return Ok(Evidence::Absent);
    }
    let Some(account) = attributed(native, short_name, intent.expected_uid) else {
        return Ok(Evidence::Unknown);
    };
    intent.generated_uid = Some(account.generated_uid.clone());
    state::write(root, &name(task, "creation"), &intent)?;
    Ok(Evidence::Created(account))
}

fn attributed(native: &impl Directory, name: &str, uid: u32) -> Option<NativeAccount> {
    let account = native.inspect(name, true).ok()?;
    let unique = native.guid_for_uid(uid).ok()??;
    (account.uid == uid
        && account.name.eq_ignore_ascii_case(name)
        && account.generated_uid.eq_ignore_ascii_case(&unique))
    .then_some(account)
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
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut random)
            .map_err(|_| Error::Unavailable)?;
        let uid = 501 + u32::from_ne_bytes(random) % 59000;
        let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let mut buffer = [0u8; 16384];
        // SAFETY: libc receives writable caller-owned storage and returns a borrowed pointer.
        let err = unsafe {
            libc::getpwuid_r(
                uid,
                record.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if err != 0 {
            return Err(Error::Unavailable);
        }
        if result.is_null() {
            return Ok(uid);
        }
    }
    Err(Error::Conflict)
}

pub(crate) fn create<P: Credential>(
    root: &Path,
    task: &w::AccessTaskSpec,
    short_name: &str,
    input: impl FnOnce() -> Result<P, Error>,
) -> Result<Outcome, Error> {
    create_with(&Local, root, task, short_name, input)
}
fn create_with<P: Credential>(
    native: &impl Directory,
    root: &Path,
    task: &w::AccessTaskSpec,
    short_name: &str,
    input: impl FnOnce() -> Result<P, Error>,
) -> Result<Outcome, Error> {
    let marker = name(task, "creation");
    match evidence_with(native, root, task, short_name)? {
        Evidence::Created(account) => return Ok(Outcome::Created(account)),
        Evidence::Absent | Evidence::Unknown => return Err(Error::Unavailable),
        Evidence::Unstarted => (),
    }
    if native.exists(short_name)? {
        return Ok(Outcome::NameConflict);
    }
    let uid = available_uid()?;
    let mut intent = Creation {
        task: task.clone(),
        expected_uid: uid,
        boot: execution_ipc::host::boot_generation().map_err(|_| Error::Identity)?,
        group: None,
        generated_uid: None,
    };
    let mut launch = Launch {
        credential: input()?,
        root,
        marker: &marker,
        intent: &mut intent,
    };
    let admin = matches!(&task.action,w::AccessAction::Create{configuration,..} if configuration.role==Some(w::AccessRole::Administrator));
    native.add(short_name, uid, admin, &mut launch)?;
    let account = attributed(native, short_name, uid).ok_or(Error::Identity)?;
    intent.generated_uid = Some(account.generated_uid.clone());
    state::write(root, &marker, &intent)?;
    Ok(Outcome::Created(account))
}

struct Launch<'a, P> {
    credential: P,
    root: &'a Path,
    marker: &'a str,
    intent: &'a mut Creation,
}
impl<P: Credential> TerminalInput for Launch<'_, P> {
    fn start(&mut self, spawn: &mut dyn FnMut() -> Result<u32, Error>) -> Result<(), Error> {
        self.credential.admit(|| {
            if self.intent.task.expires_at <= (crate::wall()? / 1000) as i64 {
                return Err(Error::Expired);
            }
            // PTY setup is complete. Only this short interval can leave a genuinely uncertain launch.
            state::write(self.root, self.marker, self.intent)?;
            let pid = match spawn() {
                Ok(id) => id,
                Err(e) => {
                    state::remove(self.root, self.marker)?;
                    return Err(e);
                }
            };
            self.intent.group = Some(i32::try_from(pid).map_err(|_| Error::Identity)?);
            state::write(self.root, self.marker, self.intent)
        })
    }
    fn send(&mut self, sink: &mut dyn Write) -> Result<(), Error> {
        if self.intent.task.expires_at <= (crate::wall()? / 1000) as i64 {
            return Err(Error::Expired);
        }
        self.credential.send(sink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use uuid::Uuid;

    struct Fake {
        calls: Cell<usize>,
        uid: Cell<Option<u32>>,
        guid: Uuid,
        fail_after_add: bool,
    }
    impl Directory for Fake {
        fn guid_for_uid(&self, uid: u32) -> Result<Option<String>, Error> {
            Ok((self.uid.get() == Some(uid)).then(|| self.guid.to_string()))
        }
        fn exists(&self, _: &str) -> Result<bool, Error> {
            Ok(self.uid.get().is_some())
        }
        fn inspect(&self, key: &str, by_name: bool) -> Result<NativeAccount, Error> {
            if (by_name && !key.eq_ignore_ascii_case("employee"))
                || (!by_name && key != self.guid.to_string())
            {
                return Err(Error::NotFound);
            }
            Ok(NativeAccount {
                name: "employee".into(),
                uid: self.uid.get().ok_or(Error::NotFound)?,
                generated_uid: self.guid.to_string(),
                administrator: false,
                secure_token: -1,
            })
        }
        fn add(
            &self,
            _: &str,
            uid: u32,
            admin: bool,
            input: &mut dyn TerminalInput,
        ) -> Result<(), Error> {
            assert!(admin);
            input.start(&mut || {
                self.calls.set(self.calls.get() + 1);
                Ok(1)
            })?;
            input.send(&mut std::io::sink())?;
            self.uid.set(Some(uid));
            if self.fail_after_add {
                return Err(Error::Unavailable);
            }
            Ok(())
        }
    }
    use crate::da_service::mac_accounts::tests::task;
    #[test]
    fn restart_reads_only_the_original_uid_and_preserves_uncertain_or_conflicting_names() {
        for fail_after_add in [false, true] {
            let root = std::env::temp_dir().join(format!("rss-mac-create-{}", Uuid::new_v4()));
            platform_private_storage::directory(&root).unwrap();
            let t = task();
            t.validate().unwrap();
            let native = Fake {
                calls: Cell::new(0),
                uid: Cell::new(Some(501)),
                guid: Uuid::new_v4(),
                fail_after_add,
            };
            assert!(matches!(
                create_with(
                    &native,
                    &root,
                    &t,
                    "Employee",
                    || -> Result<Zeroizing<String>, Error> { panic!("name exists") }
                ),
                Ok(Outcome::NameConflict)
            ));
            native.uid.set(None);
            let outcome = create_with(&native, &root, &t, "employee", || {
                Ok(Zeroizing::new("local-fixture".into()))
            });
            assert_eq!(outcome.is_err(), fail_after_add);
            let replay = create_with(
                &native,
                &root,
                &t,
                "employee",
                || -> Result<Zeroizing<String>, Error> { panic!("replayed native create") },
            );
            assert_eq!(replay.is_err(), fail_after_add);
            assert_eq!(native.calls.get(), 1);
            if fail_after_add {
                assert!(matches!(
                    evidence_with(&native, &root, &t, "employee").unwrap(),
                    Evidence::Unknown
                ));
                let key = name(&t, "creation");
                let mut intent: Creation = state::read_optional(&root, &key).unwrap().unwrap();
                intent.boot = Id::new("previous-boot").unwrap();
                state::write(&root, &key, &intent).unwrap();
                native.uid.set(Some(501));
                assert!(matches!(
                    evidence_with(&native, &root, &t, "employee").unwrap(),
                    Evidence::Unknown
                ));
                native.uid.set(None);
                assert!(matches!(
                    evidence_with(&native, &root, &t, "employee").unwrap(),
                    Evidence::Absent
                ));
                native.uid.set(Some(intent.expected_uid));
                assert!(matches!(
                    evidence_with(&native, &root, &t, "employee").unwrap(),
                    Evidence::Created(_)
                ));
                assert_eq!(
                    state::read_optional::<Creation>(&root, &key)
                        .unwrap()
                        .unwrap()
                        .generated_uid,
                    Some(native.guid.to_string())
                );
            } else {
                let Outcome::Created(actual) = replay.unwrap() else {
                    panic!("wrong effect")
                };
                assert_eq!(actual.generated_uid, native.guid.to_string());
                native.uid.set(None);
                assert!(matches!(
                    evidence_with(&native, &root, &t, "employee").unwrap(),
                    Evidence::Unknown
                ));
            }
            assert_eq!(native.calls.get(), 1);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn definite_spawn_failure_leaves_the_attempt_unstarted() {
        let root = std::env::temp_dir().join(format!("rss-mac-spawn-{}", Uuid::new_v4()));
        platform_private_storage::directory(&root).unwrap();
        let t = task();
        let key = name(&t, "creation");
        let mut intent = Creation {
            task: t.clone(),
            expected_uid: 550,
            boot: execution_ipc::host::boot_generation().unwrap(),
            group: None,
            generated_uid: None,
        };
        let mut launch = Launch {
            credential: Zeroizing::new("test".into()),
            root: &root,
            marker: &key,
            intent: &mut intent,
        };
        assert_eq!(
            launch.start(&mut || Err(Error::Unavailable)),
            Err(Error::Unavailable)
        );
        assert!(matches!(
            evidence(&root, &t, "employee").unwrap(),
            Evidence::Unstarted
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_filevault_output_is_unknown_and_a_valid_other_uid_is_absent() {
        let id = Uuid::new_v4();
        assert_eq!(filevault_list(b"list unavailable", &id.to_string()), None);
        assert_eq!(filevault_list(b"", &id.to_string()), None);
        assert_eq!(
            filevault_list(format!("one,{id}\n").as_bytes(), &id.to_string()),
            Some(Observation::Present)
        );
        assert_eq!(
            filevault_list(
                format!("one,{id}\n").as_bytes(),
                &Uuid::new_v4().to_string()
            ),
            Some(Observation::Absent)
        );
    }
}
