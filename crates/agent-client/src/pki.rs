use crate::http as h;
use crate::{
    trust::{VerifiedTrust, Watermark},
    Client, Clock, Error, IdentityProvider,
};
use rss_mdm_agent_wire::pki::{
    Acknowledgement, Observation, ObservationReason, ObservationState, Trust,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};
use uuid::Uuid;

/// Read-only projection published by the sole communication owner after trust/identity changes.
#[derive(Clone, Default)]
pub struct RuntimeGate(Arc<AtomicI64>);
impl RuntimeGate {
    /// Approved credential/trust deadline; absent means execution admission is closed.
    pub fn expires_at(&self) -> Option<i64> {
        let until = self.0.load(Ordering::Acquire);
        (until > 0).then_some(until)
    }
    /// The communication owner has received explicit credential revocation/retirement.
    pub fn revoked(&self) -> bool {
        self.0.load(Ordering::Acquire) == -1
    }
    pub(crate) fn revoke(&self) {
        self.0.store(-1, Ordering::Release);
    }
    pub(crate) fn close(&self) {
        let _ = self
            .0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                (value != -1).then_some(0)
            });
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub document: Option<Trust>,
    watermarks: BTreeMap<String, Watermark>,
    // Retain prior installation receipts as history; they grant no trust or native effects.
    #[serde(
        default,
        rename = "installations",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    historical_machine_receipts: BTreeMap<String, serde_json::Value>,
    ack: Option<Ack>,
    refresh_at: i64,
    attempts: u8,
    retry_at: i64,
    pub blocked: bool,
    observation_sequence: u64,
    observation: Option<Observation>,
    pending_observation: Option<Observation>,
    installed: Option<(u64, String)>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ack {
    request: Acknowledgement,
    registration: Uuid,
    completed: bool,
}

impl State {
    pub fn verified(&self, cfg: &crate::Config, now: i64) -> Result<VerifiedTrust, Error> {
        if self.blocked {
            return Err(Error::Untrusted);
        }
        VerifiedTrust::new(self.document.clone().ok_or(Error::Identity)?, cfg, now)
    }
}

impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Observe the existing owner's identity state during network waits, without reading or writing storage.
    pub fn runtime_gate(&self) -> RuntimeGate {
        self.runtime_gate.clone()
    }

    pub(crate) fn publish_runtime_gate(&self) {
        self.runtime_gate.0.store(
            if self
                .runtime_revoked()
                .unwrap_or(self.runtime_gate.revoked())
            {
                -1
            } else {
                self.runtime_deadline().unwrap_or(0)
            },
            Ordering::Release,
        );
    }

    pub(crate) fn pki_state(&self) -> Result<State, Error> {
        Ok(self.store.get("pki")?.unwrap_or_default())
    }

    /// Validate the current device identity against fresh approved trust, without HTTP or key creation.
    pub fn runtime_readiness(&self) -> Result<(), Error> {
        self.active()?;
        self.task_trust_ready()
    }

    /// Explicit server-confirmed retirement/revocation, independent of natural expiry.
    pub fn runtime_revoked(&self) -> Result<bool, Error> {
        use rss_mdm_agent_wire::enroll::ErrorCode;
        Ok(matches!(
            self.identity_state()?.failure,
            Some(ErrorCode::CredentialRevoked | ErrorCode::CredentialRetired)
        ) || matches!(
            self.active(),
            Err(Error::CredentialRevoked | Error::CredentialRetired)
        ))
    }

    /// Earliest credential or approved trust expiry, for the existing execution owner's admission gate.
    pub fn runtime_deadline(&self) -> Result<i64, Error> {
        self.active()?;
        self.task_trust_ready()?;
        let active = self.identity_state()?.active.ok_or(Error::Identity)?;
        Ok(active.not_after.min(self.verified_trust()?.expires))
    }

    pub(crate) fn verified_trust(&self) -> Result<VerifiedTrust, Error> {
        self.pki_state()?.verified(&self.store.cfg, self.now()?)
    }

    pub(crate) fn restore_runtime_transport(&mut self) -> Result<(), Error> {
        self.active()?;
        let active = self.identity_state()?.active.ok_or(Error::Identity)?;
        self.http = crate::tls::client(
            &self.store.cfg,
            Some((&active.key_reference, &active.chain)),
            &self.secrets,
        )?;
        Ok(())
    }

    pub(crate) async fn refresh_trust(&mut self) -> Result<(), Error> {
        let now = self.now()?;
        let mut state = self.pki_state()?;
        if state.document.is_some() && !state.blocked && now < state.refresh_at {
            state.verified(&self.store.cfg, now)?;
            return Ok(());
        }
        let (status, bytes) = self
            .agent_json_transport(
                crate::http::Call::<h::Trust>::new(
                    &self.bootstrap,
                    &self.connection_origin()?,
                    &[],
                    None,
                    None,
                )?
                .timeout(self.store.cfg.limits.request_timeout),
                self.store.cfg.limits.response_bytes,
            )
            .await?;
        if !status.is_success() {
            return Err(Error::Unavailable);
        }
        let now = self.now()?;
        let accepted = (|| {
            let document: Trust = serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)?;
            let verified = VerifiedTrust::new(document, &self.store.cfg, now)?;
            if let Some(previous) = &state.document {
                verified.monotonic(previous, &state.watermarks)?;
            }
            Ok::<_, Error>(verified)
        })();
        let verified = match accepted {
            Ok(value) => value,
            Err(error) => {
                self.runtime_gate.close();
                state.blocked = true;
                self.store.put("pki", &state)?;
                return Err(error);
            }
        };
        if state.watermarks.len()
            + verified
                .watermarks
                .keys()
                .filter(|key| !state.watermarks.contains_key(*key))
                .count()
            > 4096
        {
            return Err(Error::Capacity);
        }
        state.watermarks.extend(verified.watermarks.clone());
        state.refresh_at = now.saturating_add(300).min(
            verified
                .expires
                .saturating_sub(1)
                .max(now.saturating_add(1)),
        );
        state.document = Some(verified.document);
        state.blocked = false;
        // Approved revision and all historical CRL watermarks commit together before enrollment or acknowledgement.
        self.runtime_gate.close();
        self.store.put("pki", &state)?;
        // A verified current CRL is an explicit revocation fact, retained by the existing
        // identity owner even if later communication or trust freshness becomes unavailable.
        if matches!(self.active(), Err(Error::CredentialRevoked)) {
            self.identity_failure(rss_mdm_agent_wire::enroll::ErrorCode::CredentialRevoked)?;
        }
        self.publish_runtime_gate();
        if self.identity_state()?.active.is_some() {
            let document = self.pki_state()?.document.ok_or(Error::Storage)?;
            self.observe_pki(Some(&document), ObservationState::Received, None)?;
        }
        Ok(())
    }

    async fn acknowledge_trust(&mut self) -> Result<(), Error> {
        if self.identity_state()?.active.is_none() {
            return Ok(());
        }
        // Re-read persisted private trust and validate the current identity before every ACK.
        self.restore_runtime_transport()?;
        let registration = self.registration()?.registration_id;
        let mut state = self.pki_state()?;
        let document = state.document.as_ref().ok_or(Error::Identity)?;
        let digest = document.installation_digest();
        if !self.pki_observed
            || state.installed.as_ref() != Some(&(document.revision, digest.clone()))
            || state
                .observation
                .as_ref()
                .is_none_or(|v| v.state != ObservationState::Applied)
        {
            let document = document.clone();
            self.observe_pki(Some(&document), ObservationState::Applied, None)?;
            self.pki_observed = true;
            state = self.pki_state()?;
        }
        let document = state.document.as_ref().ok_or(Error::Identity)?;
        if !state.ack.as_ref().is_some_and(|a| {
            a.registration == registration
                && a.request.revision == document.revision
                && a.request.digest == digest
        }) {
            state.ack = Some(Ack {
                request: Acknowledgement {
                    operation: Uuid::new_v4(),
                    version: 1,
                    revision: document.revision,
                    digest,
                },
                registration,
                completed: false,
            });
            self.store.put("pki", &state)?;
        }
        let ack = state.ack.as_ref().ok_or(Error::Storage)?;
        if ack.completed {
            return Ok(());
        }
        let (status, bytes) = self
            .agent_json_transport(
                crate::http::Call::<h::TrustAck>::new(
                    &self.http,
                    &self.connection_origin()?,
                    &[],
                    None,
                    Some(&ack.request),
                )?
                .timeout(self.store.cfg.limits.request_timeout),
                self.store.cfg.limits.response_bytes,
            )
            .await?;
        if !status.is_success() {
            let error: rss_mdm_agent_wire::enroll::ErrorBody = crate::store::decode(&bytes)?;
            self.identity_failure(error.code)?;
            return Err(crate::identity::enrollment_error(error.code));
        }
        if status != reqwest::StatusCode::NO_CONTENT || !bytes.is_empty() {
            return Err(Error::Protocol);
        }
        state.ack.as_mut().ok_or(Error::Storage)?.completed = true;
        self.store.put("pki", &state)
    }

    /// Drive retained PKI work with durable bounded backoff; no retry changes signing intent.
    pub async fn drive_pki(&mut self) -> Result<(), Error> {
        let now = self.now()?;
        if now < self.pki_state()?.retry_at {
            return Ok(());
        }
        let _ = self.flush_pki_observation().await;
        let result = self.drive_pki_once().await;
        if let Err(error) = result {
            let reason = match error {
                Error::Storage => ObservationReason::Storage,
                Error::Unavailable => ObservationReason::Transport,
                Error::Identity
                | Error::CredentialRevoked
                | Error::CredentialRetired
                | Error::CertificateExpired => ObservationReason::Credential,
                _ => ObservationReason::Validation,
            };
            if self.identity_state()?.active.is_some() {
                let _ = self.observe_pki(None, ObservationState::Failed, Some(reason));
            }
        }
        let _ = self.flush_pki_observation().await;
        let now = self.now()?;
        let mut state = self.pki_state()?;
        if result.is_ok() {
            state.attempts = 0;
            state.retry_at = now.saturating_add(2);
        } else {
            state.attempts = state.attempts.saturating_add(1).min(9);
            state.retry_at =
                now.saturating_add((2_i64 << state.attempts.saturating_sub(1)).min(300));
        }
        self.store.put("pki", &state)?;
        result
    }

    async fn drive_pki_once(&mut self) -> Result<(), Error> {
        self.publish_runtime_gate();
        self.refresh_trust().await?;
        self.verified_trust()?;
        if self.has_pending_enrollment()? {
            self.drive_enrollment().await?;
        } else if let Some(active) = self.identity_state()?.active {
            self.restore_runtime_transport()?;
            let lifetime = active
                .not_after
                .checked_sub(active.not_before)
                .ok_or(Error::Storage)?;
            let window = (lifetime / 5).min(30 * 86400);
            if self.now()? >= active.not_after.saturating_sub(window) {
                self.renew_certificate().await?;
            }
        }
        self.acknowledge_trust().await
    }
}

