// ref: security-framework 3.5.1 src/key.rs; Apple file-based keychain SecKey APIs.
use super::{denied, reference, spki, SignatureScheme};
use crate::Keychain;
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    data::CFData,
    dictionary::CFDictionary,
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use security_framework::{
    key::{Algorithm, SecKey},
    os::macos::access::SecAccess,
};
use security_framework_sys::{base::SecAccessRef, item::*, key::SecKeyCreateRandomKey};
use std::{ffi::CString, io};
extern "C" {
    static kSecAttrIsExtractable: CFStringRef;
    static kSecAttrApplicationTag: CFStringRef;
    static kSecAttrAccess: CFStringRef;
    static kSecACLAuthorizationSign: CFStringRef;
    fn SecKeychainItemCopyAttributesAndData(
        item: *const std::ffi::c_void,
        info: *mut AttributeInfo,
        class: *mut u32,
        attributes: *mut *mut security_framework_sys::base::SecKeychainAttributeList,
        length: *mut u32,
        data: *mut *mut std::ffi::c_void,
    ) -> i32;
    fn SecKeychainItemFreeAttributesAndData(
        attributes: *mut security_framework_sys::base::SecKeychainAttributeList,
        data: *mut std::ffi::c_void,
    ) -> i32;
    fn SecKeychainItemCopyAccess(item: *const std::ffi::c_void, access: *mut SecAccessRef) -> i32;
    fn SecAccessCopyMatchingACLList(
        access: SecAccessRef,
        authorization: *const std::ffi::c_void,
    ) -> core_foundation::array::CFArrayRef;
    fn SecACLCopyContents(
        acl: *const std::ffi::c_void,
        applications: *mut core_foundation::array::CFArrayRef,
        description: *mut CFStringRef,
        prompt: *mut PromptSelector,
    ) -> i32;
    fn SecTrustedApplicationValidateWithPath(
        application: *const std::ffi::c_void,
        path: *const std::ffi::c_char,
    ) -> i32;
    fn SecAccessCreate(
        description: CFStringRef,
        trusted: core_foundation::array::CFArrayRef,
        access: *mut SecAccessRef,
    ) -> i32;
    fn SecTrustedApplicationCreateFromPath(
        path: *const std::ffi::c_char,
        application: *mut *const std::ffi::c_void,
    ) -> i32;
}
#[repr(C)]
struct AttributeInfo {
    count: u32,
    tags: *mut u32,
    formats: *mut u32,
}
#[repr(C)]
struct PromptSelector {
    version: u16,
    flags: u16,
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
            let params = unsafe {
                CFDictionary::from_CFType_pairs(&[
                    attribute(
                        kSecClass,
                        CFString::wrap_under_get_rule(kSecClassKey).as_CFType(),
                    ),
                    attribute(
                        kSecAttrKeyClass,
                        CFString::wrap_under_get_rule(kSecAttrKeyClassPrivate).as_CFType(),
                    ),
                    attribute(
                        kSecAttrApplicationTag,
                        CFData::from_buffer(name.as_bytes()).as_CFType(),
                    ),
                    attribute(
                        kSecAttrKeyType,
                        CFString::wrap_under_get_rule(kSecAttrKeyTypeRSA).as_CFType(),
                    ),
                    attribute(
                        kSecMatchLimit,
                        CFString::wrap_under_get_rule(kSecMatchLimitAll).as_CFType(),
                    ),
                    attribute(
                        kSecMatchSearchList,
                        CFArray::from_CFTypes(std::slice::from_ref(&self.0)).as_CFType(),
                    ),
                    attribute(kSecReturnRef, CFBoolean::true_value().as_CFType()),
                ])
            };
            let mut item = std::ptr::null();
            // SAFETY: Query and out pointer are valid; retained result is checked before wrapping.
            let status = unsafe {
                security_framework_sys::keychain_item::SecItemCopyMatching(
                    params.as_concrete_TypeRef(),
                    &mut item,
                )
            };
            if status == -25300 {
                return Err(io::ErrorKind::NotFound.into());
            }
            if status != 0 || item.is_null() {
                return Err(denied());
            }
            // SAFETY: matchLimitAll returns an owned array of retained key references.
            let keys = unsafe { CFArray::<CFType>::wrap_under_create_rule(item.cast()) };
            if keys.len() != 1 {
                return Err(denied());
            }
            let key = keys.get(0).ok_or_else(denied)?;
            if key.type_of() != SecKey::type_id() {
                return Err(denied());
            }
            // SAFETY: Type checked key retained under the get rule.
            DeviceKey::checked(unsafe {
                SecKey::wrap_under_get_rule(key.as_CFTypeRef().cast_mut().cast())
            })
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
            if unsafe { SecTrustedApplicationCreateFromPath(path.as_ptr(), &mut application) } != 0
                || application.is_null()
            {
                return Err(denied());
            }
            // SAFETY: The successful create call transfers one retained CF object.
            let application = unsafe { CFType::wrap_under_create_rule(application) };
            let trusted = CFArray::from_CFTypes(&[application]);
            let label = CFString::new(name);
            let mut access = std::ptr::null_mut();
            // SAFETY: Valid retained CF arguments; successful create transfers ownership.
            if unsafe {
                SecAccessCreate(
                    label.as_concrete_TypeRef(),
                    trusted.as_concrete_TypeRef(),
                    &mut access,
                )
            } != 0
                || access.is_null()
            {
                return Err(denied());
            }
            // SAFETY: Successful SecAccessCreate returns an owned SecAccess object.
            let access = unsafe { SecAccess::wrap_under_create_rule(access) };
            // SAFETY: Security attribute constants are valid; dictionary retains its values.
            let private = unsafe {
                CFDictionary::from_CFType_pairs(&[
                    attribute(kSecAttrIsPermanent, CFBoolean::true_value().as_CFType()),
                    attribute(kSecAttrLabel, label.as_CFType()),
                    attribute(
                        kSecAttrApplicationTag,
                        CFData::from_buffer(name.as_bytes()).as_CFType(),
                    ),
                ])
            };
            // SAFETY: Security attribute constants are valid; dictionary retains nested values.
            let params = unsafe {
                CFDictionary::from_CFType_pairs(&[
                    attribute(
                        kSecAttrKeyType,
                        CFString::wrap_under_get_rule(kSecAttrKeyTypeRSA).as_CFType(),
                    ),
                    attribute(kSecAttrKeySizeInBits, CFNumber::from(2048).as_CFType()),
                    // The file-keychain generator reads these from the top-level
                    // dictionary, rather than kSecPrivateKeyAttrs (Apple SecKey.cpp).
                    attribute(kSecAttrIsExtractable, CFBoolean::false_value().as_CFType()),
                    attribute(kSecAttrAccess, access.as_CFType()),
                    attribute(kSecUseKeychain, self.0.as_CFType()),
                    attribute(kSecPrivateKeyAttrs, private.as_CFType()),
                ])
            };
            let mut error = std::ptr::null_mut();
            // SAFETY: Retained parameter dictionary and valid error out pointer.
            let raw = unsafe { SecKeyCreateRandomKey(params.as_concrete_TypeRef(), &mut error) };
            if !error.is_null() {
                // SAFETY: API transfers a retained CFError, released immediately without secret diagnostics.
                drop(unsafe { core_foundation::error::CFError::wrap_under_create_rule(error) });
            }
            if raw.is_null() {
                return Err(denied());
            }
            // SAFETY: Successful create returns a retained SecKey.
            DeviceKey::checked(unsafe { SecKey::wrap_under_create_rule(raw) })
        })
    }
}
impl DeviceKey {
    fn checked(key: SecKey) -> io::Result<Self> {
        // ref: macOS SDK SecKey.h kSecKeyExtractable/Size; SecKeychainItem.h attributes-only query.
        let mut tags = [16u32, 10u32];
        let mut formats = [2u32; 2];
        let mut info = AttributeInfo {
            count: 2,
            tags: tags.as_mut_ptr(),
            formats: formats.as_mut_ptr(),
        };
        let mut list = std::ptr::null_mut();
        // SAFETY: Live persistent key item, bounded tags/out pointer; no private data is requested.
        let status = unsafe {
            SecKeychainItemCopyAttributesAndData(
                key.as_CFTypeRef(),
                &mut info,
                std::ptr::null_mut(),
                &mut list,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if status != 0 || list.is_null() {
            return Err(denied());
        }
        // SAFETY: The OS owns the returned attribute list until the matching free call.
        let valid = unsafe {
            let attributes = &*list;
            attributes.count == 2
                && std::slice::from_raw_parts(attributes.attr, 2)
                    .iter()
                    .all(|a| {
                        if a.length != 4 || a.data.is_null() {
                            return false;
                        }
                        let value = (a.data as *const u32).read_unaligned();
                        match a.tag {
                            16 => value == 0,
                            10 => value == 2048,
                            _ => false,
                        }
                    })
        };
        // SAFETY: Free only the returned attributes; no data was requested.
        unsafe {
            SecKeychainItemFreeAttributesAndData(list, std::ptr::null_mut());
        }
        if !valid {
            return Err(denied());
        }
        Self::check_access(&key)?;
        Ok(Self(key))
    }
    fn check_access(key: &SecKey) -> io::Result<()> {
        let mut access = std::ptr::null_mut();
        // SAFETY: Live persistent key and valid retained-access output pointer.
        if unsafe { SecKeychainItemCopyAccess(key.as_CFTypeRef(), &mut access) } != 0
            || access.is_null()
        {
            return Err(denied());
        }
        // SAFETY: Successful copy transfers a retained access object.
        let access = unsafe { SecAccess::wrap_under_create_rule(access) };
        // SAFETY: Live access and immutable sign authorization constant.
        let raw = unsafe {
            SecAccessCopyMatchingACLList(
                access.as_concrete_TypeRef(),
                kSecACLAuthorizationSign.cast(),
            )
        };
        if raw.is_null() {
            return Err(denied());
        }
        // SAFETY: API transfers an owned CFArray of ACL objects.
        let acls = unsafe { CFArray::<CFType>::wrap_under_create_rule(raw) };
        if acls.is_empty() {
            return Err(denied());
        }
        let path = CString::new(
            std::env::current_exe()?
                .canonicalize()?
                .as_os_str()
                .as_encoded_bytes(),
        )
        .map_err(|_| denied())?;
        for acl in acls.iter() {
            let mut applications = std::ptr::null();
            let mut description = std::ptr::null();
            let mut prompt = PromptSelector {
                version: 0,
                flags: 0,
            };
            // SAFETY: Live ACL and valid CF output pointers; copied values are always released.
            let status = unsafe {
                SecACLCopyContents(
                    acl.as_CFTypeRef(),
                    &mut applications,
                    &mut description,
                    &mut prompt,
                )
            };
            if !description.is_null() {
                drop(unsafe { CFString::wrap_under_create_rule(description) });
            }
            if status != 0 || applications.is_null() {
                return Err(denied());
            }
            // SAFETY: Successful copy transfers an application array.
            let applications = unsafe { CFArray::<CFType>::wrap_under_create_rule(applications) };
            if applications.len() != 1 {
                return Err(denied());
            }
            let application = applications.get(0).ok_or_else(denied)?;
            // SAFETY: OS-owned trusted application and NUL-terminated current image path.
            // Validate the original code requirement, rather than its informational path.
            let status = unsafe {
                SecTrustedApplicationValidateWithPath(application.as_CFTypeRef(), path.as_ptr())
            };
            if status != 0 {
                return Err(denied());
            }
        }
        Ok(())
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
        self.0
            .create_signature(
                match scheme {
                    SignatureScheme::Pkcs1Sha256 => Algorithm::RSASignatureMessagePKCS1v15SHA256,
                    SignatureScheme::PssSha256 => Algorithm::RSASignatureMessagePSSSHA256,
                },
                message,
            )
            .map_err(|_| denied())
    }
}
