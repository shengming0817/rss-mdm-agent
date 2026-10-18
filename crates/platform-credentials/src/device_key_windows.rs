// ref: windows-sys 0.61.2 Win32/Security/Cryptography; NCryptCreatePersistedKey/SignHash.
use super::{denied, reference, rsa, SignatureScheme};
use sha2::{Digest, Sha256};
use std::{
    io,
    ptr::{null, null_mut},
};
use windows_sys::Win32::Security::{Cryptography::*, *};
/// Persisted, non-exportable RSA machine key; runtime opens only the exact name.
pub struct DeviceKey {
    provider: usize,
    key: usize,
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
fn property(key: usize, name: *const u16, bytes: &[u8], flags: u32) -> io::Result<()> {
    // SAFETY: Valid live key, property name and bounded immutable input buffer.
    if unsafe { NCryptSetProperty(key, name, bytes.as_ptr(), bytes.len() as u32, flags) } != 0 {
        return Err(denied());
    }
    Ok(())
}
impl DeviceKey {
    /// Open a machine key protected for the system service; never create on absence.
    pub fn open(name: &str) -> io::Result<Self> {
        reference(name)?;
        let mut provider = 0;
        // SAFETY: Provider name is a static NUL-terminated Windows constant.
        if unsafe { NCryptOpenStorageProvider(&mut provider, MS_KEY_STORAGE_PROVIDER, 0) } != 0 {
            return Err(denied());
        }
        let mut value = Self { provider, key: 0 };
        // SAFETY: Live provider and bounded NUL-terminated key name with valid out pointer.
        let status = unsafe {
            NCryptOpenKey(
                provider,
                &mut value.key,
                wide(name).as_ptr(),
                0,
                NCRYPT_MACHINE_KEY_FLAG | NCRYPT_SILENT_FLAG,
            )
        };
        if status as u32 == 0x80090016 {
            return Err(io::ErrorKind::NotFound.into());
        }
        if status != 0 {
            return Err(denied());
        }
        value.validate()?;
        Ok(value)
    }
    /// Remove an exact persisted machine key; never enumerate other installations.
    pub fn delete(name: &str) -> io::Result<()> {
        let mut key = match Self::open(name) {
            Ok(key) => key,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        // SAFETY: The exact live key handle is consumed by successful NCryptDeleteKey.
        if unsafe { NCryptDeleteKey(key.key, NCRYPT_SILENT_FLAG) } != 0 {
            return Err(denied());
        }
        key.key = 0;
        match Self::open(name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            _ => Err(denied()),
        }
    }
    /// Create once with export disabled and LocalSystem-only key access.
    pub fn create(name: &str) -> io::Result<Self> {
        match Self::open(name) {
            Ok(key) => return Ok(key),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        let mut provider = 0;
        // SAFETY: Static provider name and valid writable handle.
        if unsafe { NCryptOpenStorageProvider(&mut provider, MS_KEY_STORAGE_PROVIDER, 0) } != 0 {
            return Err(denied());
        }
        let mut value = Self { provider, key: 0 };
        // SAFETY: No overwrite flag; live provider/name/output and machine scope.
        if unsafe {
            NCryptCreatePersistedKey(
                provider,
                &mut value.key,
                BCRYPT_RSA_ALGORITHM,
                wide(name).as_ptr(),
                0,
                NCRYPT_MACHINE_KEY_FLAG | NCRYPT_SILENT_FLAG,
            )
        } != 0
        {
            return Err(denied());
        }
        property(value.key, NCRYPT_LENGTH_PROPERTY, &2048u32.to_le_bytes(), 0)?;
        property(
            value.key,
            NCRYPT_EXPORT_POLICY_PROPERTY,
            &0u32.to_le_bytes(),
            0,
        )?;
        // System service identity is mandatory; machine scope alone does not restrict other users.
        let mut sd = null_mut();
        // SAFETY: Constant valid SDDL, checked result; returned allocation freed after use.
        if unsafe {
            windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW(wide("D:P(A;;GA;;;SY)").as_ptr(), 1, &mut sd, null_mut())
        } == 0
        {
            return Err(denied());
        }
        // SAFETY: Conversion returned a valid self-relative security descriptor.
        let length = unsafe { windows_sys::Win32::Security::GetSecurityDescriptorLength(sd) };
        // SAFETY: Descriptor length is obtained from the OS for this allocation.
        let bytes = unsafe { std::slice::from_raw_parts(sd.cast(), length as usize) };
        let result = property(
            value.key,
            NCRYPT_SECURITY_DESCR_PROPERTY,
            bytes,
            DACL_SECURITY_INFORMATION,
        );
        // SAFETY: Release the LocalAlloc security descriptor exactly once.
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(sd);
        }
        result?;
        // SAFETY: Live configured key, no legacy private-key duplication.
        if unsafe { NCryptFinalizeKey(value.key, NCRYPT_SILENT_FLAG) } != 0 {
            return Err(denied());
        }
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> io::Result<()> {
        let mut export = 0u32;
        let mut size = 0;
        // SAFETY: Live key and correctly sized property output buffer.
        if unsafe {
            NCryptGetProperty(
                self.key,
                NCRYPT_EXPORT_POLICY_PROPERTY,
                (&mut export as *mut u32).cast(),
                4,
                &mut size,
                NCRYPT_SILENT_FLAG,
            )
        } != 0
            || size != 4
            || export != 0
        {
            return Err(denied());
        }
        let mut length = 0u32;
        // SAFETY: Live key and correctly sized integer property output.
        if unsafe {
            NCryptGetProperty(
                self.key,
                NCRYPT_LENGTH_PROPERTY,
                (&mut length as *mut u32).cast(),
                4,
                &mut size,
                NCRYPT_SILENT_FLAG,
            )
        } != 0
            || size != 4
            || length != 2048
        {
            return Err(denied());
        }
        let mut algorithm = [0u16; 16];
        // SAFETY: Bounded algorithm group output; size is checked before comparing.
        if unsafe {
            NCryptGetProperty(
                self.key,
                NCRYPT_ALGORITHM_GROUP_PROPERTY,
                algorithm.as_mut_ptr().cast(),
                32,
                &mut size,
                NCRYPT_SILENT_FLAG,
            )
        } != 0
            || size != 8
            || algorithm[..4] != [82, 83, 65, 0]
        {
            return Err(denied());
        }
        self.check_access()
    }
    fn check_access(&self) -> io::Result<()> {
        let mut descriptor = [0u32; 1024];
        let mut size = 0;
        // SAFETY: Live key, aligned bounded security descriptor output.
        if unsafe {
            NCryptGetProperty(
                self.key,
                NCRYPT_SECURITY_DESCR_PROPERTY,
                descriptor.as_mut_ptr().cast(),
                4096,
                &mut size,
                DACL_SECURITY_INFORMATION | NCRYPT_SILENT_FLAG,
            )
        } != 0
            || size < 20
            || size > 4096
        {
            return Err(denied());
        }
        let sd = descriptor.as_mut_ptr().cast();
        let mut control = 0;
        let mut revision = 0;
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = null_mut();
        // SAFETY: Validated self-relative descriptor; OS validates each queried structure.
        unsafe {
            if IsValidSecurityDescriptor(sd) == 0
                || GetSecurityDescriptorControl(sd, &mut control, &mut revision) == 0
                || control & SE_DACL_PROTECTED == 0
                || GetSecurityDescriptorDacl(sd, &mut present, &mut acl, &mut defaulted) == 0
                || present == 0
                || acl.is_null()
                || IsValidAcl(acl) == 0
                || (*acl).AceCount != 1
            {
                return Err(denied());
            }
            let mut raw = null_mut();
            if GetAce(acl, 0, &mut raw) == 0 || raw.is_null() {
                return Err(denied());
            }
            let ace = &*raw.cast::<ACCESS_ALLOWED_ACE>();
            if ace.Header.AceType != 0 || ace.Header.AceFlags != 0 || ace.Mask != 0x10000000 {
                return Err(denied());
            }
            let mut system = [0u32; 17];
            let mut system_size = 68;
            if CreateWellKnownSid(
                WinLocalSystemSid,
                null_mut(),
                system.as_mut_ptr().cast(),
                &mut system_size,
            ) == 0
                || EqualSid(
                    (&ace.SidStart as *const u32).cast_mut().cast(),
                    system.as_mut_ptr().cast(),
                ) == 0
            {
                return Err(denied());
            }
        }
        Ok(())
    }
    /// Export only the RSA public key encoded as SPKI DER.
    pub fn public_spki(&self) -> io::Result<Vec<u8>> {
        let mut size = 0;
        // SAFETY: Size-only public export; no private blob type is used.
        if unsafe {
            NCryptExportKey(
                self.key,
                0,
                BCRYPT_RSAPUBLIC_BLOB,
                null(),
                null_mut(),
                0,
                &mut size,
                0,
            )
        } != 0
            || size > 4096
        {
            return Err(denied());
        }
        let mut bytes = vec![0; size as usize];
        // SAFETY: Output allocation has exactly the OS-reported bounded capacity.
        if unsafe {
            NCryptExportKey(
                self.key,
                0,
                BCRYPT_RSAPUBLIC_BLOB,
                null(),
                bytes.as_mut_ptr(),
                size,
                &mut size,
                0,
            )
        } != 0
            || size as usize != bytes.len()
            || bytes.len() < 24
        {
            return Err(denied());
        }
        if u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| denied())?)
            != BCRYPT_RSAPUBLIC_MAGIC
            || u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| denied())?) != 2048
            || bytes[16..24] != [0; 8]
        {
            return Err(denied());
        }
        let exponent = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| denied())?) as usize;
        let modulus = u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| denied())?) as usize;
        if bytes.len() != 24 + exponent + modulus || modulus != 256 {
            return Err(denied());
        }
        rsa(&bytes[24 + exponent..], &bytes[24..24 + exponent])
    }
    /// Sign with SHA256 and explicit PKCS1/PSS padding; PSS salt length equals hash length.
    pub fn sign(&self, scheme: SignatureScheme, message: &[u8]) -> io::Result<Vec<u8>> {
        let digest = Sha256::digest(message);
        let pkcs1 = BCRYPT_PKCS1_PADDING_INFO {
            pszAlgId: BCRYPT_SHA256_ALGORITHM,
        };
        let pss = BCRYPT_PSS_PADDING_INFO {
            pszAlgId: BCRYPT_SHA256_ALGORITHM,
            cbSalt: 32,
        };
        let (padding, flag) = match scheme {
            SignatureScheme::Pkcs1Sha256 => (
                (&pkcs1 as *const BCRYPT_PKCS1_PADDING_INFO).cast(),
                NCRYPT_PAD_PKCS1_FLAG,
            ),
            SignatureScheme::PssSha256 => (
                (&pss as *const BCRYPT_PSS_PADDING_INFO).cast(),
                NCRYPT_PAD_PSS_FLAG,
            ),
        };
        let mut signature = vec![0; 256];
        let mut size = 0;
        // SAFETY: Live RSA key, valid padding structure, 32-byte digest and 2048-bit output.
        if unsafe {
            NCryptSignHash(
                self.key,
                padding,
                digest.as_ptr(),
                32,
                signature.as_mut_ptr(),
                256,
                &mut size,
                flag | NCRYPT_SILENT_FLAG,
            )
        } != 0
            || size != 256
        {
            return Err(denied());
        }
        Ok(signature)
    }
}
impl Drop for DeviceKey {
    fn drop(&mut self) {
        // SAFETY: Each owned live handle is released once; persistent key is not deleted.
        unsafe {
            if self.key != 0 {
                NCryptFreeObject(self.key);
            }
            if self.provider != 0 {
                NCryptFreeObject(self.provider);
            }
        }
    }
}

