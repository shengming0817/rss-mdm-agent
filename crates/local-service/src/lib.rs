//! Private, one-query local service. No AI credentials or execution authority.
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const VERSION: u32 = 1;
const MAX_FRAME: usize = 65536;
const DEADLINE: Duration = Duration::from_secs(5);
#[cfg(target_os = "macos")]
mod macos;
mod policy;
#[cfg(windows)]
mod windows;
pub use policy::{Artifact, Policy};

/// Query the installed service. There is no unauthenticated or fixture fallback.
pub fn query() -> Result<Status, Rejected> {
    let policy = Policy::load()?;
    #[cfg(target_os = "macos")]
    {
        macos::query(&policy)
    }
    #[cfg(windows)]
    {
        windows::query(&policy)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = policy;
        Err(Rejected)
    }
}

/// Enter the OS service lifecycle; only the installed service binary calls this.
pub fn run() -> Result<(), Rejected> {
    let policy = Policy::load()?;
    policy
        .service
        .verify(&std::env::current_exe().map_err(|_| Rejected)?)?;
    #[cfg(target_os = "macos")]
    {
        macos::run(policy)
    }
    #[cfg(windows)]
    {
        windows::run(policy)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Err(Rejected)
    }
}

fn random_challenge() -> Result<String, Rejected> {
    let mut bytes = [0u8; 32];
    #[cfg(unix)]
    // SAFETY: buffer is valid for the stated length; no data is used on error.
    if unsafe { libc::getentropy(bytes.as_mut_ptr().cast(), bytes.len()) } != 0 {
        return Err(Rejected);
    }
    #[cfg(windows)]
    // SAFETY: system RNG receives a writable fixed-size byte array.
    if unsafe {
        windows_sys::Win32::Security::Cryptography::BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            32,
            windows_sys::Win32::Security::Cryptography::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } != 0
    {
        return Err(Rejected);
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Debug, thiserror::Error)]
#[error("local service unavailable or unauthorized")]
pub struct Rejected;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    StatusOnly,
}

/// A service statement, never an execution authorization or device receipt.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub version: u32,
    pub installation: String,
    pub platform: String,
    pub build: String,
    pub capability: Capability,
}

/// Native-only projection. Connected is not an authorization to execute work.
#[derive(Serialize, schemars::JsonSchema)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum ServiceView {
    NotInstalled,
    Rejected,
    Unavailable,
    Connected { status: Status },
}
pub fn inspect() -> ServiceView {
    let Ok(path) = policy::policy_path() else {
        return ServiceView::Unavailable;
    };
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ServiceView::NotInstalled
        }
        Err(_) => return ServiceView::Rejected,
        Ok(_) => (),
    }
    if Policy::load().is_err() {
        return ServiceView::Rejected;
    }
    match query() {
        Ok(status) => ServiceView::Connected { status },
        Err(_) => ServiceView::Unavailable,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Method {
    GetServiceStatus,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u32,
    method: Method,
    challenge: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Greeting {
    version: u32,
    challenge: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    version: u32,
    challenge: String,
    status: Status,
}

/// Created only by an authenticated platform connection. Never exported as a
/// bearer token or restored from disk. Even malformed attempts consume it.
struct Connection {
    challenge: String,
    deadline: Instant,
    consumed: bool,
    status: Status,
}
impl Connection {
    fn new(challenge: String, now: Instant, status: Status) -> Self {
        Self {
            challenge,
            deadline: now + DEADLINE,
            consumed: false,
            status,
        }
    }
    fn greeting(&self) -> Result<Vec<u8>, Rejected> {
        serde_json::to_vec(&Greeting {
            version: VERSION,
            challenge: self.challenge.clone(),
        })
        .map_err(|_| Rejected)
    }
    fn accept(&mut self, bytes: &[u8], now: Instant) -> Result<Vec<u8>, Rejected> {
        let consumed = std::mem::replace(&mut self.consumed, true);
        if consumed || now >= self.deadline || bytes.len() > MAX_FRAME {
            return Err(Rejected);
        }
        let request: Request = serde_json::from_slice(bytes).map_err(|_| Rejected)?;
        if request.version != VERSION || request.challenge != self.challenge {
            return Err(Rejected);
        }
        serde_json::to_vec(&Reply {
            version: VERSION,
            challenge: self.challenge.clone(),
            status: self.status.clone(),
        })
        .map_err(|_| Rejected)
    }
}

fn client_request(bytes: &[u8]) -> Result<(String, Vec<u8>), Rejected> {
    if bytes.len() > MAX_FRAME {
        return Err(Rejected);
    }
    let greeting: Greeting = serde_json::from_slice(bytes).map_err(|_| Rejected)?;
    if greeting.version != VERSION
        || greeting.challenge.len() != 64
        || !greeting.challenge.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Rejected);
    }
    let request = serde_json::to_vec(&Request {
        version: VERSION,
        method: Method::GetServiceStatus,
        challenge: greeting.challenge.clone(),
    })
    .map_err(|_| Rejected)?;
    Ok((greeting.challenge, request))
}

fn client_reply(bytes: &[u8], challenge: &str) -> Result<Status, Rejected> {
    if bytes.len() > MAX_FRAME {
        return Err(Rejected);
    }
    let reply: Reply = serde_json::from_slice(bytes).map_err(|_| Rejected)?;
    if reply.version != VERSION || reply.status.version != VERSION || reply.challenge != challenge {
        return Err(Rejected);
    }
    Ok(reply.status)
}

#[cfg(test)]
mod tests;
