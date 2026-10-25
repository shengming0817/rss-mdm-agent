use super::*;
use crate::ceremony::{Authorizer, Context, Decision};
use passkey_types::{
    ctap2::{get_assertion as get, make_credential as make},
    webauthn,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use zeroize::Zeroizing;

#[derive(Default)]
struct Keys {
    values: Mutex<HashMap<String, Vec<u8>>>,
    denied: AtomicBool,
}
impl SystemProtection for Keys {
    fn verify(&self) -> Result<()> {
        if self.denied.load(Ordering::Acquire) {
            Err(Error::Authentication)
        } else {
            Ok(())
        }
    }
    fn protect(&self, id: &str, key: &[u8]) -> Result<Vec<u8>> {
        self.values
            .lock()
            .unwrap()
            .entry(id.into())
            .or_insert_with(|| key.to_vec());
        Ok(vec![])
    }
    fn unprotect(&self, id: &str, _: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        self.values
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .map(Zeroizing::new)
            .ok_or(Error::NotFound)
    }
}
struct Lab {
    path: PathBuf,
    keys: Arc<Keys>,
    vault: Vault,
}
impl Lab {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-vault-{}", uuid::Uuid::new_v4()));
        let keys = Arc::new(Keys::default());
        let vault = Vault::open(&path, keys.clone()).unwrap();
        vault.create(&vault.begin_interaction()).unwrap();
        Self { path, keys, vault }
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
struct Consent {
    index: usize,
    verified: bool,
}
impl Authorizer for Consent {
    fn authorize(&self, c: &Context<'_>) -> Result<Decision> {
        Ok(Decision {
            selected: c.accounts.get(self.index).map(|e| e.credential_id.clone()),
            verified: self.verified,
        })
    }
}
fn register(user: u8) -> make::Request {
    make::Request {
        client_data_hash: vec![7; 32].into(),
        rp: make::PublicKeyCredentialRpEntity {
            id: "example.com".into(),
            name: None,
        },
        user: webauthn::PublicKeyCredentialUserEntity {
            id: vec![user].into(),
            name: format!("account-{user}"),
            display_name: format!("User {user}"),
        },
        pub_key_cred_params: vec![webauthn::PublicKeyCredentialParameters {
            ty: webauthn::PublicKeyCredentialType::PublicKey,
            alg: passkey_crypto::iana::Algorithm::ES256,
        }],
        exclude_list: None,
        extensions: None,
        options: make::Options {
            rk: true,
            up: true,
            uv: true,
        },
        pin_auth: None,
        pin_protocol: None,
    }
}
fn assertion() -> get::Request {
    get::Request {
        rp_id: "example.com".into(),
        client_data_hash: vec![8; 32].into(),
        allow_list: None,
        extensions: None,
        options: get::Options { up: true, uv: true },
        pin_auth: None,
        pin_protocol: None,
    }
}

#[tokio::test]
async fn independent_rp_verifies_multi_account_backup_flags_and_signature() {
    let lab = Lab::new();
    let yes = Consent {
        index: 0,
        verified: true,
    };
    let first = lab.vault.register(register(1), &yes).await.unwrap();
    lab.vault.register(register(2), &yes).await.unwrap();
    let bytes = first.auth_data.to_vec();
    assert_eq!(&bytes[..32], Sha256::digest(b"example.com").as_slice());
    assert_eq!(bytes[32], 0x4d); // UP | UV | BE | AT; BS is absent.
    assert_eq!(&bytes[33..37], &[0; 4]);
    let id_len = u16::from_be_bytes([bytes[53], bytes[54]]) as usize;
    let id = bytes[55..55 + id_len].to_vec();
    // Independent CBOR decode and ring verifier, not the production signing backend.
    let cose: passkey_crypto::coset::cbor::value::Value =
        passkey_crypto::coset::cbor::de::from_reader(&bytes[55 + id_len..]).unwrap();
    let map = cose.as_map().unwrap();
    let coordinate = |label: i128| -> Vec<u8> {
        map.iter()
            .find(|(k, _)| k.as_integer().is_some_and(|i| i128::from(i) == label))
            .unwrap()
            .1
            .as_bytes()
            .unwrap()
            .clone()
    };
    let mut public = vec![4];
    public.extend(coordinate(-2));
    public.extend(coordinate(-3));
    let output = lab.vault.assert(assertion(), &yes).await.unwrap();
    assert_eq!(output.credential.as_ref().unwrap().id.to_vec(), id);
    assert_eq!(output.user.unwrap().id.to_vec(), vec![1]);
    let mut signed = output.auth_data.to_vec();
    assert_eq!(signed[32], 0x0d);
    signed.extend([8; 32]);
    let verifier =
        ring::signature::UnparsedPublicKey::new(&ring::signature::ECDSA_P256_SHA256_ASN1, public);
    verifier.verify(&signed, &output.signature).unwrap();
    signed[32] ^= 8;
    assert!(verifier.verify(&signed, &output.signature).is_err());
    let output = lab
        .vault
        .assert(
            assertion(),
            &Consent {
                index: 1,
                verified: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(output.user.unwrap().id.to_vec(), vec![2]);
    assert!(lab
        .vault
        .assert(
            get::Request {
                rp_id: "evil.example".into(),
                ..assertion()
            },
            &yes
        )
        .await
        .is_err());
    assert!(lab
        .vault
        .assert(
            assertion(),
            &Consent {
                index: 0,
                verified: false
            }
        )
        .await
        .is_err());
    let mut excluded = register(9);
    excluded.exclude_list = Some(vec![webauthn::PublicKeyCredentialDescriptor {
        ty: webauthn::PublicKeyCredentialType::PublicKey,
        id: id.into(),
        transports: None,
    }]);
    assert_eq!(
        lab.vault.register(excluded, &yes).await.err(),
        Some(Error::Excluded)
    );
}

#[tokio::test]
async fn password_lifecycle_preserves_credentials_and_recovers_missing_system_key() {
    let lab = Lab::new();
    let yes = Consent {
        index: 0,
        verified: true,
    };
    lab.vault.register(register(1), &yes).await.unwrap();
    let id = lab.vault.status().unwrap().entries[0].credential_id.clone();
    lab.vault
        .set_password(
            UnlockMethod::System,
            None,
            "correct horse battery",
            "correct horse battery",
            &lab.vault.begin_interaction(),
        )
        .unwrap();
    lab.vault.lock();
    assert!(!lab.vault.status().unwrap().unlocked);
    assert!(lab
        .vault
        .unlock(
            UnlockMethod::Password,
            Some("wrong password"),
            &lab.vault.begin_interaction()
        )
        .is_err());
    lab.vault
        .unlock(
            UnlockMethod::Password,
            Some("correct horse battery"),
            &lab.vault.begin_interaction(),
        )
        .unwrap();
    lab.vault
        .set_password(
            UnlockMethod::Password,
            Some("correct horse battery"),
            "new password phrase",
            "new password phrase",
            &lab.vault.begin_interaction(),
        )
        .unwrap();
    lab.vault.lock();
    assert!(lab
        .vault
        .unlock(
            UnlockMethod::Password,
            Some("correct horse battery"),
            &lab.vault.begin_interaction()
        )
        .is_err());
    lab.keys.values.lock().unwrap().clear();
    lab.keys.denied.store(true, Ordering::Release);
    assert!(lab
        .vault
        .unlock(UnlockMethod::System, None, &lab.vault.begin_interaction())
        .is_err());
    lab.vault
        .unlock(
            UnlockMethod::Password,
            Some("new password phrase"),
            &lab.vault.begin_interaction(),
        )
        .unwrap();
    assert_eq!(lab.vault.status().unwrap().entries[0].credential_id, id);
    assert!(lab
        .vault
        .remove_password(&lab.vault.begin_interaction())
        .is_err()); // cannot destroy the only usable method
    lab.vault.assert(assertion(), &yes).await.unwrap();
    lab.keys.denied.store(false, Ordering::Release);
    lab.vault
        .repair_system("new password phrase", &lab.vault.begin_interaction())
        .unwrap();
    lab.vault
        .remove_password(&lab.vault.begin_interaction())
        .unwrap();
    lab.vault.lock();
    assert!(lab
        .vault
        .unlock(
            UnlockMethod::Password,
            Some("new password phrase"),
            &lab.vault.begin_interaction()
        )
        .is_err());
    lab.vault
        .unlock(UnlockMethod::System, None, &lab.vault.begin_interaction())
        .unwrap();
    let disk = std::fs::read(lab.path.join("vault.json")).unwrap();
    for secret in ["new password phrase", "example.com", "account-1", "User 1"] {
        assert!(!String::from_utf8_lossy(&disk).contains(secret));
    }
}

#[test]
fn password_envelope_is_portable_but_native_open_is_owner_bound_and_kdf_bounded() {
    let lab = Lab::new();
    lab.vault
        .set_password(
            UnlockMethod::System,
            None,
            "correct horse battery",
            "correct horse battery",
            &lab.vault.begin_interaction(),
        )
        .unwrap();
    let tx = lab.vault.store.transaction().unwrap();
    let mut envelope = tx.load(&lab.vault.store.owner).unwrap().unwrap();
    let root = envelope
        .password
        .as_ref()
        .unwrap()
        .unwrap(&envelope.aad("password-root"), "correct horse battery")
        .unwrap();
    envelope.local.owner = "different-os-identity".into();
    envelope.local.protected_root.clear();
    // Portable content is readable independently of the device protection, while the
    // production repository still refuses this owner and has no import operation.
    assert!(Vault::content(&envelope, &root).is_ok());
    tx.save(&envelope).unwrap();
    drop(tx);
    assert_eq!(lab.vault.status().err(), Some(Error::Owner));
    let mut value = serde_json::to_value(&envelope).unwrap();
    value["password"]["memory_kib"] = serde_json::json!(u32::MAX);
    let envelope: model::Envelope = serde_json::from_value(value).unwrap();
    assert_eq!(
        envelope
            .password
            .as_ref()
            .unwrap()
            .unwrap(&envelope.aad("password-root"), "correct horse battery")
            .err(),
        Some(Error::Unsupported)
    );
}

#[tokio::test]
async fn independent_instances_reject_stale_deletion_and_preserve_committed_data() {
    let lab = Lab::new();
    let yes = Consent {
        index: 0,
        verified: true,
    };
    lab.vault.register(register(1), &yes).await.unwrap();
    let second = Vault::open(&lab.path, lab.keys.clone()).unwrap();
    second
        .unlock(UnlockMethod::System, None, &second.begin_interaction())
        .unwrap();
    let old = second.status().unwrap();
    lab.vault.register(register(2), &yes).await.unwrap();
    assert_eq!(
        second.delete(
            &old.entries[0].credential_id,
            old.revision,
            &second.begin_interaction()
        ),
        Err(Error::Conflict)
    );
    let latest = second.status().unwrap();
    second
        .delete(
            &latest.entries[0].credential_id,
            latest.revision,
            &second.begin_interaction(),
        )
        .unwrap();
    assert_eq!(lab.vault.status().unwrap().entries.len(), 1);
    let tx = lab.vault.store.transaction().unwrap();
    assert_eq!(second.status().err(), Some(Error::Busy));
    drop(tx);
    // Incomplete staging from a crashed writer is never selected on reopen.
    platform_private_storage::write_new(&lab.path.join("vault.stage"), b"partial ciphertext")
        .unwrap();
    drop(second);
    let reopened = Vault::open(&lab.path, lab.keys.clone()).unwrap();
    reopened
        .unlock(UnlockMethod::System, None, &reopened.begin_interaction())
        .unwrap();
    assert_eq!(reopened.status().unwrap().entries.len(), 1);
}

#[test]
fn corrupt_and_unknown_material_is_preserved_without_automatic_reinitialization() {
    let lab = Lab::new();
    let tx = lab.vault.store.transaction().unwrap();
    tx.write("vault.json", b"corrupt material").unwrap();
    drop(tx);
    assert_eq!(
        lab.vault.create(&lab.vault.begin_interaction()),
        Err(Error::Corrupt)
    );
    assert_eq!(
        std::fs::read(lab.path.join("vault.json")).unwrap(),
        b"corrupt material"
    );
    assert_eq!(
        lab.vault.reset("yes", &lab.vault.begin_interaction()),
        Err(Error::Input)
    );
    lab.vault
        .reset("DELETE LOCAL VAULT", &lab.vault.begin_interaction())
        .unwrap();
    assert!(lab.vault.status().unwrap().entries.is_empty());
    assert!(std::fs::read_dir(&lab.path).unwrap().any(|p| p
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("retired-")));
}

#[test]
fn cancelled_system_authentication_never_installs_a_late_session() {
    struct Blocking {
        keys: Arc<Keys>,
        enter: std::sync::mpsc::SyncSender<()>,
        leave: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl SystemProtection for Blocking {
        fn verify(&self) -> Result<()> {
            self.enter.send(()).unwrap();
            self.leave.lock().unwrap().recv().unwrap();
            Ok(())
        }
        fn protect(&self, id: &str, key: &[u8]) -> Result<Vec<u8>> {
            self.keys.protect(id, key)
        }
        fn unprotect(&self, id: &str, p: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
            self.keys.unprotect(id, p)
        }
    }
    let lab = Lab::new();
    let (enter, entered) = std::sync::mpsc::sync_channel(1);
    let (leave, left) = std::sync::mpsc::sync_channel(1);
    let vault = Arc::new(
        Vault::open(
            &lab.path,
            Arc::new(Blocking {
                keys: lab.keys.clone(),
                enter,
                leave: Mutex::new(left),
            }),
        )
        .unwrap(),
    );
    let other = vault.clone();
    let thread = std::thread::spawn(move || {
        other.unlock(UnlockMethod::System, None, &other.begin_interaction())
    });
    entered.recv().unwrap();
    vault.lock();
    leave.send(()).unwrap();
    assert_eq!(thread.join().unwrap(), Err(Error::Cancelled));
    assert!(!vault.status().unwrap().unlocked);
}

// Explicit plaintext test protector: permits a second test process to consume the
// same synthetic root without touching any real user's Keychain or DPAPI material.
struct ProcessKeys;
impl SystemProtection for ProcessKeys {
    fn verify(&self) -> Result<()> {
        Ok(())
    }
    fn protect(&self, _: &str, key: &[u8]) -> Result<Vec<u8>> {
        Ok(key.to_vec())
    }
    fn unprotect(&self, _: &str, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        if bytes.len() == 32 {
            Ok(Zeroizing::new(bytes.to_vec()))
        } else {
            Err(Error::NotFound)
        }
    }
}
#[test]
#[ignore = "child of process_crash_never_selects_half_a_commit"]
fn crash_writer() {
    let path = PathBuf::from(std::env::var_os("RSS_VAULT_TEST_ROOT").unwrap());
    let vault = Vault::open(&path, Arc::new(ProcessKeys)).unwrap();
    vault
        .unlock(UnlockMethod::System, None, &vault.begin_interaction())
        .unwrap();
    let status = vault.status().unwrap();
    vault
        .delete(
            &status.entries[0].credential_id,
            status.revision,
            &vault.begin_interaction(),
        )
        .unwrap();
    panic!("crash failpoint was not reached");
}
#[tokio::test]
async fn process_crash_never_selects_half_a_commit() {
    for (phase, remaining) in [("staged", 1), ("published", 0)] {
        let lab = Lab::new();
        // Replace only this test's empty initialized Vault with the cross-process fake.
        let vault = Vault::open(&lab.path, Arc::new(ProcessKeys)).unwrap();
        vault
            .reset("DELETE LOCAL VAULT", &vault.begin_interaction())
            .unwrap();
        vault
            .register(
                register(1),
                &Consent {
                    index: 0,
                    verified: true,
                },
            )
            .await
            .unwrap();
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "tests::crash_writer"])
            .env("RSS_VAULT_TEST_ROOT", &lab.path)
            .env("RSS_VAULT_TEST_CRASH", phase)
            .output()
            .unwrap();
        assert_eq!(
            child.status.code(),
            Some(86),
            "child: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        let reopened = Vault::open(&lab.path, Arc::new(ProcessKeys)).unwrap();
        reopened
            .unlock(UnlockMethod::System, None, &reopened.begin_interaction())
            .unwrap();
        assert_eq!(reopened.status().unwrap().entries.len(), remaining);
    }
}
#[test]
fn missing_committed_snapshot_is_not_an_empty_vault_and_old_input_is_cancelled() {
    let lab = Lab::new();
    let token = lab.vault.begin_interaction();
    lab.vault.lock();
    assert_eq!(
        lab.vault.unlock(UnlockMethod::System, None, &token),
        Err(Error::Cancelled)
    );
    std::fs::remove_file(lab.path.join("vault.json")).unwrap();
    assert_eq!(lab.vault.status().err(), Some(Error::Corrupt));
    assert_eq!(
        lab.vault.create(&lab.vault.begin_interaction()),
        Err(Error::Corrupt)
    );
}

struct FileKeys(PathBuf);
impl SystemProtection for FileKeys {
    fn verify(&self) -> Result<()> {
        Ok(())
    }
    fn protect(&self, id: &str, key: &[u8]) -> Result<Vec<u8>> {
        let path = self.0.join(format!("test-key-{id}"));
        match platform_private_storage::read(&path, 32) {
            Ok(current) if current == key => Ok(vec![]),
            Ok(_) => Err(Error::Conflict),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                platform_private_storage::write_new(&path, key).map_err(|_| Error::Storage)?;
                Ok(vec![])
            }
            Err(_) => Err(Error::Storage),
        }
    }
    fn unprotect(&self, id: &str, _: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        platform_private_storage::read(&self.0.join(format!("test-key-{id}")), 32)
            .map(Zeroizing::new)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Error::NotFound
                } else {
                    Error::Storage
                }
            })
    }
}
#[test]
#[ignore = "child of initialization_recovers_exact_key_and_publication"]
fn initialize_writer() {
    let root = PathBuf::from(std::env::var_os("RSS_VAULT_TEST_ROOT").unwrap());
    let vault = Vault::open(&root, Arc::new(FileKeys(root.clone()))).unwrap();
    vault.create(&vault.begin_interaction()).unwrap();
    panic!("initialization crash hook not reached");
}
#[test]
fn initialization_recovers_exact_key_and_publication() {
    for point in [
        "initializing.json:published",
        "system:protected",
        "vault.json:published",
    ] {
        let lab = Lab::new();
        let root = lab.path.join("initialization");
        platform_private_storage::directory(&root).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "tests::initialize_writer"])
            .env("RSS_VAULT_TEST_ROOT", &root)
            .env("RSS_VAULT_TEST_CRASH", point)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(86));
        let pending: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("initializing.json")).unwrap())
                .unwrap();
        let vault = Vault::open(&root, Arc::new(FileKeys(root.clone()))).unwrap();
        if vault.status().unwrap().initialized {
            assert_eq!(
                vault.create(&vault.begin_interaction()),
                Err(Error::Conflict)
            );
            vault
                .unlock(UnlockMethod::System, None, &vault.begin_interaction())
                .unwrap();
        } else {
            vault.create(&vault.begin_interaction()).unwrap();
        }
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("vault.json")).unwrap()).unwrap();
        assert_eq!(saved["vault_id"], pending["vault_id"]);
        assert_eq!(saved["key_id"], pending["key_id"]);
        assert_eq!(
            std::fs::read_dir(&root)
                .unwrap()
                .filter(|e| e
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("test-key-"))
                .count(),
            1
        );
        assert!(vault.status().unwrap().unlocked);
    }
}
#[test]
fn explicit_reset_after_snapshot_loss_preserves_receipt_and_restores_use() {
    let lab = Lab::new();
    let old = std::fs::read(lab.path.join("initializing.json")).unwrap();
    std::fs::remove_file(lab.path.join("vault.json")).unwrap();
    assert_eq!(
        lab.vault.create(&lab.vault.begin_interaction()),
        Err(Error::Corrupt)
    );
    lab.vault
        .reset("DELETE LOCAL VAULT", &lab.vault.begin_interaction())
        .unwrap();
    assert!(lab.vault.status().unwrap().unlocked);
    let archived = std::fs::read_dir(&lab.path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("-receipt.json")
        })
        .unwrap();
    assert_eq!(std::fs::read(archived).unwrap(), old);
}

