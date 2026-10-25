//! Registration is current-user state. The installer never acts as a Vault authority.
use super::*;
use passkey_crypto::coset::cbor::value::Value;
use windows_sys::Win32::{
    Storage::Packaging::Appx::GetCurrentPackageFamilyName, UI::Shell::ShellExecuteW,
};
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Owner {
    family: String,
    executable: std::path::PathBuf,
}
pub fn require_identity() -> Result<String> {
    let mut length = 0;
    unsafe {
        GetCurrentPackageFamilyName(&mut length, std::ptr::null_mut());
    }
    if !(2..=256).contains(&length) {
        return Err(Error::Owner);
    }
    let mut value = vec![0u16; length as usize];
    if unsafe { GetCurrentPackageFamilyName(&mut length, value.as_mut_ptr()) } != 0 {
        return Err(Error::Owner);
    }
    let family = String::from_utf16(&value[..length as usize - 1]).map_err(|_| Error::Owner)?;
    if !family.starts_with(&format!("{}_", crate::COMPONENT)) {
        return Err(Error::Owner);
    }
    Ok(family)
}
fn current() -> Result<Owner> {
    Ok(Owner {
        family: require_identity()?,
        executable: std::env::current_exe()
            .and_then(|p| p.canonicalize())
            .map_err(|_| Error::Owner)?,
    })
}
fn directory() -> Result<(platform_private_storage::PrivateDirectory, File)> {
    let root = passkey_vault::platform::default_root()?;
    let directory =
        platform_private_storage::PrivateDirectory::create(&root).map_err(|_| Error::Storage)?;
    let lock = match directory.create_file(Path::new("provider.lock")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => directory
            .open_file(Path::new("provider.lock"))
            .map_err(|_| Error::Owner)?,
        Err(_) => return Err(Error::Storage),
    };
    lock.try_lock().map_err(|_| Error::Busy)?;
    Ok((directory, lock))
}
fn owner(directory: &platform_private_storage::PrivateDirectory) -> Result<Option<Owner>> {
    match directory.read(Path::new("provider-owner.json"), 4096) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| Error::Corrupt),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::Storage),
    }
}
fn publish(directory: &platform_private_storage::PrivateDirectory, value: &Owner) -> Result<()> {
    use std::io::Write;
    let name = format!("provider-owner-{}.stage", uuid::Uuid::new_v4());
    let mut file = directory
        .create_file(Path::new(&name))
        .map_err(|_| Error::Storage)?;
    file.write_all(&serde_json::to_vec(value).map_err(|_| Error::Storage)?)
        .map_err(|_| Error::Storage)?;
    file.sync_all().map_err(|_| Error::Storage)?;
    drop(file);
    directory
        .replace(Path::new(&name), Path::new("provider-owner.json"))
        .map_err(|_| Error::Storage)
}
pub(crate) fn absent(hr: i32) -> bool {
    matches!(hr as u32, 0x80090011 | 0x80070490 | 0x80070002)
}
pub(crate) fn state() -> Result<Option<bool>> {
    let mut value = 0;
    let hr = unsafe { (api::Api::get()?.state)(&api::ID, &mut value) };
    if absent(hr) {
        Ok(None)
    } else {
        api::hr(hr)?;
        if value > 1 {
            return Err(Error::Protocol);
        }
        Ok(Some(value == 1))
    }
}
fn info() -> Result<Vec<u8>> {
    let map = Value::Map(vec![
        (1.into(), Value::Array(vec![Value::Text("FIDO_2_0".into())])),
        (3.into(), Value::Bytes(vec![0; 16])),
        (
            4.into(),
            Value::Map(vec![
                ("rk".into(), Value::Bool(true)),
                ("up".into(), Value::Bool(true)),
                ("uv".into(), Value::Bool(true)),
            ]),
        ),
        (5.into(), 131072.into()),
        (
            10.into(),
            Value::Array(vec![Value::Map(vec![
                ("type".into(), "public-key".into()),
                ("alg".into(), (-7).into()),
            ])]),
        ),
    ]);
    crate::protocol::encode(&map)
}
pub fn register() -> Result<()> {
    identity::platform()?;
    let identity = current()?;
    let (directory, _lock) = directory()?;
    let previous = owner(&directory)?;
    if previous.as_ref().is_some_and(|p| p != &identity) {
        return Err(Error::Owner);
    }
    let existing = state()?;
    if existing.is_some() && previous.is_none() {
        return Err(Error::Owner);
    }
    publish(&directory, &identity)?; // Retain ownership through crashes and ambiguous OS results.
    let api = api::Api::get()?;
    let info = info()?;
    if existing.is_some() {
        return update(api, &info);
    }
    let name = wide("RSS Personal Passkeys");
    let rp = wide("passkeys.rss.local");
    let options = api::Add {
        name: name.as_ptr(),
        clsid: &api::ID,
        rp: rp.as_ptr(),
        light: std::ptr::null(),
        dark: std::ptr::null(),
        info_len: info.len() as u32,
        info: info.as_ptr(),
        rp_count: 0,
        rps: std::ptr::null(),
    };
    let mut response = std::ptr::null_mut();
    let result = unsafe { (api.add)(&options, &mut response) };
    if !response.is_null() {
        unsafe {
            (api.free_add)(response);
        }
    }
    api::hr(result)?;
    if state()?.is_none() {
        return Err(Error::SystemUnavailable);
    }
    Ok(())
}
fn update(api: &api::Api, info: &[u8]) -> Result<()> {
    let name = wide("RSS Personal Passkeys");
    let options = api::Update {
        name: name.as_ptr(),
        clsid: &api::ID,
        new_clsid: &api::ID,
        light: std::ptr::null(),
        dark: std::ptr::null(),
        info_len: info.len() as u32,
        info: info.as_ptr(),
        rp_count: 0,
        rps: std::ptr::null(),
    };
    api::hr(unsafe { (api.update)(&options) })
}
pub fn repair() -> Result<()> {
    register()
}
pub fn unregister() -> Result<()> {
    let identity = current()?;
    let (directory, _lock) = directory()?;
    match owner(&directory)? {
        Some(value) if value == identity => {}
        None if state()?.is_none() => return Ok(()),
        _ => return Err(Error::Owner),
    }
    let api = api::Api::get()?;
    if state()?.is_some() {
        api::hr(unsafe { (api.clear)(&api::ID) })?;
        api::hr(unsafe { (api.remove)(&api::ID) })?;
    }
    if state()?.is_some() {
        return Err(Error::Conflict);
    }
    // Only the exact owner receipt is removed. Never touch vault, keys, or backups.
    std::fs::remove_file(passkey_vault::platform::default_root()?.join("provider-owner.json"))
        .map_err(|_| Error::Storage)
}
pub fn open_settings() -> Result<()> {
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide("ms-settings:passkeys").as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err(Error::SystemUnavailable)
    } else {
        Ok(())
    }
}
pub(crate) fn sync_cache(entries: &[passkey_vault::Entry]) -> Result<()> {
    let api = api::Api::get()?;
    // Full replacement is bounded by the existing Vault limit and always derives from committed state.
    api::hr(unsafe { (api.clear)(&api::ID) })?;
    for chunk in entries.chunks(64) {
        let strings: Vec<_> = chunk
            .iter()
            .map(|e| (wide(&e.rp_id), wide(&e.username), wide(&e.display_name)))
            .collect();
        let details: Vec<_> = chunk
            .iter()
            .zip(&strings)
            .map(|(e, (rp, name, display))| api::Credential {
                id_len: e.credential_id.len() as u32,
                id: e.credential_id.as_ptr(),
                rp: rp.as_ptr(),
                rp_name: rp.as_ptr(),
                user_len: e.user_handle.len() as u32,
                user: e.user_handle.as_ptr(),
                name: name.as_ptr(),
                display: display.as_ptr(),
            })
            .collect();
        api::hr(unsafe { (api.cache)(&api::ID, details.len() as u32, details.as_ptr()) })?;
    }
    Ok(())
}

