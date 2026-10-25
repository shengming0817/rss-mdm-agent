//! Actual offline SQLite and task envelope seam; Fake does not establish Windows evidence.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use device_access::{Clock, Protector, Store};
use execution_ipc::device_access::WindowsComponentState as Component;
use rsa::{pkcs8::EncodePublicKey, rand_core::OsRng, RsaPrivateKey};
use std::sync::LazyLock;
use uuid::Uuid;
use zeroize::Zeroizing;
static KEY: LazyLock<RsaPrivateKey> =
    LazyLock::new(|| RsaPrivateKey::new(&mut OsRng, 2048).unwrap());
const SID: &str = "S-1-5-21-1-2-3-1001";
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Fake {
    clock: Clock,
    hello_facts: Option<w::AccessWhfbFacts>,
    creates: usize,
    enables: usize,
    role: w::AccessRole,
    fail_create: bool,
    fail_dispose: bool,
    installed: bool,
    reboot_required: bool,
    inspect_unavailable: bool,
    registered: Option<(PathBuf, Uuid)>,
}
impl Protector for Fake {
    fn seal(&self, bytes: &[u8]) -> Result<Vec<u8>, device_access::Error> {
        Ok(bytes.to_vec())
    }
    fn open(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, device_access::Error> {
        Ok(Zeroizing::new(bytes.to_vec()))
    }
}
impl engine::Native for Fake {
    fn hello(
        &mut self,
        _: &device_access::hello::HelloRecord,
        _: bool,
    ) -> Result<(w::AccessWhfbFacts, bool), w::AccessFailure> {
        self.hello_facts
            .clone()
            .map(|f| (f, true))
            .ok_or(w::AccessFailure::Unsupported)
    }
    fn disposition(
        &self,
        _: &w::AccessAccount,
        kind: w::AccessAccountDisposition,
    ) -> Result<bool, w::AccessFailure> {
        Ok(kind != w::AccessAccountDisposition::Standard || self.role == w::AccessRole::Standard)
    }
    fn identity(
        &self,
        _: &str,
    ) -> Result<(w::AccessWindowsAuthority, String, String), w::AccessFailure> {
        if let Some(f) = &self.hello_facts {
            return Ok((
                w::AccessWindowsAuthority::Entra {
                    tenant_id: f.tenant_id,
                    object_id: f.object_id,
                },
                "AzureAD\\user".into(),
                "entra-join".into(),
            ));
        }
        Ok((
            w::AccessWindowsAuthority::Local,
            "LOCAL\\test".into(),
            "local-machine".into(),
        ))
    }
    fn dispose(
        &mut self,
        _: &w::AccessAccount,
        disposition: w::AccessAccountDisposition,
    ) -> Result<(), w::AccessFailure> {
        if self.fail_dispose {
            return Err(w::AccessFailure::NativeFailure);
        }
        if disposition == w::AccessAccountDisposition::Standard {
            self.role = w::AccessRole::Standard;
        }
        Ok(())
    }
    fn integration_ready(&self, root: &Path, installation: Uuid) -> Result<Component, Error> {
        if let Some((actual_root, actual_id)) = &self.registered {
            verify_installation(root, installation, actual_root, *actual_id)?;
            Store::open(actual_root, *actual_id).map_err(|_| Error::Storage)?;
        } else if self.installed {
            return Err(Error::Identity);
        }
        Ok(if !self.installed {
            Component::NotInstalled
        } else if self.reboot_required {
            Component::RebootRequired
        } else {
            Component::Ready
        })
    }
    fn clock(&self) -> Result<Clock, Error> {
        Ok(self.clock)
    }
    fn password(&mut self) -> Result<Zeroizing<String>, Error> {
        Ok(Zeroizing::new("local-only-test-password".into()))
    }
    fn create(&mut self, _: &str, _: &str) -> Result<String, w::AccessFailure> {
        self.creates += 1;
        if self.fail_create {
            Err(w::AccessFailure::NativeFailure)
        } else {
            Ok(SID.into())
        }
    }
    fn inspect(&self, _: &str) -> Result<w::AccessRole, w::AccessFailure> {
        if self.inspect_unavailable {
            return Err(w::AccessFailure::NeedsUser);
        }
        Ok(self.role)
    }
    fn role(&mut self, _: &str, role: w::AccessRole) -> Result<(), w::AccessFailure> {
        self.role = role;
        Ok(())
    }
    fn enable(&mut self, _: &str) -> Result<(), w::AccessFailure> {
        self.enables += 1;
        Ok(())
    }
}
fn otp(secret: &[u8], utc: i64) -> String {
    use ring::hmac;
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret),
        &((utc / 30) as u64).to_be_bytes(),
    );
    let bytes = tag.as_ref();
    let offset = (bytes[19] & 15) as usize;
    format!(
        "{:06}",
        (u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff)
            % 1_000_000
    )
}
fn fixture(create: bool) -> (Root, Input, Fake, Vec<u8>, String) {
    let root = Root(
        std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-access-worker-{}", Uuid::new_v4())),
    );
    platform_private_storage::directory(&root.0).unwrap();
    let now = (crate::wall().unwrap() / 1000) as i64;
    let mut native = Fake {
        hello_facts: None,
        clock: Clock {
            utc: now,
            monotonic: 1000,
            boot: Uuid::new_v4(),
        },
        creates: 0,
        enables: 0,
        role: w::AccessRole::Administrator,
        fail_create: false,
        fail_dispose: false,
        installed: true,
        reboot_required: false,
        inspect_unavailable: false,
        registered: None,
    };
    let configuration = w::AccessConfiguration {
        mac: None,
        windows: rss_mdm_agent_wire::runtime::AccessWindowsConfiguration::RssFactor {
            accounts: vec![w::AccessWindowsKind::Local],
        },
        self_enrollment: true,
        enabled: true,
        role: None,
    };
    let task = w::AccessTaskSpec {
        wire_version: 1,
        tenant_id: Uuid::new_v4(),
        device_id: "device".into(),
        registration_id: Uuid::new_v4(),
        generation: 1,
        platform: w::TaskPlatform::Windows,
        architecture: w::TaskArchitecture::X86_64,
        task_id: Uuid::new_v4(),
        attempt_id: Uuid::new_v4(),
        permit: w::TaskPermit::Start,
        expires_at: now + 30,
        deadline: now + 60,
        binding_id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        principal_id: Uuid::new_v4(),
        binding_generation: 1,
        factor_generation: 1,
        revision: 1,
        definition_digest: [7; 32],
        action: if create {
            w::AccessAction::Create {
                name: "newuser".into(),
                configuration,
            }
        } else {
            w::AccessAction::Apply {
                account: w::AccessAccount::Windows {
                    sid: SID.into(),
                    authority: w::AccessWindowsAuthority::Local,
                },
                configuration,
            }
        },
        windows: Some(w::AccessWindowsPlan::Initialize {
            recipient: w::CredentialRecipient {
                key_id: "a".repeat(64),
                public_key: STANDARD
                    .encode(KEY.to_public_key().to_public_key_der().unwrap().as_bytes()),
            },
            expected_revision: 0,
        }),
        authorization: w::TaskAuthorization::Policy {
            policy_id: Uuid::new_v4(),
            policy_version: Uuid::new_v4(),
        },
    };
    task.validate().unwrap();
    let input = Input {
        task,
        root: root.0.clone(),
        installation: Uuid::new_v4(),
        image_sha256: Digest::new("a".repeat(64)).unwrap(),
        run_as: RunAs::System {
            platform: execution_contract::Platform::Windows,
        },
        session: SessionRequirement::NotRequired {},
    };
    let mut store = Store::create(&root.0, input.installation).unwrap();
    store
        .bind_registration(authority(&input.task, SID, &native).unwrap().registration())
        .unwrap();
    native.registered = Some((input.root.clone(), input.installation));
    let mut candidate = device_access::Candidate::generate().unwrap();
    let secret = candidate.totp_secret().to_vec();
    let recovery = candidate.recovery_secret().to_owned();
    assert!(candidate.confirm(&otp(&secret, now), native.clock).unwrap());
    store
        .stage(input.task.task_id, &candidate, &native)
        .unwrap();
    (root, input, native, secret, recovery)
}
fn ack(input: &Input, expected: String) -> std::thread::JoinHandle<w::CredentialReceipt> {
    let root = input.root.clone();
    let task = input.task.clone();
    std::thread::spawn(move || {
        for _ in 0..500 {
            if let Some(escrow) = pending(&root, &task).unwrap() {
                let prepared = prepared(&root, &task)
                    .unwrap()
                    .expect("Prepared precedes escrow");
                assert_eq!(
                    prepared.account,
                    w::AccessAccount::Windows {
                        sid: SID.into(),
                        authority: w::AccessWindowsAuthority::Local
                    }
                );
                let material = &escrow.materials[0];
                let key = KEY
                    .decrypt(
                        rsa::Oaep::new::<sha2::Sha256>(),
                        &STANDARD.decode(&material.wrapped_key).unwrap(),
                    )
                    .unwrap();
                let aead = ring::aead::LessSafeKey::new(
                    ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, &key).unwrap(),
                );
                let mut bytes = STANDARD.decode(&material.ciphertext).unwrap();
                let plain = aead
                    .open_in_place(
                        ring::aead::Nonce::assume_unique_for_key(material.nonce),
                        ring::aead::Aad::from(task.material_aad(&material.subject, 0).unwrap()),
                        &mut bytes,
                    )
                    .unwrap();
                assert_eq!(plain, expected.as_bytes());
                let receipt = w::CredentialReceipt {
                    subject: material.subject.clone(),
                    material: Uuid::new_v4(),
                    version: 1,
                };
                acknowledge(&root, &task, &escrow, vec![receipt.clone()]).unwrap();
                return receipt;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("escrow not published")
    })
}
#[test]
fn initialize_preserve_and_unbind_use_real_factor_state_and_exact_ack() {
    let (_root, mut input, mut native, secret, recovery) = fixture(false);
    let ack = ack(&input, recovery.clone());
    let result = engine::execute(&input, &mut native).unwrap();
    let receipt = ack.join().unwrap();
    let w::AccessEffect::Applied {
        windows: Some(w::AccessWindowsFacts::RssFactor(facts)),
        role,
        ..
    } = result.effect
    else {
        panic!("not applied")
    };
    assert_eq!(role, w::AccessRole::Administrator);
    assert_eq!(facts.recovery, receipt);
    let mut store = Store::open(&input.root, input.installation).unwrap();
    assert_eq!(
        store
            .verify(
                &authority(&input.task, SID, &native).unwrap(),
                device_access::Factor::Totp(&otp(&secret, native.clock.utc)),
                native.clock,
                &native
            )
            .unwrap(),
        device_access::Decision::Denied
    );
    native.clock.utc += 30;
    native.clock.monotonic += 30;
    let next = otp(&secret, native.clock.utc);
    assert_eq!(
        store
            .verify(
                &authority(&input.task, SID, &native).unwrap(),
                device_access::Factor::Totp(&next),
                native.clock,
                &native
            )
            .unwrap(),
        device_access::Decision::Accepted
    );
    input.task.task_id = Uuid::new_v4();
    input.task.attempt_id = Uuid::new_v4();
    input.task.revision = 2;
    input.task.windows = Some(w::AccessWindowsPlan::Preserve { facts });
    assert!(engine::execute(&input, &mut native)
        .unwrap()
        .effect
        .complete(
            &w::AccessConfiguration {
                mac: None,
                windows: rss_mdm_agent_wire::runtime::AccessWindowsConfiguration::RssFactor {
                    accounts: vec![w::AccessWindowsKind::Local]
                },
                self_enrollment: true,
                enabled: true,
                role: None
            },
            w::TaskPlatform::Windows
        ));
    assert_eq!(
        store
            .verify(
                &authority(&input.task, SID, &native).unwrap(),
                device_access::Factor::Totp(&next),
                native.clock,
                &native
            )
            .unwrap(),
        device_access::Decision::Denied
    );
    input.task.task_id = Uuid::new_v4();
    input.task.attempt_id = Uuid::new_v4();
    input.task.binding_generation = 2;
    input.task.revision = 3;
    input.task.windows = None;
    input.task.action = w::AccessAction::Unbind {
        disposition: w::AccessAccountDisposition::Keep,
        account: w::AccessAccount::Windows {
            sid: SID.into(),
            authority: w::AccessWindowsAuthority::Local,
        },
    };
    assert_eq!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unbound {
            disposition: w::AccessAccountDisposition::Keep
        }
    );
    assert_eq!(
        store
            .verify(
                &authority(&input.task, SID, &native).unwrap(),
                device_access::Factor::Recovery(&recovery),
                native.clock,
                &native
            )
            .unwrap(),
        device_access::Decision::Disabled
    );
    assert_eq!(native.role, w::AccessRole::Administrator);
    assert_eq!(native.creates, 0);
}
#[test]
fn creation_activates_before_enable_and_replay_does_not_recreate_account() {
    let (_root, input, mut native, _, recovery) = fixture(true);
    let ack = ack(&input, recovery);
    assert!(engine::execute(&input, &mut native)
        .unwrap()
        .effect
        .complete(
            &w::AccessConfiguration {
                mac: None,
                windows: rss_mdm_agent_wire::runtime::AccessWindowsConfiguration::RssFactor {
                    accounts: vec![w::AccessWindowsKind::Local]
                },
                self_enrollment: true,
                enabled: true,
                role: None
            },
            w::TaskPlatform::Windows
        ));
    ack.join().unwrap();
    assert_eq!(native.role, w::AccessRole::Standard);
    assert_eq!(native.creates, 1);
    assert_eq!(native.enables, 1);
    assert!(engine::execute(&input, &mut native)
        .unwrap()
        .effect
        .complete(
            &w::AccessConfiguration {
                mac: None,
                windows: rss_mdm_agent_wire::runtime::AccessWindowsConfiguration::RssFactor {
                    accounts: vec![w::AccessWindowsKind::Local]
                },
                self_enrollment: true,
                enabled: true,
                role: None
            },
            w::TaskPlatform::Windows
        ));
    assert_eq!(native.creates, 1);
    assert_eq!(native.enables, 1);
}
#[test]
fn uncertain_creation_never_adopts_an_existing_name_on_retry() {
    let (_root, input, mut native, _, _) = fixture(true);
    native.fail_create = true;
    assert_eq!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unknown
    );
    native.fail_create = false;
    assert_eq!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unknown
    );
    assert_eq!(native.creates, 1);
    assert_eq!(native.enables, 0);
}
#[test]
fn missing_provider_does_not_change_accounts_or_claim_applied() {
    let (_root, input, mut native, _, _) = fixture(true);
    native.installed = false;
    assert_eq!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Failed {
            reason: w::AccessFailure::Unsupported
        }
    );
    assert_eq!(native.creates, 0);
    assert_eq!(native.enables, 0);
}

