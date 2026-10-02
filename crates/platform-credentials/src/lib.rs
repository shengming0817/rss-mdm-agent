//! OS credential primitives. Product names, references, formats and key sizes belong to callers.
use std::io;
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
        security_framework::os::macos::keychain::SecKeychain::disable_user_interaction()
            .map(|guard| NonInteractiveGuard { _guard: guard })
            .map_err(io::Error::other)
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
        use security_framework::os::macos::keychain::SecKeychain;
        let allowed = SecKeychain::user_interaction_allowed().map_err(io::Error::other)?;
        let _guard = if allowed {
            Some(SecKeychain::disable_user_interaction().map_err(io::Error::other)?)
        } else {
            None
        };
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
    _guard: security_framework::os::macos::keychain::KeychainUserInteractionLock,
}
