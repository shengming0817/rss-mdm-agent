pub(crate) mod hello;
use super::*;
use device_access::{windows::account, Protector, SystemProtector};
use execution_ipc::device_access::WindowsComponentState as Component;
use zeroize::Zeroizing;
pub(super) struct NativeWindows {
    pub root: std::path::PathBuf,
    pub installation: uuid::Uuid,
}
impl Protector for NativeWindows {
    fn seal(&self, bytes: &[u8]) -> Result<Vec<u8>, device_access::Error> {
        SystemProtector.seal(bytes)
    }
    fn open(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, device_access::Error> {
        SystemProtector.open(bytes)
    }
}
impl engine::Native for NativeWindows {
    fn hello_prepare(
        &self,
        record: &device_access::hello::HelloRecord,
    ) -> Result<Option<bool>, w::AccessFailure> {
        hello::prepare(self, record)
    }
    fn hello(
        &mut self,
        record: &device_access::hello::HelloRecord,
        mutate: bool,
    ) -> Result<(w::AccessWhfbFacts, bool), w::AccessFailure> {
        hello::observe(self, record, mutate)
    }
    fn disposition(
        &self,
        target: &w::AccessAccount,
        disposition: w::AccessAccountDisposition,
    ) -> Result<bool, w::AccessFailure> {
        account::disposition_matches(target, disposition)
    }
    fn identity(
        &self,
        sid: &str,
    ) -> Result<(w::AccessWindowsAuthority, String, String), w::AccessFailure> {
        let mut store = device_access::Store::open(&self.root, self.installation)
            .map_err(|_| w::AccessFailure::NativeFailure)?;
        if let Some(value) = store
            .stored_authority(sid)
            .map_err(|_| w::AccessFailure::NativeFailure)?
        {
            if account::verify_bound(&value).is_ok() {
                return Ok((value.source, value.login, value.join));
            }
        }
        let value = account::inspect(sid)?;
        if let Some(logon) = value.logon_millis {
            store
                .require_join_session(
                    &value.join,
                    logon,
                    crate::wall().map_err(|_| w::AccessFailure::NativeFailure)? as i64,
                )
                .map_err(|_| w::AccessFailure::NeedsUser)?;
        }
        Ok((value.source.clone(), account::qualified(&value), value.join))
    }
    fn dispose(
        &mut self,
        target: &w::AccessAccount,
        disposition: w::AccessAccountDisposition,
    ) -> Result<(), w::AccessFailure> {
        account::dispose(target, disposition)
    }
    fn integration_ready(&self, root: &Path, installation: uuid::Uuid) -> Result<Component, Error> {
        crate::access_component::readiness(root, installation)
    }
    fn clock(&self) -> Result<device_access::Clock, Error> {
        device_access::windows::clock().map_err(|_| Error::Clock)
    }
    fn password(&mut self) -> Result<Zeroizing<String>, Error> {
        use std::io::Read;
        let mut bytes = Zeroizing::new(Vec::new());
        std::io::stdin().take(1025).read_to_end(&mut bytes)?;
        if bytes.len() > 1024 || bytes.is_empty() {
            return Err(Error::Capacity);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| Error::Protocol)?;
        if text.contains('\0') {
            return Err(Error::Protocol);
        }
        Ok(Zeroizing::new(text.to_owned()))
    }
    fn create(&mut self, name: &str, password: &str) -> Result<String, w::AccessFailure> {
        account::create(name, password)
    }
    fn inspect(&self, sid: &str) -> Result<w::AccessRole, w::AccessFailure> {
        account::configured_role(sid)
    }
    fn role(&mut self, sid: &str, role: w::AccessRole) -> Result<(), w::AccessFailure> {
        account::set_role(sid, role)
    }
    fn enable(&mut self, sid: &str) -> Result<(), w::AccessFailure> {
        account::enable(sid)
    }
}
