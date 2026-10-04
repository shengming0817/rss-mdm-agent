use crate::{Error, IdentityProvider};
use agent_api::{enroll, runtime};
use serde::{Deserialize, Serialize};
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityState {
    pub active: Option<Active>,
    pub pending: Option<Pending>,
    pub failure: Option<enroll::ErrorCode>,
}
impl IdentityState {
    pub(crate) fn check_runtime_failure(&self) -> Result<(), Error> {
        if let Some(code) = self.failure.filter(|code| blocks_runtime(*code)) {
            return Err(enrollment_error(code));
        }
        Ok(())
    }
    pub(crate) fn active_identity(&self, now: i64) -> Result<&Active, Error> {
        self.check_runtime_failure()?;
        let active = self.active.as_ref().ok_or(Error::Identity)?;
        agent_api::validate(&active.receipt).map_err(|_| Error::Protocol)?;
        if now < active.not_before {
            return Err(Error::Clock);
        }
        if now >= active.not_after {
            return Err(Error::CertificateExpired);
        }
        Ok(active)
    }
    pub(crate) fn validate_runtime(
        &self,
        cfg: &crate::Config,
        provider: &impl IdentityProvider,
        now: i64,
        trust: &crate::trust::VerifiedTrust,
    ) -> Result<&runtime::BindingProfile, Error> {
        let active = self.active_identity(now)?;
        let (before, after, device) = crate::tls::validity(
            cfg,
            &active.chain,
            &active.key_reference,
            provider,
            now,
            trust,
        )?;
        if before != active.not_before
            || after != active.not_after
            || device != active.receipt.runtime.device_id
        {
            return Err(Error::Identity);
        }
        Ok(&active.receipt.runtime)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Active {
    pub receipt: enroll::ActivationReceipt,
    pub key_reference: String,
    pub secret_reference: String,
    pub chain: Vec<String>,
    pub not_before: i64,
    pub not_after: i64,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Purpose {
    Authorization,
    Import,
    Renewal,
    Recovery,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pending {
    pub purpose: Purpose,
    pub operation: uuid::Uuid,
    pub certificate_operation: uuid::Uuid,
    pub activation_operation: uuid::Uuid,
    pub key_reference: String,
    pub secret_reference: String,
    pub csr: Option<String>,
    pub profile: enroll::RuntimeProfile,
    pub grant: Option<enroll::GrantReceipt>,
    pub candidate: Option<enroll::Candidate>,
    pub failure: Option<enroll::ErrorCode>,
    pub expected: Option<runtime::BindingProfile>,
}
pub(crate) fn blocks_runtime(code: enroll::ErrorCode) -> bool {
    matches!(
        code,
        enroll::ErrorCode::CredentialRevoked
            | enroll::ErrorCode::CredentialRetired
            | enroll::ErrorCode::CertificateExpired
    )
}
pub(crate) fn enrollment_error(code: enroll::ErrorCode) -> Error {
    use enroll::ErrorCode as C;
    match code {
        C::CredentialRevoked => Error::CredentialRevoked,
        C::CredentialRetired => Error::CredentialRetired,
        C::CertificateExpired => Error::CertificateExpired,
        C::IssuanceUnknown => Error::IssuanceUnknown,
        C::ActivationUnknown => Error::ActivationUnknown,
        C::GrantExpired => Error::Expired,
        C::GrantRevoked | C::PermissionDenied => Error::Denied,
        C::OperationConflict => Error::Conflict,
        C::RegistrationLimit => Error::Capacity,
        C::ServiceUnavailable => Error::Unavailable,
        C::InvalidIdentity => Error::Identity,
        C::MalformedRequest => Error::Protocol,
    }
}
/// Encode the fixed producer-compatible RSA PKCS#10 request using the platform signer.
pub(crate) fn csr(provider: &impl IdentityProvider, reference: &str) -> Result<String, Error> {
    use base64::Engine;
    use der::{Decode, Encode};
    let key = provider.key(reference)?;
    let public = key.public_key().ok_or(Error::Identity)?;
    let info = x509_cert::request::CertReqInfo {
        version: x509_cert::request::Version::V1,
        subject: Default::default(),
        public_key: spki::SubjectPublicKeyInfoOwned::from_der(public.as_ref())
            .map_err(|_| Error::Identity)?,
        attributes: Default::default(),
    };
    let bytes = info.to_der().map_err(|_| Error::Protocol)?;
    let signer = key
        .choose_scheme(&[rustls::SignatureScheme::RSA_PKCS1_SHA256])
        .ok_or(Error::Unsupported)?;
    let signature = signer.sign(&bytes).map_err(|_| Error::Identity)?;
    let request = x509_cert::request::CertReq {
        info,
        algorithm: spki::AlgorithmIdentifierOwned {
            oid: der::asn1::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.11"),
            parameters: Some(der::asn1::Any::null()),
        },
        signature: der::asn1::BitString::from_bytes(&signature).map_err(|_| Error::Protocol)?,
    };
    Ok(base64::engine::general_purpose::STANDARD
        .encode(request.to_der().map_err(|_| Error::Protocol)?))
}
