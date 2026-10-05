//! Consumer verification uses mature X.509 signature and TLS path validation.
use crate::{store::hash, Config, Error};
use base64::Engine;
use rss_mdm_agent_wire::pki::{Mode, Purpose, Trust};
use rustls::pki_types::{CertificateDer, CertificateRevocationListDer, UnixTime};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
use x509_parser::{prelude::*, revocation_list::CertificateRevocationList};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Watermark {
    number: Vec<u8>,
    this_update: i64,
    digest: String,
}

struct Crl {
    watermark: Watermark,
    expires: i64,
    revoked: BTreeSet<Vec<u8>>,
    der: Vec<u8>,
}

struct Generation {
    issuer: Vec<u8>,
    root: Vec<u8>,
    crl: Crl,
    root_crl: Crl,
    verifier: Arc<dyn rustls::server::danger::ClientCertVerifier>,
}

pub(crate) struct VerifiedTrust {
    pub document: Trust,
    generations: Vec<Generation>,
    pub watermarks: BTreeMap<String, Watermark>,
    pub expires: i64,
}

fn bytes(text: &str, max: usize) -> Result<Vec<u8>, Error> {
    if text.len() > max.saturating_mul(4).div_ceil(3) + 4 {
        return Err(Error::Capacity);
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| Error::Protocol)?;
    if bytes.is_empty()
        || bytes.len() > max
        || base64::engine::general_purpose::STANDARD.encode(&bytes) != text
    {
        return Err(Error::Protocol);
    }
    Ok(bytes)
}

fn strong_algorithm(algorithm: &x509_parser::x509::AlgorithmIdentifier<'_>) -> Result<(), Error> {
    match algorithm.algorithm.to_id_string().as_str() {
        "1.2.840.113549.1.1.11"
        | "1.2.840.113549.1.1.12"
        | "1.2.840.113549.1.1.13"
        | "1.2.840.113549.1.1.10"
        | "1.2.840.10045.4.3.2"
        | "1.2.840.10045.4.3.3"
        | "1.3.101.112" => Ok(()),
        _ => Err(Error::Untrusted),
    }
}

fn certificate(der: &[u8]) -> Result<X509Certificate<'_>, Error> {
    let (remaining, certificate) = X509Certificate::from_der(der).map_err(|_| Error::Untrusted)?;
    if !remaining.is_empty()
        || certificate.signature_algorithm != certificate.tbs_certificate.signature
    {
        return Err(Error::Untrusted);
    }
    strong_algorithm(&certificate.signature_algorithm)?;
    certificate.extensions_map().map_err(|_| Error::Untrusted)?;
    Ok(certificate)
}

fn ca(certificate: &X509Certificate<'_>) -> Result<(), Error> {
    let bc = certificate
        .basic_constraints()
        .map_err(|_| Error::Untrusted)?
        .ok_or(Error::Untrusted)?;
    let ku = certificate
        .key_usage()
        .map_err(|_| Error::Untrusted)?
        .ok_or(Error::Untrusted)?;
    if !bc.critical
        || !bc.value.ca
        || !ku.critical
        || !ku.value.key_cert_sign()
        || !ku.value.crl_sign()
    {
        return Err(Error::Untrusted);
    }
    for e in certificate.extensions() {
        if matches!(e.parsed_extension(), ParsedExtension::ParseError { .. }) {
            return Err(Error::Untrusted);
        }
        if e.critical
            && !matches!(
                e.parsed_extension(),
                ParsedExtension::BasicConstraints(_)
                    | ParsedExtension::KeyUsage(_)
                    | ParsedExtension::ExtendedKeyUsage(_)
                    | ParsedExtension::SubjectKeyIdentifier(_)
                    | ParsedExtension::AuthorityKeyIdentifier(_)
            )
        {
            return Err(Error::Untrusted);
        }
    }
    Ok(())
}

fn valid_at(certificate: &X509Certificate<'_>, now: i64) -> Result<(), Error> {
    if now < certificate.validity().not_before.timestamp()
        || now >= certificate.validity().not_after.timestamp()
    {
        return Err(Error::Untrusted);
    }
    Ok(())
}

fn serial(value: &[u8]) -> Vec<u8> {
    let first = value
        .iter()
        .position(|b| *b != 0)
        .unwrap_or(value.len().saturating_sub(1));
    value[first..].to_vec()
}

