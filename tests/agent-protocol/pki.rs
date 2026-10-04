//! Disposable protocol fixture keys. These keys are public test assets, never platform credentials.
use base64::Engine;
use der::{Decode, DecodePem, Encode, EncodePem};
use rustls::sign::SigningKey;
use std::sync::Arc;
use x509_cert::{
    ext::{
        pkix::{
            name::GeneralName, BasicConstraints, ExtendedKeyUsage, KeyUsage, KeyUsages,
            SubjectAltName,
        },
        Extension,
    },
    serial_number::SerialNumber,
    time::{Time, Validity},
    Certificate, TbsCertificate, Version,
};
const ROOT: &[u8] = include_bytes!("pki/root-test-key.pk8");
const DEVICE: &[u8] = include_bytes!("pki/device-test-key.pk8");
pub fn key() -> Arc<dyn SigningKey> {
    signing_key(DEVICE)
}
pub fn renewal_key() -> Arc<dyn SigningKey> {
    signing_key(include_bytes!("pki/renewal-test-key.pk8"))
}
fn signing_key(bytes: &[u8]) -> Arc<dyn SigningKey> {
    rustls::crypto::ring::sign::any_supported_type(&rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(bytes.to_vec()),
    ))
    .unwrap()
}
fn extension<T: Encode + der::oid::AssociatedOid>(value: T, critical: bool) -> Extension {
    Extension {
        extn_id: T::OID,
        critical,
        extn_value: der::asn1::OctetString::new(value.to_der().unwrap()).unwrap(),
    }
}
fn certificate(
    issuer: &str,
    subject: &str,
    public: Vec<u8>,
    extensions: Vec<Extension>,
) -> Certificate {
    let root = signing_key(ROOT);
    let algorithm = spki::AlgorithmIdentifierOwned {
        oid: der::asn1::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.11"),
        parameters: Some(der::asn1::Any::null()),
    };
    let tbs = TbsCertificate {
        version: Version::V3,
        serial_number: SerialNumber::new(&[1]).unwrap(),
        signature: algorithm.clone(),
        issuer: issuer.parse().unwrap(),
        subject: subject.parse().unwrap(),
        subject_public_key_info: spki::SubjectPublicKeyInfoOwned::from_der(&public).unwrap(),
        validity: Validity {
            not_before: Time::try_from(std::time::UNIX_EPOCH).unwrap(),
            not_after: Time::try_from(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(4_070_908_800),
            )
            .unwrap(),
        },
        issuer_unique_id: None,
        subject_unique_id: None,
        extensions: Some(extensions),
    };
    let signature = root
        .choose_scheme(&[rustls::SignatureScheme::RSA_PKCS1_SHA256])
        .unwrap()
        .sign(&tbs.to_der().unwrap())
        .unwrap();
    Certificate {
        tbs_certificate: tbs,
        signature_algorithm: algorithm,
        signature: der::asn1::BitString::from_bytes(&signature).unwrap(),
    }
}
pub fn ca() -> Vec<u8> {
    certificate(
        "CN=Agent fixture CA",
        "CN=Agent fixture CA",
        signing_key(ROOT).public_key().unwrap().as_ref().to_vec(),
        vec![
            extension(
                BasicConstraints {
                    ca: true,
                    path_len_constraint: Some(0),
                },
                true,
            ),
            extension(KeyUsage(KeyUsages::KeyCertSign.into()), true),
        ],
    )
    .to_pem(der::pem::LineEnding::LF)
    .unwrap()
    .into_bytes()
}
pub fn chain(tenant: uuid::Uuid) -> Vec<String> {
    chain_for(tenant, key().public_key().unwrap().as_ref().to_vec())
}
pub fn chain_for(tenant: uuid::Uuid, public: Vec<u8>) -> Vec<String> {
    let uri = format!(
        "urn:rss-mdm:agent:v1:{tenant}:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("device-1")
    );
    let certificate = certificate(
        "CN=Agent fixture CA",
        "CN=rss-mdm-agent",
        public,
        vec![
            extension(
                BasicConstraints {
                    ca: false,
                    path_len_constraint: None,
                },
                true,
            ),
            extension(KeyUsage(KeyUsages::DigitalSignature.into()), true),
            extension(
                ExtendedKeyUsage(vec![der::asn1::ObjectIdentifier::new_unwrap(
                    "1.3.6.1.5.5.7.3.2",
                )]),
                false,
            ),
            extension(
                SubjectAltName(vec![GeneralName::UniformResourceIdentifier(
                    der::asn1::Ia5String::new(&uri).unwrap(),
                )]),
                false,
            ),
        ],
    );
    vec![base64::engine::general_purpose::STANDARD.encode(certificate.to_der().unwrap())]
}

pub fn server() -> rustls::ServerConfig {
    let certificate = certificate(
        "CN=Agent fixture CA",
        "CN=localhost",
        key().public_key().unwrap().as_ref().to_vec(),
        vec![
            extension(
                BasicConstraints {
                    ca: false,
                    path_len_constraint: None,
                },
                true,
            ),
            extension(KeyUsage(KeyUsages::DigitalSignature.into()), true),
            extension(
                ExtendedKeyUsage(vec![der::asn1::ObjectIdentifier::new_unwrap(
                    "1.3.6.1.5.5.7.3.1",
                )]),
                false,
            ),
            extension(
                SubjectAltName(vec![
                    GeneralName::DnsName(der::asn1::Ia5String::new("localhost").unwrap()),
                    GeneralName::IpAddress(
                        der::asn1::OctetString::new(vec![127, 0, 0, 1]).unwrap(),
                    ),
                ]),
                false,
            ),
        ],
    );
    let issuer = x509_cert::Certificate::from_pem(ca()).unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(
            issuer.to_der().unwrap(),
        ))
        .unwrap();
    let crypto = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots),
        crypto.clone(),
    )
    .allow_unauthenticated()
    .build()
    .unwrap();
    rustls::ServerConfig::builder_with_provider(crypto)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(
                certificate.to_der().unwrap(),
            )],
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                DEVICE.to_vec(),
            )),
        )
        .unwrap()
}
