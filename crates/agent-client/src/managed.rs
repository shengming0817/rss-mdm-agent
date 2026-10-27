//! MDM-installed identities only bootstrap the existing Agent registration owner.
use crate::{identity::Purpose, Client, Clock, Error, IdentityProvider};
use der::Decode;
use rss_mdm_agent_wire::{enroll, runtime};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

/// An externally owned native certificate and opaque signing handle. Never a daily Agent key.
pub struct NativeIdentity {
    /// DER certificate chain, leaf first.
    pub chain: Vec<Vec<u8>>,
    /// Native signing handle; private material cannot be exported through this interface.
    pub key: Arc<dyn rustls::sign::SigningKey>,
}
/// Read the producer's bootstrap purpose and installation/attempt coordinates from X.509.
/// This identifies a candidate; only the server's certificate and installation records authorize it.
pub fn bootstrap_coordinates(der: &[u8]) -> Result<(Uuid, Uuid), Error> {
    use der::{Tag, Tagged};
    let certificate = x509_cert::Certificate::from_der(der).map_err(|_| Error::Protocol)?;
    let subject = &certificate.tbs_certificate.subject;
    if subject.0.len() != 2 {
        return Err(Error::Untrusted);
    }
    let mut common = None;
    let mut purpose = false;
    for rdn in &subject.0 {
        if rdn.0.len() != 1 {
            return Err(Error::Untrusted);
        }
        let attribute = rdn.0.iter().next().ok_or(Error::Untrusted)?;
        if !matches!(
            attribute.value.tag(),
            Tag::Utf8String | Tag::PrintableString
        ) {
            return Err(Error::Untrusted);
        }
        match attribute.oid.to_string().as_str() {
            "2.5.4.11" if attribute.value.value() == b"rss-mdm-agent-bootstrap" => purpose = true,
            "2.5.4.3" => {
                common = Some(
                    std::str::from_utf8(attribute.value.value()).map_err(|_| Error::Untrusted)?,
                )
            }
            _ => return Err(Error::Untrusted),
        }
    }
    let common = common.ok_or(Error::Untrusted)?;
    if !purpose
        || common.len() != 64
        || !common
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Untrusted);
    }
    let installation = Uuid::parse_str(&common[..32]).map_err(|_| Error::Untrusted)?;
    let attempt = Uuid::parse_str(&common[32..]).map_err(|_| Error::Untrusted)?;
    if installation.is_nil() || attempt.is_nil() {
        return Err(Error::Untrusted);
    }
    Ok((installation, attempt))
}
fn valid(identity: &NativeIdentity, now: i64) -> Result<(Uuid, Uuid, [u8; 32]), Error> {
    let leaf = identity.chain.first().ok_or(Error::Identity)?;
    let coordinates = bootstrap_coordinates(leaf)?;
    let cert = x509_cert::Certificate::from_der(leaf).map_err(|_| Error::Protocol)?;
    let validity = cert.tbs_certificate.validity;
    if now < 0
        || validity.not_before.to_unix_duration().as_secs() > now as u64
        || validity.not_after.to_unix_duration().as_secs() <= now as u64
    {
        return Err(Error::Expired);
    }
    Ok((coordinates.0, coordinates.1, Sha256::digest(leaf).into()))
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Discover/resume managed installation without replacing explicit registration or active identity.
    pub fn prepare_managed_enrollment(
        &mut self,
        capabilities: Vec<runtime::Capability>,
    ) -> Result<bool, Error> {
        let mut state = self.identity_state()?;
        if state.active.is_some() {
            return Ok(false);
        }
        let installation = match state.pending.as_ref() {
            Some(p) => match p.purpose {
                Purpose::Managed { installation, .. } if p.grant.is_none() => Some(installation),
                _ => return Ok(false),
            },
            None => None,
        };
        let now = self.clock.now()?;
        let candidates = self
            .secrets
            .native_identities()?
            .into_iter()
            .filter_map(|identity| valid(&identity, now).ok())
            .filter(|(id, _, _)| installation.is_none_or(|expected| expected == *id))
            .collect::<Vec<_>>();
        let [(installation, bootstrap, fingerprint)] = candidates.as_slice() else {
            return if candidates.is_empty() {
                Ok(false)
            } else {
                Err(Error::Conflict)
            };
        };
        let purpose = Purpose::Managed {
            installation: *installation,
            bootstrap: *bootstrap,
            fingerprint: *fingerprint,
        };
        if let Some(pending) = &mut state.pending {
            pending.purpose = purpose;
            self.store.put("identity", &state)?;
        }
        let operation = state
            .pending
            .as_ref()
            .map_or_else(Uuid::new_v4, |p| p.operation);
        self.prepare_enrollment(
            purpose,
            operation,
            format!("bootstrap-{operation}"),
            capabilities,
            None,
        )?;
        Ok(true)
    }
    pub(crate) async fn exchange_managed(
        &self,
        pending: &crate::identity::Pending,
    ) -> Result<enroll::GrantReceipt, Error> {
        let Purpose::Managed {
            installation,
            bootstrap,
            fingerprint,
        } = pending.purpose
        else {
            return Err(Error::Identity);
        };
        let identity = self
            .secrets
            .native_identities()?
            .into_iter()
            .find(|identity| {
                valid(identity, self.clock.now().unwrap_or(-1))
                    .is_ok_and(|value| value == (installation, bootstrap, fingerprint))
            })
            .ok_or(Error::Unavailable)?;
        let certified = Arc::new(rustls::sign::CertifiedKey::new(
            identity
                .chain
                .into_iter()
                .map(rustls::pki_types::CertificateDer::from)
                .collect(),
            identity.key,
        ));
        certified.keys_match().map_err(|_| Error::Identity)?;
        let http = crate::tls::client_identity(&self.store.cfg, Some(certified))?;
        self.enrollment_json::<crate::http::NativeGrant>(
            &http,
            &enroll::NativeGrantRequest {
                wire_version: enroll::API_VERSION,
                operation_id: pending.operation,
                installation_operation: installation,
                secret: self.secrets.resolve(&pending.secret_reference)?,
                csr: pending.csr.clone().ok_or(Error::Storage)?,
                runtime: pending.profile.clone(),
            },
        )
        .await
    }
}
