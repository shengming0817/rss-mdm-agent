// ref: security-framework 3.5.1 src/key.rs; Apple file-based keychain SecKey APIs.
use super::{denied, reference, spki, SignatureScheme};
use crate::Keychain;
use core_foundation::{array::CFArray, base::{CFType, TCFType}, boolean::CFBoolean, dictionary::CFDictionary, number::CFNumber, string::{CFString, CFStringRef}};
use security_framework::{key::{SecKey, Algorithm}, os::macos::access::SecAccess};
use security_framework_sys::{base::{SecAccessRef, SecKeyRef}, item::*, key::SecKeyCreateRandomKey};
use std::{io, ffi::CString};
extern "C" {
    static kSecAttrIsExtractable: CFStringRef;
    static kSecAttrAccess: CFStringRef;
    fn SecAccessCreate(description: CFStringRef, trusted: core_foundation::array::CFArrayRef, access: *mut SecAccessRef) -> i32;
    fn SecTrustedApplicationCreateFromPath(path: *const std::ffi::c_char, application: *mut *const std::ffi::c_void) -> i32;
}
/// Permanently held, non-exportable private RSA key in the explicit daemon keychain.
pub struct DeviceKey(SecKey);
fn attribute(key: CFStringRef, value: CFType) -> (CFString, CFType) {
    // SAFETY: All callers pass immutable Security framework string constants.
    (unsafe { CFString::wrap_under_get_rule(key) }, value)
}
impl Keychain {
    /// Reopen one exact existing key; absence never generates another key.
    pub fn open_device_key(&self, name: &str) -> io::Result<DeviceKey> {
        reference(name)?;
        self.without_interaction(|| {
            // SAFETY: Constants are valid for the lifetime of the query; CF objects retain values.
            let params = unsafe { CFDictionary::from_CFType_pairs(&[
                attribute(kSecClass, CFString::wrap_under_get_rule(kSecClassKey).as_CFType()),
                attribute(kSecAttrKeyClass, CFString::wrap_under_get_rule(kSecAttrKeyClassPrivate).as_CFType()),
                attribute(kSecAttrLabel, CFString::new(name).as_CFType()),
                attribute(kSecMatchSearchList, CFArray::from_CFTypes(&[self.0.clone()]).as_CFType()),
                attribute(kSecReturnRef, CFBoolean::true_value().as_CFType()),
            ]) };
            let mut item = std::ptr::null();
            // SAFETY: Query and out pointer are valid; retained result is checked before wrapping.
            let status = unsafe { security_framework_sys::keychain_item::SecItemCopyMatching(params.as_concrete_TypeRef(), &mut item) };
            if status == -25300 { return Err(io::ErrorKind::NotFound.into()); }
            if status != 0 || item.is_null() { return Err(denied()); }
            // SAFETY: A successful class-key/private query returns an owned SecKey.
            let key = unsafe { SecKey::wrap_under_create_rule(item as SecKeyRef) };
            DeviceKey::checked(key)
        })
    }
    /// Create once with access limited to the current trusted service executable.
    pub fn create_device_key(&self, name: &str) -> io::Result<DeviceKey> {
        match self.open_device_key(name) {
            Ok(key) => return Ok(key),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        self.without_interaction(|| {
            let path = std::env::current_exe()?.canonicalize()?;
            let path = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| denied())?;
            let mut application = std::ptr::null();
            // SAFETY: NUL-terminated executable path and writable out pointer; status checked.
            if unsafe { SecTrustedApplicationCreateFromPath(path.as_ptr(), &mut application) } != 0 || application.is_null() { return Err(denied()); }
            // SAFETY: The successful create call transfers one retained CF object.
            let application = unsafe { CFType::wrap_under_create_rule(application) };
            let trusted = CFArray::from_CFTypes(&[application]);
            let label = CFString::new(name);
            let mut access = std::ptr::null_mut();
            // SAFETY: Valid retained CF arguments; successful create transfers ownership.
            if unsafe { SecAccessCreate(label.as_concrete_TypeRef(), trusted.as_concrete_TypeRef(), &mut access) } != 0 || access.is_null() { return Err(denied()); }
            // SAFETY: Successful SecAccessCreate returns an owned SecAccess object.
            let access = unsafe { SecAccess::wrap_under_create_rule(access) };
            // SAFETY: Security attribute constants are valid; dictionary retains its values.
            let private = unsafe { CFDictionary::from_CFType_pairs(&[
                attribute(kSecAttrIsPermanent, CFBoolean::true_value().as_CFType()),
                attribute(kSecAttrIsExtractable, CFBoolean::false_value().as_CFType()),
                attribute(kSecAttrLabel, label.as_CFType()),
                attribute(kSecAttrAccess, access.as_CFType()),
                attribute(kSecUseKeychain, self.0.as_CFType()),
            ]) };
            // SAFETY: Security attribute constants are valid; dictionary retains nested values.
            let params = unsafe { CFDictionary::from_CFType_pairs(&[
                attribute(kSecAttrKeyType, CFString::wrap_under_get_rule(kSecAttrKeyTypeRSA).as_CFType()),
                attribute(kSecAttrKeySizeInBits, CFNumber::from(2048).as_CFType()),
                attribute(kSecUseKeychain, self.0.as_CFType()),
                attribute(kSecPrivateKeyAttrs, private.as_CFType()),
            ]) };
            let mut error = std::ptr::null_mut();
            // SAFETY: Retained parameter dictionary and valid error out pointer.
            let raw = unsafe { SecKeyCreateRandomKey(params.as_concrete_TypeRef(), &mut error) };
            if !error.is_null() {
                // SAFETY: API transfers a retained CFError, released immediately without secret diagnostics.
                drop(unsafe { core_foundation::error::CFError::wrap_under_create_rule(error) });
            }
            if raw.is_null() { return Err(denied()); }
            // SAFETY: Successful create returns a retained SecKey.
            DeviceKey::checked(unsafe { SecKey::wrap_under_create_rule(raw) })
        })
    }
}
impl DeviceKey {
    fn checked(key: SecKey) -> io::Result<Self> {
        // Query attributes, never attempt private-key export as a validation side effect.
        let attributes = key.attributes();
        // SAFETY: Extractability constant names a CFBoolean attribute in the returned dictionary.
        let extractable = attributes.find(unsafe { kSecAttrIsExtractable.cast() }).ok_or_else(denied)?;
        // SAFETY: The attribute is a borrowed CFBoolean owned by the dictionary.
        if unsafe { CFBoolean::wrap_under_get_rule(extractable.cast()) }.into() { return Err(denied()); }
        Ok(Self(key))
    }
    /// Export only the public key in standard SPKI DER.
    pub fn public_spki(&self) -> io::Result<Vec<u8>> {
        let key = self.0.public_key().ok_or_else(denied)?;
        let public = key.external_representation().ok_or_else(denied)?;
        spki(public.bytes())
    }
    /// Sign a message without exporting the private key or prompting a user.
    pub fn sign(&self, scheme: SignatureScheme, message: &[u8]) -> io::Result<Vec<u8>> {
        let _guard = Keychain::disable_interaction()?;
        self.0.create_signature(match scheme {
            SignatureScheme::Pkcs1Sha256 => Algorithm::RSASignatureMessagePKCS1v15SHA256,
            SignatureScheme::PssSha256 => Algorithm::RSASignatureMessagePSSSHA256,
        }, message).map_err(|_| denied())
    }
}
