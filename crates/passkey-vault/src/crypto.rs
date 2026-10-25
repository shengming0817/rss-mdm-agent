// ref: RustCrypto password-hashes argon2/src/lib.rs@argon2-v0.5.3;
// RFC 9106 section 4, second recommended Argon2id profile.
use crate::{Error, Result};
use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub(crate) type Key = Zeroizing<[u8; 32]>;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Sealed {
    nonce: [u8; 12],
    bytes: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasswordSlot {
    algorithm: String,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
    salt: [u8; 16],
    root: Sealed,
}
pub(crate) fn random<const N: usize>() -> Result<[u8; N]> {
    platform_credentials::random_bytes(N)
        .map_err(|_| Error::SystemUnavailable)?
        .try_into()
        .map_err(|_| Error::SystemUnavailable)
}
pub(crate) fn seal(key: &Key, aad: &[u8], bytes: &[u8]) -> Result<Sealed> {
    let nonce = random()?;
    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| Error::Corrupt)?;
    let bytes = cipher
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: bytes, aad })
        .map_err(|_| Error::Corrupt)?;
    Ok(Sealed { nonce, bytes })
}
pub(crate) fn open(key: &Key, aad: &[u8], data: &Sealed) -> Result<Zeroizing<Vec<u8>>> {
    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| Error::Corrupt)?;
    cipher
        .decrypt(
            Nonce::from_slice(&data.nonce),
            Payload {
                msg: &data.bytes,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Authentication)
}
pub(crate) fn validate_password(password: &str) -> Result<()> {
    if password.chars().count() < 12 || password.len() > 1024 {
        return Err(Error::Input);
    }
    Ok(())
}
fn derive(password: &str, salt: &[u8; 16]) -> Result<Key> {
    if password.len() > 1024 {
        return Err(Error::Input);
    }
    let params = argon2::Params::new(65536, 3, 4, Some(32)).map_err(|_| Error::Input)?;
    let mut key = Zeroizing::new([0; 32]);
    // Explicitly wipe the memory-hard workspace as well as the output on every exit.
    let mut memory = Zeroizing::new(vec![argon2::Block::default(); params.block_count()]);
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
        .hash_password_into_with_memory(
            password.as_bytes(),
            salt,
            key.as_mut(),
            memory.as_mut_slice(),
        )
        .map_err(|_| Error::Authentication)?;
    Ok(key)
}
impl PasswordSlot {
    pub(crate) fn new(root: &Key, aad: &[u8], password: &str) -> Result<Self> {
        validate_password(password)?;
        let salt = random()?;
        let root = seal(&derive(password, &salt)?, aad, root.as_ref())?;
        Ok(Self {
            algorithm: "argon2id-v19".into(),
            memory_kib: 65536,
            iterations: 3,
            lanes: 4,
            salt,
            root,
        })
    }
    pub(crate) fn unwrap(&self, aad: &[u8], password: &str) -> Result<Key> {
        if self.algorithm != "argon2id-v19"
            || self.memory_kib != 65536
            || self.iterations != 3
            || self.lanes != 4
        {
            return Err(Error::Unsupported);
        }
        let bytes = open(&derive(password, &self.salt)?, aad, &self.root)?;
        let mut key = Zeroizing::new([0; 32]);
        if bytes.len() != 32 {
            return Err(Error::Corrupt);
        }
        key.copy_from_slice(&bytes);
        Ok(key)
    }
}
