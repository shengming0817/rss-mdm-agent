//! Verify exactly the original CTAP buffer, never a reserialized request.
//! ref: Windows-classic-samples@434f6002bdf9cf9829406c3ff2b33387982d6168
//! Samples/PasskeyManager/cpp/PluginAuthenticator/PluginAuthenticatorImpl.cpp VerifySignatureHelper.
use crate::{Error, Result};
use ring::signature;
fn word(bytes: &[u8], offset: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(Error::Authentication)?
            .try_into()
            .map_err(|_| Error::Authentication)?,
    ) as usize)
}
pub(crate) fn verify(key: &[u8], message: &[u8], sig: &[u8]) -> Result<()> {
    if key.len() > 4096 || sig.len() > 1024 || message.is_empty() || message.len() > 131072 {
        return Err(Error::Authentication);
    }
    match word(key, 0)? {
        0x31415352 => {
            // BCRYPT_RSAPUBLIC_MAGIC, RSA-PSS SHA256, salt 32.
            let bits = word(key, 4)?;
            let e = word(key, 8)?;
            let n = word(key, 12)?;
            if !(2048..=8192).contains(&bits)
                || n * 8 != bits
                || !(1..=8).contains(&e)
                || word(key, 16)? != 0
                || word(key, 20)? != 0
                || key.len() != 24 + e + n
            {
                return Err(Error::Authentication);
            }
            signature::RsaPublicKeyComponents {
                n: &key[24 + e..],
                e: &key[24..24 + e],
            }
            .verify(&signature::RSA_PSS_2048_8192_SHA256, message, sig)
            .map_err(|_| Error::Authentication)
        }
        0x31534345 => {
            // BCRYPT_ECDSA_PUBLIC_P256_MAGIC
            if word(key, 4)? != 32 || key.len() != 72 {
                return Err(Error::Authentication);
            }
            let mut point = [0u8; 65];
            point[0] = 4;
            point[1..].copy_from_slice(&key[8..]);
            signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, point)
                .verify(message, sig)
                .map_err(|_| Error::Authentication)
        }
        _ => Err(Error::Authentication),
    }
}
