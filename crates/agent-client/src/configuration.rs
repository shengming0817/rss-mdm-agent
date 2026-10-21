//! Signed configuration admission and durable exchange, owned by the existing enrollment state.
use crate::identity::{Pending, Purpose};
use crate::{Client, Clock, Config, Error, IdentityProvider};
use base64::Engine;
use rss_mdm_agent_wire::{
    enroll::{self, configuration as wire},
    runtime,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shared installer/native import filename defined by the server's delivery contract.
pub const CONFIGURATION_FILENAME: &str = "rss-agent-enrollment.json";
/// Maximum complete configuration file; enforced before IPC, parsing and persistence.
pub const CONFIGURATION_BYTES: usize = 65536;

/// Verify external configuration against installed trust, never against file-provided trust roots.
pub fn verify_configuration(
    file: &wire::ConfigurationFile,
    cfg: &Config,
    now: i64,
) -> Result<(), Error> {
    let c = &file.configuration;
    let key = cfg
        .registration_keys
        .get(&c.key_id)
        .ok_or(Error::Untrusted)?;
    let bytes = c.signing_bytes(&file.secret)?;
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&file.signature)
        .map_err(|_| Error::Untrusted)?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key)
        .verify(&bytes, &signature)
        .map_err(|_| Error::Untrusted)?;
    if c.origin != cfg.origin.origin().ascii_serialization()
        || c.tenant_id != cfg.tenant
        || !c.targets.contains(&wire::Target {
            platform: cfg.platform,
            architecture: cfg.architecture,
        })
    {
        return Err(Error::Identity);
    }
    if now < 0 || c.expires_at <= now {
        return Err(Error::Expired);
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigurationExchange {
    pub configuration: wire::Configuration,
    pub signature: String,
    pub secret_reference: String,
    pub installer_format: wire::InstallerFormat,
    pub installation: Option<wire::Installation>,
    // Written before HTTP. Once set, expiry never skips the read-only recovery lookup.
    pub submitted: bool,
    pub rejection: Option<enroll::ErrorCode>,
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Admit one complete signed file without replacing an active or pending machine identity.
    pub fn prepare_configuration(
        &mut self,
        file: wire::ConfigurationFile,
        installer_format: wire::InstallerFormat,
        capabilities: Vec<runtime::Capability>,
    ) -> Result<(), Error> {
        let state = self.identity_state()?;
        if state.active.is_some() {
            return Err(Error::Conflict);
        }
        if let Some(p) = &state.pending {
            let Some(old) = &p.configuration else {
                return Err(Error::Conflict);
            };
            if crate::store::encode(&old.configuration)?
                != crate::store::encode(&file.configuration)?
                || old.signature != file.signature
                || old.installer_format != installer_format
                || self.secrets.resolve(&old.secret_reference)?.expose() != file.secret.expose()
            {
                return Err(Error::Conflict);
            }
            return Ok(());
        }
        verify_configuration(&file, &self.store.cfg, self.now()?)?;
        let operation = Uuid::new_v4();
        let config_reference = format!("configuration-{operation}");
        self.secrets
            .import_secret(&config_reference, &file.secret)?;
        let secret_reference = format!("bootstrap-{operation}");
        self.secrets.create_secret(&secret_reference)?;
        // Configuration metadata and the original operation are committed together, before key/HTTP effects.
        self.prepare_enrollment(
            Purpose::Configuration,
            operation,
            secret_reference,
            capabilities,
            Some(ConfigurationExchange {
                configuration: file.configuration,
                signature: file.signature,
                secret_reference: config_reference,
                installer_format,
                installation: None,
                submitted: false,
                rejection: None,
            }),
        )?;
        Ok(())
    }
    pub(crate) async fn exchange_configuration(
        &mut self,
        p: &mut Pending,
        http: &reqwest::Client,
    ) -> Result<enroll::GrantReceipt, Error> {
        let mut c = p.configuration.clone().ok_or(Error::Storage)?;
        if let Some(code) = c.rejection {
            return Err(crate::identity::enrollment_error(code));
        }
        let had_uncertain_submission = c.submitted;
        if c.submitted {
            // A lost response must never mint a new operation, even after configuration expiry/revocation.
            let status = wire::ExchangeStatus {
                wire_version: 1,
                configuration_id: c.configuration.configuration_id,
                operation_id: p.operation,
                secret: self.secrets.resolve(&p.secret_reference)?,
            };
            match self
                .enrollment_response(http, "configurations/status", &status, false)
                .await
            {
                Ok(grant) => return Ok(grant),
                // Unknown original operation may have never reached the server; exact replay is safe.
                Err(Error::Identity) => (),
                Err(e) => return Err(e),
            }
        }
        let file = wire::ConfigurationFile {
            configuration: c.configuration.clone(),
            signature: c.signature.clone(),
            secret: self.secrets.resolve(&c.secret_reference)?,
        };
        verify_configuration(&file, &self.store.cfg, self.now()?)?;
        if c.installation.is_none() {
            let url = self
                .store
                .cfg
                .origin
                .join("api/v1/agent/enroll/packages")
                .map_err(|_| Error::Configuration)?;
            let (status, bytes) = crate::client::json_transport(
                http.get(url).timeout(self.store.cfg.limits.request_timeout),
                self.store.cfg.limits.response_bytes,
            )
            .await?;
            if !status.is_success() {
                return Err(Error::Unavailable);
            }
            let packages: Vec<wire::Package> = crate::store::decode(&bytes)?;
            let package = select_package(&packages, &self.store.cfg, c.installer_format)?;
            c.installation = Some(wire::Installation {
                release_id: package.release_id,
            });
        }
        c.submitted = true;
        p.configuration = Some(c.clone());
        let mut state = self.identity_state()?;
        state.pending = Some(p.clone());
        self.store.put("identity", &state)?;
        let outcome = self
            .enrollment_outcome(
                http,
                "configurations/exchange",
                &wire::Exchange {
                    file,
                    operation_id: p.operation,
                    secret: self.secrets.resolve(&p.secret_reference)?,
                    csr: p.csr.clone().ok_or(Error::Storage)?,
                    runtime: p.profile.clone(),
                    installation: c.installation.clone().ok_or(Error::Storage)?,
                },
            )
            .await;
        match outcome {
            Ok(grant) => Ok(grant),
            Err(crate::enrollment::EnrollmentFailure::Local(error)) => Err(error),
            Err(crate::enrollment::EnrollmentFailure::Rejected(code)) => {
                // Only an explicit first-attempt business refusal proves no allocation. A
                // timeout, malformed response, local budget failure or retry after Unknown does not.
                if !had_uncertain_submission
                    && matches!(
                        code,
                        enroll::ErrorCode::PermissionDenied
                            | enroll::ErrorCode::RegistrationLimit
                            | enroll::ErrorCode::GrantExpired
                            | enroll::ErrorCode::GrantRevoked
                    )
                {
                    c.rejection = Some(code);
                    p.configuration = Some(c);
                }
                p.failure = Some(code);
                let mut state = self.identity_state()?;
                state.pending = Some(p.clone());
                self.store.put("identity", &state)?;
                Err(crate::identity::enrollment_error(code))
            }
        }
    }
}

// ref: version-compare 0.2.1 src/version.rs (numeric component ordering).
fn select_package<'a>(
    packages: &'a [wire::Package],
    cfg: &Config,
    format: wire::InstallerFormat,
) -> Result<&'a wire::Package, Error> {
    let mut selected = None;
    for package in packages.iter().filter(|p| {
        p.tenant_id == cfg.tenant
            && p.target.platform == cfg.platform
            && p.target.architecture == cfg.architecture
            && p.format == format
    }) {
        let version = version_compare::Version::from(&package.version)
            .filter(|v| !v.parts().is_empty())
            .ok_or(Error::Protocol)?;
        if selected.as_ref().is_none_or(
            |(old, old_version): &(&wire::Package, version_compare::Version<'_>)| {
                version > *old_version
                    || (version == *old_version && package.release_id > old.release_id)
            },
        ) {
            selected = Some((package, version));
        }
    }
    selected.map(|(p, _)| p).ok_or(Error::Denied)
}
