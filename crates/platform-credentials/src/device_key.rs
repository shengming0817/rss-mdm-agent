//! System-owned RSA keys. No private export, overwrite or plaintext fallback API exists.
#[cfg(windows)]
use der::{asn1::UintRef, Sequence};
use der::{
    asn1::{Any, BitString},
    Encode,
};
use std::io;
/// Supported RSA message-signature schemes. The platform hashes the message exactly once.
#[derive(Clone, Copy, Debug)]
pub enum SignatureScheme {
    Pkcs1Sha256,
    PssSha256,
}
#[cfg(target_os = "macos")]
#[path = "device_key_macos.rs"]
mod native;
#[cfg(windows)]
#[path = "device_key_windows.rs"]
mod native;
#[cfg(any(target_os = "macos", windows))]
pub use native::DeviceKey;
#[cfg(not(any(target_os = "macos", windows)))]
pub struct DeviceKey;
fn reference(value: &str) -> io::Result<()> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(())
}
fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "device signing key unavailable",
    )
}
fn spki(public: &[u8]) -> io::Result<Vec<u8>> {
    spki::SubjectPublicKeyInfoOwned {
        algorithm: spki::AlgorithmIdentifierOwned {
            oid: der::asn1::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1"),
            parameters: Some(Any::null()),
        },
        subject_public_key: BitString::from_bytes(public).map_err(|_| denied())?,
    }
    .to_der()
    .map_err(|_| denied())
}
#[cfg(windows)]
#[derive(Sequence)]
struct PublicRsa<'a> {
    modulus: UintRef<'a>,
    exponent: UintRef<'a>,
}
#[cfg(windows)]
fn rsa(modulus: &[u8], exponent: &[u8]) -> io::Result<Vec<u8>> {
    let public = PublicRsa {
        modulus: UintRef::new(modulus).map_err(|_| denied())?,
        exponent: UintRef::new(exponent).map_err(|_| denied())?,
    };
    spki(&public.to_der().map_err(|_| denied())?)
}
