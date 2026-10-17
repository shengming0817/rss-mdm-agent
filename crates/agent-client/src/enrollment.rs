use crate::{
    client::json_transport,
    identity::{Active, IdentityState, Pending, Purpose},
    store::{decode, encode},
    Client, Clock, Error, IdentityProvider,
};
use rss_mdm_agent_wire::{enroll, runtime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
/// Safe local registration progress; it is never device execution authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EnrollmentPhase {
    /// No explicit registration has started.
    Unregistered,
    /// Exact request preparation or grant submission is pending.
    Preparing,
    /// Original configuration allocation must be queried or exactly replayed.
    ConfigurationUnknown,
    /// Waiting for server-held user authorization.
    AwaitingAuthorization,
    /// Authorized grant can be consumed by its original certificate operation.
    Authorized,
    /// Candidate must prove possession on the activation TLS connection.
    Activating,
    /// An active runtime binding is durably retained.
    Active,
    /// Preserve the original signing intent; administrator recovery may be required.
    IssuanceUnknown,
    /// Query/replay the original activation; never register again.
    ActivationUnknown,
    /// Original authorization or credential is unavailable; retain all state.
    Rejected,
    /// Administrative admission pause; pending material and original expiry are retained.
    Paused,
}
/// Bounded public progress, without secrets, key references, certificate bodies or storage paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnrollmentStatus {
    /// Closed progress phase.
    pub phase: EnrollmentPhase,
    /// Exact grant/request identity for native user authorization.
    pub grant_id: Option<Uuid>,
    /// Original authorization deadline, when known.
    pub expires_at: Option<i64>,
    /// Public-key SHA256 identifying the exact machine request.
    pub public_key_fingerprint: Option<String>,
    /// Whether a durable runtime registration exists, independent of certificate readiness.
    pub registered: bool,
    /// Classified original failure; no untrusted error text.
    pub failure: Option<enroll::ErrorCode>,
}
/// Administrator-protected bootstrap metadata; the secret is resolved separately by reference.
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantImport {
    /// Original grant creation operation, required for exact status recovery.
    #[schemars(
        with = "String",
        regex(
            pattern = "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
        )
    )]
    pub operation_id: Uuid,
    /// Server-issued grant.
    pub grant: enroll::GrantReceipt,
}
impl GrantImport {
    /// Reject malformed, expired or foreign metadata before any initialization effect.
    pub fn validate(&self, platform: runtime::TaskPlatform, now: i64) -> Result<(), Error> {
        if self.operation_id.is_nil()
            || self.grant.wire_version != enroll::API_VERSION
            || self.grant.grant_id.is_nil()
            || self.grant.platform != platform
            || self.grant.state != enroll::GrantState::Available
            || self.grant.expires_at <= now
        {
            return Err(Error::Protocol);
        }
        Ok(())
    }
}
impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Whether an original certificate operation remains to be recovered.
    pub fn has_pending_enrollment(&self) -> Result<bool, Error> {
        Ok(self.identity_state()?.pending.is_some())
    }
    /// Import only bootstrap secret material through the platform owner.
    pub fn import_bootstrap_secret(
        &self,
        reference: &str,
        secret: &crate::Secret,
    ) -> Result<(), Error> {
        self.secrets.import_secret(reference, secret)
    }
    pub(crate) fn identity_state(&self) -> Result<IdentityState, Error> {
        Ok(self.store.get("identity")?.unwrap_or_default())
    }
    pub(crate) fn identity_failure(&self, code: enroll::ErrorCode) -> Result<(), Error> {
        let mut state = self.identity_state()?;
        if !crate::identity::blocks_runtime(code) && state.pending.is_some() {
            state.pending.as_mut().ok_or(Error::Storage)?.failure = Some(code);
        } else {
            state.failure = Some(code);
            if crate::identity::blocks_runtime(code) {
                self.runtime_gate.close();
            }
        }
        self.store.put("identity", &state)
    }
    /// Read safe registration progress without driving HTTP or changing operations.
    pub fn enrollment_status(&self) -> Result<EnrollmentStatus, Error> {
        let state = self.identity_state()?;
        let failure = state.failure.or_else(|| {
            state
                .active
                .as_ref()
                .filter(|a| self.clock.now().is_ok_and(|now| now >= a.not_after))
                .map(|_| enroll::ErrorCode::CertificateExpired)
        });
        let operation_failure = state.pending.as_ref().and_then(|p| p.failure);
        let phase = match operation_failure.or(failure) {
            Some(enroll::ErrorCode::EnrollmentPaused) => EnrollmentPhase::Paused,
            Some(enroll::ErrorCode::IssuanceUnknown) => EnrollmentPhase::IssuanceUnknown,
            Some(enroll::ErrorCode::ActivationUnknown) => EnrollmentPhase::ActivationUnknown,
            Some(code) if code != enroll::ErrorCode::ServiceUnavailable => {
                EnrollmentPhase::Rejected
            }
            _ => match &state.pending {
                Some(p) if p.candidate.is_some() => EnrollmentPhase::Activating,
                Some(p)
                    if p.grant.as_ref().is_some_and(|g| {
                        g.state == enroll::GrantState::Available
                            || g.state == enroll::GrantState::Consumed
                    }) =>
                {
                    EnrollmentPhase::Authorized
                }
                Some(p) if p.grant.is_some() => EnrollmentPhase::AwaitingAuthorization,
                Some(p) if p.configuration.as_ref().is_some_and(|c| c.submitted) => {
                    EnrollmentPhase::ConfigurationUnknown
                }
                Some(_) => EnrollmentPhase::Preparing,
                None if state.active.is_some() => EnrollmentPhase::Active,
                None => EnrollmentPhase::Unregistered,
            },
        };
        let fingerprint = state
            .pending
            .as_ref()
            .and_then(|p| p.csr.as_ref())
            .map(|csr| {
                use base64::Engine;
                use der::{Decode, Encode};
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(csr)
                    .map_err(|_| Error::Storage)?;
                let request =
                    x509_cert::request::CertReq::from_der(&bytes).map_err(|_| Error::Storage)?;
                Ok::<_, Error>(crate::store::hash(
                    &request
                        .info
                        .public_key
                        .to_der()
                        .map_err(|_| Error::Storage)?,
                ))
            })
            .transpose()?;
        Ok(EnrollmentStatus {
            phase,
            registered: state.active.is_some(),
            failure: failure.or(operation_failure),
            grant_id: state
                .pending
                .as_ref()
                .and_then(|p| p.grant.as_ref().map(|g| g.grant_id)),
            expires_at: state
                .pending
                .as_ref()
                .and_then(|p| p.grant.as_ref().map(|g| g.expires_at)),
            public_key_fingerprint: fingerprint,
        })
    }
    pub(crate) fn prepare_enrollment(
        &mut self,
        purpose: Purpose,
        operation: Uuid,
        secret: String,
        capabilities: Vec<runtime::Capability>,
        configuration: Option<crate::configuration::ConfigurationExchange>,
    ) -> Result<(), Error> {
        if operation.is_nil()
            || secret.is_empty()
            || secret.len() > 128
            || capabilities.contains(&runtime::Capability::MdmEnrollmentV1)
        {
            return Err(Error::Configuration);
        }
        let mut state = self.identity_state()?;
        if let Some(p) = &state.pending {
            if p.purpose != purpose
                || p.operation != operation
                || p.secret_reference != secret
                || p.profile.capabilities != capabilities
            {
                return Err(Error::Conflict);
            }
            return self.prepare_key();
        }
        if matches!(purpose, Purpose::Authorization | Purpose::Configuration)
            && state.active.is_some()
        {
            return Err(Error::Conflict);
        }
        let profile = enroll::RuntimeProfile {
            capabilities,
            platform: self.store.cfg.platform,
            architecture: self.store.cfg.architecture,
            execution_context: self.execution_context()?,
        };
        profile.validate()?;
        let certificate_operation = Uuid::new_v4();
        state.pending = Some(Pending {
            purpose,
            operation,
            certificate_operation,
            activation_operation: Uuid::new_v4(),
            key_reference: format!("key-{certificate_operation}"),
            secret_reference: secret,
            profile,
            csr: None,
            grant: None,
            candidate: None,
            failure: None,
            expected: state.active.as_ref().map(|a| a.receipt.runtime.clone()),
            configuration,
        });
        self.store.put("identity", &state)?;
        self.prepare_key()
    }
    fn prepare_key(&mut self) -> Result<(), Error> {
        let mut state = self.identity_state()?;
        let p = state.pending.as_mut().ok_or(Error::Identity)?;
        if p.csr.is_none() {
            self.secrets.create_key(&p.key_reference)?;
            p.csr = Some(crate::identity::csr(&self.secrets, &p.key_reference)?);
            self.store.put("identity", &state)?;
        } else {
            // Missing private keys never trigger replacement after a CSR was frozen.
            self.secrets.key(&p.key_reference)?;
        }
        Ok(())
    }
    /// Finish only local preparation for the original durable operation, before any consumer
    /// requires its credentials. A frozen CSR always reopens its key; it cannot mint a replacement.
    pub(crate) fn prepare_pending_enrollment(&mut self) -> Result<Option<Pending>, Error> {
        if self.identity_state()?.pending.is_none() {
            return Ok(None);
        }
        self.prepare_key()?;
        let pending = self.identity_state()?.pending.ok_or(Error::Identity)?;
        if pending.grant.is_none()
            && !matches!(pending.purpose, Purpose::Import | Purpose::Recovery)
        {
            self.secrets.create_secret(&pending.secret_reference)?;
        }
        Ok(Some(pending))
    }
    /// Start or resume a fixed machine-owned native user authorization request.
    pub async fn request_authorization(
        &mut self,
        capabilities: Vec<runtime::Capability>,
    ) -> Result<EnrollmentStatus, Error> {
        self.prepare_authorization(capabilities)?;
        self.drive_enrollment().await?;
        self.enrollment_status()
    }
    /// Retain the explicitly requested registration operation before PKI/native preparation can fail.
    pub fn prepare_authorization(
        &mut self,
        capabilities: Vec<runtime::Capability>,
    ) -> Result<EnrollmentStatus, Error> {
        let mut state = self.identity_state()?;
        if let Some(pending) = &mut state.pending {
            if let Some(configuration) = &pending.configuration {
                // Before the first allocation request, explicit user login may reuse this
                // machine key/operation. Unknown settlement can never take this path.
                if configuration.submitted || pending.grant.is_some() || pending.candidate.is_some()
                {
                    return Err(Error::Conflict);
                }
                pending.configuration = None;
                pending.purpose = Purpose::Authorization;
                pending.failure = None;
                self.store.put("identity", &state)?;
            }
        }
        let operation = state
            .pending
            .as_ref()
            .map_or_else(Uuid::new_v4, |p| p.operation);
        let reference = format!("bootstrap-{operation}");
        self.prepare_enrollment(
            Purpose::Authorization,
            operation,
            reference.clone(),
            capabilities,
            None,
        )?;
        self.secrets.create_secret(&reference)?;
        self.enrollment_status()
    }
    /// Admit an explicit administrator recovery grant without discarding registration or pending intent.
    pub fn validate_recovery_import(
        &self,
        material: &GrantImport,
        reference: &str,
    ) -> Result<(), Error> {
        material.validate(self.store.cfg.platform, self.now()?)?;
        let state = self.identity_state()?;
        let active = state.active.as_ref().ok_or(Error::Identity)?;
        if let Some(p) = state.pending {
            if p.purpose != Purpose::Recovery
                || p.operation != material.operation_id
                || p.secret_reference != reference
                || p.profile.capabilities != active.receipt.runtime.capabilities
                || p.grant.as_ref().is_some_and(|g| g != &material.grant)
            {
                return Err(Error::Conflict);
            }
        }
        Ok(())
    }
    /// Recovery uses bootstrap HTTPS and requires activation to retain the exact runtime binding.
    pub fn import_recovery_grant(
        &mut self,
        material: GrantImport,
        reference: &str,
    ) -> Result<(), Error> {
        self.validate_recovery_import(&material, reference)?;
        self.secrets.resolve(reference)?;
        let capabilities = self.registration()?.capabilities;
        self.prepare_enrollment(
            Purpose::Recovery,
            material.operation_id,
            reference.into(),
            capabilities,
            None,
        )?;
        let mut state = self.identity_state()?;
        state.pending.as_mut().ok_or(Error::Storage)?.grant = Some(material.grant);
        self.store.put("identity", &state)
    }
    /// Begin a manually requested same-registration renewal with the current mTLS identity.
    pub async fn renew_certificate(&mut self) -> Result<EnrollmentStatus, Error> {
        let state = self.identity_state()?;
        if state
            .pending
            .as_ref()
            .is_some_and(|p| p.purpose == Purpose::Renewal)
        {
            self.drive_enrollment().await?;
            return self.enrollment_status();
        }
        self.active()?;
        let capabilities = state
            .active
            .as_ref()
            .ok_or(Error::Identity)?
            .receipt
            .runtime
            .capabilities
            .clone();
        let operation = state
            .pending
            .as_ref()
            .map_or_else(Uuid::new_v4, |p| p.operation);
        let reference = format!("bootstrap-{operation}");
        self.prepare_enrollment(
            Purpose::Renewal,
            operation,
            reference.clone(),
            capabilities,
            None,
        )?;
        self.secrets.create_secret(&reference)?;
        self.drive_enrollment().await?;
        self.enrollment_status()
    }
    pub(crate) async fn enrollment_json<T: serde::de::DeserializeOwned>(
        &self,
        http: &reqwest::Client,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, Error> {
        self.enrollment_response(http, path, body, true).await
    }
    pub(crate) async fn enrollment_response<T: serde::de::DeserializeOwned>(
        &self,
        http: &reqwest::Client,
        path: &str,
        body: &impl Serialize,
        record_failure: bool,
    ) -> Result<T, Error> {
        let url = self
            .store
            .cfg
            .origin
            .join(&format!("api/v1/agent/enroll/{path}"))
            .map_err(|_| Error::Configuration)?;
        let request = http
            .post(url)
            .header("content-type", "application/json")
            .body(encode(body)?)
            .timeout(self.store.cfg.limits.request_timeout);
        let (status, bytes) = json_transport(request, self.store.cfg.limits.response_bytes).await?;
        if !status.is_success() {
            let error: enroll::ErrorBody = decode(&bytes)?;
            if record_failure {
                self.identity_failure(error.code)?;
            }
            return Err(crate::identity::enrollment_error(error.code));
        }
        decode(&bytes)
    }
    /// Drive one retained registration operation. No retry creates a new operation or key.
    pub async fn drive_enrollment(&mut self) -> Result<(), Error> {
        let Some(mut p) = self.prepare_pending_enrollment()? else {
            return Ok(());
        };
        self.refresh_trust().await?;
        let mut state = self.identity_state()?;
        let bootstrap = crate::tls::client(&self.store.cfg, None, &self.secrets)?;
        if p.grant.is_none() {
            let request = enroll::GrantRequest {
                wire_version: enroll::API_VERSION,
                operation_id: p.operation,
                secret: self.secrets.resolve(&p.secret_reference)?,
                platform: self.store.cfg.platform,
                csr: p.csr.clone().ok_or(Error::Storage)?,
            };
            let grant: enroll::GrantReceipt = if p.purpose == Purpose::Configuration {
                self.exchange_configuration(&mut p, &bootstrap).await?
            } else if p.purpose == Purpose::Renewal {
                if self.restore_runtime_transport().is_ok() {
                    self.enrollment_json(&self.http, "renewal-grants", &request)
                        .await?
                } else {
                    self.enrollment_json(
                        &bootstrap,
                        "renewal-grants/status",
                        &rss_mdm_agent_wire::enroll::RenewalStatusRequest {
                            wire_version: 1,
                            operation_id: p.operation,
                            secret: self.secrets.resolve(&p.secret_reference)?,
                        },
                    )
                    .await?
                }
            } else {
                self.enrollment_json(&bootstrap, "requests", &request)
                    .await?
            };
            if grant.grant_id.is_nil()
                || grant.platform != self.store.cfg.platform
                || grant.expires_at <= self.clock.now()?
            {
                return Err(Error::Protocol);
            }
            p.grant = Some(grant);
            p.failure = None;
            state.pending = Some(p.clone());
            self.store.put("identity", &state)?;
        }
        let grant = p.grant.clone().ok_or(Error::Identity)?;
        let progress: enroll::Progress = self
            .enrollment_json(
                &bootstrap,
                "status",
                &enroll::StatusRequest {
                    wire_version: enroll::API_VERSION,
                    grant_id: grant.grant_id,
                    operation_id: p.operation,
                    secret: self.secrets.resolve(&p.secret_reference)?,
                },
            )
            .await?;
        if progress.grant.grant_id != grant.grant_id
            || progress.grant.platform != grant.platform
            || progress.grant.expires_at != grant.expires_at
        {
            return Err(Error::Protocol);
        }
        p.grant = Some(progress.grant.clone());
        if let Some(receipt) = progress.activation {
            if p.candidate.is_none() {
                // Unknown CA results cannot be replaced by a profile-only status receipt.
                return Err(Error::ActivationUnknown);
            }
            return self.commit_activation(p, receipt);
        }
        if progress.issuance_unknown {
            self.identity_failure(enroll::ErrorCode::IssuanceUnknown)?;
            return Err(Error::IssuanceUnknown);
        }
        match progress.grant.state {
            enroll::GrantState::Pending => {
                p.failure = None;
                state.pending = Some(p);
                self.store.put("identity", &state)?;
                return Ok(());
            }
            enroll::GrantState::Revoked => {
                self.identity_failure(enroll::ErrorCode::GrantRevoked)?;
                return Err(Error::Denied);
            }
            enroll::GrantState::Expired => {
                self.identity_failure(enroll::ErrorCode::GrantExpired)?;
                return Err(Error::Expired);
            }
            _ => (),
        }
        if self.clock.now()? >= grant.expires_at {
            return Err(Error::Expired);
        }
        if p.candidate.is_none() {
            let candidate: enroll::Candidate = self
                .enrollment_json(
                    &bootstrap,
                    "certificates",
                    &enroll::CertificateRequest {
                        wire_version: enroll::API_VERSION,
                        operation_id: p.certificate_operation,
                        grant_id: grant.grant_id,
                        secret: self.secrets.resolve(&p.secret_reference)?,
                        csr: p.csr.clone().ok_or(Error::Storage)?,
                        runtime: p.profile.clone(),
                    },
                )
                .await?;
            if candidate.operation_id != p.certificate_operation
                || candidate.issuance_id.is_nil()
                || candidate.activate_before != grant.expires_at
                || progress
                    .issuance_id
                    .is_some_and(|id| id != candidate.issuance_id)
            {
                return Err(Error::Protocol);
            }
            crate::tls::validity(
                &self.store.cfg,
                &candidate.certificate_chain,
                &p.key_reference,
                &self.secrets,
                self.clock.now()?,
                &self.verified_trust()?,
            )?;
            p.candidate = Some(candidate);
            p.failure = None;
            state.pending = Some(p.clone());
            self.store.put("identity", &state)?;
        }
        let candidate = p.candidate.as_ref().ok_or(Error::Storage)?;
        let http = crate::tls::client(
            &self.store.cfg,
            Some((&p.key_reference, &candidate.certificate_chain)),
            &self.secrets,
        )?;
        let receipt: enroll::ActivationReceipt = self
            .enrollment_json(
                &http,
                "activate",
                &enroll::ActivationRequest {
                    wire_version: enroll::API_VERSION,
                    issuance_id: candidate.issuance_id,
                    operation_id: p.activation_operation,
                },
            )
            .await?;
        self.commit_activation(p, receipt)
    }
    fn commit_activation(
        &mut self,
        p: Pending,
        receipt: enroll::ActivationReceipt,
    ) -> Result<(), Error> {
        let candidate = p.candidate.as_ref().ok_or(Error::Storage)?;
        if receipt.issuance_id != candidate.issuance_id
            || receipt.operation_id != p.activation_operation
            || receipt.runtime.capabilities != p.profile.capabilities
            || p.expected
                .as_ref()
                .is_some_and(|old| *old != receipt.runtime)
        {
            return Err(Error::Identity);
        }
        let (not_before, not_after, device) = crate::tls::validity(
            &self.store.cfg,
            &candidate.certificate_chain,
            &p.key_reference,
            &self.secrets,
            self.clock.now()?,
            &self.verified_trust()?,
        )?;
        if device != receipt.runtime.device_id {
            return Err(Error::Identity);
        }
        let http = crate::tls::client(
            &self.store.cfg,
            Some((&p.key_reference, &candidate.certificate_chain)),
            &self.secrets,
        )?;
        self.store.put(
            "identity",
            &IdentityState {
                active: Some(Active {
                    receipt,
                    key_reference: p.key_reference,
                    secret_reference: p.secret_reference,
                    chain: candidate.certificate_chain.clone(),
                    not_before,
                    not_after,
                }),
                pending: None,
                failure: None,
            },
        )?;
        self.http = http;
        self.publish_runtime_gate();
        Ok(())
    }
}
