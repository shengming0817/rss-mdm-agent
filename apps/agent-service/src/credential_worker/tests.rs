use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use rsa::{pkcs8::EncodePublicKey, rand_core::OsRng, RsaPrivateKey};
use std::sync::{Arc, LazyLock, Mutex};
use w::{CredentialFailure as Failure, CredentialOperation as Op, CredentialSubject as Subject};
static KEY: LazyLock<RsaPrivateKey> =
    LazyLock::new(|| RsaPrivateKey::new(&mut OsRng, 2048).unwrap());
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let p = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-credential-{}", uuid::Uuid::new_v4()));
        platform_private_storage::PrivateDirectory::create(&p).unwrap();
        Self(p)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[derive(Default)]
struct OsState {
    secret: Option<String>,
    subject: Option<Subject>,
    changes: usize,
    removals: usize,
}
struct Fake {
    state: Arc<Mutex<OsState>>,
    conflict: bool,
    protect_failed: bool,
    ambiguous: bool,
    remove_failed: bool,
    collect_failure: bool,
    logon_denied: bool,
}
impl Fake {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(OsState::default())),
            conflict: false,
            protect_failed: false,
            ambiguous: false,
            remove_failed: false,
            collect_failure: false,
            logon_denied: false,
        }
    }
}
fn crypt(bytes: &[u8], seal: bool) -> Result<Zeroizing<Vec<u8>>, Error> {
    use ring::aead::*;
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &[7; 32]).unwrap());
    let mut value = Zeroizing::new(bytes.to_vec());
    if seal {
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key([0; 12]),
            Aad::empty(),
            &mut *value,
        )
        .map_err(|_| Error::Storage)?;
    } else {
        let len = key
            .open_in_place(
                Nonce::assume_unique_for_key([0; 12]),
                Aad::empty(),
                &mut value,
            )
            .map_err(|_| Error::Storage)?
            .len();
        value.truncate(len);
    }
    Ok(value)
}
impl Native for Fake {
    fn collect(&mut self) -> Result<Inventory, Failure> {
        Ok(Inventory {
            materials: vec![Material {
                subject: Subject::Bitlocker {
                    volume: "volume".into(),
                    protector: uuid::Uuid::from_u128(10),
                },
                secret: Zeroizing::new("old numerical password".into()),
            }],
            failure: self.collect_failure.then_some(Failure::NativeFailure),
        })
    }
    fn bitlocker(&mut self, volume: &str, protector: uuid::Uuid) -> Result<Material, Failure> {
        self.collect()?
            .materials
            .into_iter()
            .find(|m| {
                m.subject
                    == Subject::Bitlocker {
                        volume: volume.into(),
                        protector,
                    }
            })
            .ok_or(Failure::TargetMissing)
    }
    fn apply(&mut self, op: &Op, secret: &str) -> Result<(), Failure> {
        if self.conflict {
            return Err(Failure::OwnerConflict);
        }
        let mut s = self.state.lock().unwrap();
        s.changes += 1;
        s.secret = Some(secret.into());
        s.subject = Some(match op {
            Op::RotateBitlocker { volume, .. } => Subject::Bitlocker {
                volume: volume.clone(),
                protector: uuid::Uuid::from_u128(11),
            },
            Op::RotateLocalAdministrator { sid } => {
                Subject::LocalAdministrator { sid: sid.clone() }
            }
            _ => unreachable!(),
        });
        if self.ambiguous {
            Err(Failure::NativeFailure)
        } else {
            Ok(())
        }
    }
    fn locate(&mut self, _: &Op, secret: &str) -> Result<Option<Subject>, Failure> {
        if self.conflict {
            return Err(Failure::OwnerConflict);
        }
        if self.logon_denied {
            return Err(Failure::PermissionDenied);
        }
        let s = self.state.lock().unwrap();
        Ok((s.secret.as_deref() == Some(secret))
            .then(|| s.subject.clone())
            .flatten())
    }
    fn remove(&mut self, _: &str, protector: uuid::Uuid) -> Result<(), Failure> {
        if self.remove_failed {
            return Err(Failure::PermissionDenied);
        }
        assert_eq!(protector, uuid::Uuid::from_u128(10));
        self.state.lock().unwrap().removals += 1;
        Ok(())
    }
    fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        if self.protect_failed {
            Err(Error::Storage)
        } else {
            Ok(crypt(bytes, true)?.to_vec())
        }
    }
    fn unprotect(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
        crypt(bytes, false)
    }
}
fn input(root: &Root, operation: Op) -> Input {
    let mut task = w::CredentialTaskSpec {
        wire_version: 1,
        tenant_id: uuid::Uuid::from_u128(1),
        device_id: "device".into(),
        platform: w::TaskPlatform::Windows,
        architecture: w::TaskArchitecture::X86_64,
        registration_id: uuid::Uuid::from_u128(2),
        generation: 1,
        task_id: uuid::Uuid::new_v4(),
        attempt_id: uuid::Uuid::new_v4(),
        permit: w::TaskPermit::Start,
        expires_at: (crate::wall().unwrap() / 1000 + 10) as i64,
        deadline: (crate::wall().unwrap() / 1000 + 15) as i64,
        authorization: w::TaskAuthorization::Policy {
            policy_id: uuid::Uuid::from_u128(3),
            policy_version: uuid::Uuid::from_u128(4),
        },
        operation,
        recipient: w::CredentialRecipient {
            key_id: "a".repeat(64),
            public_key: STANDARD
                .encode(KEY.to_public_key().to_public_key_der().unwrap().as_bytes()),
        },
        expected: vec![],
        recovery: None,
        definition_digest: [0; 32],
    };
    task.definition_digest = w::CredentialTaskSpec::digest(
        &task.operation,
        &task.recipient,
        &task.expected,
        &task.recovery,
    )
    .unwrap();
    task.validate().unwrap();
    Input {
        task,
        root: root.0.clone(),
        image_sha256: Digest::new("a".repeat(64)).unwrap(),
        run_as: RunAs::System {
            platform: execution_contract::Platform::Windows,
        },
        session: SessionRequirement::NotRequired {},
    }
}
fn ack(input: &Input) -> std::thread::JoinHandle<()> {
    let root = input.root.clone();
    let task = input.task.clone();
    std::thread::spawn(move || {
        for _ in 0..500 {
            if let Some(value) = pending(&root, &task).unwrap() {
                acknowledge(&root, &task, &value, &[]).unwrap();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("worker never published escrow")
    })
}
fn rotate() -> Op {
    Op::RotateLocalAdministrator {
        sid: "S-1-5-21-1-2-3-1001".into(),
    }
}
#[test]
fn protected_candidate_precedes_mutation_and_replay_never_rotates_again() {
    let root = Root::new();
    let input = input(&root, rotate());
    let mut n = Fake::new();
    n.ambiguous = true;
    let a = ack(&input);
    let result = engine::execute(&input, &mut n).unwrap();
    a.join().unwrap();
    assert_eq!(result.items[0].outcome, w::CredentialOutcome::Verified);
    assert_eq!(engine::execute(&input, &mut n).unwrap(), result);
    assert_eq!(n.state.lock().unwrap().changes, 1);
    let secret = n.state.lock().unwrap().secret.clone().unwrap();
    for file in std::fs::read_dir(&root.0).unwrap() {
        let bytes = std::fs::read(file.unwrap().path()).unwrap();
        assert!(!bytes.windows(secret.len()).any(|v| v == secret.as_bytes()));
    }
}
#[test]
fn protection_failure_and_owner_conflict_prevent_os_changes() {
    let root = Root::new();
    let input = input(&root, rotate());
    let mut n = Fake::new();
    n.protect_failed = true;
    assert!(engine::execute(&input, &mut n).is_err());
    assert_eq!(n.state.lock().unwrap().changes, 0);
    n.protect_failed = false;
    n.conflict = true;
    assert_eq!(
        engine::execute(&input, &mut n).unwrap().failure,
        Some(Failure::OwnerConflict)
    );
    assert_eq!(n.state.lock().unwrap().changes, 0);
}
#[test]
fn bitlocker_cleanup_waits_for_exact_escrow_ack() {
    let root = Root::new();
    let input = input(
        &root,
        Op::RotateBitlocker {
            volume: "volume".into(),
            protector: uuid::Uuid::from_u128(10),
        },
    );
    let root_path = root.0.clone();
    let task = input.task.clone();
    let n = Fake::new();
    let state = n.state.clone();
    let worker = std::thread::spawn(move || {
        let mut n = n;
        engine::execute(&input, &mut n)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let escrow = loop {
        if worker.is_finished() || std::time::Instant::now() >= deadline {
            panic!("worker stopped before escrow: {:?}", worker.join());
        }
        if let Some(value) = pending(&root_path, &task).unwrap() {
            break value;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    state::write(&root_path, &state::name(&task, "ack"), &"wrong digest").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert_eq!(state.lock().unwrap().removals, 0);
    acknowledge(&root_path, &task, &escrow, &[]).unwrap();
    assert!(worker.join().unwrap().unwrap().failure.is_none());
    assert_eq!(state.lock().unwrap().removals, 1);
}
#[test]
fn recovery_reuses_issued_candidate_after_original_worker_stops() {
    let root = Root::new();
    let original = input(&root, rotate());
    let mut n = Fake::new();
    let secret = "Aa9!known candidate";
    let protected = state::seal(&n, &original.task, secret).unwrap();
    state::write(
        &root.0,
        &state::name(&original.task, "state"),
        &state::Record {
            task: original.task.clone(),
            phase: state::Phase::Issued,
            protected,
            subject: None,
        },
    )
    .unwrap();
    n.apply(&original.task.operation, secret).unwrap();
    let mut next = input(
        &root,
        Op::VerifyLocalAdministrator {
            sid: "S-1-5-21-1-2-3-1001".into(),
        },
    );
    next.task.recovery = Some(w::CredentialRecovery {
        task_id: original.task.task_id,
        attempt_id: original.task.attempt_id,
    });
    next.task.definition_digest = w::CredentialTaskSpec::digest(
        &next.task.operation,
        &next.task.recipient,
        &next.task.expected,
        &next.task.recovery,
    )
    .unwrap();
    let a = ack(&next);
    let result = engine::execute(&next, &mut n).unwrap();
    a.join().unwrap();
    assert_eq!(result.recovery, Some(w::CredentialRecoveryOutcome::Applied));
    assert_eq!(n.state.lock().unwrap().changes, 1);
}
#[test]
fn prepared_recovery_proves_not_attempted_and_mismatched_generation_cannot_recover() {
    let root = Root::new();
    let original = input(&root, rotate());
    let mut n = Fake::new();
    state::write(
        &root.0,
        &state::name(&original.task, "state"),
        &state::Record {
            task: original.task.clone(),
            phase: state::Phase::Prepared,
            protected: state::seal(&n, &original.task, "candidate").unwrap(),
            subject: None,
        },
    )
    .unwrap();
    let mut next = input(
        &root,
        Op::VerifyLocalAdministrator {
            sid: "S-1-5-21-1-2-3-1001".into(),
        },
    );
    next.task.recovery = Some(w::CredentialRecovery {
        task_id: original.task.task_id,
        attempt_id: original.task.attempt_id,
    });
    next.task.definition_digest = w::CredentialTaskSpec::digest(
        &next.task.operation,
        &next.task.recipient,
        &next.task.expected,
        &next.task.recovery,
    )
    .unwrap();
    assert_eq!(
        engine::execute(&next, &mut n).unwrap().recovery,
        Some(w::CredentialRecoveryOutcome::NotAttempted)
    );
    next.task.task_id = uuid::Uuid::new_v4();
    next.task.generation = 2;
    assert_eq!(
        engine::execute(&next, &mut n).unwrap().failure,
        Some(Failure::RecoveryState)
    );
    assert_eq!(n.state.lock().unwrap().changes, 0);
}
#[test]
fn envelope_uses_wire_aad_and_roundtrips_only_with_matching_subject() {
    let root = Root::new();
    let input = input(&root, rotate());
    let material = Material {
        subject: Subject::LocalAdministrator {
            sid: "S-1-5-21-1-2-3-1001".into(),
        },
        secret: Zeroizing::new("Aa9!original".into()),
    };
    let envelope = envelope::encrypt(&input.task, &material).unwrap();
    let key = KEY
        .decrypt(
            rsa::Oaep::new::<sha2::Sha256>(),
            &STANDARD.decode(envelope.wrapped_key).unwrap(),
        )
        .unwrap();
    let aead = ring::aead::LessSafeKey::new(
        ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, &key).unwrap(),
    );
    let mut bytes = STANDARD.decode(envelope.ciphertext).unwrap();
    let mut tampered = bytes.clone();
    assert!(aead
        .open_in_place(
            ring::aead::Nonce::assume_unique_for_key(envelope.nonce),
            ring::aead::Aad::from(b"different task"),
            &mut tampered
        )
        .is_err());
    let plain = aead
        .open_in_place(
            ring::aead::Nonce::assume_unique_for_key(envelope.nonce),
            ring::aead::Aad::from(input.task.material_aad(&material.subject, 0).unwrap()),
            &mut bytes,
        )
        .unwrap();
    assert_eq!(plain, material.secret.as_bytes());
}

#[test]
fn cleanup_failure_retains_escrow_subject_and_unknown_effect() {
    let root = Root::new();
    let input = input(
        &root,
        Op::RotateBitlocker {
            volume: "volume".into(),
            protector: uuid::Uuid::from_u128(10),
        },
    );
    let mut n = Fake::new();
    n.remove_failed = true;
    let a = ack(&input);
    let result = engine::result(&input, &mut n).unwrap();
    a.join().unwrap();
    assert_eq!(result.failure, Some(Failure::NativeFailure));
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].outcome, w::CredentialOutcome::Unknown);
    let interrupted = failure_with_materials(&root.0, &input.task, Failure::RecoveryState).unwrap();
    assert_eq!(interrupted.items, result.items);
    retire(&root.0, &input.task).unwrap();
    assert!(
        state::read_optional::<state::Record>(&root.0, &state::name(&input.task, "state"))
            .unwrap()
            .is_some()
    );
    assert!(pending(&root.0, &input.task).unwrap().is_none());
}
#[test]
fn exact_verification_ignores_unrelated_volume_failure_and_retires_files() {
    let root = Root::new();
    let input = input(
        &root,
        Op::VerifyBitlocker {
            volume: "volume".into(),
            protector: uuid::Uuid::from_u128(10),
        },
    );
    let mut n = Fake::new();
    n.collect_failure = true;
    let a = ack(&input);
    let result = engine::execute(&input, &mut n).unwrap();
    a.join().unwrap();
    assert!(result.complete);
    assert!(result.failure.is_none());
    state::write(
        &root.0,
        &format!("{}.tmp", uuid::Uuid::new_v4()),
        &"interrupted write",
    )
    .unwrap();
    retire(&root.0, &input.task).unwrap();
    retire(&root.0, &input.task).unwrap();
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 1);
}
#[test]
fn account_current_survives_retirement_and_later_rotation_does_not_accumulate_state() {
    let root = Root::new();
    let mut n = Fake::new();
    for _ in 0..2 {
        let input = input(&root, rotate());
        let a = ack(&input);
        engine::execute(&input, &mut n).unwrap();
        a.join().unwrap();
        retire(&root.0, &input.task).unwrap();
    }
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 2); // owner lock and current account
    let input = input(
        &root,
        Op::VerifyLocalAdministrator {
            sid: "S-1-5-21-1-2-3-1001".into(),
        },
    );
    let a = ack(&input);
    assert!(engine::execute(&input, &mut n).unwrap().failure.is_none());
    a.join().unwrap();
}

#[test]
fn acknowledged_no_effect_retires_candidate_but_unknown_keeps_it() {
    let root = Root::new();
    let input = input(&root, rotate());
    let native = Fake::new();
    let record = state::Record {
        task: input.task.clone(),
        phase: state::Phase::Issued,
        protected: state::seal(&native, &input.task, "candidate").unwrap(),
        subject: None,
    };
    state::write(&root.0, &state::name(&input.task, "state"), &record).unwrap();
    state::write(
        &root.0,
        &state::name(&input.task, "result"),
        &failed(&input.task, Failure::PermissionDenied),
    )
    .unwrap();
    retire(&root.0, &input.task).unwrap();
    assert!(
        state::read_optional::<state::Record>(&root.0, &state::name(&input.task, "state"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn denied_logon_still_escrows_changed_password_and_recovery_never_resets_again() {
    let root = Root::new();
    let original = input(&root, rotate());
    let mut n = Fake::new();
    n.logon_denied = true;
    let a = ack(&original);
    let result = engine::result(&original, &mut n).unwrap();
    a.join().unwrap();
    assert_eq!(n.state.lock().unwrap().changes, 1);
    assert!(!result.complete);
    assert_eq!(result.failure, Some(Failure::NativeFailure));
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].outcome, w::CredentialOutcome::Unknown);
    assert!(pending(&root.0, &original.task).unwrap().is_some());
    let mut recovery = input(
        &root,
        Op::VerifyLocalAdministrator {
            sid: "S-1-5-21-1-2-3-1001".into(),
        },
    );
    recovery.task.recovery = Some(w::CredentialRecovery {
        task_id: original.task.task_id,
        attempt_id: original.task.attempt_id,
    });
    recovery.task.definition_digest = w::CredentialTaskSpec::digest(
        &recovery.task.operation,
        &recovery.task.recipient,
        &recovery.task.expected,
        &recovery.task.recovery,
    )
    .unwrap();
    let a = ack(&recovery);
    let denied = engine::result(&recovery, &mut n).unwrap();
    a.join().unwrap();
    assert_eq!(denied.items[0].outcome, w::CredentialOutcome::Unknown);
    assert_eq!(n.state.lock().unwrap().changes, 1);
    n.logon_denied = false;
    n.conflict = true; // Ownership changes after the mutation cannot become a NoEffect proof.
    let conflict = engine::result(&recovery, &mut n).unwrap();
    assert_eq!(conflict.items[0].outcome, w::CredentialOutcome::Unknown);
    assert_eq!(n.state.lock().unwrap().changes, 1);
    n.conflict = false;
    let recovered = engine::result(&recovery, &mut n).unwrap();
    assert!(recovered.complete);
    assert_eq!(
        recovered.recovery,
        Some(w::CredentialRecoveryOutcome::Applied)
    );
    assert_eq!(recovered.items[0].outcome, w::CredentialOutcome::Verified);
    assert_eq!(n.state.lock().unwrap().changes, 1);
}

impl device_access::Protector for Fake {
    fn seal(&self, bytes: &[u8]) -> Result<Vec<u8>, device_access::Error> {
        self.protect(bytes)
            .map_err(|_| device_access::Error::Storage)
    }
    fn open(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, device_access::Error> {
        self.unprotect(bytes)
            .map_err(|_| device_access::Error::Storage)
    }
}
fn factor_code(secret: &[u8], utc: i64) -> String {
    use ring::hmac;
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret),
        &((utc / 30) as u64).to_be_bytes(),
    );
    let b = tag.as_ref();
    let offset = (b[19] & 15) as usize;
    format!(
        "{:06}",
        (u32::from_be_bytes(b[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff) % 1_000_000
    )
}
fn rss_ack(input: &Input, receipt: w::CredentialReceipt) -> std::thread::JoinHandle<()> {
    let root = input.root.clone();
    let task = input.task.clone();
    std::thread::spawn(move || {
        for _ in 0..500 {
            if let Some(value) = pending(&root, &task).unwrap() {
                assert_eq!(value.materials[0].subject, receipt.subject);
                acknowledge(&root, &task, &value, &[receipt]).unwrap();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("RSS escrow missing")
    })
}
#[test]
fn rss_rotation_changes_only_recovery_after_ack_and_verification_reuses_material() {
    use device_access::{Authority, Candidate, Clock, Decision, Factor, Store};
    let root = Root::new();
    let offline = root.0.join("offline");
    platform_private_storage::directory(&offline).unwrap();
    let installation = uuid::Uuid::new_v4();
    let mut store = Store::create(&offline, installation).unwrap();
    let native = Fake::new();
    let binding = uuid::Uuid::new_v4();
    let sid = "S-1-5-21-1-2-3-1001".to_owned();
    let mut task = input(
        &root,
        Op::RotateRssLogin {
            binding,
            binding_generation: 1,
            sid: sid.clone(),
        },
    );
    let subject = Subject::RssLogin {
        binding,
        binding_generation: 1,
        sid: sid.clone(),
    };
    let authority = Authority {
        instance: uuid::Uuid::new_v4(),
        principal: uuid::Uuid::new_v4(),
        join: "local-machine".into(),
        source: rss_mdm_agent_wire::runtime::AccessWindowsAuthority::Local,
        login: "LOCAL\\test".into(),
        tenant: task.task.tenant_id,
        device: task.task.device_id.clone(),
        registration: task.task.registration_id,
        registration_generation: 1,
        binding,
        binding_generation: 1,
        factor_generation: 1,
        sid: sid.clone(),
    };
    let clock = Clock {
        utc: (crate::wall().unwrap() / 1000) as i64,
        monotonic: 1000,
        boot: uuid::Uuid::new_v4(),
    };
    let mut candidate = Candidate::generate().unwrap();
    let totp = candidate.totp_secret().to_vec();
    let old = candidate.recovery_secret().to_owned();
    assert!(candidate
        .confirm(&factor_code(&totp, clock.utc), clock)
        .unwrap());
    let setup = uuid::Uuid::new_v4();
    store.stage(setup, &candidate, &native).unwrap();
    let mut receipt = w::CredentialReceipt {
        subject: subject.clone(),
        material: uuid::Uuid::new_v4(),
        version: 1,
    };
    store.bind_registration(authority.registration()).unwrap();
    store
        .activate(
            authority.clone(),
            1,
            true,
            setup,
            w::AccessWindowsFacts {
                factor_generation: 1,
                recovery: receipt.clone(),
            },
            clock,
            &native,
        )
        .unwrap();
    task.task.expected = vec![w::CredentialVersion {
        subject: subject.clone(),
        revision: 1,
    }];
    task.task.definition_digest = w::CredentialTaskSpec::digest(
        &task.task.operation,
        &task.task.recipient,
        &task.task.expected,
        &None,
    )
    .unwrap();
    receipt.version = 2;
    let ack = rss_ack(&task, receipt.clone());
    let result = super::rss::execute(&task, &native, &mut store).unwrap();
    ack.join().unwrap();
    assert!(result.complete);
    assert_eq!(result.items[0].outcome, w::CredentialOutcome::Verified);
    let (new, actual) = store
        .recovery_material(&task.task, &subject, &native)
        .unwrap();
    assert_eq!(actual, receipt);
    assert_ne!(&*new, &old);
    assert_eq!(
        store
            .verify(&authority, Factor::Recovery(&old), clock, &native)
            .unwrap(),
        Decision::Denied
    );
    for _ in 0..2 {
        assert_eq!(
            store
                .verify(&authority, Factor::Recovery(&new), clock, &native)
                .unwrap(),
            Decision::Accepted
        );
    }
    assert_eq!(
        store
            .verify(
                &authority,
                Factor::Totp(&factor_code(&totp, clock.utc)),
                clock,
                &native
            )
            .unwrap(),
        Decision::Denied
    );
    let next = Clock {
        utc: clock.utc + 30,
        monotonic: 1030,
        ..clock
    };
    assert_eq!(
        store
            .verify(
                &authority,
                Factor::Totp(&factor_code(&totp, next.utc)),
                next,
                &native
            )
            .unwrap(),
        Decision::Accepted
    );
    let mut verify = input(
        &root,
        Op::VerifyRssLogin {
            binding,
            binding_generation: 1,
            sid,
        },
    );
    verify.task.expected = vec![w::CredentialVersion {
        subject,
        revision: 2,
    }];
    verify.task.definition_digest = w::CredentialTaskSpec::digest(
        &verify.task.operation,
        &verify.task.recipient,
        &verify.task.expected,
        &None,
    )
    .unwrap();
    let ack = rss_ack(&verify, receipt.clone());
    assert!(
        super::rss::execute(&verify, &native, &mut store)
            .unwrap()
            .complete
    );
    ack.join().unwrap();

    // Crash after the SQLite commit, before a verified result: terminal Unknown is ACKed.
    let mut original: state::Record =
        state::read_optional(&root.0, &state::name(&task.task, "state"))
            .unwrap()
            .unwrap();
    original.phase = state::Phase::Issued;
    state::write(&root.0, &state::name(&task.task, "state"), &original).unwrap();
    let unknown = failure_with_materials(&root.0, &task.task, Failure::RecoveryState).unwrap();
    state::write(&root.0, &state::name(&task.task, "result"), &unknown).unwrap();
    retire(&root.0, &task.task).unwrap();
    assert!(state::read_optional::<Vec<w::CredentialReceipt>>(
        &root.0,
        &state::name(&task.task, "receipts")
    )
    .unwrap()
    .is_some());
    verify.task.task_id = uuid::Uuid::new_v4();
    verify.task.attempt_id = uuid::Uuid::new_v4();
    verify.task.recovery = Some(w::CredentialRecovery {
        task_id: task.task.task_id,
        attempt_id: task.task.attempt_id,
    });
    verify.task.definition_digest = w::CredentialTaskSpec::digest(
        &verify.task.operation,
        &verify.task.recipient,
        &verify.task.expected,
        &verify.task.recovery,
    )
    .unwrap();
    let ack = rss_ack(&verify, receipt.clone());
    let recovered = super::rss::execute(&verify, &native, &mut store).unwrap();
    ack.join().unwrap();
    assert!(recovered.complete);
    assert_eq!(
        recovered.recovery,
        Some(w::CredentialRecoveryOutcome::Applied)
    );
    state::write(&root.0, &state::name(&verify.task, "result"), &recovered).unwrap();
    retire(&root.0, &verify.task).unwrap();
    assert!(state::read_optional::<Vec<w::CredentialReceipt>>(
        &root.0,
        &state::name(&task.task, "receipts")
    )
    .unwrap()
    .is_none());
    assert_eq!(
        store
            .recovery_material(&task.task, &receipt.subject, &native)
            .unwrap()
            .1,
        receipt
    );
    assert_eq!(
        native.state.lock().unwrap().changes,
        0,
        "no Windows password mutation"
    );
}