#[test]
fn different_valid_provider_store_rejects_enable_disable_and_unbind_before_mutation() {
    let (_root, mut input, mut native, _, recovery) = fixture(false);
    let ack = ack(&input, recovery);
    let initial = engine::execute(&input, &mut native).unwrap();
    ack.join().unwrap();
    let w::AccessEffect::Applied {
        windows: Some(w::AccessWindowsFacts::RssFactor(facts)),
        ..
    } = initial.effect
    else {
        panic!("not applied")
    };
    let (_other_root, other, _, _, _) = fixture(false);
    native.registered = Some((other.root.clone(), other.installation));
    for action in [
        w::AccessAction::Apply {
            account: w::AccessAccount::Windows {
                sid: SID.into(),
                authority: w::AccessWindowsAuthority::Local,
            },
            configuration: w::AccessConfiguration {
                mac: None,
                windows: rss_mdm_agent_wire::runtime::AccessWindowsConfiguration::RssFactor {
                    accounts: vec![w::AccessWindowsKind::Local],
                },
                enabled: true,
                self_enrollment: true,
                role: Some(w::AccessRole::Standard),
            },
        },
        w::AccessAction::Apply {
            account: w::AccessAccount::Windows {
                sid: SID.into(),
                authority: w::AccessWindowsAuthority::Local,
            },
            configuration: w::AccessConfiguration {
                mac: None,
                windows: rss_mdm_agent_wire::runtime::AccessWindowsConfiguration::RssFactor {
                    accounts: vec![w::AccessWindowsKind::Local],
                },
                enabled: false,
                self_enrollment: true,
                role: None,
            },
        },
        w::AccessAction::Unbind {
            disposition: w::AccessAccountDisposition::Keep,
            account: w::AccessAccount::Windows {
                sid: SID.into(),
                authority: w::AccessWindowsAuthority::Local,
            },
        },
    ] {
        input.task.task_id = Uuid::new_v4();
        input.task.attempt_id = Uuid::new_v4();
        input.task.revision += 1;
        input.task.windows = if matches!(
            action,
            w::AccessAction::Unbind {
                disposition: w::AccessAccountDisposition::Keep,
                ..
            }
        ) {
            None
        } else {
            Some(w::AccessWindowsPlan::Preserve {
                facts: facts.clone(),
            })
        };
        input.task.action = action;
        assert_eq!(
            engine::execute(&input, &mut native).unwrap().effect,
            w::AccessEffect::Failed {
                reason: w::AccessFailure::AccountChanged
            }
        );
        assert!(Store::open(&input.root, input.installation)
            .unwrap()
            .enabled(SID)
            .unwrap());
        assert!(!Store::open(&other.root, other.installation)
            .unwrap()
            .enabled(SID)
            .unwrap());
        assert!(prepared(&input.root, &input.task).unwrap().is_none());
        assert_eq!(native.role, w::AccessRole::Administrator);
    }
    assert!(verify_installation(
        &input.root,
        input.installation,
        &input.root,
        other.installation
    )
    .is_err());
    assert!(verify_installation(
        &input.root,
        input.installation,
        &other.root,
        input.installation
    )
    .is_err());
}
#[test]
fn absent_provider_allows_disable_and_unbind_of_the_current_store() {
    let (_root, mut input, mut native, _, recovery) = fixture(false);
    let ack = ack(&input, recovery);
    let initial = engine::execute(&input, &mut native).unwrap();
    ack.join().unwrap();
    let w::AccessEffect::Applied {
        windows: Some(w::AccessWindowsFacts::RssFactor(facts)),
        ..
    } = initial.effect
    else {
        panic!("not applied")
    };
    native.installed = false;
    native.registered = None;
    input.task.task_id = Uuid::new_v4();
    input.task.attempt_id = Uuid::new_v4();
    input.task.revision += 1;
    input.task.windows = Some(w::AccessWindowsPlan::Preserve { facts });
    if let w::AccessAction::Apply { configuration, .. } = &mut input.task.action {
        configuration.enabled = false;
    }
    assert!(matches!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Applied { .. }
    ));
    assert!(!Store::open(&input.root, input.installation)
        .unwrap()
        .enabled(SID)
        .unwrap());
    input.task.task_id = Uuid::new_v4();
    input.task.attempt_id = Uuid::new_v4();
    input.task.revision += 1;
    input.task.windows = None;
    input.task.action = w::AccessAction::Unbind {
        disposition: w::AccessAccountDisposition::Keep,
        account: w::AccessAccount::Windows {
            sid: SID.into(),
            authority: w::AccessWindowsAuthority::Local,
        },
    };
    assert_eq!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unbound {
            disposition: w::AccessAccountDisposition::Keep
        }
    );
}

