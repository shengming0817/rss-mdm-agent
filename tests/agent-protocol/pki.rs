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
    certificate_until(issuer, subject, public, extensions, 4_102_444_800)
}
fn certificate_until(
    issuer: &str,
    subject: &str,
    public: Vec<u8>,
    extensions: Vec<Extension>,
    until: u64,
) -> Certificate {
    certificate_signed(
        signing_key(ROOT),
        issuer,
        subject,
        public,
        extensions,
        until,
        1,
    )
}
fn certificate_signed(
    root: Arc<dyn SigningKey>,
    issuer: &str,
    subject: &str,
    public: Vec<u8>,
    extensions: Vec<Extension>,
    until: u64,
    serial: u8,
) -> Certificate {
    let algorithm = spki::AlgorithmIdentifierOwned {
        oid: der::asn1::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.11"),
        parameters: Some(der::asn1::Any::null()),
    };
    let tbs = TbsCertificate {
        version: Version::V3,
        serial_number: SerialNumber::new(&[serial]).unwrap(),
        signature: algorithm.clone(),
        issuer: issuer.parse().unwrap(),
        subject: subject.parse().unwrap(),
        subject_public_key_info: spki::SubjectPublicKeyInfoOwned::from_der(&public).unwrap(),
        validity: Validity {
            not_before: Time::try_from(std::time::UNIX_EPOCH).unwrap(),
            not_after: Time::try_from(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(until),
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
            extension(KeyUsage(KeyUsages::KeyCertSign | KeyUsages::CRLSign), true),
        ],
    )
    .to_pem(der::pem::LineEnding::LF)
    .unwrap()
    .into_bytes()
}
pub fn chain(tenant: uuid::Uuid) -> Vec<String> {
    chain_for(tenant, key().public_key().unwrap().as_ref().to_vec())
}

pub fn crl(number: u64, this_update: u64, next_update: u64, revoked: &[u8]) -> String {
    crl_signed(
        signing_key(ROOT),
        "CN=Agent fixture CA",
        number,
        this_update,
        next_update,
        revoked,
    )
}
fn crl_signed(
    root: Arc<dyn SigningKey>,
    issuer: &str,
    number: u64,
    this_update: u64,
    next_update: u64,
    revoked: &[u8],
) -> String {
    use x509_cert::{
        crl::{CertificateList, RevokedCert, TbsCertList},
        ext::pkix::crl::CrlNumber,
    };
    let algorithm = spki::AlgorithmIdentifierOwned {
        oid: der::asn1::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.11"),
        parameters: Some(der::asn1::Any::null()),
    };
    let time = |value| {
        Time::try_from(std::time::UNIX_EPOCH + std::time::Duration::from_secs(value)).unwrap()
    };
    let tbs = TbsCertList {
        version: Version::V2,
        signature: algorithm.clone(),
        issuer: issuer.parse().unwrap(),
        this_update: time(this_update),
        next_update: Some(time(next_update)),
        revoked_certificates: (!revoked.is_empty()).then(|| {
            revoked
                .iter()
                .map(|serial| RevokedCert {
                    serial_number: SerialNumber::new(&[*serial]).unwrap(),
                    revocation_date: time(this_update),
                    crl_entry_extensions: None,
                })
                .collect()
        }),
        crl_extensions: Some(vec![extension(
            CrlNumber(der::asn1::Uint::new(&number.to_be_bytes()).unwrap()),
            false,
        )]),
    };
    let signature = root
        .choose_scheme(&[rustls::SignatureScheme::RSA_PKCS1_SHA256])
        .unwrap()
        .sign(&tbs.to_der().unwrap())
        .unwrap();
    base64::engine::general_purpose::STANDARD.encode(
        CertificateList {
            tbs_cert_list: tbs,
            signature_algorithm: algorithm,
            signature: der::asn1::BitString::from_bytes(&signature).unwrap(),
        }
        .to_der()
        .unwrap(),
    )
}

pub fn trust(origin: &str, tenant: uuid::Uuid) -> serde_json::Value {
    use sha2::{Digest, Sha256};
    let ca = Certificate::from_pem(ca()).unwrap().to_der().unwrap();
    let certificate = base64::engine::general_purpose::STANDARD.encode(&ca);
    let crl = crl(1, 0, 4_102_444_800, &[]);
    serde_json::json!({ "version":1, "origin":origin.trim_end_matches('/'), "tenant":tenant, "purpose":"agent", "revision":1, "issuers":[{
        "digest":format!("{:x}", Sha256::digest(&ca)), "mode":"active", "certificate":certificate, "root":certificate,
        "crl":crl, "root_crl":crl, "provisioner":"rss-agent", "kid":"fixture-agent-key"
    }] })
}
pub fn chain_for(tenant: uuid::Uuid, public: Vec<u8>) -> Vec<String> {
    chain_until(tenant, public, 4_070_908_800)
}
pub fn chain_until(tenant: uuid::Uuid, public: Vec<u8>, until: u64) -> Vec<String> {
    let uri = format!(
        "urn:rss-mdm:agent:v1:{tenant}:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("device-1")
    );
    let certificate = certificate_until(
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
        until,
    );
    vec![base64::engine::general_purpose::STANDARD.encode(certificate.to_der().unwrap())]
}

fn rotation_material() -> (Certificate, Certificate) {
    let root_key = renewal_key();
    let extensions = |depth| {
        vec![
            extension(
                BasicConstraints {
                    ca: true,
                    path_len_constraint: Some(depth),
                },
                true,
            ),
            extension(KeyUsage(KeyUsages::KeyCertSign | KeyUsages::CRLSign), true),
        ]
    };
    let root = certificate_signed(
        root_key.clone(),
        "CN=Agent rotation root",
        "CN=Agent rotation root",
        root_key.public_key().unwrap().as_ref().to_vec(),
        extensions(1),
        4_102_444_800,
        8,
    );
    let issuer = certificate_signed(
        root_key,
        "CN=Agent rotation root",
        "CN=Agent rotation issuer",
        key().public_key().unwrap().as_ref().to_vec(),
        extensions(0),
        4_102_444_800,
        7,
    );
    (root, issuer)
}
pub fn rotation_issuer(root_revoked: bool) -> serde_json::Value {
    use sha2::{Digest, Sha256};
    let (root, issuer) = rotation_material();
    let der = issuer.to_der().unwrap();
    serde_json::json!({ "digest":format!("{:x}",Sha256::digest(&der)), "mode":"prepared", "certificate":base64::engine::general_purpose::STANDARD.encode(der),
        "root":base64::engine::general_purpose::STANDARD.encode(root.to_der().unwrap()),
        "crl":crl_signed(key(), "CN=Agent rotation issuer", 1, 0, 4_102_444_800, &[]),
        "root_crl":crl_signed(renewal_key(), "CN=Agent rotation root", if root_revoked {2} else {1}, 0, 4_102_444_800, if root_revoked {&[7]} else {&[]}),
        "provisioner":"rss-agent","kid":"fixture-rotation" })
}
pub fn rotation_chain(tenant: uuid::Uuid, public: Vec<u8>, until: u64) -> Vec<String> {
    let mut leaf = Certificate::from_der(
        &base64::engine::general_purpose::STANDARD
            .decode(chain_until(tenant, public, until)[0].as_bytes())
            .unwrap(),
    )
    .unwrap();
    leaf.tbs_certificate.issuer = "CN=Agent rotation issuer".parse().unwrap();
    let signature = key()
        .choose_scheme(&[rustls::SignatureScheme::RSA_PKCS1_SHA256])
        .unwrap()
        .sign(&leaf.tbs_certificate.to_der().unwrap())
        .unwrap();
    leaf.signature = der::asn1::BitString::from_bytes(&signature).unwrap();
    let (_, issuer) = rotation_material();
    vec![
        base64::engine::general_purpose::STANDARD.encode(leaf.to_der().unwrap()),
        base64::engine::general_purpose::STANDARD.encode(issuer.to_der().unwrap()),
    ]
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
    roots
        .add(rustls::pki_types::CertificateDer::from(
            rotation_material().0.to_der().unwrap(),
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

pub fn bootstrap_chain(installation: uuid::Uuid, attempt: uuid::Uuid) -> Vec<Vec<u8>> {
    let certificate = certificate_until(
        "CN=Agent fixture CA",
        &format!(
            "OU=rss-mdm-agent-bootstrap,CN={}{}",
            installation.simple(),
            attempt.simple()
        ),
        renewal_key().public_key().unwrap().as_ref().to_vec(),
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
        ],
        4_070_908_800,
    );
    vec![certificate.to_der().unwrap()]
}
