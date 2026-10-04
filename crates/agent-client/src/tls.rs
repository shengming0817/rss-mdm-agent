use crate::{Config, Error, IdentityProvider, Transport};
use base64::Engine;
use der::{Decode, DecodePem, Encode};
use rustls::{pki_types::CertificateDer, sign::CertifiedKey};
use std::sync::Arc;

pub(crate) fn certificates(chain: &[String]) -> Result<Vec<CertificateDer<'static>>, Error> {
    if chain.is_empty() || chain.len() > 8 {
        return Err(Error::Protocol);
    }
    chain
        .iter()
        .map(|value| {
            if value.len() > 65536 {
                return Err(Error::Capacity);
            }
            base64::engine::general_purpose::STANDARD
                .decode(value)
                .map(CertificateDer::from)
                .map_err(|_| Error::Protocol)
        })
        .collect()
}
#[derive(Debug)]
struct Resolver(Arc<CertifiedKey>);
impl rustls::client::ResolvesClientCert for Resolver {
    fn resolve(
        &self,
        _: &[&[u8]],
        schemes: &[rustls::SignatureScheme],
    ) -> Option<Arc<CertifiedKey>> {
        self.0.key.choose_scheme(schemes).map(|_| self.0.clone())
    }
    fn has_certs(&self) -> bool {
        true
    }
}
pub(crate) fn client(
    cfg: &Config,
    identity: Option<(&String, &[String])>,
    provider: &impl IdentityProvider,
) -> Result<reqwest::Client, Error> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(cfg.limits.connect_timeout)
        .read_timeout(cfg.limits.read_timeout)
        .https_only(cfg.transport == Transport::Https);
    let crypto = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = match &cfg.ca_pem {
        Some(pem) => {
            let text = std::str::from_utf8(pem).map_err(|_| Error::Configuration)?;
            let certificate =
                x509_cert::Certificate::from_pem(text).map_err(|_| Error::Configuration)?;
            rustls_platform_verifier::Verifier::new_with_extra_roots(
                vec![CertificateDer::from(
                    certificate.to_der().map_err(|_| Error::Configuration)?,
                )],
                crypto.clone(),
            )
        }
        None => rustls_platform_verifier::Verifier::new(crypto.clone()),
    }
    .map_err(|_| Error::Configuration)?;
    let config = rustls::ClientConfig::builder_with_provider(crypto)
        .with_safe_default_protocol_versions()
        .map_err(|_| Error::Configuration)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier));
    let config = match identity {
        Some((reference, chain)) => {
            let key = Arc::new(CertifiedKey::new(
                certificates(chain)?,
                provider.key(reference)?,
            ));
            key.keys_match().map_err(|_| Error::Identity)?;
            config.with_client_cert_resolver(Arc::new(Resolver(key)))
        }
        None => config.with_no_client_auth(),
    };
    // ref: reqwest v0.13.5 src/async_impl/client.rs (ClientConfig Any downcast).
    builder = builder.tls_backend_preconfigured(config);
    builder.build().map_err(|_| Error::Configuration)
}
pub(crate) fn validity(
    cfg: &Config,
    chain: &[String],
    reference: &str,
    provider: &impl IdentityProvider,
    now: i64,
) -> Result<(i64, i64, String), Error> {
    use x509_cert::ext::pkix::{
        name::GeneralName, BasicConstraints, ExtendedKeyUsage, KeyUsage, KeyUsages, SubjectAltName,
    };
    if now < 0 {
        return Err(Error::Clock);
    }
    let certificates = certificates(chain)?;
    let certified = CertifiedKey::new(certificates.clone(), provider.key(reference)?);
    certified.keys_match().map_err(|_| Error::Identity)?;
    let leaf =
        x509_cert::Certificate::from_der(certificates[0].as_ref()).map_err(|_| Error::Protocol)?;
    let t = &leaf.tbs_certificate;
    let before = i64::try_from(t.validity.not_before.to_unix_duration().as_secs())
        .map_err(|_| Error::Protocol)?;
    let after = i64::try_from(t.validity.not_after.to_unix_duration().as_secs())
        .map_err(|_| Error::Protocol)?;
    if now < before {
        return Err(Error::Clock);
    }
    if now >= after {
        return Err(Error::CertificateExpired);
    }
    let bc = t
        .get::<BasicConstraints>()
        .map_err(|_| Error::Protocol)?
        .ok_or(Error::Protocol)?;
    let ku = t
        .get::<KeyUsage>()
        .map_err(|_| Error::Protocol)?
        .ok_or(Error::Protocol)?;
    let eku = t
        .get::<ExtendedKeyUsage>()
        .map_err(|_| Error::Protocol)?
        .ok_or(Error::Protocol)?;
    if bc.1.ca
        || bc.1.path_len_constraint.is_some()
        || ku.1 != KeyUsage(KeyUsages::DigitalSignature.into())
        || eku.1 .0 != vec![der::asn1::ObjectIdentifier::new_unwrap("1.3.6.1.5.5.7.3.2")]
        || t.subject.to_string() != "CN=rss-mdm-agent"
        || t.subject_public_key_info.algorithm.oid
            != der::asn1::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1")
    {
        return Err(Error::Untrusted);
    }
    let issuer =
        x509_cert::Certificate::from_pem(&cfg.device_ca_pem).map_err(|_| Error::Configuration)?;
    if t.issuer != issuer.tbs_certificate.subject {
        return Err(Error::Untrusted);
    }
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from(
            issuer.to_der().map_err(|_| Error::Configuration)?,
        ))
        .map_err(|_| Error::Configuration)?;
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots),
        Arc::new(rustls::crypto::ring::default_provider()),
    )
    .build()
    .map_err(|_| Error::Configuration)?;
    // Trust only the configured direct Agent issuer; supplied chain members cannot delegate it.
    verifier
        .verify_client_cert(
            &certificates[0],
            &[],
            rustls::pki_types::UnixTime::since_unix_epoch(std::time::Duration::from_secs(
                now as u64,
            )),
        )
        .map_err(|_| Error::Untrusted)?;
    let (_, san) = t
        .get::<SubjectAltName>()
        .map_err(|_| Error::Protocol)?
        .ok_or(Error::Untrusted)?;
    let identity = match san.0.as_slice() {
        [GeneralName::UniformResourceIdentifier(uri)] => uri.as_str(),
        _ => return Err(Error::Untrusted),
    };
    let prefix = format!("urn:rss-mdm:agent:v1:{}:", cfg.tenant);
    let encoded = identity.strip_prefix(&prefix).ok_or(Error::Untrusted)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| Error::Untrusted)?;
    let device = String::from_utf8(bytes).map_err(|_| Error::Untrusted)?;
    if device.is_empty()
        || device.len() > 256
        || device.chars().any(char::is_control)
        || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&device) != encoded
    {
        return Err(Error::Untrusted);
    }
    Ok((before, after, device))
}
