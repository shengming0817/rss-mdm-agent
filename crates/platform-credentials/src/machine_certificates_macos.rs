// ref: security-framework 3.5.1 src/item.rs, src/trust_settings.rs;
// Apple Security SecTrustSettingsSetTrustSettings / SecTrustSettingsRemoveTrustSettings.
use super::{input, CertificateStore, MachineCertificates};
use core_foundation::{
    array::CFArray, base::TCFType, dictionary::CFDictionary, number::CFNumber, string::CFString,
};
use security_framework::{
    certificate::SecCertificate,
    item::{ItemClass, ItemSearchOptions, Limit, SearchResult},
    os::macos::keychain::SecKeychain,
};
use security_framework_sys::{
    base::{errSecItemNotFound, errSecNoTrustSettings, SecCertificateRef},
    item::{kSecClass, kSecClassCertificate, kSecMatchSearchList, kSecValueRef},
    keychain_item::SecItemDelete,
    trust_settings::*,
};
use std::{io, ptr};

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecTrustSettingsRemoveTrustSettings(
        certificate: SecCertificateRef,
        domain: SecTrustSettingsDomain,
    ) -> i32;
}

fn status(value: i32) -> io::Result<()> {
    if value == 0 {
        Ok(())
    } else {
        Err(io::Error::other(
            security_framework::base::Error::from_code(value),
        ))
    }
}

fn settings(
    cert: &SecCertificate,
    domain: SecTrustSettingsDomain,
) -> io::Result<Option<CFArray<CFDictionary>>> {
    let mut array = ptr::null();
    // SAFETY: live certificate, valid domain, writable result; result follows the CF create rule.
    let code = unsafe {
        SecTrustSettingsCopyTrustSettings(cert.as_concrete_TypeRef(), domain, &mut array)
    };
    if code == errSecItemNotFound || code == errSecNoTrustSettings {
        return Ok(None);
    }
    status(code)?;
    if array.is_null() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Some(unsafe { CFArray::wrap_under_create_rule(array) }))
}

fn unconditional_root(value: &CFArray<CFDictionary>) -> bool {
    // An empty array means unconditional TrustRoot according to Security's API contract.
    if value.is_empty() {
        return true;
    }
    if value.len() != 1 {
        return false;
    }
    let Some(dict) = value.get(0) else {
        return false;
    };
    if dict.len() != 1 {
        return false;
    }
    let key = CFString::new("kSecTrustSettingsResult");
    dict.find(key.as_CFTypeRef().cast()).is_some_and(|number| {
        if unsafe { core_foundation::base::CFGetTypeID((*number).cast()) } != CFNumber::type_id() {
            return false;
        }
        let number = unsafe { CFNumber::wrap_under_get_rule((*number).cast()) };
        number.to_i64() == Some(i64::from(kSecTrustSettingsResultTrustRoot))
    })
}

fn find(keychain: &SecKeychain, der: &[u8]) -> io::Result<Option<SecCertificate>> {
    let results = ItemSearchOptions::new()
        .keychains(std::slice::from_ref(keychain))
        .class(ItemClass::certificate())
        .load_refs(true)
        .limit(Limit::All)
        .search();
    let results = match results {
        Ok(value) => value,
        Err(e) if e.code() == errSecItemNotFound => return Ok(None),
        Err(e) => return Err(io::Error::other(e)),
    };
    let mut found = None;
    for item in results {
        if let SearchResult::Ref(security_framework::item::Reference::Certificate(cert)) = item {
            if cert.to_der() == der {
                if found.is_some() {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                found = Some(cert);
            }
        }
    }
    Ok(found)
}

fn delete_exact(keychain: &SecKeychain, cert: &SecCertificate) -> io::Result<()> {
    let query = CFDictionary::from_CFType_pairs(&[
        (
            unsafe { CFString::wrap_under_get_rule(kSecClass) },
            unsafe { CFString::wrap_under_get_rule(kSecClassCertificate) }.as_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecMatchSearchList) },
            CFArray::from_CFTypes(std::slice::from_ref(keychain)).as_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecValueRef) },
            cert.as_CFType(),
        ),
    ]);
    // SAFETY: exact certificate loaded from this keychain; query also pins the search list.
    let code = unsafe { SecItemDelete(query.as_concrete_TypeRef()) };
    if code == errSecItemNotFound {
        Ok(())
    } else {
        status(code)
    }
}

fn system() -> io::Result<SecKeychain> {
    SecKeychain::open(std::path::Path::new("/Library/Keychains/System.keychain"))
        .map_err(io::Error::other)
}

fn write_access() -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

