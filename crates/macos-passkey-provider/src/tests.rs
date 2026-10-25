use super::*;
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
use zeroize::Zeroizing;
#[derive(Default)]
struct Protection(Mutex<HashMap<String, Vec<u8>>>);
impl passkey_vault::SystemProtection for Protection {
    fn verify(&self) -> Result<()> {
        Ok(())
    }
    fn protect(&self, id: &str, key: &[u8]) -> Result<Vec<u8>> {
        self.0.lock().unwrap().insert(id.into(), key.to_vec());
        Ok(vec![])
    }
    fn unprotect(&self, id: &str, _: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .map(Zeroizing::new)
            .ok_or(Error::NotFound)
    }
}
struct Lab {
    root: PathBuf,
    protection: Arc<Protection>,
    vault: Arc<Vault>,
}
impl Lab {
    fn new() -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("rss-macos-passkey-{}", uuid::Uuid::new_v4()));
        let protection = Arc::new(Protection::default());
        let vault = Arc::new(Vault::open(&root, protection.clone()).unwrap());
        vault.create(&vault.begin_interaction()).unwrap();
        Self {
            root,
            protection,
            vault,
        }
    }
    fn session(&self, request: Request) -> Session {
        Session::new(
            request,
            Arc::new(Vault::open(&self.root, self.protection.clone()).unwrap()),
        )
        .unwrap()
    }
    fn register(&self, user: u8) -> Vec<u8> {
        let session = self.session(register(user));
        assert!(session.execute(None).is_err()); // failed unauthenticated requests cannot be reused
        let session = self.session(register(user));
        session.unlock(UnlockMethod::System, None).unwrap();
        let response = session.execute(None).unwrap();
        let attestation: Vec<u8> =
            serde_json::from_value(response["attestationObject"].clone()).unwrap();
        let value: passkey_crypto::coset::cbor::value::Value =
            passkey_crypto::coset::cbor::de::from_reader(attestation.as_slice()).unwrap();
        assert!(value
            .as_map()
            .unwrap()
            .iter()
            .any(|(k, _)| k.as_text() == Some("authData")));
        assert_eq!(session.execute(None).err(), Some(Error::Cancelled));
        serde_json::from_value(response["credentialId"].clone()).unwrap()
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn register(user: u8) -> Request {
    Request::Register {
        rp: "example.com".into(),
        hash: vec![7; 32],
        user_handle: vec![user],
        username: format!("account-{user}"),
        algorithms: vec![-7],
        exclude: vec![],
    }
}
fn assertion(allowed: Vec<Vec<u8>>) -> Request {
    Request::Assert {
        rp: "example.com".into(),
        hash: vec![8; 32],
        allowed,
        identity: None,
    }
}
#[test]
fn register_assert_select_and_exclude_match_the_vault() {
    let lab = Lab::new();
    let first = lab.register(1);
    let second = lab.register(2);
    let session = lab.session(assertion(vec![]));
    session.unlock(UnlockMethod::System, None).unwrap();
    assert_eq!(session.accounts().unwrap().len(), 2);
    let response = session.execute(Some(second.clone())).unwrap();
    assert_eq!(
        serde_json::from_value::<Vec<u8>>(response["userHandle"].clone()).unwrap(),
        vec![2]
    );
    let filtered = lab.session(assertion(vec![first.clone()]));
    filtered.unlock(UnlockMethod::System, None).unwrap();
    assert_eq!(filtered.accounts().unwrap().len(), 1);
    assert_eq!(filtered.execute(Some(second)).err(), Some(Error::NotFound));
    let mut excluded = register(3);
    if let Request::Register { exclude, .. } = &mut excluded {
        exclude.push(first);
    }
    let session = lab.session(excluded);
    session.unlock(UnlockMethod::System, None).unwrap();
    assert_eq!(session.execute(None).err(), Some(Error::Excluded));
}
#[test]
fn wrong_rp_account_stale_identity_cancel_and_external_lock_refuse() {
    let lab = Lab::new();
    let id = lab.register(1);
    let mut wrong = assertion(vec![]);
    if let Request::Assert { rp, .. } = &mut wrong {
        *rp = "wrong.example.com".into();
    }
    let session = lab.session(wrong);
    session.unlock(UnlockMethod::System, None).unwrap();
    assert!(session.accounts().unwrap().is_empty());
    assert_eq!(
        session.execute(Some(id.clone())).err(),
        Some(Error::NotFound)
    );
    let wrong = Request::Assert {
        rp: "example.com".into(),
        hash: vec![8; 32],
        allowed: vec![],
        identity: Some(crate::request::Identity {
            credential_id: id.clone(),
            user_handle: vec![2],
        }),
    };
    let session = lab.session(wrong);
    session.unlock(UnlockMethod::System, None).unwrap();
    assert!(session.accounts().unwrap().is_empty());
    let session = lab.session(assertion(vec![]));
    session.unlock(UnlockMethod::System, None).unwrap();
    session.cancel();
    assert_eq!(
        session.execute(Some(id.clone())).err(),
        Some(Error::Cancelled)
    );
    let session = lab.session(assertion(vec![]));
    session.unlock(UnlockMethod::System, None).unwrap();
    lab.vault.lock().unwrap();
    assert_eq!(session.execute(Some(id.clone())).err(), Some(Error::Locked));
    lab.vault
        .unlock(UnlockMethod::System, None, &lab.vault.begin_interaction())
        .unwrap();
    let session = lab.session(assertion(vec![]));
    session.unlock(UnlockMethod::System, None).unwrap();
    lab.vault
        .delete(
            &id,
            lab.vault.status().unwrap().revision,
            &lab.vault.begin_interaction(),
        )
        .unwrap();
    assert_eq!(session.execute(Some(id)).err(), Some(Error::NotFound));
}
#[test]
fn malformed_or_unsupported_native_requests_are_rejected() {
    assert!(Request::decode(
        br#"{"kind":"assert","rp":"example.com","hash":[],"allowed":[],"identity":null}"#
    )
    .is_err());
    let mut value = register(1);
    if let Request::Register { algorithms, .. } = &mut value {
        *algorithms = vec![-257];
    }
    assert_eq!(value.validate(), Err(Error::Input));
    let mut value = assertion(vec![vec![]]);
    assert!(value.validate().is_err());
    if let Request::Assert { allowed, .. } = &mut value {
        *allowed = vec![vec![1]; 129];
    }
    assert!(value.validate().is_err());
}
