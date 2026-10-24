//! PSSO decisions and public recovery metadata. Apple owns keys, tokens and system registration.
//! ref: Apple AuthenticationServices/ASAuthorizationProviderExtensionLoginManager.h (SDK 26.4)
#![forbid(unsafe_code)]
use rss_mdm_agent_wire::platform_sso as w;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Closed errors: no remote body, nonce, token or account password is formatted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("unsupported Platform SSO configuration")]
    Unsupported,
    #[error("Platform SSO identity mismatch")]
    Identity,
    #[error("Platform SSO operation expired")]
    Expired,
    #[error("invalid Platform SSO protocol")]
    Protocol,
}
/// Deployment and local-account coordinates, never accepted as remote authority.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub tenant: Uuid,
    pub agent_registration: Uuid,
    pub installation: Uuid,
    pub origin: String,
    pub kind: w::Kind,
    pub binding: Option<Uuid>,
    pub binding_generation: Option<i64>,
    pub generated_uid: Option<String>,
}
impl Context {
    pub fn validate(&self) -> Result<(), Error> {
        if self.agent_registration.is_nil() || self.installation.is_nil() {
            return Err(Error::Identity);
        }
        w::Endpoints::new(&self.origin, self.tenant).map_err(|_| Error::Identity)?;
        match self.kind {
            w::Kind::Device
                if self.binding.is_none()
                    && self.binding_generation.is_none()
                    && self.generated_uid.is_none() =>
            {
                Ok(())
            }
            w::Kind::User
                if self.binding.is_some_and(|v| !v.is_nil())
                    && self.binding_generation.is_some_and(|v| v > 0)
                    && self
                        .generated_uid
                        .as_deref()
                        .is_some_and(rss_mdm_agent_wire::runtime::generated_uid_valid) =>
            {
                Ok(())
            }
            _ => Err(Error::Identity),
        }
    }
    pub fn check_bootstrap(&self, response: &w::BootstrapResponse, now: i64) -> Result<(), Error> {
        self.validate()?;
        let expected = w::Endpoints::new(&self.origin, self.tenant).map_err(|_| Error::Identity)?;
        if response.id.is_nil()
            || response.source_id.is_nil()
            || response.client_id.is_nil()
            || response.source_version < 1
            || response.login_user_name != response.source_id.to_string()
            || response.nonce.is_empty()
            || response.nonce.len() > 256
            || response.endpoints.base != expected.base
            || response.endpoints.issuer != expected.issuer
            || response.endpoints.nonce != expected.nonce
            || response.endpoints.token != expected.token
            || response.endpoints.jwks != expected.jwks
        {
            return Err(Error::Identity);
        }
        if now <= 0
            || response.expires_at <= now
            || response.expires_at > now.saturating_add(w::NONCE_SECONDS)
        {
            return Err(Error::Expired);
        }
        Ok(())
    }
}
/// Public coordinates needed after a lost registration response. Contains no token or nonce.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registration {
    pub context: Context,
    pub source_id: Uuid,
    pub source_version: i64,
    pub client_id: Uuid,
    pub signing_key: w::PublicKey,
    pub encryption_key: w::PublicKey,
}
impl Registration {
    pub fn prepare(
        context: Context,
        response: &w::BootstrapResponse,
        signing_key: w::PublicKey,
        encryption_key: w::PublicKey,
        now: i64,
    ) -> Result<Self, Error> {
        context.check_bootstrap(response, now)?;
        signing_key.key_id().map_err(|_| Error::Protocol)?;
        encryption_key.key_id().map_err(|_| Error::Protocol)?;
        Ok(Self {
            context,
            source_id: response.source_id,
            source_version: response.source_version,
            client_id: response.client_id,
            signing_key,
            encryption_key,
        })
    }
    pub fn proof(&self, challenge: &w::BootstrapResponse, now: i64) -> Result<String, Error> {
        self.context.check_bootstrap(challenge, now)?;
        if self.source_id != challenge.source_id
            || self.source_version != challenge.source_version
            || self.client_id != challenge.client_id
        {
            return Err(Error::Identity);
        }
        w::RegistrationClaims {
            iss: self.context.agent_registration,
            aud: format!("{}/register", challenge.endpoints.base),
            challenge_id: challenge.id,
            source_id: self.source_id,
            source_version: self.source_version,
            nonce: challenge.nonce.clone(),
            encryption_key_id: self.encryption_key.key_id().map_err(|_| Error::Protocol)?,
            iat: now,
            exp: challenge
                .expires_at
                .min(now.saturating_add(w::NONCE_SECONDS)),
        }
        .signing_input(&self.signing_key)
        .map_err(|_| Error::Protocol)
    }
    pub fn accepts(&self, receipt: &w::RegistrationReceipt) -> bool {
        receipt.registered
            && receipt.source_id == self.source_id
            && receipt.source_version == self.source_version
            && receipt.kind == self.context.kind
            && !receipt.hardware_attested
            && receipt.registration_trust == w::RegistrationTrust::MdmBootstrapPop
    }
    pub fn recovered(&self, status: &w::Status) -> bool {
        let (registered, source) = match self.context.kind {
            w::Kind::Device => (status.device_registered, &status.device_source),
            w::Kind::User => (status.user_registered, &status.user_source),
        };
        registered
            && source.as_ref().is_some_and(|s| {
                !s.revoked
                    && s.id == self.source_id
                    && s.version == self.source_version
                    && s.signing_key.as_ref() == Some(&self.signing_key)
                    && s.encryption_key == self.encryption_key
            })
    }
    /// Accept only the configured browser completion URL, with no alternate host/query/fragment.
    pub fn browser_input(
        &self,
        url: &str,
        nonce: &str,
        access_token: &str,
        now: i64,
    ) -> Result<String, Error> {
        self.context.validate()?;
        let endpoints = w::Endpoints::new(&self.context.origin, self.context.tenant)
            .map_err(|_| Error::Identity)?;
        if self.context.kind != w::Kind::User
            || url != format!("{}/browser/complete", endpoints.base)
            || self.source_id.is_nil()
            || self.source_version < 1
            || now <= 0
            || nonce.is_empty()
            || nonce.len() > 256
            || access_token.len() != 43
        {
            return Err(Error::Identity);
        }
        w::BrowserClaims {
            iss: self.source_id,
            aud: format!("{}/login", self.context.origin),
            nonce: nonce.into(),
            source_version: self.source_version,
            access_token: access_token.into(),
            iat: now,
            exp: now.saturating_add(120),
        }
        .signing_input(&self.signing_key)
        .map_err(|_| Error::Protocol)
    }
}
/// Assemble a native ES256 signature; native adapters convert DER into 64-byte JOSE first.
pub fn signed(input: &str, signature: &[u8]) -> Result<String, Error> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    if signature.len() != 64 || input.len() > 65536 || input.split('.').count() != 2 {
        return Err(Error::Protocol);
    }
    Ok(format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature)))
}
/// Export a system-provided public point. Never accepts or creates a private key.
pub fn public_key(point: &[u8]) -> Result<w::PublicKey, Error> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    if point.len() != 65 || point[0] != 4 {
        return Err(Error::Protocol);
    }
    Ok(w::PublicKey {
        kty: "EC".into(),
        crv: "P-256".into(),
        x: URL_SAFE_NO_PAD.encode(&point[1..33]),
        y: URL_SAFE_NO_PAD.encode(&point[33..]),
    })
}

