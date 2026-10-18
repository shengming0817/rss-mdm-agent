//! Immutable device credentials protected by the actual OS account and key store.
//! ref: security-framework src/os/macos/passwords.rs (add, never set/update).
#[cfg(target_os = "macos")]
use platform_credentials::{Keychain, Scope};
use platform_private_storage as files;
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};
use zeroize::Zeroizing;

/// One deployment/enrollment namespace. This value never confers device identity.
pub struct SecretStore {
    root: PathBuf,
    namespace: String,
    #[cfg(target_os = "macos")]
    keychain: Keychain,
    access: Mutex<()>,
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    struct Isolated {
        root: PathBuf,
        path: PathBuf,
    }
    impl Drop for Isolated {
        fn drop(&mut self) {
            let _ = std::process::Command::new("/usr/bin/security")
                .args(["delete-keychain", self.path.to_str().unwrap()])
                .output();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_key_completes_real_mtls_and_reopens_original_binding() {
        let _budget = crate::scenario_budget::enter();
        use agent_client::{Client, OpenMode};
        use rss_mdm_agent_wire::runtime::{Capability, FailureCode, ReportBody};
        let server = crate::protocol_test_support::Server::tls().await;
        let communication = crate::protocol_test_support::Root::new();
        let root = crate::protocol_test_support::Root::new();
        let isolated = Isolated {
            root: root.path.clone(),
            path: root.path.join("isolated.keychain"),
        };
        let keychain = Keychain::create_file(&isolated.path, "isolated-test-password").unwrap();
        let provider = || {
            crate::DeviceSecrets(SecretStore {
                root: root.path.clone(),
                namespace: "c".repeat(64),
                keychain: keychain.clone(),
                access: Mutex::new(()),
            })
        };
        let mut client = Client::open(
            &communication.path,
            server.config(),
            OpenMode::Create,
            provider(),
            server.time.clone(),
        )
        .unwrap();
        client
            .request_authorization(vec![
                Capability::InventoryCollectionV1,
                Capability::TaskExecuteV1,
            ])
            .await
            .unwrap();
        let binding = client.registration().unwrap();
        client
            .queue_report(
                "inventory",
                ReportBody::Failed {
                    code: FailureCode::CollectionFailed,
                },
                1,
            )
            .unwrap();
        assert_eq!(client.flush_reports(1).await.unwrap(), 1);
        drop(client);
        let mut client = Client::open(
            &communication.path,
            server.config(),
            OpenMode::Existing,
            provider(),
            server.time.clone(),
        )
        .unwrap();
        assert_eq!(client.registration().unwrap(), binding);
        client
            .queue_report(
                "inventory",
                ReportBody::Failed {
                    code: FailureCode::CollectionFailed,
                },
                1,
            )
            .unwrap();
        assert_eq!(client.flush_reports(1).await.unwrap(), 1);
        drop(client);
    }
    #[test]
    fn real_keychain_keeps_credentials_immutable_and_namespaces_separate() {
        let _budget = crate::scenario_budget::enter();
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "agent-keychain-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        files::directory(&root).unwrap();
        let isolated = Isolated {
            path: root.join("isolated.keychain"),
            root: root.clone(),
        };
        let password = "controlled-keychain-test-password-not-a-device-secret";
        let keychain = Keychain::create_file(&isolated.path, password).unwrap();
        let own_root = root.join("installation-a");
        files::directory(&own_root).unwrap();
        let peer_root = root.join("installation-b");
        files::directory(&peer_root).unwrap();
        let store = SecretStore {
            root: own_root,
            namespace: "a".repeat(64),
            keychain: keychain.clone(),
            access: Mutex::new(()),
        };
        store.initialize_catalog().unwrap();
        store.initialize_catalog().unwrap();
        assert_eq!(
            store.read("credential").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        let first = store.credential("credential").unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(*first, *store.credential("credential").unwrap());
        assert!(store.import("credential", &[42; 32]).is_err());
        assert_eq!(*first, *store.read("credential").unwrap());
        let other = SecretStore {
            root: peer_root,
            namespace: "b".repeat(64),
            keychain,
            access: Mutex::new(()),
        };
        assert_eq!(
            other.read("credential").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_ne!(*first, *other.credential("credential").unwrap());
        assert!(store.read("../credential").is_err());
        store.signing_key("retirement-key", true).unwrap();
        other.signing_key("retirement-key", true).unwrap();
        assert!(store.retire(&[]).unwrap());
        assert!(store.retire(&[]).unwrap());
        assert!(store.signing_key("retirement-key", false).is_err());
        assert_eq!(
            store.read("credential").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(other.signing_key("retirement-key", false).is_ok());
        assert!(other.read("credential").is_ok());
        let old_account = other.account("legacy-rotated-secret").unwrap();
        other
            .keychain
            .create_new("RSS MDM Agent device", &old_account, &[9; 32])
            .unwrap();
        assert!(
            !other.retire(&[("credential".into(), false)]).unwrap(),
            "partial new catalog cannot prove cleanup of an upgraded legacy namespace"
        );
        assert_eq!(
            other.read("legacy-rotated-secret").unwrap().as_slice(),
            &[9; 32]
        );
        assert!(!other.root.join("credential-owner").exists());
    }
}
fn denied() -> io::Error {
    io::Error::other("protected device credential unavailable")
}
impl SecretStore {
    /// Open an existing private root. No public path, default secret or plaintext fallback exists.
    pub fn open(root: &Path, namespace: &str) -> io::Result<Self> {
        files::validate(root)?;
        let canonical = root.canonicalize()?;
        if !root.is_absolute()
            || (cfg!(unix) && canonical != root)
            || !root.is_dir()
            || namespace.len() != 64
            || !namespace.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(denied());
        }
        #[cfg(target_os = "macos")]
        let keychain = Keychain::open(if unsafe { libc::geteuid() } == 0 {
            Scope::System
        } else {
            Scope::User
        })
        .map_err(|_| denied())?;
        Ok(Self {
            root: canonical,
            namespace: namespace.into(),
            #[cfg(target_os = "macos")]
            keychain,
            access: Mutex::new(()),
        })
    }
    pub(crate) fn initialize_catalog(&self) -> io::Result<()> {
        // Only the fresh installation initializer may establish complete ownership.
        // Discovering a legacy namespace never silently manufactures this proof.
        if self.catalog_complete()? {
            return Ok(());
        }
        if std::fs::read_dir(&self.root)?.next().is_some() {
            return Err(denied());
        }
        files::write_new(
            &self.root.join("credential-owner"),
            self.namespace.as_bytes(),
        )
    }
    fn catalog_complete(&self) -> io::Result<bool> {
        match files::read(&self.root.join("credential-owner"), 128) {
            Ok(bytes) if bytes == self.namespace.as_bytes() => Ok(true),
            Ok(_) => Err(denied()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }
    fn track(&self, reference: &str, key: bool) -> io::Result<()> {
        self.account(reference)?;
        let bytes = serde_json::to_vec(&(reference, key)).map_err(io::Error::other)?;
        let path = self
            .root
            .join(format!("{:x}.credential.json", Sha256::digest(&bytes)));
        match files::write_new(&path, &bytes) {
            Ok(()) => Ok(()),
            Err(e)
                if e.kind() == io::ErrorKind::AlreadyExists
                    && files::read(&path, 1024)? == bytes =>
            {
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    pub(crate) fn retire(&self, references: &[(String, bool)]) -> io::Result<bool> {
        let _guard = self.access.lock().map_err(|_| denied())?;
        let complete = self.catalog_complete()?;
        for (name, key) in references {
            self.track(name, *key)?;
        }
        self.track("journal-location", false)?;
        for entry in std::fs::read_dir(&self.root)? {
            let path = entry?.path();
            if !path
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.ends_with(".credential.json"))
            {
                continue;
            }
            let bytes = files::read(&path, 1024)?;
            if path.file_name()
                != Some(std::ffi::OsStr::new(&format!(
                    "{:x}.credential.json",
                    Sha256::digest(&bytes)
                )))
            {
                return Err(denied());
            }
            let (reference, key): (String, bool) =
                serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            let account = self.account(&reference)?;
            #[cfg(target_os = "macos")]
            self.keychain.without_interaction(|| {
                if key {
                    self.keychain.delete_device_key(&account)
                } else {
                    self.keychain.delete("RSS MDM Agent device", &account)
                }
            })?;
            #[cfg(windows)]
            if key {
                platform_credentials::DeviceKey::delete(&account)?;
            } else {
                let secret = self
                    .root
                    .join(format!("{:x}.dpapi", Sha256::digest(account.as_bytes())));
                match std::fs::remove_file(secret) {
                    Ok(()) => (),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                    Err(e) => return Err(e),
                }
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            return Err(io::ErrorKind::Unsupported.into());
        }
        Ok(complete)
    }
    fn account(&self, reference: &str) -> io::Result<String> {
        if reference.is_empty()
            || reference.len() > 128
            || !reference
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(denied());
        }
        Ok(format!("{}-{reference}", self.namespace))
    }
    /// Resolve exactly one protected reference; unknown and denied remain errors.
    pub fn read(&self, reference: &str) -> io::Result<Zeroizing<Vec<u8>>> {
        let _guard = self.access.lock().map_err(|_| denied())?;
        self.read_inner(&self.account(reference)?)
    }
    #[cfg(target_os = "macos")]
    fn read_inner(&self, account: &str) -> io::Result<Zeroizing<Vec<u8>>> {
        self.keychain.without_interaction(|| {
            match self.keychain.read("RSS MDM Agent device", account) {
                Ok(secret) if secret.len() == 32 => Ok(Zeroizing::new(secret)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Err(e),
                _ => Err(denied()),
            }
        })
    }
    #[cfg(windows)]
    fn read_inner(&self, account: &str) -> io::Result<Zeroizing<Vec<u8>>> {
        let path = self
            .root
            .join(format!("{:x}.dpapi", Sha256::digest(account.as_bytes())));
        let encrypted = files::read(&path, 65536)?;
        let bytes = Zeroizing::new(platform_credentials::unprotect_key(&encrypted)?);
        if bytes.len() != account.len() + 1 + 32
            || bytes.get(..account.len()) != Some(account.as_bytes())
            || bytes[account.len()] != 0
        {
            return Err(denied());
        }
        Ok(Zeroizing::new(bytes[account.len() + 1..].to_vec()))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    fn read_inner(&self, _: &str) -> io::Result<Zeroizing<Vec<u8>>> {
        Err(denied())
    }
    fn add(&self, account: &str, secret: &[u8]) -> io::Result<()> {
        if secret.len() != 32 {
            return Err(denied());
        }
        #[cfg(target_os = "macos")]
        {
            self.keychain
                .without_interaction(|| {
                    self.keychain
                        .create_new("RSS MDM Agent device", account, secret)
                })
                .map_err(|_| denied())
        }
        #[cfg(windows)]
        {
            let mut bytes = Zeroizing::new(account.as_bytes().to_vec());
            bytes.push(0);
            bytes.extend_from_slice(secret);
            let encrypted = platform_credentials::protect_key(&bytes)?;
            files::write_new(
                &self
                    .root
                    .join(format!("{:x}.dpapi", Sha256::digest(account.as_bytes()))),
                &encrypted,
            )
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = account;
            Err(denied())
        }
    }
    pub(crate) fn signing_key(
        &self,
        reference: &str,
        create: bool,
    ) -> Result<std::sync::Arc<dyn rustls::sign::SigningKey>, agent_client::Error> {
        let name = self.account(reference)?;
        if create {
            self.track(reference, true)?;
        }
        #[cfg(target_os = "macos")]
        let key = if create {
            self.keychain.create_device_key(&name)?
        } else {
            self.keychain.open_device_key(&name)?
        };
        #[cfg(windows)]
        let key = if create {
            platform_credentials::DeviceKey::create(&name)?
        } else {
            platform_credentials::DeviceKey::open(&name)?
        };
        #[cfg(not(any(target_os = "macos", windows)))]
        return Err(agent_client::Error::Unsupported);
        #[cfg(any(target_os = "macos", windows))]
        {
            let public = key.public_spki()?;
            Ok(std::sync::Arc::new(NativeSigningKey {
                key: std::sync::Arc::new(std::sync::Mutex::new(key)),
                public,
            }))
        }
    }
    pub(crate) fn native_identities(
        &self,
    ) -> Result<Vec<agent_client::NativeIdentity>, agent_client::Error> {
        let accept = |der: &[u8]| agent_client::bootstrap_coordinates(der).is_ok();
        #[cfg(target_os = "macos")]
        let identities = self.keychain.identities_matching(accept)?;
        #[cfg(windows)]
        let identities = platform_credentials::DeviceKey::identities_matching(accept)?;
        #[cfg(not(any(windows, target_os = "macos")))]
        return Ok(Vec::new());
        #[cfg(any(windows, target_os = "macos"))]
        identities
            .into_iter()
            .map(|(chain, key)| {
                let public = key.public_spki()?;
                Ok(agent_client::NativeIdentity {
                    chain,
                    key: std::sync::Arc::new(NativeSigningKey {
                        key: std::sync::Arc::new(std::sync::Mutex::new(key)),
                        public,
                    }),
                })
            })
            .collect()
    }
    /// Import an enrollment secret once. Conflicting existing values never get overwritten.
    pub fn import(&self, reference: &str, secret: &[u8]) -> io::Result<()> {
        let _guard = self.access.lock().map_err(|_| denied())?;
        let account = self.account(reference)?;
        self.track(reference, false)?;
        match self.read_inner(&account) {
            Ok(existing) if existing.as_slice() == secret => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => self.add(&account, secret),
            _ => Err(denied()),
        }
    }
    /// Create once with the system RNG before first registration; retries use the same secret.
    pub fn credential(&self, reference: &str) -> io::Result<Zeroizing<Vec<u8>>> {
        use fs2::FileExt;
        let _guard = self.access.lock().map_err(|_| denied())?;
        let account = self.account(reference)?;
        self.track(reference, false)?;
        let path = self
            .root
            .join(format!("{:x}.lock", Sha256::digest(account.as_bytes())));
        let file = match files::create_new(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => files::open_existing(&path)?,
            Err(e) => return Err(e),
        };
        file.try_lock_exclusive()?;
        match self.read_inner(&account) {
            Ok(value) => Ok(value),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                #[cfg(any(target_os = "macos", windows))]
                let value = Zeroizing::new(platform_credentials::random_bytes(32)?);
                #[cfg(not(any(target_os = "macos", windows)))]
                return Err(denied());
                #[cfg(any(target_os = "macos", windows))]
                {
                    self.add(&account, &value)?;
                    self.read_inner(&account)
                }
            }
            Err(e) => Err(e),
        }
    }
}

struct NativeSigningKey {
    key: std::sync::Arc<std::sync::Mutex<platform_credentials::DeviceKey>>,
    public: Vec<u8>,
}
impl std::fmt::Debug for NativeSigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeSigningKey")
    }
}
impl rustls::sign::SigningKey for NativeSigningKey {
    fn choose_scheme(
        &self,
        offered: &[rustls::SignatureScheme],
    ) -> Option<Box<dyn rustls::sign::Signer>> {
        [
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
        ]
        .into_iter()
        .find(|scheme| offered.contains(scheme))
        .map(|scheme| {
            Box::new(NativeSigner {
                key: self.key.clone(),
                scheme,
            }) as Box<dyn rustls::sign::Signer>
        })
    }
    fn public_key(&self) -> Option<rustls::pki_types::SubjectPublicKeyInfoDer<'_>> {
        Some(self.public.as_slice().into())
    }
    fn algorithm(&self) -> rustls::SignatureAlgorithm {
        rustls::SignatureAlgorithm::RSA
    }
}
struct NativeSigner {
    key: std::sync::Arc<std::sync::Mutex<platform_credentials::DeviceKey>>,
    scheme: rustls::SignatureScheme,
}
impl std::fmt::Debug for NativeSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeSigner")
    }
}
impl rustls::sign::Signer for NativeSigner {
    fn scheme(&self) -> rustls::SignatureScheme {
        self.scheme
    }
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        let scheme = match self.scheme {
            rustls::SignatureScheme::RSA_PSS_SHA256 => {
                platform_credentials::SignatureScheme::PssSha256
            }
            rustls::SignatureScheme::RSA_PKCS1_SHA256 => {
                platform_credentials::SignatureScheme::Pkcs1Sha256
            }
            _ => {
                return Err(rustls::Error::General(
                    "unsupported device signature scheme".into(),
                ))
            }
        };
        self.key
            .lock()
            .map_err(|_| rustls::Error::General("device signing unavailable".into()))?
            .sign(scheme, message)
            .map_err(|_| rustls::Error::General("device signing unavailable".into()))
    }
}
