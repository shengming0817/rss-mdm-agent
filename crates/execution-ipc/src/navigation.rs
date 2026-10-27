//! Navigation transport never submits or confirms a business operation.
use crate::host::{current_session, current_subject};
use execution_app::Error;
use execution_contract::EntryIntent;

/// Open only the installed pinned desktop in the caller's actual interactive session.
pub fn open(uri: &str) -> Result<(), Error> {
    EntryIntent::parse(uri).map_err(|_| Error::InvalidInput)?;
    let subject = current_subject()?;
    if current_session()? == 0 || subject == "0" || subject == "S-1-5-18" {
        return Err(Error::Unsupported);
    }
    #[cfg(target_os = "macos")]
    if !crate::macos_identity::console_user(subject.parse().map_err(|_| Error::Unbound)?) {
        return Err(Error::Unsupported);
    }
    #[derive(serde::Deserialize)]
    struct Pins {
        clients: crate::host::PeerPolicy,
    }
    let path = installation_security::deployment_path().map_err(|_| Error::Configuration)?;
    let bytes = installation_security::read_protected(&path).map_err(|_| Error::Denied)?;
    let pins: Pins = serde_json::from_slice(&bytes).map_err(|_| Error::Configuration)?;
    let desktop = pins
        .clients
        .images
        .iter()
        .find(|i| {
            i.path
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|name| matches!(name, "rss-mdm-desktop" | "rss-mdm-desktop.exe"))
        })
        .ok_or(Error::Unsupported)?;
    desktop.verify(&desktop.path).map_err(|_| Error::Denied)?;
    #[cfg(target_os = "macos")]
    {
        let bundle = desktop
            .path
            .ancestors()
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .ok_or(Error::Configuration)?;
        let status = std::process::Command::new("/usr/bin/open")
            .arg("-a")
            .arg(bundle)
            .arg(uri)
            .status()
            .map_err(|_| Error::Unavailable)?;
        if !status.success() {
            return Err(Error::Unavailable);
        }
    }
    #[cfg(windows)]
    {
        std::process::Command::new(&desktop.path)
            .arg(uri)
            .spawn()
            .map_err(|_| Error::Unavailable)?;
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    return Err(Error::Unsupported);
    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(target_os = "macos")]
#[path = "navigation/macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "navigation/windows.rs"]
mod platform;
#[cfg(any(target_os = "macos", windows))]
pub use platform::NavigationInstance;

/// Stable opaque OS-login-specific name; never selected by an external link.
fn name() -> Result<String, Error> {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(&(current_subject()?, crate::host::current_session_binding()?))
        .map_err(|_| Error::InvalidInput)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
