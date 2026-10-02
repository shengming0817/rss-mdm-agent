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
    #[test]
    fn real_keychain_keeps_credentials_immutable_and_namespaces_separate() {
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
        let store = SecretStore {
            root: root.clone(),
            namespace: "a".repeat(64),
            keychain: keychain.clone(),
            access: Mutex::new(()),
        };
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
            root,
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
    /// Import an enrollment secret once. Conflicting existing values never get overwritten.
    pub fn import(&self, reference: &str, secret: &[u8]) -> io::Result<()> {
        let _guard = self.access.lock().map_err(|_| denied())?;
        let account = self.account(reference)?;
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
