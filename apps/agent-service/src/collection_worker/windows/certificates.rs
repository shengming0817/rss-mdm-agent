//! Read-only certificate metadata; key handles and encoded certificate bytes never leave this module.
//! ref: microsoft/windows-rs 0.61.3 Win32/Security/Cryptography.
use super::*;
use ::windows::{
    core::{w, PCWSTR},
    Win32::{Foundation::GetLastError, Security::Cryptography::*},
};
struct Store(HCERTSTORE);
impl Drop for Store {
    fn drop(&mut self) {
        unsafe {
            let _ = CertCloseStore(Some(self.0), 0);
        }
    }
}
struct Certificate(*const CERT_CONTEXT);
impl Drop for Certificate {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = CertFreeCertificateContext(Some(self.0));
            }
        }
    }
}
fn name(context: *const CERT_CONTEXT, issuer: bool) -> Result<String, Status> {
    unsafe {
        let flags = if issuer { CERT_NAME_ISSUER_FLAG } else { 0 };
        let count = CertGetNameStringW(context, CERT_NAME_SIMPLE_DISPLAY_TYPE, flags, None, None);
        if count == 0 {
            return Err(Status::Failed);
        }
        if count > 4096 {
            return Err(Status::Truncated);
        }
        let mut value = vec![0u16; count as usize];
        if CertGetNameStringW(
            context,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            flags,
            None,
            Some(&mut value),
        ) != count
        {
            return Err(Status::Failed);
        }
        if value.pop() != Some(0) {
            return Err(Status::Invalid);
        }
        String::from_utf16(&value).map_err(|_| Status::Invalid)
    }
}
fn timestamp(time: ::windows::Win32::Foundation::FILETIME) -> Result<i64, Status> {
    let ticks = ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
    i64::try_from(ticks / 10_000_000)
        .map_err(|_| Status::Invalid)?
        .checked_sub(11_644_473_600)
        .ok_or(Status::Invalid)
}
pub(super) fn read(
    input: &Input,
    query: &w::NativeSystemQuery,
    rows: &mut Rows<'_>,
    budget: &Budget,
) -> Result<(), Status> {
    let w::NativeSystemQuery::WindowsCertificates { store, user } = query else {
        return Err(Status::Invalid);
    };
    let owner = if *user {
        let w::SoftwareExecutionTarget::User { identity, .. } = &input.task.execution.target else {
            return Err(Status::Denied);
        };
        if matches!(identity.as_str(), "S-1-5-18" | "S-1-5-19" | "S-1-5-20") {
            return Err(Status::Denied);
        }
        identity.as_str()
    } else {
        ""
    };
    let store_name: PCWSTR = match store {
        w::WindowsCertificateStore::My => w!("MY"),
        w::WindowsCertificateStore::Root => w!("ROOT"),
        w::WindowsCertificateStore::Ca => w!("CA"),
        w::WindowsCertificateStore::TrustedPublisher => w!("TrustedPublisher"),
    };
    unsafe {
        let flags = CERT_STORE_READONLY_FLAG
            | CERT_STORE_OPEN_EXISTING_FLAG
            | CERT_OPEN_STORE_FLAGS(if *user {
                CERT_SYSTEM_STORE_CURRENT_USER_ID << 16
            } else {
                CERT_SYSTEM_STORE_LOCAL_MACHINE_ID << 16
            });
        let handle = CertOpenStore(
            CERT_STORE_PROV_SYSTEM_W,
            CERT_QUERY_ENCODING_TYPE(0),
            None,
            flags,
            Some(store_name.0.cast()),
        )
        .map_err(|e| {
            if e.code().0 as u32 == 0x80070005 {
                Status::Denied
            } else {
                Status::StoreUnavailable
            }
        })?;
        let store = Store(handle);
        let mut cert = Certificate(std::ptr::null());
        loop {
            budget.check()?;
            // Enumeration consumes the previous context even on exhaustion.
            let previous = std::mem::replace(&mut cert.0, std::ptr::null());
            cert.0 = CertEnumCertificatesInStore(
                store.0,
                if previous.is_null() {
                    None
                } else {
                    Some(previous)
                },
            );
            if cert.0.is_null() {
                if GetLastError().0 == 0x80092004 {
                    return Ok(());
                }
                return Err(Status::Failed);
            }
            let context = &*cert.0;
            if context.pCertInfo.is_null() || context.pbCertEncoded.is_null() {
                return Err(Status::Invalid);
            }
            if context.cbCertEncoded > 1024 * 1024 {
                return Err(Status::Truncated);
            }
            let id = hex(std::slice::from_raw_parts(
                context.pbCertEncoded,
                context.cbCertEncoded as usize,
            ));
            let info = &*context.pCertInfo;
            rows.push(Row::from([
                ("id".into(), id.into()),
                ("subject".into(), name(cert.0, false)?.into()),
                ("issuer".into(), name(cert.0, true)?.into()),
                ("scope".into(), if *user { "user" } else { "device" }.into()),
                ("owner".into(), owner.into()),
                (
                    "store".into(),
                    query.certificate_store().ok_or(Status::Invalid)?.into(),
                ),
                ("not_before".into(), timestamp(info.NotBefore)?.into()),
                ("not_after".into(), timestamp(info.NotAfter)?.into()),
            ]))?;
        }
    }
}
