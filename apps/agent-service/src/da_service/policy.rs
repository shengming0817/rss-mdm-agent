//! The module's active local configuration and the native source snapshot have different
//! owners. A successful file publication cannot establish that a login extension is active.
use agent_client::{
    da::{ManagedSnapshot, ManagedSource, PolicyConsumer},
    Error,
};
use rss_mdm_agent_wire::{
    da::{Health, SourceState},
    runtime::AccessConfiguration,
};
use std::path::PathBuf;

pub(crate) struct Machine {
    root: PathBuf,
    source: Option<Vec<ManagedSource>>,
    state: SourceState,
}
impl Machine {
    pub(crate) async fn observe(root: PathBuf) -> Self {
        let sources = snapshot().await;
        let state = match &sources {
            Ok(v) if v.is_empty() => SourceState::Missing,
            Ok(_) => SourceState::Present,
            Err(Error::Untrusted | Error::Protocol) => SourceState::Invalid,
            Err(_) => SourceState::ReadFailed,
        };
        Self {
            root,
            source: sources.ok(),
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
        settings: Option<&AccessConfiguration>,
    ) -> Result<Option<AccessConfiguration>, Error> {
        use platform_private_storage as fs;
        let active = self.root.join("effective-configuration.json");
        let bytes = serde_json::to_vec(&settings).map_err(|_| Error::Protocol)?;
        if fs::read(&active, 65536).ok().as_deref() != Some(&bytes) {
            let staged = self.root.join(format!("{}.pending", uuid::Uuid::new_v4()));
            fs::write_new(&staged, &bytes)?;
            if let Err(e) = fs::replace(&staged, &active) {
                let _ = std::fs::remove_file(staged);
                return Err(e.into());
            }
        }
        let installed: Option<AccessConfiguration> =
            serde_json::from_slice(&fs::read(&active, 65536)?).map_err(|_| Error::Storage)?;
        // Only the installed client policy has been committed here. If the selected configuration
        // requires a native mechanism, its independent readback is required for an Applied report.
        if let Some(config) = installed.as_ref().filter(|c| c.enabled) {
            #[cfg(target_os = "macos")]
            if config.mac.is_some()
                && !self.source.as_ref().is_some_and(|s| {
                    s.iter().any(|s| {
                        s.envelope
                            .decode()
                            .is_ok_and(|m| m.content.mac == config.mac)
                    })
                })
            {
                return Ok(None);
            }
            #[cfg(windows)]
            if config.windows != rss_mdm_agent_wire::runtime::AccessWindowsConfiguration::Disabled {
                return Ok(None);
            }
        }
        Ok(installed)
    }
    fn health(&self) -> (Health, Health) {
        (Health::Unknown, Health::Unknown)
    }
}
#[cfg(target_os = "macos")]
async fn snapshot() -> Result<Vec<ManagedSource>, Error> {
    let bytes =
        crate::mdm_state::profiles(&["show", "-type", "configuration", "-output", "stdout-xml"])
            .await
            .ok_or(Error::Unavailable)?;
    let doc = plist::Value::from_reader_xml(bytes.as_slice()).map_err(|_| Error::Protocol)?;
    super::macos::snapshot(&doc)
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