#[test]
fn unbind_before_initialize_blocks_late_account_creation() {
    let (_root, input, mut native, _, _) = fixture(true);
    let mut unlink = input.task.clone();
    unlink.task_id = Uuid::new_v4();
    unlink.attempt_id = Uuid::new_v4();
    unlink.revision += 1;
    unlink.windows = None;
    unlink.action = w::AccessAction::Unbind {
        account: w::AccessAccount::Windows {
            sid: SID.into(),
            authority: w::AccessWindowsAuthority::Local,
        },
        disposition: w::AccessAccountDisposition::Keep,
    };
    let removal = Input {
        task: unlink,
        root: input.root.clone(),
        installation: input.installation,
        image_sha256: input.image_sha256.clone(),
        run_as: input.run_as.clone(),
        session: input.session.clone(),
    };
    assert!(matches!(
        engine::execute(&removal, &mut native).unwrap().effect,
        w::AccessEffect::Unbound { .. }
    ));
    assert!(matches!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unknown
    ));
    assert_eq!(native.creates, 0);
    assert!(pending(&input.root, &input.task).unwrap().is_none());
}

#[test]
fn uncertain_os_disposition_cannot_restore_revoked_factors() {
    let (_root, mut input, mut native, _, recovery) = fixture(false);
    let acknowledgement = ack(&input, recovery.clone());
    let _ = engine::execute(&input, &mut native).unwrap();
    acknowledgement.join().unwrap();
    input.task.task_id = Uuid::new_v4();
    input.task.attempt_id = Uuid::new_v4();
    input.task.revision += 1;
    input.task.windows = None;
    input.task.action = w::AccessAction::Unbind {
        account: w::AccessAccount::Windows {
            sid: SID.into(),
            authority: w::AccessWindowsAuthority::Local,
        },
        disposition: w::AccessAccountDisposition::Disable,
    };
    native.fail_dispose = true;
    assert!(matches!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unknown
    ));
    let mut store = Store::open(&input.root, input.installation).unwrap();
    assert_eq!(
        store
            .verify(
                &authority(&input.task, SID, &native).unwrap(),
                device_access::Factor::Recovery(&recovery),
                native.clock,
                &native
            )
            .unwrap(),
        device_access::Decision::Disabled
    );
    native.fail_dispose = false;
    assert!(matches!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unbound {
            disposition: w::AccessAccountDisposition::Disable
        }
    ));
}