fn crl(der: Vec<u8>, signer: &X509Certificate<'_>, now: i64) -> Result<Crl, Error> {
    let (remaining, value) =
        CertificateRevocationList::from_der(&der).map_err(|_| Error::Untrusted)?;
    if !remaining.is_empty()
        || value.signature_algorithm != value.tbs_cert_list.signature
        || value.issuer().as_raw() != signer.subject().as_raw()
    {
        return Err(Error::Untrusted);
    }
    strong_algorithm(&value.signature_algorithm)?;
    // ref: x509-parser 0.18.1 (33b15d2db5a19b15c17bb15fa57b08691316ee95)
    // src/revocation_list.rs, src/verify.rs (ring verifier).
    value
        .verify_signature(signer.public_key())
        .map_err(|_| Error::Untrusted)?;
    let mut seen = BTreeSet::new();
    for e in value.extensions() {
        if !seen.insert(e.oid.to_id_string())
            || matches!(e.parsed_extension(), ParsedExtension::ParseError { .. })
            || e.critical
            || e.oid.to_id_string() == "2.5.29.27"
            || matches!(
                e.parsed_extension(),
                ParsedExtension::IssuingDistributionPoint(_)
            )
        {
            return Err(Error::Untrusted);
        }
    }
    let mut number = value.crl_number().ok_or(Error::Untrusted)?.to_bytes_be();
    if number.is_empty() {
        number.push(0);
    }
    if number.len() > 20 {
        return Err(Error::Untrusted);
    }
    let this_update = value.last_update().timestamp();
    let expires = value.next_update().ok_or(Error::Untrusted)?.timestamp();
    if this_update > now || this_update < 0 || expires <= this_update || now >= expires {
        return Err(Error::Untrusted);
    }
    let mut revoked = BTreeSet::new();
    for entry in value.iter_revoked_certificates() {
        if entry.extensions().iter().any(|e| {
            e.critical || matches!(e.parsed_extension(), ParsedExtension::ParseError { .. })
        }) {
            return Err(Error::Untrusted);
        }
        if !revoked.insert(serial(entry.raw_serial())) {
            return Err(Error::Untrusted);
        }
    }
    let watermark = Watermark {
        number,
        this_update,
        digest: hash(&der),
    };
    Ok(Crl {
        watermark,
        expires,
        revoked,
        der,
    })
}

