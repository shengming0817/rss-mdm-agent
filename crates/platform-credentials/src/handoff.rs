//! Explicit, interactive transfer of existing file-keychain ACLs between verified images.
//! ref: Apple Security OSX/libsecurity_keychain/lib/{SecACL,SecAccess,SecKeychainItem}.h.
use crate::Keychain;
use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFType, TCFType},
    string::{CFString, CFStringRef},
    url::CFURL,
};
use security_framework::os::macos::{
    access::SecAccess,
    code_signing::{Flags, SecCode, SecRequirement, SecStaticCode},
};
use security_framework_sys::base::{SecAccessRef, SecKeychainAttribute, SecKeychainAttributeList};
use std::{
    ffi::{c_void, CString},
    io,
    path::Path,
};

extern "C" {
    fn SecKeychainSearchCreateFromAttributes(
        keychain: *const c_void,
        class: u32,
        attributes: *const SecKeychainAttributeList,
        search: *mut *const c_void,
    ) -> i32;
    fn SecKeychainSearchCopyNext(search: *const c_void, item: *mut *const c_void) -> i32;
    fn SecKeychainItemCopyAccess(item: *const c_void, access: *mut SecAccessRef) -> i32;
    fn SecKeychainItemSetAccess(item: *const c_void, access: SecAccessRef) -> i32;
    fn SecAccessCopyACLList(access: SecAccessRef, acls: *mut CFArrayRef) -> i32;
    fn SecAccessCopyMatchingACLList(
        access: SecAccessRef,
        authorization: *const c_void,
    ) -> CFArrayRef;
    fn SecACLCopyContents(
        acl: *const c_void,
        apps: *mut CFArrayRef,
        description: *mut CFStringRef,
        prompt: *mut u16,
    ) -> i32;
    fn SecACLSetContents(
        acl: *const c_void,
        apps: CFArrayRef,
        description: CFStringRef,
        prompt: u16,
    ) -> i32;
    fn SecTrustedApplicationCreateFromPath(
        path: *const std::ffi::c_char,
        app: *mut *const c_void,
    ) -> i32;
    fn SecTrustedApplicationCopyRequirement(
        app: *const c_void,
        requirement: *mut security_framework_sys::code_signing::SecRequirementRef,
    ) -> i32;
    static kSecACLAuthorizationSign: CFStringRef;
    static kSecACLAuthorizationDecrypt: CFStringRef;
}

