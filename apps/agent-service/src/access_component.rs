//! Independent repair path; never loads the credential provider DLL.
use agent_client::Error;
use device_access::windows::{Installation, CONFIG_KEY, PROVIDER_CLSID};
use sha2::{Digest, Sha256};
use std::{ffi::OsString, path::Path, ptr::null_mut};
use windows_sys::Win32::System::Registry::*;
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
fn provider() -> String {
    format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Authentication\\Credential Providers\\{PROVIDER_CLSID}")
}
fn class() -> String {
    format!("SOFTWARE\\Classes\\CLSID\\{PROVIDER_CLSID}")
}
fn set(path: &str, name: &str, value: &str) -> Result<(), Error> {
    let mut key = null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            wide(path).as_ptr(),
            0,
            null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_WOW64_64KEY,
            null_mut(),
            &mut key,
            null_mut(),
        )
    };
    if status != 0 {
        return Err(Error::Unavailable);
    }
    let value = wide(value);
    let status = unsafe {
        RegSetValueExW(
            key,
            wide(name).as_ptr(),
            0,
            REG_SZ,
            value.as_ptr().cast(),
            (value.len() * 2) as u32,
        )
    };
    let flushed = unsafe { RegFlushKey(key) };
    unsafe {
        RegCloseKey(key);
    }
    if status != 0 || flushed != 0 {
        return Err(Error::Storage);
    }
    Ok(())
}
fn get(path: &str, name: &str) -> Result<String, Error> {
    let mut bytes = 8192u32;
    let mut buffer = [0u16; 4096];
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(path).as_ptr(),
            wide(name).as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status != 0 || !(2..=8192).contains(&bytes) || bytes % 2 != 0 {
        return Err(Error::Storage);
    }
    let text = &buffer[..bytes as usize / 2];
    if text.last() != Some(&0) || text[..text.len() - 1].contains(&0) {
        return Err(Error::Storage);
    }
    String::from_utf16(&text[..text.len() - 1]).map_err(|_| Error::Storage)
}
fn delete(path: &str) -> Result<(), Error> {
    // The native component is x64. Every key is this component's exact fixed CLSID.
    let status = unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, wide(path).as_ptr()) };
    if ![0, 2, 3].contains(&status) {
        return Err(Error::Storage);
    }
    Ok(())
}
fn image(path: &Path) -> Result<String, Error> {
    if !platform_private_storage::is_canonical_path(path)? {
        return Err(Error::Configuration);
    }
    platform_private_storage::validate_single_link(path)?;
    let mut file = platform_private_storage::open_existing(path)?;
    if file.metadata()?.len() > 64 * 1024 * 1024 {
        return Err(Error::Capacity);
    }
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}
/// SYSTEM command: register <private-root> <installation-uuid> <absolute-dll>, health, disable, cleanup.
pub fn run(args: &[OsString]) -> Result<(), Error> {
    if !cfg!(target_arch = "x86_64") {
        return Err(Error::Unsupported);
    }
    device_access::windows::require_system().map_err(|_| Error::Denied)?;
    let values: Vec<_> = args
        .iter()
        .map(|s| s.to_str().ok_or(Error::Configuration))
        .collect::<Result<_, _>>()?;
    match values.as_slice() {
        ["disable"] => {
            delete(&provider())?;
            println!("RSS provider disabled; system providers preserved");
        }
        ["cleanup"] => {
            delete(&provider())?;
            delete(&class())?;
            delete(CONFIG_KEY)?;
            println!("RSS component registration removed; account and factor storage preserved");
        }
        ["health"] => {
            health()?;
            println!("RSS component paths, image and offline storage verified; DLL not loaded");
        }
        ["register", root, id, dll] => {
            let config = Installation {
                root: Path::new(root).into(),
                id: id.parse().map_err(|_| Error::Configuration)?,
            };
            let hash = image(Path::new(dll))?;
            // Never initialize during repair: service initialization is the only create path.
            device_access::Store::open(&config.root, config.id).map_err(|_| Error::Storage)?;
            delete(&provider())?;
            set(
                CONFIG_KEY,
                "Installation",
                &serde_json::to_string(&config).map_err(|_| Error::Configuration)?,
            )?;
            set(CONFIG_KEY, "ImageSha256", &hash)?;
            let inproc = format!("{}\\InprocServer32", class());
            set(&inproc, "", dll)?;
            set(&inproc, "ThreadingModel", "Apartment")?;
            set(&provider(), "", "RSS Device Access")?;
            if let Err(error) = health() {
                delete(&provider())?;
                return Err(error);
            }
            println!("RSS provider registered; system providers preserved");
        }
        _ => return Err(Error::Configuration),
    }
    Ok(())
}
pub(crate) fn health() -> Result<(), Error> {
    let config = device_access::windows::installation().map_err(|_| Error::Configuration)?;
    device_access::Store::open(&config.root, config.id).map_err(|_| Error::Storage)?;
    let inproc = format!("{}\\InprocServer32", class());
    let path = get(&inproc, "")?;
    if get(&inproc, "ThreadingModel")? != "Apartment"
        || get(&provider(), "")? != "RSS Device Access"
        || image(Path::new(&path))? != get(CONFIG_KEY, "ImageSha256")?
    {
        return Err(Error::Untrusted);
    }
    Ok(())
}

pub(crate) fn remove_for(deployment: &crate::deployment::Deployment) -> Result<(), Error> {
    let mut key = null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            wide(CONFIG_KEY).as_ptr(),
            0,
            KEY_READ | KEY_WOW64_64KEY,
            &mut key,
        )
    };
    if [2, 3].contains(&status) {
        return Ok(());
    }
    if status != 0 {
        return Err(Error::Storage);
    }
    unsafe {
        RegCloseKey(key);
    }
    let config = device_access::windows::installation().map_err(|_| Error::Identity)?;
    if config.id != deployment.service_id
        || config.root != crate::access_worker::root(&deployment.execution)
    {
        return Err(Error::Identity);
    }
    delete(&provider())?;
    delete(&class())?;
    delete(CONFIG_KEY)
}
