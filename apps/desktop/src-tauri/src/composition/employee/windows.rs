//! Re-open the enrolled per-user CNG key. Recovery cannot create or substitute one.
//! ref: Microsoft Windows SDK ncrypt.h NCryptOpenKey, NCryptExportKey, NCryptSignHash.
use crate::self_service::{error, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rss_mdm_agent_wire::{
    enterprise::KeyRegistration,
    platform_sso::PublicKey,
    runtime::{AccessBindingStatus, SelfServiceUser},
};
use sha2::{Digest, Sha256};
use windows_sys::Win32::Security::Cryptography::*;

struct Handle(usize);
impl Drop for Handle {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                NCryptFreeObject(self.0);
            }
        }
    }
}
pub(super) fn sign(b: &AccessBindingStatus, employee: &SelfServiceUser) -> Result<KeyRegistration> {
    let missing = || error("registration_required", "当前账号需要重新登记企业证明密钥");
    let unavailable = || error("key_unavailable", "无法使用已登记的企业证明密钥");
    let mut provider = Handle(0);
    let mut key = Handle(0);
    // NCryptOpenKey defaults to the current user. No machine flag or creation path.
    let name: Vec<u16> = format!("RSS.DeviceAccess.{}", b.id)
        .encode_utf16()
        .chain([0])
        .collect();
    if unsafe { NCryptOpenStorageProvider(&mut provider.0, MS_PLATFORM_CRYPTO_PROVIDER, 0) } != 0 {
        return Err(unavailable());
    }
    let status = unsafe { NCryptOpenKey(provider.0, &mut key.0, name.as_ptr(), 0, 0) };
    if status == windows_sys::Win32::Foundation::NTE_BAD_KEYSET {
        return Err(missing());
    }
    if status != 0 {
        return Err(unavailable());
    }
    let mut point = [0u8; 72];
    let mut size = 0u32;
    if unsafe {
        NCryptExportKey(
            key.0,
            0,
            BCRYPT_ECCPUBLIC_BLOB,
            std::ptr::null(),
            point.as_mut_ptr(),
            72,
            &mut size,
            0,
        )
    } != 0
        || size != 72
        || point[..4] != BCRYPT_ECDSA_PUBLIC_P256_MAGIC.to_le_bytes()
        || point[4..8] != 32u32.to_le_bytes()
    {
        return Err(unavailable());
    }
    let public_key = PublicKey {
        kty: "EC".into(),
        crv: "P-256".into(),
        x: URL_SAFE_NO_PAD.encode(&point[8..40]),
        y: URL_SAFE_NO_PAD.encode(&point[40..72]),
    };
    let mut proof = KeyRegistration {
        binding_id: b.id,
        binding_generation: b.binding_generation,
        factor_generation: b.factor_generation,
        binding_revision: b.revision,
        public_key,
        signature: String::new(),
    };
    let input = proof
        .signing_bytes(
            employee.tenant_id,
            employee.instance_id,
            employee.principal_id,
            b.operation_id,
        )
        .map_err(|_| unavailable())?;
    let digest = Sha256::digest(input);
    let mut signature = [0u8; 64];
    if unsafe {
        NCryptSignHash(
            key.0,
            std::ptr::null(),
            digest.as_ptr(),
            32,
            signature.as_mut_ptr(),
            64,
            &mut size,
            0,
        )
    } != 0
        || size != 64
    {
        return Err(unavailable());
    }
    proof.signature = URL_SAFE_NO_PAD.encode(signature);
    Ok(proof)
}