impl VerifiedTrust {
    pub fn new(document: Trust, cfg: &Config, now: i64) -> Result<Self, Error> {
        if now < 0 {
            return Err(Error::Clock);
        }
        document.validate()?;
        if document.purpose != Purpose::Agent
            || document.origin != cfg.origin.as_str().trim_end_matches('/')
            || document.tenant != cfg.tenant
        {
            return Err(Error::Untrusted);
        }
        let mut generations = Vec::new();
        let mut seen = BTreeSet::new();
        let mut watermarks = BTreeMap::new();
        let mut expires = i64::MAX;
        for g in &document.issuers {
            if g.provisioner != "rss-agent" || g.kid.is_empty() || g.kid.len() > 128 {
                return Err(Error::Untrusted);
            }
            let issuer_der = bytes(&g.certificate, 32768)?;
            let root_der = bytes(&g.root, 32768)?;
            if hash(&issuer_der) != g.digest || !seen.insert(g.digest.clone()) {
                return Err(Error::Untrusted);
            }
            let issuer = certificate(&issuer_der)?;
            let root = certificate(&root_der)?;
            ca(&issuer)?;
            ca(&root)?;
            if g.mode != Mode::VerifyOnly {
                valid_at(&issuer, now)?;
                valid_at(&root, now)?;
            }
            for ca in [&issuer, &root] {
                let until = ca.validity().not_after.timestamp();
                if until > now {
                    expires = expires.min(until);
                }
            }
            if root.subject().as_raw() != root.issuer().as_raw()
                || issuer.issuer().as_raw() != root.subject().as_raw()
                || (issuer_der != root_der
                    && root
                        .basic_constraints()
                        .map_err(|_| Error::Untrusted)?
                        .ok_or(Error::Untrusted)?
                        .value
                        .path_len_constraint
                        == Some(0))
            {
                return Err(Error::Untrusted);
            }
            root.verify_signature(Some(root.public_key()))
                .map_err(|_| Error::Untrusted)?;
            issuer
                .verify_signature(Some(root.public_key()))
                .map_err(|_| Error::Untrusted)?;
            let issuer_crl = crl(bytes(&g.crl, 16 * 1024 * 1024)?, &issuer, now)?;
            let root_crl = crl(bytes(&g.root_crl, 16 * 1024 * 1024)?, &root, now)?;
            for (der, list) in [(&issuer_der, &issuer_crl), (&root_der, &root_crl)] {
                if watermarks
                    .insert(hash(der), list.watermark.clone())
                    .is_some_and(|old| old != list.watermark)
                {
                    return Err(Error::Conflict);
                }
                expires = expires.min(list.expires);
            }
            let mut roots = rustls::RootCertStore::empty();
            roots
                .add(CertificateDer::from(root_der.clone()))
                .map_err(|_| Error::Untrusted)?;
            let mut lists = vec![CertificateRevocationListDer::from(issuer_crl.der.clone())];
            if issuer_der != root_der {
                lists.push(CertificateRevocationListDer::from(root_crl.der.clone()));
            }
            let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
                Arc::new(roots),
                Arc::new(rustls::crypto::ring::default_provider()),
            )
            .with_crls(lists)
            .enforce_revocation_expiration()
            .build()
            .map_err(|_| Error::Untrusted)?;
            generations.push(Generation {
                issuer: issuer_der,
                root: root_der,
                crl: issuer_crl,
                root_crl,
                verifier,
            });
        }
        Ok(Self {
            document,
            generations,
            watermarks,
            expires,
        })
    }

    pub fn monotonic(
        &self,
        previous: &Trust,
        watermarks: &BTreeMap<String, Watermark>,
    ) -> Result<(), Error> {
        if self.document.revision < previous.revision
            || (self.document.revision == previous.revision
                && self.document.installation_digest() != previous.installation_digest())
        {
            return Err(Error::Conflict);
        }
        for (id, new) in &self.watermarks {
            if let Some(old) = watermarks.get(id) {
                let number = (new.number.len(), &new.number).cmp(&(old.number.len(), &old.number));
                if number.is_lt()
                    || new.this_update < old.this_update
                    || (number.is_eq() && new != old)
                {
                    return Err(Error::Conflict);
                }
            }
        }
        Ok(())
    }

    pub fn verify_leaf(&self, chain: &[CertificateDer<'static>], now: i64) -> Result<(), Error> {
        if now < 0 {
            return Err(Error::Clock);
        }
        let leaf = certificate(chain.first().ok_or(Error::Untrusted)?.as_ref())?;
        let mut revoked = false;
        for g in &self.generations {
            let issuer = certificate(&g.issuer)?;
            if leaf.issuer().as_raw() != issuer.subject().as_raw() {
                continue;
            }
            if leaf.verify_signature(Some(issuer.public_key())).is_err() {
                continue;
            }
            let root = certificate(&g.root)?;
            if g.crl.revoked.contains(&serial(leaf.raw_serial()))
                || (g.issuer != g.root && g.root_crl.revoked.contains(&serial(issuer.raw_serial())))
            {
                revoked = true;
                continue;
            }
            valid_at(&issuer, now)?;
            valid_at(&root, now)?;
            if now >= g.crl.expires || now >= g.root_crl.expires {
                return Err(Error::Untrusted);
            }
            let supplied = &chain[1..];
            if supplied.len() > 2
                || (supplied.first().is_some_and(|v| v.as_ref() != g.issuer)
                    || supplied.get(1).is_some_and(|v| v.as_ref() != g.root))
            {
                return Err(Error::Untrusted);
            }
            let intermediates = if g.issuer == g.root {
                Vec::new()
            } else {
                vec![CertificateDer::from(g.issuer.clone())]
            };
            if g.verifier
                .verify_client_cert(
                    &chain[0],
                    &intermediates,
                    UnixTime::since_unix_epoch(Duration::from_secs(now as u64)),
                )
                .is_ok()
            {
                return Ok(());
            }
        }
        Err(if revoked {
            Error::CredentialRevoked
        } else {
            Error::Untrusted
        })
    }

    pub fn certificates(&self) -> BTreeMap<String, (crate::MachineStore, Vec<u8>)> {
        let mut out = BTreeMap::new();
        for g in &self.generations {
            out.insert(
                format!("root/{}", hash(&g.root)),
                (crate::MachineStore::Root, g.root.clone()),
            );
            if g.issuer != g.root {
                out.insert(
                    format!("ca/{}", hash(&g.issuer)),
                    (crate::MachineStore::Intermediate, g.issuer.clone()),
                );
            }
        }
        out
    }
}
