use super::*;
#[cfg(any(windows, test))]
use base64::{engine::general_purpose::STANDARD, Engine};
#[cfg(any(windows, test))]
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
#[cfg(any(windows, test))]
use rsa::{pkcs8::DecodePublicKey, rand_core::OsRng, traits::PublicKeyParts, Oaep, RsaPublicKey};
use sha2::{Digest, Sha256};
pub(super) fn digest(value: &w::CredentialEscrow) -> Result<String, Error> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(|_| Error::Protocol)?)
    ))
}
#[cfg(any(windows, test))]
pub(super) fn encrypt(
    task: &w::CredentialTaskSpec,
    material: &Material,
) -> Result<w::CredentialEnvelope, Error> {
    let public = RsaPublicKey::from_public_key_der(
        &STANDARD
            .decode(&task.recipient.public_key)
            .map_err(|_| Error::Protocol)?,
    )
    .map_err(|_| Error::Protocol)?;
    if !(2048..=8192).contains(&public.n().bits()) {
        return Err(Error::Untrusted);
    }
    let mut key = Zeroizing::new([0u8; 32]);
    let mut nonce = [0u8; 12];
    SystemRandom::new()
        .fill(key.as_mut())
        .map_err(|_| Error::Unavailable)?;
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| Error::Unavailable)?;
    let wrapped = public
        .encrypt(&mut OsRng, Oaep::new::<Sha256>(), key.as_ref())
        .map_err(|_| Error::Protocol)?;
    let expected_revision = task
        .expected
        .iter()
        .find(|v| v.subject == material.subject)
        .map_or(0, |v| v.revision);
    let aad = task.material_aad(&material.subject, expected_revision)?;
    let aead = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_256_GCM, key.as_ref()).map_err(|_| Error::Protocol)?,
    );
    let mut ciphertext = material.secret.as_bytes().to_vec();
    aead.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(aad),
        &mut ciphertext,
    )
    .map_err(|_| Error::Protocol)?;
    Ok(w::CredentialEnvelope {
        subject: material.subject.clone(),
        expected_revision,
        wrapped_key: STANDARD.encode(wrapped),
        nonce,
        ciphertext: STANDARD.encode(ciphertext),
    })
}
#[cfg(any(windows, test))]
pub(super) fn password(bitlocker: bool) -> Result<Zeroizing<String>, Error> {
    let mut random = Zeroizing::new([0u8; 32]);
    SystemRandom::new()
        .fill(random.as_mut())
        .map_err(|_| Error::Unavailable)?;
    if bitlocker {
        Ok(Zeroizing::new(
            random[..16]
                .chunks_exact(2)
                .map(|v| format!("{:06}", u32::from(u16::from_le_bytes([v[0], v[1]])) * 11))
                .collect::<Vec<_>>()
                .join("-"),
        ))
    } else {
        // Fixed prefix satisfies Windows complexity; 256 random bits remain in the suffix.
        Ok(Zeroizing::new(format!(
            "Aa9!{}",
            STANDARD.encode(random.as_ref())
        )))
    }
}
