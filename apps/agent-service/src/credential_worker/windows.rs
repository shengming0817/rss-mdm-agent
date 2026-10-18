//! Closed Windows calls; no PowerShell, shell output or caller-selected WMI methods.
//! ref: microsoft/windows-rs@0.61.0 Win32/System/Wmi/mod.rs and NetManagement/mod.rs.
use super::*;
use w::{CredentialFailure as F, CredentialOperation as Op, CredentialSubject as Subject};
mod account;
mod bitlocker;
pub(super) struct NativeWindows;
impl NativeWindows {
    pub fn open() -> Result<Self, F> {
        Ok(Self)
    }
}
impl Native for NativeWindows {
    fn collect(&mut self) -> Result<Inventory, F> {
        bitlocker::collect()
    }
    fn check_account(&mut self, sid: &str) -> Result<(), F> {
        account::check(sid).map(|_| ())
    }
    fn apply(&mut self, op: &Op, secret: &str) -> Result<(), F> {
        match op {
            Op::RotateBitlocker { volume, .. } => bitlocker::create(volume, secret),
            Op::RotateLocalAdministrator { sid } => account::reset(sid, secret),
            _ => Err(F::Unsupported),
        }
    }
    fn locate(&mut self, op: &Op, secret: &str) -> Result<Option<Subject>, F> {
        match op {
            Op::RotateBitlocker { volume, .. } | Op::VerifyBitlocker { volume, .. } => {
                let values = bitlocker::collect()?;
                let matches = values
                    .materials
                    .into_iter()
                    .filter(|m| {
                        matches!(&m.subject,Subject::Bitlocker{volume:v,..} if v==volume)
                            && m.secret.as_str() == secret
                    })
                    .collect::<Vec<_>>();
                if matches.len() > 1 {
                    return Err(F::NativeFailure);
                }
                Ok(matches.into_iter().next().map(|m| m.subject))
            }
            Op::RotateLocalAdministrator { sid } | Op::VerifyLocalAdministrator { sid } => {
                Ok(account::verify(sid, secret)?
                    .then(|| Subject::LocalAdministrator { sid: sid.clone() }))
            }
            _ => Err(F::Unsupported),
        }
    }
    fn remove(&mut self, volume: &str, protector: uuid::Uuid) -> Result<(), F> {
        bitlocker::remove(volume, protector)
    }
    fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        platform_credentials::protect_key(bytes).map_err(Into::into)
    }
    fn unprotect(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
        platform_credentials::unprotect_key(bytes)
            .map(Zeroizing::new)
            .map_err(Into::into)
    }
}
fn error(e: ::windows::core::Error) -> F {
    if e.code().0 as u32 == 0x80070005 {
        F::PermissionDenied
    } else {
        F::NativeFailure
    }
}
