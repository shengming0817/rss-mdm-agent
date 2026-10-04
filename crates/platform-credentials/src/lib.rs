//! OS credential primitives. Product names, references, formats and key sizes belong to callers.
use std::io;
mod device_key;
pub use device_key::{DeviceKey, SignatureScheme};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{protect_key, unprotect_key};
/// Operating-system keychain scope, selected explicitly by the composition root.
#[derive(Clone, Copy)]
pub enum Scope {
    User,
    System,
}
#[cfg(target_os = "macos")]
#[derive(Clone)]
pub struct Keychain(security_framework::os::macos::keychain::SecKeychain);
#[cfg(target_os = "macos")]
impl Keychain {
    /// Create an isolated native keychain for explicit acceptance; the caller owns its path/lifetime.
    pub fn create_file(path: &std::path::Path, password: &str) -> io::Result<Self> {
        let mut keychain = security_framework::os::macos::keychain::CreateOptions::new()
            .password(password)
            .prompt_user(false)
            .create(path)
            .map_err(io::Error::other)?;
        keychain.unlock(Some(password)).map_err(io::Error::other)?;
        Ok(Self(keychain))
    }
    pub fn disable_interaction() -> io::Result<NonInteractiveGuard> {
        use security_framework::os::macos::keychain::SecKeychain;
        let mut state = INTERACTION
            .lock()
            .map_err(|_| io::Error::other("keychain interaction unavailable"))?;
        if state.guards == 0 && SecKeychain::user_interaction_allowed().map_err(io::Error::other)? {
            state.disabled =
                Some(SecKeychain::disable_user_interaction().map_err(io::Error::other)?);
        }
        state.guards += 1;
        Ok(NonInteractiveGuard { _private: () })
    }