/// Root-owned desired account integration, written only from an admitted signed task.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalBinding {
    pub installation: Uuid,
    pub tenant: Uuid,
    pub agent_registration: Uuid,
    pub registration_generation: u64,
    pub binding: Uuid,
    pub binding_generation: i64,
    pub revision: i64,
    pub generated_uid: String,
    pub enabled: bool,
}
impl LocalBinding {
    pub fn matches_identity(
        &self,
        installation: Uuid,
        tenant: Uuid,
        registration: &rss_mdm_agent_wire::runtime::BindingProfile,
        account: &str,
    ) -> bool {
        self.installation == installation
            && self.tenant == tenant
            && self.agent_registration == registration.registration_id
            && self.registration_generation == registration.generation
            && self.generated_uid == account
            && self.binding_generation > 0
            && self.revision > 0
            && !self.binding.is_nil()
    }
}

/// Recovery decision for the exact registration recorded before local teardown.
#[derive(Debug, PartialEq, Eq)]
pub enum Retirement {
    Revoke,
    AlreadyRevoked,
    Superseded,
}
pub fn retirement(
    source: Uuid,
    version: i64,
    current: Option<&w::SourceStatus>,
) -> Result<Retirement, Error> {
    if source.is_nil() || version < 1 {
        return Err(Error::Identity);
    }
    let Some(current) = current else {
        return Ok(Retirement::Superseded);
    };
    if current.id != source {
        return Ok(Retirement::Superseded);
    }
    if current.revoked && version.checked_add(1) == Some(current.version) {
        return Ok(Retirement::AlreadyRevoked);
    }
    if !current.revoked && current.version == version {
        return Ok(Retirement::Revoke);
    }
    Ok(Retirement::Superseded)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> (Context, w::BootstrapResponse, w::PublicKey) {
        let tenant = Uuid::from_u128(1);
        let source = Uuid::from_u128(2);
        let context = Context {
            tenant,
            agent_registration: Uuid::from_u128(3),
            installation: Uuid::from_u128(4),
            origin: "https://mdm.example".into(),
            kind: w::Kind::User,
            binding: Some(Uuid::from_u128(5)),
            binding_generation: Some(1),
            generated_uid: Some(Uuid::from_u128(6).to_string()),
        };
        let bootstrap = w::BootstrapResponse {
            id: Uuid::from_u128(7),
            nonce: "one-use-nonce".into(),
            expires_at: 1120,
            source_id: source,
            source_version: 1,
            endpoints: w::Endpoints::new(&context.origin, tenant).unwrap(),
            client_id: Uuid::from_u128(8),
            login_user_name: source.to_string(),
        };
        let key = w::PublicKey {
            kty: "EC".into(),
            crv: "P-256".into(),
            x: "BkFHRYQoleq39LplGqlcmsEdnw64w0wbcbHAEjrM4pw".into(),
            y: "jbOoWZbgDFTEfLa1O_7ZuJy3R8d2XAw0CHWUKmJLsbU".into(),
        };
        (context, bootstrap, key)
    }
    #[test]
    fn delayed_teardown_never_selects_a_replacement_and_lost_receipt_recovers() {
        let (_, challenge, key) = sample();
        let mut current = w::SourceStatus {
            id: challenge.source_id,
            version: 1,
            revoked: false,
            signing_key: Some(key.clone()),
            encryption_key: key,
        };
        assert_eq!(
            retirement(current.id, 1, Some(&current)).unwrap(),
            Retirement::Revoke
        );
        current.version = 2;
        assert_eq!(
            retirement(current.id, 1, Some(&current)).unwrap(),
            Retirement::Superseded
        );
        current.revoked = true;
        assert_eq!(
            retirement(current.id, 1, Some(&current)).unwrap(),
            Retirement::AlreadyRevoked
        );
        assert_eq!(
            retirement(Uuid::new_v4(), 1, Some(&current)).unwrap(),
            Retirement::Superseded
        );
        assert!(retirement(current.id, 0, Some(&current)).is_err());
    }
    #[test]
    fn bootstrap_binds_exact_authority_scope_subject_and_deadline() {
        let (context, mut response, key) = sample();
        assert!(context.check_bootstrap(&response, 1000).is_ok());
        assert_eq!(
            context.check_bootstrap(&response, 1120),
            Err(Error::Expired)
        );
        response.endpoints.token = "https://other.example/token".into();
        assert_eq!(
            context.check_bootstrap(&response, 1000),
            Err(Error::Identity)
        );
        let (mut device, _, _) = sample();
        device.kind = w::Kind::Device;
        assert_eq!(device.validate(), Err(Error::Identity));
        assert!(public_key(&key.x963().unwrap()).is_ok());
        assert!(public_key(&[0; 65]).is_err());
    }
    #[test]
    fn unknown_registration_outcome_requires_exact_current_keys_and_version() {
        let (context, challenge, key) = sample();
        let registration =
            Registration::prepare(context, &challenge, key.clone(), key.clone(), 1000).unwrap();
        let mut status = w::Status {
            password_exchange: None,
            application_observed: true,
            profile_observed: true,
            device_registered: true,
            user_registered: true,
            login_ready: false,
            device_source: None,
            user_source: Some(w::SourceStatus {
                id: registration.source_id,
                version: 1,
                revoked: false,
                signing_key: Some(key.clone()),
                encryption_key: key,
            }),
            registration_trust: w::RegistrationTrust::MdmBootstrapPop,
            hardware_attested: false,
        };
        assert!(registration.recovered(&status)); // Login readiness is independent of registration.
        status.user_source.as_mut().unwrap().version = 2;
        assert!(!registration.recovered(&status));
        status.user_source.as_mut().unwrap().version = 1;
        status.user_source.as_mut().unwrap().revoked = true;
        assert!(!registration.recovered(&status));
        status.user_source.as_mut().unwrap().revoked = false;
        status.user_registered = false;
        assert!(!registration.recovered(&status));
    }
    #[test]
    fn browser_proof_cannot_cross_origin_or_target_and_never_uses_device_scope() {
        let (context, challenge, key) = sample();
        let mut registration =
            Registration::prepare(context, &challenge, key.clone(), key, 1000).unwrap();
        let url = format!("{}/browser/complete", challenge.endpoints.base);
        let token = "a".repeat(43);
        assert!(registration
            .browser_input(&url, "nonce", &token, 1000)
            .is_ok());
        for bad in [
            format!("{url}?redirect=https://evil.example"),
            format!("{url}#fragment"),
            url.replace("mdm.example", "evil.example"),
            url.replace("/complete", "/begin"),
        ] {
            assert!(registration
                .browser_input(&bad, "nonce", &token, 1000)
                .is_err());
        }
        registration.context.kind = w::Kind::Device;
        assert!(registration
            .browser_input(&url, "nonce", &token, 1000)
            .is_err());
    }
    #[test]
    fn registration_proof_binds_challenge_and_rejects_late_reuse() {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let (context, challenge, key) = sample();
        let registration =
            Registration::prepare(context, &challenge, key.clone(), key, 1000).unwrap();
        let input = registration.proof(&challenge, 1000).unwrap();
        let claims: w::RegistrationClaims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(input.split_once('.').unwrap().1)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(claims.source_id, registration.source_id);
        assert_eq!(claims.challenge_id, challenge.id);
        assert_eq!(claims.exp, 1120);
        assert_eq!(registration.proof(&challenge, 1120), Err(Error::Expired));
        assert!(signed(&input, &[0; 63]).is_err());
    }
}