/// Exact item selectors supplied from the caller's protected ownership catalog.
pub enum CredentialItem {
    Password { service: String, account: String },
    PrivateKey { account: String },
}
struct Change {
    item: CFType,
    original: SecAccess,
    next: SecAccess,
    applied: bool,
}
/// Prepared copies retain all unrelated ACLs, authorizations, owner and prompt flags.
/// No secret bytes or private-key representations are read by this operation.
pub struct KeychainHandoff {
    changes: Vec<Change>,
}
fn status(value: i32) -> io::Result<()> {
    if value == 0 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "keychain authorization failed ({value})"
        )))
    }
}
fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "keychain image authorization mismatch",
    )
}
fn application(path: &Path) -> io::Result<CFType> {
    let path = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| denied())?;
    let mut raw = std::ptr::null();
    // SAFETY: A NUL-terminated path and valid retained-object output pointer.
    status(unsafe { SecTrustedApplicationCreateFromPath(path.as_ptr(), &mut raw) })?;
    if raw.is_null() {
        return Err(denied());
    }
    Ok(unsafe { CFType::wrap_under_create_rule(raw) })
}
fn requirement(app: &CFType) -> io::Result<SecRequirement> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: A live trusted application and retained typed output pointer.
    status(unsafe { SecTrustedApplicationCopyRequirement(app.as_CFTypeRef(), &mut raw) })?;
    if raw.is_null() {
        return Err(denied());
    }
    Ok(unsafe { SecRequirement::wrap_under_create_rule(raw) })
}
fn access(item: &CFType) -> io::Result<SecAccess> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: A live persistent keychain item; no data requested.
    status(unsafe { SecKeychainItemCopyAccess(item.as_CFTypeRef(), &mut raw) })?;
    if raw.is_null() {
        return Err(denied());
    }
    Ok(unsafe { SecAccess::wrap_under_create_rule(raw) })
}
fn contents(acl: &CFType) -> io::Result<(Option<CFArray<CFType>>, CFString, u16)> {
    let mut apps = std::ptr::null();
    let mut description = std::ptr::null();
    let mut prompt = 0u16;
    // SAFETY: Live ACL; correct SecKeychainPromptSelector ABI (a uint16, not the deprecated CSSM struct).
    let result =
        unsafe { SecACLCopyContents(acl.as_CFTypeRef(), &mut apps, &mut description, &mut prompt) };
    let apps = if apps.is_null() {
        None
    } else {
        Some(unsafe { CFArray::wrap_under_create_rule(apps) })
    };
    let description = if description.is_null() {
        None
    } else {
        Some(unsafe { CFString::wrap_under_create_rule(description) })
    };
    status(result)?;
    Ok((apps, description.ok_or_else(denied)?, prompt))
}
fn matches(apps: &CFArray<CFType>, old: &SecStaticCode, new: &SecStaticCode) -> io::Result<bool> {
    if apps.len() != 1 {
        return Err(denied());
    }
    let app = apps.get(0).ok_or_else(denied)?;
    let req = requirement(&app)?;
    let already_new = new.check_validity(Flags::NONE, &req).is_ok();
    if !already_new && old.check_validity(Flags::NONE, &req).is_err() {
        return Err(denied());
    }
    Ok(already_new)
}
impl Keychain {
    fn password_item(&self, service: &str, account: &str) -> io::Result<Option<CFType>> {
        if service.is_empty() || service.len() > 256 || account.is_empty() || account.len() > 256 {
            return Err(denied());
        }
        let mut attrs = [
            SecKeychainAttribute {
                tag: u32::from_be_bytes(*b"svce"),
                length: service.len() as u32,
                data: service.as_ptr().cast_mut().cast(),
            },
            SecKeychainAttribute {
                tag: u32::from_be_bytes(*b"acct"),
                length: account.len() as u32,
                data: account.as_ptr().cast_mut().cast(),
            },
        ];
        let list = SecKeychainAttributeList {
            count: 2,
            attr: attrs.as_mut_ptr(),
        };
        let mut search = std::ptr::null();
        // SAFETY: Attribute buffers live until the synchronous search is created. Search is scoped to this keychain.
        status(unsafe {
            SecKeychainSearchCreateFromAttributes(
                self.0.as_CFTypeRef(),
                u32::from_be_bytes(*b"genp"),
                &list,
                &mut search,
            )
        })?;
        if search.is_null() {
            return Err(denied());
        }
        let search = unsafe { CFType::wrap_under_create_rule(search) };
        let mut item = std::ptr::null();
        // SAFETY: Live search and retained-item output; attributes-only lookup never reads a password.
        let result = unsafe { SecKeychainSearchCopyNext(search.as_CFTypeRef(), &mut item) };
        if result == -25300 {
            return Ok(None);
        }
        status(result)?;
        if item.is_null() {
            return Err(denied());
        }
        let item = unsafe { CFType::wrap_under_create_rule(item) };
        let mut duplicate = std::ptr::null();
        let result = unsafe { SecKeychainSearchCopyNext(search.as_CFTypeRef(), &mut duplicate) };
        if !duplicate.is_null() {
            drop(unsafe { CFType::wrap_under_create_rule(duplicate) });
        }
        if result != -25300 {
            return Err(denied());
        }
        Ok(Some(item))
    }
}
impl KeychainHandoff {
    /// Validate every item before any write. The new signed image must be this process.
    /// Caller must authenticate/protect both executable paths and stop their services first.
    pub fn prepare(
        keychain: &Keychain,
        old: &Path,
        new: &Path,
        required: &str,
        items: &[CredentialItem],
    ) -> io::Result<Self> {
        if items.is_empty()
            || items.len() > 4096
            || std::env::current_exe()?.canonicalize()? != new.canonicalize()?
        {
            return Err(denied());
        }
        let code = |path: &Path| -> io::Result<SecStaticCode> {
            SecStaticCode::from_path(
                &CFURL::from_path(path, false).ok_or_else(denied)?,
                Flags::NONE,
            )
            .map_err(io::Error::other)
        };
        let old = code(old)?;
        let new_code = code(new)?;
        let required: SecRequirement = required.parse().map_err(io::Error::other)?;
        new_code
            .check_validity(Flags::NONE, &required)
            .map_err(io::Error::other)?;
        let new_app = application(new)?;
        let new_req = requirement(&new_app)?;
        SecCode::for_self(Flags::NONE)
            .and_then(|c| c.check_validity(Flags::NONE, &new_req))
            .map_err(io::Error::other)?;
        let trusted = CFArray::from_CFTypes(&[new_app]);
        let mut changes = Vec::new();
        let mut found = false;
        for selector in items {
            let (item, authorization) = match selector {
                CredentialItem::Password { service, account } => {
                    (keychain.password_item(service, account)?, unsafe {
                        kSecACLAuthorizationDecrypt
                    })
                }
                CredentialItem::PrivateKey { account } => {
                    let key = match keychain.device_key_item(account) {
                        Ok(key) => Some(key.as_CFType()),
                        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                        Err(e) => return Err(e),
                    };
                    (key, unsafe { kSecACLAuthorizationSign })
                }
            };
            let Some(item) = item else {
                continue;
            }; // retired catalog entries may be absent
            found = true;
            let original = access(&item)?;
            let next = access(&item)?; // independent copy for rollback
            let raw = unsafe {
                SecAccessCopyMatchingACLList(next.as_concrete_TypeRef(), authorization.cast())
            };
            if raw.is_null() {
                return Err(denied());
            }
            let required_acls = unsafe { CFArray::<CFType>::wrap_under_create_rule(raw) };
            if required_acls.is_empty() {
                return Err(denied());
            }
            for acl in required_acls.iter() {
                let (apps, _, _) = contents(&acl)?;
                matches(&apps.ok_or_else(denied)?, &old, &new_code)?;
            }
            let mut raw = std::ptr::null();
            status(unsafe { SecAccessCopyACLList(next.as_concrete_TypeRef(), &mut raw) })?;
            if raw.is_null() {
                return Err(denied());
            }
            let acls = unsafe { CFArray::<CFType>::wrap_under_create_rule(raw) };
            let mut changed = false;
            for acl in acls.iter() {
                let (apps, description, prompt) = contents(&acl)?;
                let Some(apps) = apps else {
                    continue;
                };
                if apps.is_empty() {
                    continue;
                } // preserve owner/partition/deny ACLs
                if matches(&apps, &old, &new_code)? {
                    continue;
                }
                // Keep authorization tags, description and prompt flags exactly as stored.
                status(unsafe {
                    SecACLSetContents(
                        acl.as_CFTypeRef(),
                        trusted.as_concrete_TypeRef(),
                        description.as_concrete_TypeRef(),
                        prompt,
                    )
                })?;
                changed = true;
            }
            if changed {
                changes.push(Change {
                    item,
                    original,
                    next,
                    applied: false,
                });
            }
        }
        if !found {
            return Err(denied());
        }
        Ok(Self { changes })
    }
    /// May request native OS authorization. Never call from an unattended daemon.
    pub fn apply(&mut self) -> io::Result<()> {
        for change in &mut self.changes {
            // Mark first: an uncertain native failure must also be compensated.
            change.applied = true;
            status(unsafe {
                SecKeychainItemSetAccess(
                    change.item.as_CFTypeRef(),
                    change.next.as_concrete_TypeRef(),
                )
            })?;
        }
        Ok(())
    }
    /// Explicit compensation; failure must block restarting the old image.
    pub fn rollback(&mut self) -> io::Result<()> {
        let mut failed = false;
        for change in self.changes.iter_mut().rev().filter(|c| c.applied) {
            if unsafe {
                SecKeychainItemSetAccess(
                    change.item.as_CFTypeRef(),
                    change.original.as_concrete_TypeRef(),
                )
            } == 0
            {
                change.applied = false;
            } else {
                failed = true;
            }
        }
        if failed {
            Err(io::Error::other(
                "keychain authorization rollback requires recovery",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_catalog_handoff_preserves_secret_key_and_foreign_items() {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "rss-handoff-{}-{}.keychain",
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
        let keychain = Keychain::create_file(&path, "isolated-test-password").unwrap();
        keychain
            .create_new("handoff-test", "owned", b"retained-secret")
            .unwrap();
        keychain
            .create_new("handoff-test", "foreign", b"untouched")
            .unwrap();
        let key = keychain.create_device_key("owned-key").unwrap();
        let public = key.public_spki().unwrap();
        let before = key
            .sign(crate::SignatureScheme::Pkcs1Sha256, b"retained-proof")
            .unwrap();
        let exe = std::env::current_exe().unwrap().canonicalize().unwrap();
        let items = [
            CredentialItem::Password {
                service: "handoff-test".into(),
                account: "owned".into(),
            },
            CredentialItem::PrivateKey {
                account: "owned-key".into(),
            },
        ];
        assert!(KeychainHandoff::prepare(&keychain, &exe, &exe, "anchor apple", &items).is_err());
        assert!(KeychainHandoff::prepare(
            &keychain,
            &exe,
            Path::new("/usr/bin/true"),
            "always",
            &items
        )
        .is_err());
        let mut handoff =
            KeychainHandoff::prepare(&keychain, &exe, &exe, "always", &items).unwrap();
        handoff.apply().unwrap();
        assert_eq!(
            keychain.read("handoff-test", "owned").unwrap(),
            b"retained-secret"
        );
        assert_eq!(
            keychain.read("handoff-test", "foreign").unwrap(),
            b"untouched"
        );
        let retained = keychain.open_device_key("owned-key").unwrap();
        assert_eq!(retained.public_spki().unwrap(), public);
        assert_eq!(
            retained
                .sign(crate::SignatureScheme::Pkcs1Sha256, b"retained-proof")
                .unwrap(),
            before
        );
        handoff.rollback().unwrap();
        assert_eq!(
            keychain.read("handoff-test", "owned").unwrap(),
            b"retained-secret"
        );
        assert_eq!(
            keychain
                .open_device_key("owned-key")
                .unwrap()
                .public_spki()
                .unwrap(),
            public
        );
    }
}