impl DeviceKey {
    /// Open matching LocalMachine/My identities using native silent CNG handles.
    pub fn identities_matching(
        accept: impl Fn(&[u8]) -> bool,
    ) -> io::Result<Vec<(Vec<Vec<u8>>, DeviceKey)>> {
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
                if !self.0.is_null() {
                    unsafe {
                        CertFreeCertificateContext(self.0);
                    }
                }
            }
        }
        struct Chain(*mut CERT_CHAIN_CONTEXT);
        impl Drop for Chain {
            fn drop(&mut self) {
                unsafe {
                    CertFreeCertificateChain(self.0);
                }
            }
        }
        let store = Store(unsafe {
            CertOpenStore(
                CERT_STORE_PROV_SYSTEM_REGISTRY_W,
                0,
                0,
                CERT_SYSTEM_STORE_LOCAL_MACHINE
                    | CERT_STORE_OPEN_EXISTING_FLAG
                    | CERT_STORE_READONLY_FLAG,
                wide("My").as_ptr().cast(),
            )
        });
        if store.0.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut certificate = Certificate(null_mut());
        let mut identities = Vec::new();
        loop {
            certificate.0 = unsafe { CertEnumCertificatesInStore(store.0, certificate.0) };
            if certificate.0.is_null() {
                if unsafe { windows_sys::Win32::Foundation::GetLastError() }
                    != windows_sys::Win32::Foundation::CRYPT_E_NOT_FOUND as u32
                {
                    return Err(io::Error::last_os_error());
                }
                break;
            }
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (*certificate.0).pbCertEncoded,
                    (*certificate.0).cbCertEncoded as usize,
                )
            };
            if !accept(bytes) {
                continue;
            }
            let mut key = 0;
            let mut spec = 0;
            let mut owned = 0;
            if unsafe {
                CryptAcquireCertificatePrivateKey(
                    certificate.0,
                    CRYPT_ACQUIRE_ONLY_NCRYPT_KEY_FLAG
                        | CRYPT_ACQUIRE_SILENT_FLAG
                        | CRYPT_ACQUIRE_COMPARE_KEY_FLAG,
                    null(),
                    &mut key,
                    &mut spec,
                    &mut owned,
                )
            } == 0
            {
                return Err(denied());
            }
            // No CACHE flag: the returned CNG key must be owned by this adapter.
            if spec != CERT_NCRYPT_KEY_SPEC || owned == 0 {
                return Err(denied());
            }
            let key = DeviceKey { provider: 0, key };
            let parameters = CERT_CHAIN_PARA {
                cbSize: std::mem::size_of::<CERT_CHAIN_PARA>() as u32,
                ..Default::default()
            };
            let mut chain = null_mut();
            if unsafe {
                CertGetCertificateChain(
                    null_mut(),
                    certificate.0,
                    null(),
                    store.0,
                    &parameters,
                    CERT_CHAIN_CACHE_ONLY_URL_RETRIEVAL,
                    null(),
                    &mut chain,
                )
            } == 0
            {
                return Err(denied());
            }
            let chain = Chain(chain);
            let mut certificates = Vec::new();
            unsafe {
                if (*chain.0).cChain == 0 {
                    return Err(denied());
                }
                let simple = *(*chain.0).rgpChain;
                for i in 0..(*simple).cElement as usize {
                    let context = (*(*(*simple).rgpElement.add(i))).pCertContext;
                    certificates.push(
                        std::slice::from_raw_parts(
                            (*context).pbCertEncoded,
                            (*context).cbCertEncoded as usize,
                        )
                        .to_vec(),
                    );
                }
            }
            identities.push((certificates, key));
        }
        Ok(identities)
    }
}