impl<S: IdentityProvider, C: Clock> Client<S, C> {
    /// Last private PKI application observation; it is not evidence of current CRL freshness.
    pub fn pki_observation(&self) -> Result<Option<Observation>, Error> {
        Ok(self.pki_state()?.observation)
    }
    /// Whether the current applied PKI revision has a confirmed server ACK.
    pub fn pki_acknowledged(&self) -> Result<bool, Error> {
        let state = self.pki_state()?;
        let registration = self
            .identity_state()?
            .active
            .map(|_| self.registration())
            .transpose()?;
        Ok(state.ack.as_ref().is_some_and(|a| {
            a.completed
                && registration
                    .as_ref()
                    .is_some_and(|r| r.registration_id == a.registration)
                && state.document.as_ref().is_some_and(|d| {
                    d.revision == a.request.revision && d.installation_digest() == a.request.digest
                })
        }))
    }
    fn observe_pki(
        &self,
        target: Option<&Trust>,
        status: ObservationState,
        reason: Option<ObservationReason>,
    ) -> Result<(), Error> {
        let registration = self.registration()?;
        let mut state = self.pki_state()?;
        state.observation_sequence = state
            .observation_sequence
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(Error::Capacity)?;
        if status == ObservationState::Applied {
            let target = target.ok_or(Error::Storage)?;
            state.installed = Some((target.revision, target.installation_digest()));
        }
        let observation = Observation {
            operation: Uuid::new_v4(),
            version: 1,
            registration: registration.registration_id,
            generation: registration.generation,
            sequence: state.observation_sequence,
            target_revision: target.map(|v| v.revision),
            target_digest: target.map(Trust::installation_digest),
            actual_revision: state.installed.as_ref().map(|v| v.0),
            actual_digest: state.installed.as_ref().map(|v| v.1.clone()),
            state: status,
            reason,
            observed_at: self.now()?,
        };
        observation.validate()?;
        if state.pending_observation.is_none() {
            state.pending_observation = Some(observation.clone());
        }
        state.observation = Some(observation);
        self.store.put("pki", &state)
    }
    async fn flush_pki_observation(&mut self) -> Result<(), Error> {
        let mut state = self.pki_state()?;
        let Some(pending) = state.pending_observation.as_ref() else {
            return Ok(());
        };
        self.restore_runtime_transport()?;
        let (status, bytes) = self
            .agent_json_transport(
                crate::http::Call::<h::TrustObservation>::new(
                    &self.http,
                    &self.connection_origin()?,
                    &[],
                    None,
                    Some(pending),
                )?
                .timeout(self.store.cfg.limits.request_timeout),
                self.store.cfg.limits.response_bytes,
            )
            .await?;
        if !status.is_success() {
            let error: rss_mdm_agent_wire::enroll::ErrorBody = crate::store::decode(&bytes)?;
            self.identity_failure(error.code)?;
            return Err(crate::identity::enrollment_error(error.code));
        }
        let reply: Observation = crate::store::decode(&bytes)?;
        if &reply != pending {
            return Err(Error::Protocol);
        }
        state.pending_observation = state
            .observation
            .clone()
            .filter(|v| v.sequence > reply.sequence);
        self.store.put("pki", &state)
    }
}