#[tokio::test]
async fn provider_cancellation_before_publication_cannot_save_or_return_a_signature() {
    struct CancelAtPublication(std::sync::atomic::AtomicUsize);
    impl Authorizer for CancelAtPublication {
        fn authorize(&self, context: &Context<'_>) -> Result<Decision> {
            Ok(Decision {
                selected: context.accounts.first().map(|e| e.credential_id.clone()),
                verified: true,
            })
        }
        fn check(&self) -> Result<()> {
            if self.0.fetch_add(1, Ordering::SeqCst) >= 2 {
                Err(Error::Cancelled)
            } else {
                Ok(())
            }
        }
    }
    let lab = Lab::new();
    let before = lab.vault.status().unwrap().revision;
    assert!(matches!(
        lab.vault
            .register(register(1), &CancelAtPublication(0.into()))
            .await,
        Err(Error::Cancelled)
    ));
    let status = lab.vault.status().unwrap();
    assert_eq!(status.revision, before);
    assert!(status.entries.is_empty());
}

#[tokio::test]
async fn native_request_rejects_invalid_rp_and_changed_credential_selection() {
    let lab = Lab::new();
    let consent = Consent {
        index: 0,
        verified: true,
    };
    for rp in [
        "https://example.com",
        "EXAMPLE.COM",
        "com",
        "bad..example.com",
        "-bad.example.com",
        "example.com/",
    ] {
        let mut request = register(1);
        request.rp.id = rp.into();
        assert!(matches!(
            lab.vault.register(request, &consent).await,
            Err(Error::Input)
        ));
    }
    lab.vault.register(register(1), &consent).await.unwrap();
    struct Unknown;
    impl Authorizer for Unknown {
        fn authorize(&self, _: &Context<'_>) -> Result<Decision> {
            Ok(Decision {
                selected: Some(vec![99; 32]),
                verified: true,
            })
        }
    }
    assert!(matches!(
        lab.vault.assert(assertion(), &Unknown).await,
        Err(Error::NotFound)
    ));
}