/// Request a graceful stop; bounded caller wait never kills an operation or rewrites data.
pub fn stop() -> Result<()> {
    let owner = passkey_vault::platform::owner()?;
    let event = unsafe {
        OpenEventW(
            EVENT_MODIFY_STATE,
            0,
            wide(&format!("Local\\RSS.PersonalPasskeys.Stop.{owner}")).as_ptr(),
        )
    };
    if !event.is_null() {
        let event = Handle(event);
        if unsafe { SetEvent(event.0) } == 0 {
            return Err(Error::SystemUnavailable);
        }
    }
    let start = std::time::Instant::now();
    loop {
        match directory() {
            Ok(_) => return Ok(()),
            Err(Error::Busy) if start.elapsed() < std::time::Duration::from_secs(10) => {
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
            Err(e) => return Err(e),
        }
    }
}

pub(crate) fn verify_owner() -> Result<()> {
    let directory =
        platform_private_storage::PrivateDirectory::open(&passkey_vault::platform::default_root()?)
            .map_err(|_| Error::Owner)?;
    if owner(&directory)?.as_ref() != Some(&current()?) {
        return Err(Error::Owner);
    }
    Ok(())
}
pub(crate) fn clear_dirty() -> Result<()> {
    let root = passkey_vault::platform::default_root()?;
    let directory =
        platform_private_storage::PrivateDirectory::open(&root).map_err(|_| Error::Storage)?;
    match directory.read(Path::new("provider-index-dirty"), 16) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(Error::Storage),
        Ok(_) => {}
    }
    let lock = directory
        .open_file(Path::new("vault.lock"))
        .map_err(|_| Error::Storage)?;
    lock.try_lock().map_err(|_| Error::Busy)?;
    match directory.read(Path::new("provider-index-dirty"), 16) {
        Ok(_) => {
            api::hr(unsafe { (api::Api::get()?.clear)(&api::ID) })?;
            std::fs::remove_file(root.join("provider-index-dirty")).map_err(|_| Error::Storage)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Error::Storage),
    }
}
/// Wake the registered current-user COM host after a desktop mutation. No credential data crosses COM.
pub fn invalidate_index() -> Result<()> {
    super::com::refresh_index()
}

pub fn inspect() -> Result<Status> {
    identity::platform()?;
    require_identity()?;
    let state = state()?;
    if state.is_some() {
        verify_owner()?;
    }
    Ok(Status {
        registered: state.is_some(),
        enabled: state == Some(true),
        locked: true,
    })
}
