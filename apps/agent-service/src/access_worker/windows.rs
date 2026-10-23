use super::*;
use device_access::{windows::account, Protector, SystemProtector};
use zeroize::Zeroizing;
pub(super) struct NativeWindows;
impl Protector for NativeWindows {
    fn seal(&self, bytes: &[u8]) -> Result<Vec<u8>, device_access::Error> {
        SystemProtector.seal(bytes)
    }
    fn open(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, device_access::Error> {
        SystemProtector.open(bytes)
    }
}
impl engine::Native for NativeWindows {
    fn integration_ready(&self, root: &Path, installation: uuid::Uuid) -> Result<bool, Error> {
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
        account::inspect(sid).map(|v| v.role)
    }
    fn role(&mut self, sid: &str, role: w::AccessRole) -> Result<(), w::AccessFailure> {
        account::set_role(sid, role)
    }
    fn enable(&mut self, sid: &str) -> Result<(), w::AccessFailure> {
        account::enable(sid)
    }
}
