//! The module's active local configuration and the native source snapshot have different
//! owners. A successful file publication cannot establish that a login extension is active.
use agent_client::{
    da::{ManagedSnapshot, ManagedSource, PolicyConsumer, Selection},
    Error,
};
use rss_mdm_agent_wire::{
    da::{Health, SourceState},
    runtime::AccessConfiguration,
};
use std::path::PathBuf;

pub(crate) struct Machine {
    #[cfg(windows)]
    pub(super) local_rss: bool,
    root: PathBuf,
    #[cfg(windows)]
    native_root: PathBuf,
    #[cfg(windows)]
    installation: uuid::Uuid,
    source: Option<Vec<ManagedSource>>,
    state: SourceState,
    #[cfg(target_os = "macos")]
    inventory: Option<plist::Value>,
}
impl Machine {
    #[cfg(target_os = "macos")]
    pub(crate) fn matches_platform(&self, target: &Selection) -> Result<bool, Error> {
        super::macos::readback(self.inventory.as_ref().ok_or(Error::Unavailable)?, target)
    }
    pub(crate) async fn observe(
        root: PathBuf,
        #[cfg(windows)] native_root: PathBuf,
        #[cfg(windows)] installation: uuid::Uuid,
    ) -> Self {
        #[cfg(target_os = "macos")]
        let inventory = inventory().await;
        #[cfg(target_os = "macos")]
        let sources = inventory
            .as_ref()
            .map_err(|e| *e)
            .and_then(super::macos::snapshot);
        #[cfg(not(target_os = "macos"))]
        let sources = snapshot().await;
        let state = match &sources {
            Ok(v) if v.is_empty() => SourceState::Missing,
            Ok(_) => SourceState::Present,
            Err(Error::Untrusted | Error::Protocol) => SourceState::Invalid,
            Err(_) => SourceState::ReadFailed,
        };
        Self {
            #[cfg(windows)]
            local_rss: false,
            root,
            #[cfg(windows)]
            native_root,
            #[cfg(windows)]
            installation,
            source: sources.ok(),
            #[cfg(target_os = "macos")]
            inventory: inventory.ok(),
            state,
        }
    }
}
impl PolicyConsumer for Machine {
    fn managed_snapshot(&self) -> ManagedSnapshot {
        ManagedSnapshot {
            state: self.state,
            sources: self
                .source
                .as_ref()
                .map(|s| {
                    s.iter()
                        .map(|source| ManagedSource {
                            envelope: source.envelope.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
    fn apply(
        &mut self,
        settings: Option<&Selection>,
    ) -> Result<Option<AccessConfiguration>, Error> {
        #[cfg(windows)]
        {
            self.local_rss = false;
        }
        use platform_private_storage as fs;
        let active = self.root.join("selected-configuration.json");
        if settings.is_none() && !active.try_exists()? {
            return Ok(None);
        }
        let bytes = serde_json::to_vec(&settings).map_err(|_| Error::Protocol)?;
        if fs::read(&active, 65536).ok().as_deref() != Some(&bytes) {
            let staged = self.root.join(format!("{}.pending", uuid::Uuid::new_v4()));
            fs::write_new(&staged, &bytes)?;
            if let Err(e) = fs::replace(&staged, &active) {
                let _ = std::fs::remove_file(staged);
                return Err(e.into());
            }
        }
        let Some(target) = settings else {
            return Ok(None);
        };
        if !matches!(self.state, SourceState::Missing | SourceState::Present) {
            return Ok(None);
        }
        #[cfg(windows)]
        {
            self.local_rss = target.settings.enabled
                && target
                    .settings
                    .windows
                    .accounts()
                    .contains(&rss_mdm_agent_wire::runtime::AccessWindowsKind::Local);
        }
        #[cfg(target_os = "macos")]
        let confirmed = self.matches_platform(target)?;
        #[cfg(windows)]
        let confirmed = super::windows::readback(&self.native_root, self.installation, target)?;
        #[cfg(not(any(windows, target_os = "macos")))]
        let confirmed = false;
        Ok(confirmed.then(|| target.settings.clone()))
    }
    fn health(&self) -> (Health, Health) {
        (Health::Unknown, Health::Unknown)
    }
}
#[cfg(target_os = "macos")]
async fn inventory() -> Result<plist::Value, Error> {
    let bytes =
        crate::mdm_state::profiles(&["show", "-type", "configuration", "-output", "stdout-xml"])
            .await
            .ok_or(Error::Unavailable)?;
    let doc = plist::Value::from_reader_xml(bytes.as_slice()).map_err(|_| Error::Protocol)?;
    Ok(doc)
}
#[cfg(windows)]
async fn snapshot() -> Result<Vec<ManagedSource>, Error> {
    use windows_sys::Win32::System::Registry::*;
    let key = "Software\\Policies\\RSS\\DeviceAccess\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let value = "SignedConfiguration\0".encode_utf16().collect::<Vec<_>>();
    let mut units = vec![0u16; 131073];
    let mut bytes = (units.len() * 2) as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            units.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    match result {
        2 | 3 => Ok(vec![]),
        0 if bytes >= 2 && bytes.is_multiple_of(2) => {
            let (end, text) = units[..bytes as usize / 2]
                .split_last()
                .ok_or(Error::Protocol)?;
            if *end != 0 || text.contains(&0) {
                return Err(Error::Protocol);
            }
            let json = String::from_utf16(text).map_err(|_| Error::Protocol)?;
            Ok(vec![ManagedSource {
                envelope: serde_json::from_str(&json).map_err(|_| Error::Protocol)?,
            }])
        }
        _ => Err(Error::Unavailable),
    }
}
#[cfg(not(any(target_os = "macos", windows)))]
async fn snapshot() -> Result<Vec<ManagedSource>, Error> {
    Err(Error::Unsupported)
}