impl MachineCertificates {
    /// Read exact System Keychain material and its machine trust class.
    pub fn read(&self, class: CertificateStore, der: &[u8]) -> io::Result<bool> {
        input(der)?;
        let _interaction = crate::Keychain::disable_interaction()?;
        let Some(cert) = find(&system()?, der)? else {
            return Ok(false);
        };
        let admin = settings(&cert, kSecTrustSettingsDomainAdmin)?;
        match class {
            CertificateStore::Root => Ok(admin.as_ref().is_some_and(unconditional_root)),
            CertificateStore::Intermediate => {
                Ok(admin.is_none() && settings(&cert, kSecTrustSettingsDomainSystem)?.is_none())
            }
        }
    }
    /// Add-only; existing certificate or trust settings are never rewritten.
    pub fn add(&self, _class: CertificateStore, der: &[u8]) -> io::Result<bool> {
        input(der)?;
        write_access()?;
        let _interaction = crate::Keychain::disable_interaction()?;
        let keychain = system()?;
        let cert = SecCertificate::from_der(der).map_err(io::Error::other)?;
        if find(&keychain, der)?.is_some()
            || settings(&cert, kSecTrustSettingsDomainAdmin)?.is_some()
            || settings(&cert, kSecTrustSettingsDomainSystem)?.is_some()
        {
            return Ok(false);
        }
        match cert.add_to_keychain(Some(keychain)) {
            Ok(()) => (),
            Err(e) if e.code() == -25299 => return Ok(false),
            Err(e) => return Err(io::Error::other(e)),
        }
        Ok(true)
    }
    /// Caller has durable ownership. Complete missing settings without changing external trust.
    pub fn configure(&self, class: CertificateStore, der: &[u8]) -> io::Result<()> {
        input(der)?;
        write_access()?;
        let _interaction = crate::Keychain::disable_interaction()?;
        let cert = find(&system()?, der)?.ok_or(io::ErrorKind::NotFound)?;
        if settings(&cert, kSecTrustSettingsDomainSystem)?.is_some() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        if let Some(admin) = settings(&cert, kSecTrustSettingsDomainAdmin)? {
            return if class == CertificateStore::Root && unconditional_root(&admin) {
                Ok(())
            } else {
                Err(io::ErrorKind::PermissionDenied.into())
            };
        }
        if class == CertificateStore::Root {
            let entry = CFDictionary::from_CFType_pairs(&[(
                CFString::new("kSecTrustSettingsResult"),
                CFNumber::from(
                    i32::try_from(kSecTrustSettingsResultTrustRoot).map_err(io::Error::other)?,
                )
                .as_CFType(),
            )]);
            let array = CFArray::from_CFTypes(&[entry]);
            // SAFETY: the caller persisted ownership; a single unconditional Root setting in Admin domain.
            status(unsafe {
                SecTrustSettingsSetTrustSettings(
                    cert.as_concrete_TypeRef(),
                    kSecTrustSettingsDomainAdmin,
                    array.as_CFTypeRef(),
                )
            })?;
        }
        Ok(())
    }
    /// Caller owns this exact certificate; refuse to erase subsequently changed external settings.
    pub fn delete(&self, class: CertificateStore, der: &[u8]) -> io::Result<()> {
        input(der)?;
        write_access()?;
        let _interaction = crate::Keychain::disable_interaction()?;
        let keychain = system()?;
        let cert = SecCertificate::from_der(der).map_err(io::Error::other)?;
        if settings(&cert, kSecTrustSettingsDomainSystem)?.is_some() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        if let Some(admin) = settings(&cert, kSecTrustSettingsDomainAdmin)? {
            if class != CertificateStore::Root || !unconditional_root(&admin) {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            // SAFETY: durable caller ownership and unchanged setting were checked before this exact removal.
            status(unsafe {
                SecTrustSettingsRemoveTrustSettings(
                    cert.as_concrete_TypeRef(),
                    kSecTrustSettingsDomainAdmin,
                )
            })?;
        }
        if let Some(existing) = find(&keychain, der)? {
            delete_exact(&keychain, &existing)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_certificate_readback_and_delete_use_only_the_selected_native_keychain() {
        let path = std::env::temp_dir().join(format!(
            "rss-machine-cert-{}-{}.keychain",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::process::Command::new("/usr/bin/security")
                    .arg("delete-keychain")
                    .arg(&self.0)
                    .output();
            }
        }
        let _cleanup = Cleanup(path.clone());
        let mut keychain = security_framework::os::macos::keychain::CreateOptions::new()
            .password("isolated-certificate-test")
            .prompt_user(false)
            .create(&path)
            .unwrap();
        keychain.unlock(Some("isolated-certificate-test")).unwrap();
        let der = include_bytes!("../../../tests/agent-protocol/pki/root-test-ca.pem");
        // The certificate fixture is decoded by OpenSSL only in this isolated native test.
        let output = std::process::Command::new("/usr/bin/openssl")
            .args(["x509", "-outform", "DER"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.take().unwrap().write_all(der)?;
                child.wait_with_output()
            })
            .unwrap();
        assert!(output.status.success());
        let certificate = SecCertificate::from_der(&output.stdout).unwrap();
        assert!(find(&keychain, &output.stdout).unwrap().is_none());
        certificate.add_to_keychain(Some(keychain.clone())).unwrap();
        assert_eq!(
            find(&keychain, &output.stdout).unwrap().unwrap().to_der(),
            output.stdout
        );
        delete_exact(
            &keychain,
            &find(&keychain, &output.stdout).unwrap().unwrap(),
        )
        .unwrap();
        assert!(find(&keychain, &output.stdout).unwrap().is_none());
    }
}