#[test]
fn reboot_barrier_retries_the_same_attempt_after_reboot() {
    let (_root, input, mut native, _, recovery) = fixture(false);
    native.reboot_required = true;
    assert_eq!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unknown
    );
    assert!(stored_result(&input.root, &input.task).unwrap().is_none());
    native.reboot_required = false;
    let ack = ack(&input, recovery);
    assert!(matches!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Applied { .. }
    ));
    ack.join().unwrap();
}

#[test]
fn disabling_factors_does_not_require_an_interactive_role_observation() {
    let (_root, mut input, mut native, _, recovery) = fixture(false);
    let ack = ack(&input, recovery.clone());
    let w::AccessEffect::Applied {
        windows: Some(w::AccessWindowsFacts::RssFactor(facts)),
        ..
    } = engine::execute(&input, &mut native).unwrap().effect
    else {
        panic!("not applied")
    };
    ack.join().unwrap();
    input.task.task_id = Uuid::new_v4();
    input.task.attempt_id = Uuid::new_v4();
    input.task.revision += 1;
    input.task.windows = Some(w::AccessWindowsPlan::Preserve { facts });
    let w::AccessAction::Apply { configuration, .. } = &mut input.task.action else {
        unreachable!()
    };
    configuration.enabled = false;
    native.inspect_unavailable = true;
    assert_eq!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Unknown
    );
    let mut store = Store::open(&input.root, input.installation).unwrap();
    assert!(!store.enabled(SID).unwrap());
    assert_eq!(
        store
            .verify(
                &authority(&input.task, SID, &native).unwrap(),
                device_access::Factor::Recovery(&recovery),
                native.clock,
                &native
            )
            .unwrap(),
        device_access::Decision::Disabled
    );
    native.inspect_unavailable = false;
    assert!(matches!(
        engine::execute(&input, &mut native).unwrap().effect,
        w::AccessEffect::Applied { .. }
    ));
}

