//! Only observations from the native owner, bound to this DA device, confirm settings.
use agent_client::{da::Selection, Error};
use rss_mdm_agent_wire::runtime::{AccessObservation, AccessWindowsConfiguration as Windows};
use std::path::Path;

pub(super) fn readback(root: &Path, target: &Selection) -> Result<bool, Error> {
    match &target.settings.windows {
        Windows::Whfb => {
            let dir = root;
            if !dir.join("device-access.sqlite").try_exists()? {
                return Ok(false);
            }
            let store = device_access::Store::open(&dir, target.identity.service_id)
                .map_err(|_| Error::Storage)?;
            let mut seen = false;
            for record in store.hello_records().map_err(|_| Error::Storage)? {
                if record.authority.tenant != target.identity.tenant
                    || record.authority.device != target.identity.device_id
                {
                    continue;
                }
                let facts = crate::access_worker::windows::hello::read_policy(&record)
                    .map_err(|_| Error::Unavailable)?;
                if record.retired
                    || record.enabled != target.settings.enabled
                    || facts
                        != if target.settings.enabled {
                            AccessObservation::Present
                        } else {
                            AccessObservation::Absent
                        }
                {
                    return Ok(false);
                }
                seen = true;
            }
            Ok(seen)
        }
        Windows::RssFactor { accounts } => {
            let Ok(installed) = device_access::windows::installation() else {
                return Ok(false);
            };
            if installed.id != target.identity.service_id || installed.root != root {
                return Ok(false);
            }
            let store = device_access::Store::open(&installed.root, installed.id)
                .map_err(|_| Error::Storage)?;
            store
                .confirms_rss_policy(
                    target.identity.tenant,
                    target.identity.device_id,
                    target.settings.enabled,
                    accounts,
                )
                .map_err(|_| Error::Storage)
        }
        Windows::Disabled => Ok(false),
    }
}
