// ref: Microsoft wincrypt CertOpenStore / CertAddEncodedCertificateToStore / CertDeleteCertificateFromStore.
use super::{input, CertificateStore, MachineCertificates};
use std::{io, ptr};
use windows_sys::Win32::{
    Foundation::{GetLastError, CRYPT_E_EXISTS, CRYPT_E_NOT_FOUND},
    Security::Cryptography::*,
};

struct Store(HCERTSTORE);
impl Drop for Store {
    fn drop(&mut self) {
        unsafe {
            CertCloseStore(self.0, 0);
        }
    }
}
struct Certificate(*mut CERT_CONTEXT);
impl Drop for Certificate {
    fn drop(&mut self) {
        unsafe {
            CertFreeCertificateContext(self.0);
        }
    }
}

fn open(class: CertificateStore, write: bool) -> io::Result<Store> {
    let name = match class {
        CertificateStore::Root => "Root",
        CertificateStore::Intermediate => "CA",
    }
    .encode_utf16()
    .chain([0])
    .collect::<Vec<_>>();
    let flags = CERT_SYSTEM_STORE_LOCAL_MACHINE
        | CERT_STORE_OPEN_EXISTING_FLAG
        | if write { 0 } else { CERT_STORE_READONLY_FLAG };
    // SAFETY: fixed machine physical store, live UTF-16 name, no provider handle.
    let store = unsafe {
        CertOpenStore(
            CERT_STORE_PROV_SYSTEM_REGISTRY_W,
            0,
            0,
            flags,
            name.as_ptr().cast(),
        )
    };
    if store.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(Store(store))
}

fn find(store: &Store, der: &[u8]) -> io::Result<Option<Certificate>> {
    let mut previous = ptr::null_mut();
    loop {
        // SAFETY: live store; enumeration consumes the previous context exactly once.
        let current = unsafe { CertEnumCertificatesInStore(store.0, previous) };
        if current.is_null() {
            let code = unsafe { GetLastError() };
            if code == CRYPT_E_NOT_FOUND as u32 {
                return Ok(None);
            }
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        let bytes = unsafe {
            std::slice::from_raw_parts((*current).pbCertEncoded, (*current).cbCertEncoded as usize)
        };
        if bytes == der {
            return Ok(Some(Certificate(current)));
        }
        previous = current;
    }
}

impl MachineCertificates {
    /// Physical exact DER in the selected LocalMachine store, independently of readiness rules.
    pub fn contains(&self, class: CertificateStore, der: &[u8]) -> io::Result<bool> {
        input(der)?;
        Ok(find(&open(class, false)?, der)?.is_some())
    }
    /// Independently read exact DER from LocalMachine Root or CA.
    pub fn read(&self, class: CertificateStore, der: &[u8]) -> io::Result<bool> {
        input(der)?;
        if class == CertificateStore::Intermediate
            && find(&open(CertificateStore::Root, false)?, der)?.is_some()
        {
            return Ok(false);
        }
        Ok(find(&open(class, false)?, der)?.is_some())
    }
    /// Add exact DER without replacing any existing certificate.
    pub fn add(&self, class: CertificateStore, der: &[u8]) -> io::Result<bool> {
        input(der)?;
        if class == CertificateStore::Intermediate
            && find(&open(CertificateStore::Root, false)?, der)?.is_some()
        {
            return Ok(false);
        }
        let store = open(class, true)?;
        if find(&store, der)?.is_some() {
            return Ok(false);
        }
        // SAFETY: bounded DER, live machine store, ADD_NEW prohibits replacement; no returned context requested.
        if unsafe {
            CertAddEncodedCertificateToStore(
                store.0,
                X509_ASN_ENCODING,
                der.as_ptr(),
                der.len() as u32,
                CERT_STORE_ADD_NEW,
                ptr::null_mut(),
            )
        } == 0
        {
            let code = unsafe { GetLastError() };
            if code == CRYPT_E_EXISTS as u32 && find(&store, der)?.is_some() {
                return Ok(false);
            }
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        Ok(true)
    }
    /// Windows certificate class is assigned atomically by the physical store add.
    pub fn configure(&self, class: CertificateStore, der: &[u8]) -> io::Result<()> {
        if self.read(class, der)? {
            Ok(())
        } else {
            Err(io::ErrorKind::InvalidData.into())
        }
    }
    /// Delete only the exact DER selected by durable caller ownership.
    pub fn delete(&self, class: CertificateStore, der: &[u8]) -> io::Result<()> {
        input(der)?;
        let store = open(class, true)?;
        if let Some(certificate) = find(&store, der)? {
            let context = certificate.0;
            std::mem::forget(certificate); // Delete consumes the context even when it fails.
                                           // SAFETY: exact context from this machine physical store, transferred once.
            if unsafe { CertDeleteCertificateFromStore(context) } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}