    pub fn open(scope: Scope) -> io::Result<Self> {
        use security_framework::os::macos::keychain::{SecKeychain, SecPreferencesDomain};
        let root = unsafe { libc::geteuid() } == 0;
        if matches!(scope, Scope::System) != root {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        SecKeychain::default_for_domain(match scope {
            Scope::User => SecPreferencesDomain::User,
            Scope::System => SecPreferencesDomain::System,
        })
        .map(Self)
        .map_err(io::Error::other)
    }
    /// Open a caller-owned isolated keychain, used by native acceptance without touching personal secrets.
    pub fn open_file(path: &std::path::Path) -> io::Result<Self> {
        security_framework::os::macos::keychain::SecKeychain::open(path)
            .map(Self)
            .map_err(io::Error::other)
    }
    pub fn read(&self, service: &str, account: &str) -> io::Result<Vec<u8>> {
        self.0
            .find_generic_password(service, account)
            .map(|(secret, _)| secret.to_vec())
            .map_err(|e| {
                if e.code() == -25300 {
                    io::ErrorKind::NotFound.into()
                } else {
                    io::Error::other(e)
                }
            })
    }
    /// Add only: a duplicate never replaces an existing credential.
    pub fn create_new(&self, service: &str, account: &str, bytes: &[u8]) -> io::Result<()> {
        self.0
            .add_generic_password(service, account, bytes)
            .map_err(io::Error::other)
    }
    /// Device access must not prompt in the system-service or helper session.
    pub fn without_interaction<T>(
        &self,
        operation: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        let _guard = Self::disable_interaction()?;
        operation()
    }
}
pub fn random_bytes(length: usize) -> io::Result<Vec<u8>> {
    if length == 0 || length > 65536 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    #[cfg(target_os = "macos")]
    {
        let mut bytes = vec![0; length];
        security_framework::random::SecRandom::default()
            .copy_bytes(&mut bytes)
            .map_err(io::Error::other)?;
        Ok(bytes)
    }
    #[cfg(windows)]
    {
        windows::random_bytes(length)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[cfg(target_os = "macos")]
pub struct NonInteractiveGuard {
    _private: (),
}
#[cfg(target_os = "macos")]
struct InteractionState {
    guards: usize,
    disabled: Option<security_framework::os::macos::keychain::KeychainUserInteractionLock>,
}
#[cfg(target_os = "macos")]
static INTERACTION: std::sync::Mutex<InteractionState> = std::sync::Mutex::new(InteractionState {
    guards: 0,
    disabled: None,
});
#[cfg(target_os = "macos")]
impl Drop for NonInteractiveGuard {
    fn drop(&mut self) {
        // The Security switch is process-wide. Restore it only after every overlapping
        // operation exits; nested guards do not retain the mutex during an OS call.
        if let Ok(mut state) = INTERACTION.lock() {
            state.guards -= 1;
            if state.guards == 0 {
                drop(state.disabled.take());
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn read_user_options(
    options: security_framework::passwords::PasswordOptions,
) -> io::Result<Vec<u8>> {
    security_framework::passwords::generic_password(options).map_err(|e| {
        if e.code() == -25300 {
            io::ErrorKind::NotFound.into()
        } else {
            io::Error::other(e)
        }
    })
}
/// Search the current user's keychain list, preserving credentials outside the default keychain.
#[cfg(target_os = "macos")]
pub fn read_user_password(service: &str, account: &str) -> io::Result<Vec<u8>> {
    if unsafe { libc::geteuid() } == 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    read_user_options(
        security_framework::passwords::PasswordOptions::new_generic_password(service, account),
    )
}
/// Add to the current default user keychain only when no searchable item already exists.
#[cfg(target_os = "macos")]
pub fn create_user_password(service: &str, account: &str, bytes: &[u8]) -> io::Result<()> {
    match read_user_password(service, account) {
        Ok(_) => Err(io::ErrorKind::AlreadyExists.into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Keychain::open(Scope::User)?.create_new(service, account, bytes)
        }
        Err(e) => Err(e),
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use core_foundation::{array::CFArray, base::TCFType, string::CFString};
    use security_framework::passwords::PasswordOptions;
    use security_framework_sys::item::kSecMatchSearchList;
    struct Isolated(std::path::PathBuf);
    impl Drop for Isolated {
        fn drop(&mut self) {
            for name in ["old.keychain", "new.keychain"] {
                let _ = std::process::Command::new("/usr/bin/security")
                    .arg("delete-keychain")
                    .arg(self.0.join(name))
                    .output();
            }
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn nonexportable_device_key_reopens_and_signs_without_interaction() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("device-keychain-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let _cleanup = Isolated(root.clone());
        let keychain =
            Keychain::create_file(&root.join("old.keychain"), "isolated-test-password").unwrap();
        assert_eq!(
            keychain
                .open_device_key("machine-key")
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::NotFound
        );
        let key = keychain.create_device_key("machine-key").unwrap();
        let public = key.public_spki().unwrap();
        let signature = key
            .sign(SignatureScheme::Pkcs1Sha256, b"machine proof")
            .unwrap();
        assert_eq!(signature.len(), 256);
        drop(key);
        let reopened = keychain.open_device_key("machine-key").unwrap();
        assert_eq!(public, reopened.public_spki().unwrap());
        assert_eq!(
            signature,
            reopened
                .sign(SignatureScheme::Pkcs1Sha256, b"machine proof")
                .unwrap()
        );
        assert_eq!(
            reopened
                .sign(SignatureScheme::PssSha256, b"machine proof")
                .unwrap()
                .len(),
            256
        );
        assert_eq!(
            keychain
                .create_device_key("machine-key")
                .unwrap()
                .public_spki()
                .unwrap(),
            public
        );
        let executable = root.join("updated-key-test");
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let child = |denied: bool| {
            std::process::Command::new(&executable)
                .args([
                    "--ignored",
                    "--exact",
                    "tests::device_key_child",
                    "--test-threads=1",
                ])
                .env("RSS_TEST_KEYCHAIN", root.join("old.keychain"))
                .env("RSS_TEST_KEY_DENIED", if denied { "yes" } else { "no" })
                .output()
                .unwrap()
        };
        let result = child(false);
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(std::process::Command::new("/usr/bin/codesign")
            .args([
                "--force",
                "--sign",
                "-",
                "--identifier",
                "rss.untrusted-key-test"
            ])
            .arg(&executable)
            .status()
            .unwrap()
            .success());
        let result = child(true);
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    #[test]
    #[ignore = "child process of nonexportable_device_key_reopens_and_signs_without_interaction"]
    fn device_key_child() {
        use security_framework::os::macos::keychain::SecKeychain;
        let allowed = SecKeychain::user_interaction_allowed().unwrap();
        let first = Keychain::disable_interaction().unwrap();
        let second = Keychain::disable_interaction().unwrap();
        drop(first);
        assert!(!SecKeychain::user_interaction_allowed().unwrap());
        drop(second);
        assert_eq!(SecKeychain::user_interaction_allowed().unwrap(), allowed);
        let path = std::path::PathBuf::from(std::env::var_os("RSS_TEST_KEYCHAIN").unwrap());
        let keychain = Keychain::open_file(&path).unwrap();
        let key = keychain.open_device_key("machine-key");
        if std::env::var("RSS_TEST_KEY_DENIED").unwrap() == "yes" {
            assert!(key.is_err());
        } else {
            assert_eq!(
                key.unwrap()
                    .sign(SignatureScheme::PssSha256, b"updated image")
                    .unwrap()
                    .len(),
                256
            );
        }
    }
    #[test]
    #[allow(deprecated)] // Upstream exposes no setter for the test's explicit isolated search list.
    fn a_changed_default_does_not_hide_a_secret_in_the_isolated_search_list() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("user-keychains-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let _cleanup = Isolated(root.clone());
        let _interaction = Keychain::disable_interaction().unwrap();
        let old =
            Keychain::create_file(&root.join("old.keychain"), "isolated-test-password").unwrap();
        let new =
            Keychain::create_file(&root.join("new.keychain"), "isolated-test-password").unwrap();
        old.create_new("isolated-continuity", "master", b"original-key")
            .unwrap();
        assert_eq!(
            new.read("isolated-continuity", "master")
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        for search in [
            [old.0.clone(), new.0.clone()],
            [new.0.clone(), old.0.clone()],
        ] {
            let mut options =
                PasswordOptions::new_generic_password("isolated-continuity", "master");
            // Same SecItem query as production, constrained to two isolated chains rather than personal data.
            options.query.push((
                unsafe { CFString::wrap_under_get_rule(kSecMatchSearchList) },
                CFArray::from_CFTypes(&search).as_CFType(),
            ));
            assert_eq!(read_user_options(options).unwrap(), b"original-key");
        }
        assert!(old
            .create_new("isolated-continuity", "master", b"replacement")
            .is_err());
        assert_eq!(
            old.read("isolated-continuity", "master").unwrap(),
            b"original-key"
        );
    }
}