#[test]
fn health_success_requires_ready_not_merely_an_observed_state() {
    assert!(component_health(Component::Ready).is_ok());
    for state in [
        Component::Unavailable,
        Component::NotInstalled,
        Component::MaintenanceDisabled,
        Component::RebootRequired,
    ] {
        assert_eq!(component_health(state), Err(Error::Unavailable));
    }
}

#[test]
fn hello_ignores_cp_and_totp_but_keeps_uncertain_hardware_pending() {
    let (_root, mut input, mut native, _, _) = fixture(false);
    let object = Uuid::from_bytes_le([1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]);
    let source = w::AccessWindowsAuthority::Entra {
        tenant_id: input.task.tenant_id,
        object_id: object,
    };
    input.task.action = w::AccessAction::Apply {
        account: w::AccessAccount::Windows {
            sid: "S-1-12-1-1-2-3-4".into(),
            authority: source,
        },
        configuration: w::AccessConfiguration {
            windows: w::AccessWindowsConfiguration::Whfb,
            mac: None,
            enabled: true,
            self_enrollment: true,
            role: None,
        },
    };
    input.task.windows = Some(w::AccessWindowsPlan::Whfb {
        retire: false,
        previous_key: None,
    });
    native.installed = false;
    native.registered = None;
    native.hello_facts = Some(w::AccessWhfbFacts {
        factor_generation: 1,
        tenant_id: input.task.tenant_id,
        object_id: object,
        policy: w::AccessObservation::Present,
        registration: w::AccessObservation::Present,
        hardware_key: w::AccessObservation::Unknown,
        key_id: Some(Uuid::new_v4()),
        observed_at: native.clock.utc,
    });
    let report = engine::execute(&input, &mut native).unwrap();
    assert!(matches!(
        report.effect,
        w::AccessEffect::Applied {
            windows: Some(w::AccessWindowsFacts::Whfb(_)),
            ..
        }
    ));
    assert!(!complete(&input.task, &report.effect));
    assert!(pending(&input.root, &input.task).unwrap().is_none());
    assert!(prepared(&input.root, &input.task).unwrap().is_none());
    assert_eq!(native.creates, 0);
    assert_eq!(native.enables, 0);
}
